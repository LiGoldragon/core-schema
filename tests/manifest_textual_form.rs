//! The manifest is the multi-file TextualForm input boundary. This witness proves
//! dependency ordering, one shared NameTable, a StructureTree-driven re-emission, and
//! loud rejection of undeclared source chunks. The encoded schema carries no paths.

use core_schema::{SchemaManifest, SchemaManifestFile, SchemaManifestStructure, TextualSchema};
use name_table::NameTable;
use raw_discovery::Recognizer;
use structural_codec::{CanonicalText, ChunkName, TextChunk, TextualForm};

const TYPES: &str = "{}\n[]\n[]\n{\n  Note.String\n}\n{}\n{}";
const ROOT: &str = "{}\n[Record.Entry]\n[Stored.Entry]\n{\n  Entry.{ Note Integer }\n}\n{}\n{}";

fn path(value: &str) -> ChunkName {
    ChunkName(value.to_owned())
}

fn canonical_document(source: &str) -> String {
    Recognizer::standard()
        .recognize(source)
        .expect("recognize fixture source")
        .root_objects()
        .iter()
        .map(CanonicalText::canonical_text)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn manifest_resolves_dependencies_and_round_trips_the_same_file_index() {
    let manifest = SchemaManifest::new(vec![
        SchemaManifestFile::new(path("signal.schema"), vec![path("types.schema")]),
        SchemaManifestFile::new(path("types.schema"), Vec::new()),
    ])
    .expect("acyclic manifest");
    let source = TextualForm::from_chunks(vec![
        TextChunk {
            name: path("signal.schema"),
            text: ROOT.to_owned(),
        },
        TextChunk {
            name: path("types.schema"),
            text: TYPES.to_owned(),
        },
    ]);
    let textual = TextualSchema::schema_document().expect("document grammar");
    let mut names = NameTable::new(name_table::IdentifierNamespace::Schema);
    let decoded = textual
        .decode_manifest(&manifest, &source, &mut names)
        .expect("decode dependency-first manifest");

    assert_eq!(decoded.encoded().declarations().len(), 4);
    assert_eq!(decoded.structure().files().len(), 2);
    assert!(
        decoded
            .structure()
            .files()
            .iter()
            .all(|file| !file.declaration_positions().is_empty()),
        "every explicit source file owns its structural declaration positions"
    );

    let reemitted = textual
        .encode_manifest(&manifest, &decoded, &mut names)
        .expect("emit from encoded form plus manifest structure");
    assert_eq!(
        reemitted
            .chunks()
            .iter()
            .map(|chunk| chunk.name.clone())
            .collect::<Vec<_>>(),
        source
            .chunks()
            .iter()
            .map(|chunk| chunk.name.clone())
            .collect::<Vec<_>>(),
        "emission preserves the manifest's explicit file index"
    );
    assert_eq!(
        reemitted.chunks()[0].text,
        canonical_document(ROOT),
        "the root file emits through the shared canonical structuretree"
    );
    assert_eq!(
        reemitted.chunks()[1].text,
        canonical_document(TYPES),
        "the dependency file emits through the shared canonical structuretree"
    );

    let mut names_again = NameTable::new(name_table::IdentifierNamespace::Schema);
    let decoded_again = textual
        .decode_manifest(&manifest, &reemitted, &mut names_again)
        .expect("re-decode emitted manifest");
    assert_eq!(decoded, decoded_again);
}

#[test]
fn manifest_rejects_cycles_and_undeclared_textual_files() {
    let cycle = SchemaManifest::new(vec![
        SchemaManifestFile::new(path("one.schema"), vec![path("two.schema")]),
        SchemaManifestFile::new(path("two.schema"), vec![path("one.schema")]),
    ]);
    assert!(cycle.is_err(), "source dependencies cannot cycle");

    let manifest = SchemaManifest::new(vec![SchemaManifestFile::new(
        path("only.schema"),
        Vec::new(),
    )])
    .expect("one-file manifest");
    let view = TextualForm::from_chunks(vec![
        TextChunk {
            name: path("only.schema"),
            text: TYPES.to_owned(),
        },
        TextChunk {
            name: path("undeclared.schema"),
            text: TYPES.to_owned(),
        },
    ]);
    let mut names = NameTable::new(name_table::IdentifierNamespace::Schema);
    let error = TextualSchema::schema_document()
        .expect("document grammar")
        .decode_manifest(&manifest, &view, &mut names)
        .expect_err("manifest does not accept undeclared source files");
    assert!(matches!(
        error,
        core_schema::TextualError::Manifest(
            core_schema::SchemaManifestError::UnexpectedSourceFile { .. }
        )
    ));
}

#[test]
fn manifest_structure_cannot_drop_encoded_declarations() {
    let manifest = SchemaManifest::new(vec![SchemaManifestFile::new(
        path("only.schema"),
        Vec::new(),
    )])
    .expect("one-file manifest");
    let source = TextualForm::from_chunks(vec![TextChunk {
        name: path("only.schema"),
        text: TYPES.to_owned(),
    }]);
    let textual = TextualSchema::schema_document().expect("document grammar");
    let mut names = NameTable::new(name_table::IdentifierNamespace::Schema);
    let decoded = textual
        .decode_manifest(&manifest, &source, &mut names)
        .expect("decode one file");
    let malformed = core_schema::ManifestSchema::new(
        decoded.encoded().clone(),
        SchemaManifestStructure::new(Vec::new()),
    );
    assert!(
        textual
            .encode_manifest(&manifest, &malformed, &mut names)
            .is_err(),
        "a structure tree must account for every encoded declaration"
    );
}
