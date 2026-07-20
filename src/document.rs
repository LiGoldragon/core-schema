//! The seven-slot document grammar: the universe types and authored `structural-codec`
//! forms that let [`TextualSchema`] decode a whole spirit-min-shaped document —
//! `imports {} input [] output [] types {} generics {} impls {}` — into a full
//! [`EncodedSchema`], and encode it back.
//!
//! Unlike the per-declaration fixture universe ([`crate::fixture`]), these are the
//! GRAMMAR types self-hosted in `schema-language`'s `root.schema`: a `TypeReference`
//! disjoint (scalar leaves, single-type projections, and the `Plain` name fallback),
//! a `Declaration` disjoint (newtype, struct, enumeration), the `types` and interface
//! brackets, and the `Field` meta-type. Decoding dispatches by KIND and PROJECTION
//! through the disjoint constructors — a scalar or projection keyword is matched as a
//! `Literal`, and the winning constructor index (never a head string a reifier reads
//! ad hoc) names the Core reference kind. [`ReferenceConstructor`] and
//! [`DeclarationConstructor`] name those constructor-index contact points so the
//! authored table and the reifier can never drift.
//!
//! [`TextualSchema`]: crate::textual::TextualSchema
//! [`EncodedSchema`]: crate::declaration::EncodedSchema

use std::collections::BTreeMap;

use name_table::{IdentifierNamespace, Name, NameTable};
use raw_discovery::Delimiter;
use structural_codec::ids::{
    EncodedConstructorId, PositionalSignature, ScopedEncodedTypeId, StructuralRevision,
};
use structural_codec::table::{
    AddressedStructuralTable, EncodedLayoutIdentity, RawProfileIdentity, TableIdentityPayload,
};
use structural_codec::{ConstructorCodec, SequenceForm, StructuralEntry, StructuralForm};

use crate::error::UniverseError;
use crate::reference::{EncodedReference, SingleTypeReferenceProjection};
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
/// generics impls streaming`, in that order. Imports remain manifest dependency
/// edges; the trailing streaming slot is a typed relation vector.
pub const DOCUMENT_SLOTS: usize = 7;

/// The first streaming-relation slot: an input-interface variant identifier.
pub const STREAMING_INPUT_VARIANT_REFERENCE: ScopedEncodedTypeId =
    ScopedEncodedTypeId::fixture(106);
/// The second streaming-relation slot: an output-interface variant identifier.
pub const STREAMING_OUTPUT_VARIANT_REFERENCE: ScopedEncodedTypeId =
    ScopedEncodedTypeId::fixture(107);
/// The token, event, and close-token streaming-relation slots.
pub const STREAMING_REFERENCE: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(108);
/// One ordered five-position streaming relation.
pub const STREAMING_RELATION: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(109);
/// The trailing bracketed vector of streaming relations.
pub const STREAMING_RELATIONS: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(110);

/// The disjoint constructors of the [`TYPE_REFERENCE`] grammar type, in the fixed
/// order the authored table lists them and the reifier reads them. The index of the
/// winning constructor — not a head string — names the Core reference kind, so the
/// dispatch is by kind and projection. `Plain` is last: it matches any name atom, so
/// every keyword constructor is tried before it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceConstructor {
    Integer,
    String,
    Boolean,
    Bytes,
    Vector,
    Optional,
    ScopeOf,
    Plain,
}

impl ReferenceConstructor {
    /// Every reference constructor, in authored-table order.
    pub const ALL: [Self; 8] = [
        Self::Integer,
        Self::String,
        Self::Boolean,
        Self::Bytes,
        Self::Vector,
        Self::Optional,
        Self::ScopeOf,
        Self::Plain,
    ];

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

    /// The reference constructor a single-type projection lowers through.
    pub fn from_single_projection(projection: SingleTypeReferenceProjection) -> Self {
        match projection {
            SingleTypeReferenceProjection::Vector => Self::Vector,
            SingleTypeReferenceProjection::Optional => Self::Optional,
            SingleTypeReferenceProjection::ScopeOf => Self::ScopeOf,
        }
    }

    /// The scalar leaf reference this constructor decodes to, if it is a scalar.
    pub fn scalar(self) -> Option<EncodedReference> {
        match self {
            Self::Integer => Some(EncodedReference::Integer),
            Self::String => Some(EncodedReference::String),
            Self::Boolean => Some(EncodedReference::Boolean),
            Self::Bytes => Some(EncodedReference::Bytes),
            Self::Vector | Self::Optional | Self::ScopeOf | Self::Plain => None,
        }
    }

    /// The single-type projection this constructor applies, if it is a projection.
    pub fn single_projection(self) -> Option<SingleTypeReferenceProjection> {
        match self {
            Self::Vector => Some(SingleTypeReferenceProjection::Vector),
            Self::Optional => Some(SingleTypeReferenceProjection::Optional),
            Self::ScopeOf => Some(SingleTypeReferenceProjection::ScopeOf),
            Self::Integer | Self::String | Self::Boolean | Self::Bytes | Self::Plain => None,
        }
    }

    /// The grammar keyword this constructor matches (a scalar or projection keyword),
    /// or `None` for the `Plain` fallback which matches any name atom. `String` is the
    /// string leaf's keyword — its canonical spelling under the 2026-07-17 ruling
    /// ("Strings are Strings"); `Text` is no longer a recognized spelling (no aliases).
    pub fn keyword(self) -> Option<&'static str> {
        match self {
            Self::Integer => Some("Integer"),
            Self::String => Some("String"),
            Self::Boolean => Some("Boolean"),
            Self::Bytes => Some("Bytes"),
            Self::Vector => Some("Vector"),
            Self::Optional => Some("Optional"),
            Self::ScopeOf => Some("ScopeOf"),
            Self::Plain => None,
        }
    }
}

/// The disjoint constructors of the [`DECLARATION`] grammar type, in authored-table
/// order. The winning index names the declared Core type's shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationConstructor {
    Newtype,
    Struct,
    Enumeration,
}

/// The two disjoint shapes of one ordered interface alternative. A bare PascalCase
/// atom is unit; a glued-dot application carries exactly one typed payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterfaceVariantConstructor {
    Unit,
    Payload,
}

impl InterfaceVariantConstructor {
    pub const ALL: [Self; 2] = [Self::Unit, Self::Payload];

    pub fn index(self) -> u32 {
        Self::ALL
            .iter()
            .position(|constructor| *constructor == self)
            .expect("every interface constructor is in ALL") as u32
    }

    pub fn from_index(index: u32) -> Option<Self> {
        Self::ALL.get(index as usize).copied()
    }
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

/// The document grammar as a sealed `structural-codec` table plus the keyword
/// lexicon its `Literal` forms resolve through. One value drives both decode
/// (`with_lexicon`) and encode of the seven-slot layout.
#[derive(Clone, Debug)]
pub struct SchemaDocumentGrammar {
    table: AddressedStructuralTable,
    lexicon: NameTable,
}

impl SchemaDocumentGrammar {
    /// Author and seal the grammar table with its keyword lexicon.
    pub fn build() -> Result<Self, UniverseError> {
        let mut author = DocumentTableAuthor::new();
        let entries: BTreeMap<ScopedEncodedTypeId, StructuralEntry> = author
            .entries()
            .into_iter()
            .map(|entry| (entry.core_type, entry))
            .collect();
        let payload = TableIdentityPayload {
            core_universe: ENCODED_UNIVERSE,
            // The grammar table targets no single Core layout — it decodes many — so
            // its layout identity is a fixed grammar marker, not a schema hash. Table
            // identity is excluded from Core value identity by construction.
            core_layout_identity: EncodedLayoutIdentity([0x6d; 32]),
            raw_profile_identity: RawProfileIdentity([1u8; 32]),
            committed_lexicon: b"core-schema-document-grammar".to_vec(),
            leaf_codec_contracts: Vec::new(),
            entries,
        };
        let table = AddressedStructuralTable::seal(StructuralRevision::new(2), payload)?;
        Ok(Self {
            table,
            lexicon: author.into_lexicon(),
        })
    }

    /// The sealed grammar table, keyed by the grammar type ids.
    pub fn table(&self) -> &AddressedStructuralTable {
        &self.table
    }

    /// The keyword lexicon the `TypeReference` literals resolve through on decode.
    pub fn lexicon(&self) -> &NameTable {
        &self.lexicon
    }
}

/// Builds the grammar entries, owning the keyword lexicon its `Literal` forms index.
struct DocumentTableAuthor {
    lexicon: NameTable,
}

impl DocumentTableAuthor {
    fn new() -> Self {
        Self {
            lexicon: NameTable::new(IdentifierNamespace::Schema),
        }
    }

    fn into_lexicon(self) -> NameTable {
        self.lexicon
    }

    /// A `Literal` form matching an interned keyword verbatim.
    fn literal(&mut self, keyword: &str) -> StructuralForm {
        // This private grammar author owns an unshared fixed-size lexicon. Public
        // NameTable mutation remains fallible at the composition boundary.
        StructuralForm::Literal(
            self.lexicon
                .intern(Name::new(keyword))
                .expect("fixed grammar keyword fits its unborrowed lexicon"),
        )
    }

    /// A single-constructor entry: one disjoint decode form, the same canonical encode
    /// form, and an empty signature (the grammar is not signature-validated against a
    /// Core layout — see [`SchemaDocumentGrammar`]).
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

    fn entries(&mut self) -> Vec<StructuralEntry> {
        vec![
            self.type_reference_entry(),
            Self::field_entry(),
            self.declaration_entry(),
            Self::types_block_entry(),
            Self::interface_variant_entry(),
            Self::interface_entry(),
            Self::streaming_reference_entry(STREAMING_INPUT_VARIANT_REFERENCE),
            Self::streaming_reference_entry(STREAMING_OUTPUT_VARIANT_REFERENCE),
            Self::streaming_reference_entry(STREAMING_REFERENCE),
            Self::streaming_relation_entry(),
            Self::streaming_relations_entry(),
        ]
    }

    /// The `TypeReference` disjoint: a scalar keyword `Literal`, a projection
    /// `keyword.TypeReference` application, or a bare name atom (`Plain`, last).
    fn type_reference_entry(&mut self) -> StructuralEntry {
        let constructors = ReferenceConstructor::ALL
            .iter()
            .map(|constructor| {
                let form = match constructor.keyword() {
                    None => StructuralForm::pascal_atom(),
                    Some(keyword) if constructor.single_projection().is_some() => {
                        StructuralForm::application(
                            self.literal(keyword),
                            StructuralForm::Delegate(TYPE_REFERENCE),
                        )
                    }
                    Some(keyword) => self.literal(keyword),
                };
                ConstructorCodec::new(
                    EncodedConstructorId::new(TYPE_REFERENCE, constructor.index()),
                    vec![form.clone()],
                    form,
                    PositionalSignature::default(),
                )
            })
            .collect();
        StructuralEntry::new(TYPE_REFERENCE, constructors)
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
    /// or enumeration `Name.[ Variant* ]`.
    fn declaration_entry(&mut self) -> StructuralEntry {
        let newtype = StructuralForm::application(
            StructuralForm::pascal_atom(),
            StructuralForm::Delegate(TYPE_REFERENCE),
        );
        let structure = StructuralForm::application(
            StructuralForm::pascal_atom(),
            StructuralForm::Delimited {
                delimiter: Delimiter::Brace,
                sequence: SequenceForm::zero_or_more(StructuralForm::Delegate(FIELD)),
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
                sequence: SequenceForm::zero_or_more(StructuralForm::Delegate(DECLARATION)),
            },
        )
    }

    /// One interface alternative: either a unit PascalCase atom or a glued-dot
    /// application carrying one typed payload. The expected `InterfaceVariant`
    /// position selects this closed two-constructor algebra.
    fn interface_variant_entry() -> StructuralEntry {
        let unit = StructuralForm::pascal_atom();
        let payload = StructuralForm::application(
            StructuralForm::pascal_atom(),
            StructuralForm::Delegate(TYPE_REFERENCE),
        );
        let forms = [unit, payload];
        let constructors = InterfaceVariantConstructor::ALL
            .iter()
            .zip(forms)
            .map(|(constructor, form)| {
                ConstructorCodec::new(
                    EncodedConstructorId::new(INTERFACE_VARIANT, constructor.index()),
                    vec![form.clone()],
                    form,
                    PositionalSignature::default(),
                )
            })
            .collect();
        StructuralEntry::new(INTERFACE_VARIANT, constructors)
    }

    /// An interface line: a bracket of interface-alternative delegates.
    fn interface_entry() -> StructuralEntry {
        Self::single(
            INTERFACE,
            StructuralForm::Delimited {
                delimiter: Delimiter::SquareBracket,
                sequence: SequenceForm::zero_or_more(StructuralForm::Delegate(INTERFACE_VARIANT)),
            },
        )
    }

    /// A streaming relation reference has one PascalCase-atom form; its enclosing
    /// typed position gives it input, output, token, event, or close-token meaning.
    fn streaming_reference_entry(core_type: ScopedEncodedTypeId) -> StructuralEntry {
        Self::single(core_type, StructuralForm::pascal_atom())
    }

    /// One closed streaming relation: opener, acknowledgement, token, event, close
    /// token. The five meanings come only from their ordered expected types.
    fn streaming_relation_entry() -> StructuralEntry {
        Self::single(
            STREAMING_RELATION,
            StructuralForm::Delimited {
                delimiter: Delimiter::Brace,
                sequence: SequenceForm::Product(vec![
                    StructuralForm::Delegate(STREAMING_INPUT_VARIANT_REFERENCE),
                    StructuralForm::Delegate(STREAMING_OUTPUT_VARIANT_REFERENCE),
                    StructuralForm::Delegate(STREAMING_REFERENCE),
                    StructuralForm::Delegate(STREAMING_REFERENCE),
                    StructuralForm::Delegate(STREAMING_REFERENCE),
                ]),
            },
        )
    }

    /// The trailing document slot is a homogeneous vector of closed relations.
    fn streaming_relations_entry() -> StructuralEntry {
        Self::single(
            STREAMING_RELATIONS,
            StructuralForm::Delimited {
                delimiter: Delimiter::SquareBracket,
                sequence: SequenceForm::zero_or_more(StructuralForm::Delegate(STREAMING_RELATION)),
            },
        )
    }
}
