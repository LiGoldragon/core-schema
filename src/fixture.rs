//! The proof-of-concept schema family, as REAL stringless `CoreSchema`
//! declarations and a companion authored structural table. This is slice one's
//! synthetic fixture universe made real: the ids now key genuine Core declarations
//! with genuine field signatures, so the table's authored signatures can be
//! validated against the Core layout ([`CoreUniverse::validate_table`]).
//!
//! The family: `CommitSequence`/`StateDigest` newtypes over `Integer`, a
//! `DatabaseMarker` struct `{ CommitSequence StateDigest StateDigest }` — its two
//! same-typed `StateDigest` fields told apart by position alone — the
//! `Documentation → Summary → Text` string-rejoin chain, the `Field` meta-type with
//! its single positional constructor, and the `Integer`/`Float`/`Text` leaf
//! primitives.
//!
//! [`CoreUniverse::validate_table`]: crate::universe::CoreUniverse::validate_table

use std::collections::BTreeMap;

use raw_discovery::Delimiter;
use structural_codec::authoring::{AuthoringForm, ObjectSymbolPrefixedBlock};
use structural_codec::ids::{
    CoreConstructorId, PositionalSignature, ScopedCoreTypeId, StructuralRevision,
};
use structural_codec::table::{
    AddressedStructuralTable, CoreLayoutIdentity, RawProfileIdentity, TableIdentityPayload,
};
use structural_codec::{
    AtomForm, CaseExpectation, ConstructorCodec, LeafForm, ScalarLeaf, SequenceForm,
    StructuralEntry, StructuralForm,
};

use crate::declaration::{
    CoreDeclaration, CoreField, CoreNewtype, CoreSchema, CoreStruct, CoreType,
};
use crate::error::UniverseError;
use crate::reference::CoreReference;
use crate::universe::{CORE_UNIVERSE, CoreUniverse, CoreUniverseBuilder, ScalarSlot};

// The universe type ids, local numbers echoing the slice-one worked examples.
pub const INTEGER: ScopedCoreTypeId = ScopedCoreTypeId::fixture(10);
pub const FLOAT: ScopedCoreTypeId = ScopedCoreTypeId::fixture(9);
pub const TEXT: ScopedCoreTypeId = ScopedCoreTypeId::fixture(33);
pub const SUMMARY: ScopedCoreTypeId = ScopedCoreTypeId::fixture(32);
pub const DOCUMENTATION: ScopedCoreTypeId = ScopedCoreTypeId::fixture(31);
pub const COMMIT_SEQUENCE: ScopedCoreTypeId = ScopedCoreTypeId::fixture(1);
pub const STATE_DIGEST: ScopedCoreTypeId = ScopedCoreTypeId::fixture(2);
pub const DATABASE_MARKER: ScopedCoreTypeId = ScopedCoreTypeId::fixture(3);
pub const FIELD: ScopedCoreTypeId = ScopedCoreTypeId::fixture(23);

/// The fixture family: its stringless universe (id registry, names, and Core-layout
/// signature derivation) and the whole schema as a `CoreSchema` value.
#[derive(Clone, Debug)]
pub struct FixtureFamily {
    universe: CoreUniverse,
    schema: CoreSchema,
}

impl FixtureFamily {
    /// Build the family: intern the names, construct the real declarations, and
    /// register every type in the universe.
    pub fn build() -> Self {
        let mut builder = CoreUniverseBuilder::new();

        // Scalar leaf primitives. `Text` is the string leaf the rejoin chain ends in.
        builder
            .primitive(INTEGER, "Integer", ScalarSlot::Integer)
            .expect("fixture namespace capacity");
        builder
            .primitive(TEXT, "Text", ScalarSlot::Text)
            .expect("fixture namespace capacity");
        builder
            .primitive_leaf(FLOAT, "Float")
            .expect("fixture namespace capacity");

        // The Field meta-type has one bare positional constructor.
        builder
            .field_meta(FIELD, "Field")
            .expect("fixture namespace capacity");

        // Newtypes over Integer.
        let commit_sequence = builder
            .intern("CommitSequence")
            .expect("fixture namespace capacity");
        let state_digest = builder
            .intern("StateDigest")
            .expect("fixture namespace capacity");
        let text_name = builder.intern("Text").expect("fixture namespace capacity");
        let summary_name = builder
            .intern("Summary")
            .expect("fixture namespace capacity");
        let documentation_name = builder
            .intern("Documentation")
            .expect("fixture namespace capacity");
        let database_marker = builder
            .intern("DatabaseMarker")
            .expect("fixture namespace capacity");

        // Struct field names are ALWAYS the type-derived snake_case name — field names
        // are illegal in text (psyche ruling 2026-07-19), so a field's name is a pure
        // function of its type. The two `StateDigest` fields therefore derive the SAME
        // name `state_digest`; position, not the name, tells them apart.
        let commit_field = builder
            .intern("commit_sequence")
            .expect("fixture namespace capacity");
        let state_field = builder
            .intern("state_digest")
            .expect("fixture namespace capacity");

        let commit_declaration = CoreDeclaration::public(CoreType::Newtype(CoreNewtype::new(
            commit_sequence,
            CoreReference::Integer,
        )));
        let state_declaration = CoreDeclaration::public(CoreType::Newtype(CoreNewtype::new(
            state_digest,
            CoreReference::Integer,
        )));
        let summary_declaration = CoreDeclaration::public(CoreType::Newtype(CoreNewtype::new(
            summary_name,
            CoreReference::Plain(text_name),
        )));
        let documentation_declaration = CoreDeclaration::public(CoreType::Newtype(
            CoreNewtype::new(documentation_name, CoreReference::Plain(summary_name)),
        ));
        let database_declaration = CoreDeclaration::public(CoreType::Struct(CoreStruct::new(
            database_marker,
            vec![
                CoreField::new(commit_field, CoreReference::Plain(commit_sequence)),
                CoreField::new(state_field, CoreReference::Plain(state_digest)),
                CoreField::new(state_field, CoreReference::Plain(state_digest)),
            ],
        )));

        builder.declaration(COMMIT_SEQUENCE, commit_declaration.clone());
        builder.declaration(STATE_DIGEST, state_declaration.clone());
        builder.declaration(SUMMARY, summary_declaration.clone());
        builder.declaration(DOCUMENTATION, documentation_declaration.clone());
        builder.declaration(DATABASE_MARKER, database_declaration.clone());

        let universe = builder.build(CORE_UNIVERSE);
        let schema = CoreSchema::new(vec![
            commit_declaration,
            state_declaration,
            summary_declaration,
            documentation_declaration,
            database_declaration,
        ]);
        Self { universe, schema }
    }

    pub fn universe(&self) -> &CoreUniverse {
        &self.universe
    }

    pub fn schema(&self) -> &CoreSchema {
        &self.schema
    }

    /// The standard authored structural table (brace newtype bodies).
    pub fn standard_table(&self) -> Result<AddressedStructuralTable, UniverseError> {
        self.table(Delimiter::Brace, b"fixture-lexicon-standard".to_vec(), 1)
    }

    /// An authored structural table whose newtype-declaration bodies use `delimiter`.
    /// Varying the delimiter (with a distinct lexicon and revision) yields a table
    /// that differs from another only in textual form — the law-4 material.
    pub fn table(
        &self,
        delimiter: Delimiter,
        lexicon: Vec<u8>,
        revision: u32,
    ) -> Result<AddressedStructuralTable, UniverseError> {
        let entries = self
            .entries(delimiter)
            .into_iter()
            .map(|entry| (entry.core_type, entry))
            .collect();
        self.seal_entries(entries, lexicon, revision)
    }

    /// A table whose `CommitSequence` codec signature is deliberately wrong (empty,
    /// where the Core layout has `[Integer]`). It is the negative control for the
    /// signature-vs-Core guard: `CoreUniverse::validate_table` must reject it loudly.
    pub fn corrupted_table(&self) -> Result<AddressedStructuralTable, UniverseError> {
        let mut entries: BTreeMap<ScopedCoreTypeId, StructuralEntry> = self
            .entries(Delimiter::Brace)
            .into_iter()
            .map(|entry| (entry.core_type, entry))
            .collect();
        if let Some(entry) = entries.get_mut(&COMMIT_SEQUENCE) {
            entry.constructors[0].signature = PositionalSignature::default();
        }
        self.seal_entries(entries, b"fixture-lexicon-corrupted".to_vec(), 99)
    }

    /// The Core layout identity these forms target — the schema's own content hash,
    /// tying the table to the exact stringless Core it decodes and encodes.
    fn core_layout(&self) -> Result<CoreLayoutIdentity, UniverseError> {
        self.schema
            .content_identity()
            .map(|hash| CoreLayoutIdentity(*hash.bytes()))
            .map_err(|error| match error {
                crate::error::CoreIdentityError::Archive(archive) => {
                    UniverseError::Table(structural_codec::TableError::Archive(archive))
                }
            })
    }

    fn seal_entries(
        &self,
        entries: BTreeMap<ScopedCoreTypeId, StructuralEntry>,
        lexicon: Vec<u8>,
        revision: u32,
    ) -> Result<AddressedStructuralTable, UniverseError> {
        let payload = TableIdentityPayload {
            core_universe: CORE_UNIVERSE,
            core_layout_identity: self.core_layout()?,
            raw_profile_identity: RawProfileIdentity([1u8; 32]),
            committed_lexicon: lexicon,
            leaf_codec_contracts: Vec::new(),
            entries,
        };
        Ok(AddressedStructuralTable::seal(
            StructuralRevision::new(revision),
            payload,
        )?)
    }

    /// The authored structural entries. Signatures are written explicitly here — as
    /// a table author would — so that validating them against the Core layout is a
    /// real check, not a tautology.
    fn entries(&self, newtype_delimiter: Delimiter) -> Vec<StructuralEntry> {
        vec![
            Self::leaf_entry(INTEGER, ScalarLeaf::Integer),
            Self::leaf_entry(FLOAT, ScalarLeaf::Float),
            Self::leaf_entry(TEXT, ScalarLeaf::Text),
            Self::delegate_entry(SUMMARY, TEXT),
            Self::delegate_entry(DOCUMENTATION, SUMMARY),
            self.newtype_entry(COMMIT_SEQUENCE, INTEGER, newtype_delimiter),
            self.newtype_entry(STATE_DIGEST, INTEGER, newtype_delimiter),
            Self::struct_entry(),
            Self::field_entry(),
        ]
    }

    /// A leaf primitive: one constructor, a scalar leaf form, empty signature.
    fn leaf_entry(core_type: ScopedCoreTypeId, scalar: ScalarLeaf) -> StructuralEntry {
        let form = StructuralForm::Leaf(LeafForm::scalar(scalar));
        StructuralEntry::new(
            core_type,
            vec![ConstructorCodec::new(
                CoreConstructorId::new(core_type, 0),
                vec![form.clone()],
                form,
                PositionalSignature::default(),
            )],
        )
    }

    /// A transparent newtype value wrapper: one constructor delegating to the inner
    /// type. Its signature is `[inner]` — the wrapped reference's type.
    fn delegate_entry(core_type: ScopedCoreTypeId, inner: ScopedCoreTypeId) -> StructuralEntry {
        let form = StructuralForm::Delegate(inner);
        StructuralEntry::new(
            core_type,
            vec![ConstructorCodec::new(
                CoreConstructorId::new(core_type, 0),
                vec![form.clone()],
                form,
                PositionalSignature::new(vec![inner]),
            )],
        )
    }

    /// A newtype declaration `Object.{ Inner }`, authored from the object-prefixed
    /// vocabulary and normalized to the kernel. Signature `[inner]`.
    fn newtype_entry(
        &self,
        core_type: ScopedCoreTypeId,
        inner: ScopedCoreTypeId,
        delimiter: Delimiter,
    ) -> StructuralEntry {
        let form = AuthoringForm::ObjectPrefixed(ObjectSymbolPrefixedBlock {
            object: AtomForm::with_case(CaseExpectation::PascalCase),
            delimiter,
            sequence: SequenceForm::Product(vec![StructuralForm::pascal_atom()]),
        })
        .normalize();
        StructuralEntry::new(
            core_type,
            vec![ConstructorCodec::new(
                CoreConstructorId::new(core_type, 0),
                vec![form.clone()],
                form,
                PositionalSignature::new(vec![inner]),
            )],
        )
    }

    /// The `DatabaseMarker` struct declaration `Object.{ Field Field Field }` — a
    /// fixed product of exactly three delegated fields, matching its three Core
    /// fields. Signature `[CommitSequence StateDigest StateDigest]` — the fields'
    /// referenced types, in order.
    fn struct_entry() -> StructuralEntry {
        let field_count = 3;
        let form = StructuralForm::application(
            StructuralForm::pascal_atom(),
            StructuralForm::Delimited {
                delimiter: Delimiter::Brace,
                sequence: SequenceForm::Product(
                    std::iter::repeat_with(|| StructuralForm::Delegate(FIELD))
                        .take(field_count)
                        .collect(),
                ),
            },
        );
        StructuralEntry::new(
            DATABASE_MARKER,
            vec![ConstructorCodec::new(
                CoreConstructorId::new(DATABASE_MARKER, 0),
                vec![form.clone()],
                form,
                PositionalSignature::new(vec![COMMIT_SEQUENCE, STATE_DIGEST, STATE_DIGEST]),
            )],
        )
    }

    /// The `Field` meta-type: ONE positional constructor, the bare type reference.
    /// Field names are illegal in every Protos surface (psyche ruling 2026-07-19:
    /// "field names are now COMPLETLY ILLEGAL EVERYWHERE"), so a field carries nothing
    /// but the type standing at its position — an explicit `name.Type` no longer
    /// parses. The signature is empty: a field's payload is a name atom, not a typed
    /// sub-structure.
    fn field_entry() -> StructuralEntry {
        let type_only = StructuralForm::pascal_atom();
        StructuralEntry::new(
            FIELD,
            vec![ConstructorCodec::new(
                CoreConstructorId::new(FIELD, 0),
                vec![type_only.clone()],
                type_only,
                PositionalSignature::default(),
            )],
        )
    }
}
