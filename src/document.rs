//! The six-slot document grammar: the universe types and authored `structural-codec`
//! forms that let [`TextualSchema`] decode a whole spirit-min-shaped document —
//! `imports {} input [] output [] types {} generics {} impls {}` — into a full
//! [`EncodedSchema`], and encode it back.
//!
//! Unlike the per-declaration fixture universe ([`crate::fixture`]), these are the
//! GRAMMAR types self-hosted in `schema-language`'s `root.schema`: a `TypeReference`
//! disjoint (scalar leaves, single-type projections, and the `Declared` name form),
//! a `Declaration` disjoint (newtype, struct, enumeration), the `types` and interface
//! brackets, and the `Field` meta-type. Decoding dispatches by KIND and PROJECTION
//! through the disjoint constructors — a scalar or projection keyword is matched as a
//! `Literal`, and the winning constructor index (never a head string a reifier reads
//! ad hoc) names the Encoded reference kind. [`ReferenceConstructor`] and
//! [`DeclarationConstructor`] name those constructor-index contact points so the
//! authored table and the reifier can never drift.
//!
//! [`TextualSchema`]: crate::textual::TextualSchema
//! [`EncodedSchema`]: crate::declaration::EncodedSchema

use std::collections::BTreeMap;

use raw_discovery::Delimiter;
use structural_codec::ids::{EncodedConstructorId, PositionalSignature, ScopedEncodedTypeId};
use structural_codec::table::{
    AddressedStructuralTable, EncodedLayoutIdentity, RawProfileIdentity, TableIdentityPayload,
};
use structural_codec::{ConstructorCodec, SequenceForm, StructuralEntry, StructuralForm};

use crate::error::UniverseError;
use crate::universe::ENCODED_UNIVERSE;

/// The `TypeReference` grammar type: a reference met at a use site.
pub const TYPE_REFERENCE: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(100);
/// The `Field` meta-type: a bare positional `Type` struct field — field names are
/// illegal, so there is no `name.Type` form.
pub const FIELD: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(101);
/// The `Declaration` grammar type: a newtype, struct, or enumeration declaration.
pub const DECLARATION: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(102);
/// The `types` block: a brace of declarations.
pub const TYPES_BLOCK: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(103);
/// One interface entry: a `Name.Payload` mail-type binding.
pub const INTERFACE_VARIANT: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(104);
/// An interface line: a bracket of interface entries (the `input` / `output` slot).
pub const INTERFACE: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(105);

/// The number of root slots in the document layout: `imports input output types
/// generics impls`, in that order.
pub const DOCUMENT_SLOTS: usize = 6;

/// The disjoint constructors of the [`TYPE_REFERENCE`] grammar type, in the fixed
/// order the authored table lists them and the reifier reads them. The index of the
/// winning constructor — not a head string — names the Encoded reference kind, so the
/// dispatch is by kind and projection. `Declared` is last and excludes every builtin
/// spelling, so its disjointness is structural rather than a constructor-order rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceConstructor {
    /// A bare reference name atom.
    Name,
    /// A name applied to a reference payload.
    Application,
}

impl ReferenceConstructor {
    /// Every reference constructor, in authored-table order.
    pub const ALL: [Self; 2] = [Self::Name, Self::Application];

    /// This constructor's index in [`ALL`](Self::ALL) — its `constructor` id.
    pub fn index(self) -> u32 {
        Self::ALL
            .iter()
            .position(|constructor| *constructor == self)
            .expect("every constructor is in ALL") as u32
    }

    /// The constructor a decode chose, by its index.
    pub fn from_index(index: u32) -> Option<Self> {
        Self::ALL.get(index as usize).copied()
    }
}

/// The disjoint constructors of the [`DECLARATION`] grammar type, in authored-table
/// order. The winning index names the declared Encoded type's shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationConstructor {
    Newtype,
    Struct,
    Enumeration,
}

impl DeclarationConstructor {
    /// Every declaration constructor, in authored-table order.
    pub const ALL: [Self; 3] = [Self::Newtype, Self::Struct, Self::Enumeration];

    /// This constructor's index in [`ALL`](Self::ALL).
    pub fn index(self) -> u32 {
        Self::ALL
            .iter()
            .position(|constructor| *constructor == self)
            .expect("every constructor is in ALL") as u32
    }

    /// The constructor a decode chose, by its index.
    pub fn from_index(index: u32) -> Option<Self> {
        Self::ALL.get(index as usize).copied()
    }
}

/// The document grammar as a sealed `structural-codec` table. Its two reference
/// forms are structural only; builtin meaning is supplied by the universe during
/// reification and reflection.
#[derive(Clone, Debug)]
pub struct SchemaDocumentGrammar {
    table: AddressedStructuralTable,
}

impl SchemaDocumentGrammar {
    /// Author and seal the grammar table.
    pub fn build() -> Result<Self, UniverseError> {
        let author = DocumentTableAuthor;
        let entries: BTreeMap<ScopedEncodedTypeId, StructuralEntry> = author
            .entries()?
            .into_iter()
            .map(|entry| (entry.core_type, entry))
            .collect();
        let payload = TableIdentityPayload {
            core_universe: ENCODED_UNIVERSE,
            // The grammar table targets no single Encoded layout — it decodes many — so
            // its layout identity is a fixed grammar marker, not a schema hash. Table
            // identity is excluded from Encoded value identity by construction.
            core_layout_identity: EncodedLayoutIdentity([0x6d; 32]),
            raw_profile_identity: RawProfileIdentity([1u8; 32]),
            leaf_codec_contracts: Vec::new(),
            entries,
        };
        let table = AddressedStructuralTable::seal(payload)?;
        Ok(Self { table })
    }

    /// The sealed grammar table, keyed by the grammar type ids.
    pub fn table(&self) -> &AddressedStructuralTable {
        &self.table
    }
}

/// Builds the grammar entries. Type references have exactly two textual forms:
/// a bare name atom or a name applied to a reference payload. Builtin meaning is
/// resolved only against the universe, never by grammar keywords.
struct DocumentTableAuthor;

impl DocumentTableAuthor {
    fn reference_forms(
        &self,
    ) -> Result<Vec<(ReferenceConstructor, StructuralForm)>, UniverseError> {
        Ok(vec![
            (ReferenceConstructor::Name, StructuralForm::pascal_atom()),
            (
                ReferenceConstructor::Application,
                StructuralForm::application(
                    StructuralForm::pascal_atom(),
                    StructuralForm::delegate(TYPE_REFERENCE),
                ),
            ),
        ])
    }

    /// A single-constructor entry: one disjoint decode form, the same canonical encode
    /// form, and an empty signature (the grammar is not signature-validated against a
    /// Encoded layout — see [`SchemaDocumentGrammar`]).
    fn single(core_type: ScopedEncodedTypeId, form: StructuralForm) -> StructuralEntry {
        StructuralEntry::new(
            core_type,
            vec![ConstructorCodec::new(
                EncodedConstructorId::new(core_type, 0),
                vec![form.clone()],
                form,
                PositionalSignature::default(),
            )],
        )
    }

    fn entries(&self) -> Result<Vec<StructuralEntry>, UniverseError> {
        Ok(vec![
            self.type_reference_entry()?,
            Self::field_entry(),
            self.declaration_entry(),
            Self::types_block_entry(),
            Self::interface_variant_entry(),
            Self::interface_entry(),
        ])
    }

    /// The `TypeReference` disjoint: bare name atom or name-applied payload.
    fn type_reference_entry(&self) -> Result<StructuralEntry, UniverseError> {
        let constructors = self
            .reference_forms()?
            .into_iter()
            .map(|(constructor, form)| {
                ConstructorCodec::new(
                    EncodedConstructorId::new(TYPE_REFERENCE, constructor.index()),
                    vec![form.clone()],
                    form,
                    PositionalSignature::default(),
                )
            })
            .collect();
        Ok(StructuralEntry::new(TYPE_REFERENCE, constructors))
    }

    /// The `Field` meta-type: a bare positional `Type`, and nothing else. Field names
    /// are illegal in every Protos surface (psyche ruling 2026-07-19: "field names are
    /// now COMPLETLY ILLEGAL EVERYWHERE"), so an explicit `name.Type` no longer parses
    /// — a struct field is only the type standing at its position. Field types are
    /// plain name atoms here, sufficient for the spirit-min structs whose fields are
    /// all plain declared types.
    fn field_entry() -> StructuralEntry {
        let type_only = StructuralForm::pascal_atom();
        StructuralEntry::new(
            FIELD,
            vec![ConstructorCodec::new(
                EncodedConstructorId::new(FIELD, 0),
                vec![type_only.clone()],
                type_only,
                PositionalSignature::default(),
            )],
        )
    }

    /// The `Declaration` disjoint: newtype `Name.Reference`, struct `Name.{ Field* }`,
    /// or enumeration `Name.[ Variant* ]`. Newtype delegates to `TypeReference`,
    /// retaining the reference constructor in its decoded value. Table sealing expands
    /// that delegate against the complete grammar to prove the alternatives disjoint.
    fn declaration_entry(&self) -> StructuralEntry {
        let newtype = StructuralForm::application(
            StructuralForm::pascal_atom(),
            StructuralForm::delegate(TYPE_REFERENCE),
        );
        let structure = StructuralForm::application(
            StructuralForm::pascal_atom(),
            StructuralForm::Delimited {
                delimiter: Delimiter::Brace,
                sequence: SequenceForm::zero_or_more(StructuralForm::delegate(FIELD)),
            },
        );
        let enumeration = StructuralForm::application(
            StructuralForm::pascal_atom(),
            StructuralForm::Delimited {
                delimiter: Delimiter::SquareBracket,
                sequence: SequenceForm::zero_or_more(StructuralForm::pascal_atom()),
            },
        );
        let forms = [newtype, structure, enumeration];
        let constructors = DeclarationConstructor::ALL
            .iter()
            .zip(forms)
            .map(|(constructor, form)| {
                ConstructorCodec::new(
                    EncodedConstructorId::new(DECLARATION, constructor.index()),
                    vec![form.clone()],
                    form,
                    PositionalSignature::default(),
                )
            })
            .collect();
        StructuralEntry::new(DECLARATION, constructors)
    }

    /// The `types` block: a brace of declaration delegates.
    fn types_block_entry() -> StructuralEntry {
        Self::single(
            TYPES_BLOCK,
            StructuralForm::Delimited {
                delimiter: Delimiter::Brace,
                sequence: SequenceForm::zero_or_more(StructuralForm::delegate(DECLARATION)),
            },
        )
    }

    /// One interface entry: `Name.Payload`, the payload a `TypeReference`.
    fn interface_variant_entry() -> StructuralEntry {
        Self::single(
            INTERFACE_VARIANT,
            StructuralForm::application(
                StructuralForm::pascal_atom(),
                StructuralForm::delegate(TYPE_REFERENCE),
            ),
        )
    }

    /// An interface line: a bracket of interface-entry delegates.
    fn interface_entry() -> StructuralEntry {
        Self::single(
            INTERFACE,
            StructuralForm::Delimited {
                delimiter: Delimiter::SquareBracket,
                sequence: SequenceForm::zero_or_more(StructuralForm::delegate(INTERFACE_VARIANT)),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::SchemaDocumentGrammar;

    #[test]
    fn full_grammar_seals() {
        SchemaDocumentGrammar::build().expect("the complete document grammar seals");
    }
}
