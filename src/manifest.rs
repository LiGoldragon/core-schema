//! A manifest-backed, two-way `TextualForm` boundary for schema documents.
//!
//! A manifest is deliberately only the file-path dependency index. It does not own
//! source management, names, or encoded declarations: chunks remain the
//! [`TextualForm`](structural_codec::TextualForm) view, declarations remain the
//! stringless [`EncodedSchema`](crate::EncodedSchema), and this structure records
//! which declaration positions each file structurally owns so the same file layout
//! can be emitted again. Dependencies are resolved before decoding; the dependency
//! graph is typed data at the text boundary and never enters Nomos.

use std::collections::{BTreeMap, BTreeSet};

use structural_codec::ChunkName;

/// One path-addressed schema source file and the files it depends on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaManifestFile {
    path: ChunkName,
    dependencies: Vec<ChunkName>,
}

impl SchemaManifestFile {
    /// A source file at `path`, whose dependencies must be present in the same
    /// explicit manifest.
    pub fn new(path: ChunkName, dependencies: Vec<ChunkName>) -> Self {
        Self { path, dependencies }
    }

    /// The text-view index key for this file.
    pub fn path(&self) -> &ChunkName {
        &self.path
    }

    /// The file-path dependencies, in authored resolution order.
    pub fn dependencies(&self) -> &[ChunkName] {
        &self.dependencies
    }
}

/// The explicit file manifest a caller supplies at the TextualForm input boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaManifest {
    files: Vec<SchemaManifestFile>,
}

impl SchemaManifest {
    /// Build and validate a manifest. A file name identifies exactly one source
    /// chunk; a dependency names a sibling file; and cycles are rejected before any
    /// source text is decoded.
    pub fn new(files: Vec<SchemaManifestFile>) -> Result<Self, SchemaManifestError> {
        let manifest = Self { files };
        manifest.dependency_order()?;
        Ok(manifest)
    }

    /// The files as explicitly listed by the caller. This ordering is used for
    /// re-emission; dependency resolution has its own order.
    pub fn files(&self) -> &[SchemaManifestFile] {
        &self.files
    }

    /// Resolve the file graph cargo-crate-style: every dependency precedes the file
    /// that depends on it, while otherwise preserving manifest order.
    pub fn dependency_order(&self) -> Result<Vec<&SchemaManifestFile>, SchemaManifestError> {
        let mut by_path = BTreeMap::new();
        for (index, file) in self.files.iter().enumerate() {
            let previous = by_path.insert(file.path.0.as_str(), index);
            if previous.is_some() {
                return Err(SchemaManifestError::DuplicateFile {
                    path: file.path.0.clone(),
                });
            }
        }
        for file in &self.files {
            for dependency in &file.dependencies {
                if !by_path.contains_key(dependency.0.as_str()) {
                    return Err(SchemaManifestError::UnknownDependency {
                        file: file.path.0.clone(),
                        dependency: dependency.0.clone(),
                    });
                }
            }
        }

        let mut states = vec![VisitState::Unvisited; self.files.len()];
        let mut ordered = Vec::with_capacity(self.files.len());
        for index in 0..self.files.len() {
            self.visit(index, &by_path, &mut states, &mut ordered)?;
        }
        Ok(ordered
            .into_iter()
            .map(|index| &self.files[index])
            .collect())
    }

    fn visit(
        &self,
        index: usize,
        by_path: &BTreeMap<&str, usize>,
        states: &mut [VisitState],
        ordered: &mut Vec<usize>,
    ) -> Result<(), SchemaManifestError> {
        match states[index] {
            VisitState::Visited => return Ok(()),
            VisitState::Visiting => {
                return Err(SchemaManifestError::DependencyCycle {
                    path: self.files[index].path.0.clone(),
                });
            }
            VisitState::Unvisited => {}
        }
        states[index] = VisitState::Visiting;
        for dependency in &self.files[index].dependencies {
            let dependency_index = by_path[dependency.0.as_str()];
            self.visit(dependency_index, by_path, states, ordered)?;
        }
        states[index] = VisitState::Visited;
        ordered.push(index);
        Ok(())
    }
}

/// The structural allocation of one manifest file: global declaration positions it
/// owns in the stringless schema. Paths stay in the TextualForm/StructureTree side;
/// encoded declarations stay free of file strings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaManifestFileStructure {
    path: ChunkName,
    declaration_positions: Vec<usize>,
}

impl SchemaManifestFileStructure {
    /// One structural file allocation.
    pub fn new(path: ChunkName, declaration_positions: Vec<usize>) -> Self {
        Self {
            path,
            declaration_positions,
        }
    }

    /// The text-view index key for this file.
    pub fn path(&self) -> &ChunkName {
        &self.path
    }

    /// The positions it owns in the combined EncodedSchema declaration order.
    pub fn declaration_positions(&self) -> &[usize] {
        &self.declaration_positions
    }
}

/// The structuretree portion of a manifest-backed schema view. It tells the
/// encoder how to reconstruct files from an EncodedSchema without smuggling paths
/// into the encoded truth.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaManifestStructure {
    files: Vec<SchemaManifestFileStructure>,
}

impl SchemaManifestStructure {
    /// A manifest structure over explicit files.
    pub fn new(files: Vec<SchemaManifestFileStructure>) -> Self {
        Self { files }
    }

    /// File allocations in manifest re-emission order.
    pub fn files(&self) -> &[SchemaManifestFileStructure] {
        &self.files
    }

    pub(crate) fn validate(
        &self,
        manifest: &SchemaManifest,
        declaration_count: usize,
    ) -> Result<(), SchemaManifestError> {
        let expected: BTreeSet<&str> = manifest
            .files
            .iter()
            .map(|file| file.path.0.as_str())
            .collect();
        let actual: BTreeSet<&str> = self.files.iter().map(|file| file.path.0.as_str()).collect();
        if expected != actual || expected.len() != self.files.len() {
            return Err(SchemaManifestError::StructureFilesDoNotMatchManifest);
        }

        let mut seen = BTreeSet::new();
        for file in &self.files {
            for position in &file.declaration_positions {
                if *position >= declaration_count {
                    return Err(SchemaManifestError::PositionOutOfRange {
                        path: file.path.0.clone(),
                        position: *position,
                    });
                }
                if !seen.insert(*position) {
                    return Err(SchemaManifestError::DuplicateDeclarationPosition {
                        position: *position,
                    });
                }
            }
        }
        if seen.len() != declaration_count {
            return Err(SchemaManifestError::UnassignedDeclarations {
                assigned: seen.len(),
                total: declaration_count,
            });
        }
        Ok(())
    }

    pub(crate) fn file(&self, path: &ChunkName) -> Option<&SchemaManifestFileStructure> {
        self.files.iter().find(|file| file.path == *path)
    }
}

/// The decoded truth plus the structuretree used to reproduce the same multi-file
/// TextualForm. The NameTable remains external, owned by the caller of the Textual
/// operation; it is never duplicated here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestSchema {
    encoded: crate::EncodedSchema,
    structure: SchemaManifestStructure,
}

impl ManifestSchema {
    /// Pair encoded truth with the file-layout structure that views it.
    pub fn new(encoded: crate::EncodedSchema, structure: SchemaManifestStructure) -> Self {
        Self { encoded, structure }
    }

    /// The stringless schema truth.
    pub fn encoded(&self) -> &crate::EncodedSchema {
        &self.encoded
    }

    /// The structuretree that drives re-emission of the source files.
    pub fn structure(&self) -> &SchemaManifestStructure {
        &self.structure
    }
}

/// A manifest or manifest-structure failure at the TextualForm boundary.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SchemaManifestError {
    #[error("the manifest names source file {path:?} more than once")]
    DuplicateFile { path: String },
    #[error("the manifest file {file:?} depends on unknown sibling file {dependency:?}")]
    UnknownDependency { file: String, dependency: String },
    #[error("the manifest dependency graph has a cycle through {path:?}")]
    DependencyCycle { path: String },
    #[error("the textual form includes undeclared source file {path:?}")]
    UnexpectedSourceFile { path: String },
    #[error("the manifest structure files do not exactly match the manifest files")]
    StructureFilesDoNotMatchManifest,
    #[error("manifest file {path:?} refers to out-of-range declaration position {position}")]
    PositionOutOfRange { path: String, position: usize },
    #[error("more than one manifest file owns declaration position {position}")]
    DuplicateDeclarationPosition { position: usize },
    #[error("the manifest structure assigned {assigned} of {total} declarations")]
    UnassignedDeclarations { assigned: usize, total: usize },
    #[error("more than one source file declares the same encoded identifier {identifier:?}")]
    DuplicateDeclarationIdentifier { identifier: name_table::Identifier },
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum VisitState {
    Unvisited,
    Visiting,
    Visited,
}
