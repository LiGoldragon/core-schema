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

use name_table::{IdentifierNamespace, Name, NameTable};
use raw_discovery::Delimiter;
use structural_codec::ids::{
    EncodedConstructorId, PositionalSignature, ScopedEncodedTypeId, StructuralRevision,
};
use structural_codec::table::{
    AddressedStructuralTable, EncodedLayoutIdentity, RawProfileIdentity, TableIdentityPayload,
};
use structural_codec::{
    AtomForm, CaseExpectation, ConstructorCodec, SequenceForm, StructuralEntry, StructuralForm,
};

use crate::error::UniverseError;
use crate::reference::{BuiltinReference, EncodedReference, SingleTypeReferenceProjection};
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
    Integer,
    String,
    Boolean,
    Bytes,
    Vector,
    Optional,
    ScopeOf,
    Declared,
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
        Self::Declared,
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
            Self::Vector | Self::Optional | Self::ScopeOf | Self::Declared => None,
        }
    }

    /// The single-type projection this constructor applies, if it is a projection.
    pub fn single_projection(self) -> Option<SingleTypeReferenceProjection> {
        match self {
            Self::Vector => Some(SingleTypeReferenceProjection::Vector),
            Self::Optional => Some(SingleTypeReferenceProjection::Optional),
            Self::ScopeOf => Some(SingleTypeReferenceProjection::ScopeOf),
            Self::Integer | Self::String | Self::Boolean | Self::Bytes | Self::Declared => None,
        }
    }

    /// The builtin this constructor matches, if any. `Declared` is the only
    /// constructor that carries a user-declared name.
    pub fn builtin(self) -> Option<BuiltinReference> {
        match self {
            Self::Integer => Some(BuiltinReference::Integer),
            Self::String => Some(BuiltinReference::String),
            Self::Boolean => Some(BuiltinReference::Boolean),
            Self::Bytes => Some(BuiltinReference::Bytes),
            Self::Vector => Some(BuiltinReference::Vector),
            Self::Optional => Some(BuiltinReference::Optional),
            Self::ScopeOf => Some(BuiltinReference::ScopeOf),
            Self::Declared => None,
        }
    }

    /// The grammar keyword this constructor matches (a scalar or projection keyword),
    /// or `None` for the declared-name form. `String` is the string leaf's canonical
    /// spelling; `Text` is no longer a recognized spelling.
    pub fn keyword(self) -> Option<&'static str> {
        self.builtin().map(BuiltinReference::spelling)
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

/// The document grammar as a sealed `structural-codec` table plus the keyword
/// lexicon its `Literal` forms resolve through. One value drives both decode
/// (`with_lexicon`) and encode of the six-slot layout.
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
            committed_lexicon: DocumentTableAuthor::committed_lexicon(),
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
    fn literal(&mut self, keyword: &str) -> Result<StructuralForm, UniverseError> {
        Ok(StructuralForm::Literal(
            self.lexicon.intern(Name::new(keyword))?,
        ))
    }

    /// The exact builtin spellings committed into this grammar table's identity.
    fn committed_lexicon() -> Vec<u8> {
        BuiltinReference::ALL
            .iter()
            .enumerate()
            .flat_map(|(index, builtin)| {
                (index != 0)
                    .then_some(b'\0')
                    .into_iter()
                    .chain(builtin.spelling().bytes())
            })
            .collect()
    }

    /// Intern every builtin spelling exactly once and return their committed lexicon
    /// identifiers for a declared-name form to exclude.
    fn builtin_literals(&mut self) -> Result<Vec<name_table::Identifier>, UniverseError> {
        BuiltinReference::ALL
            .iter()
            .map(|builtin| self.lexicon.intern(Name::new(builtin.spelling())))
            .collect::<Result<_, _>>()
            .map_err(UniverseError::from)
    }

    /// Every direct decode form of a type reference. The declared-name form excludes
    /// the complete builtin lexicon, so every scalar literal and every declared name
    /// are provably disjoint before this table can seal.
    fn reference_forms(
        &mut self,
    ) -> Result<Vec<(ReferenceConstructor, StructuralForm)>, UniverseError> {
        let excluded_literals = self.builtin_literals()?;
        ReferenceConstructor::ALL
            .iter()
            .map(|constructor| {
                let constructor = *constructor;
                let form = match constructor {
                    ReferenceConstructor::Declared => {
                        StructuralForm::Atom(AtomForm::excluding_literals(
                            CaseExpectation::PascalCase,
                            excluded_literals.clone(),
                        ))
                    }
                    _ if constructor.single_projection().is_some() => StructuralForm::application(
                        self.literal(
                            constructor
                                .keyword()
                                .expect("a projection constructor is builtin"),
                        )?,
                        StructuralForm::Delegate(TYPE_REFERENCE),
                    ),
                    _ => self.literal(
                        constructor
                            .keyword()
                            .expect("a scalar constructor is builtin"),
                    )?,
                };
                Ok((constructor, form))
            })
            .collect()
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

    fn entries(&mut self) -> Result<Vec<StructuralEntry>, UniverseError> {
        Ok(vec![
            self.type_reference_entry()?,
            Self::field_entry(),
            self.declaration_entry(),
            Self::types_block_entry(),
            Self::interface_variant_entry(),
            Self::interface_entry(),
        ])
    }

    /// The `TypeReference` disjoint: a scalar keyword `Literal`, a projection
    /// `keyword.TypeReference` application, or a bare declared-name atom.
    fn type_reference_entry(&mut self) -> Result<StructuralEntry, UniverseError> {
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

    /// One interface entry: `Name.Payload`, the payload a `TypeReference`.
    fn interface_variant_entry() -> StructuralEntry {
        Self::single(
            INTERFACE_VARIANT,
            StructuralForm::application(
                StructuralForm::pascal_atom(),
                StructuralForm::Delegate(TYPE_REFERENCE),
            ),
        )
    }

    /// An interface line: a bracket of interface-entry delegates.
    fn interface_entry() -> StructuralEntry {
        Self::single(
            INTERFACE,
            StructuralForm::Delimited {
                delimiter: Delimiter::SquareBracket,
                sequence: SequenceForm::zero_or_more(StructuralForm::Delegate(INTERFACE_VARIANT)),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use name_table::{IdentifierNamespace, NameTable};
    use raw_discovery::Recognizer;
    use structural_codec::{ConstructorCodec, StructuralEvaluator, StructuralValue};

    use super::{
        AddressedStructuralTable, DocumentTableAuthor, EncodedConstructorId, EncodedLayoutIdentity,
        PositionalSignature, RawProfileIdentity, ReferenceConstructor, SchemaDocumentGrammar,
        StructuralEntry, StructuralRevision, TYPE_REFERENCE, TableIdentityPayload,
    };
    use crate::universe::ENCODED_UNIVERSE;

    fn reference_entry(
        forms: Vec<(ReferenceConstructor, structural_codec::StructuralForm)>,
    ) -> StructuralEntry {
        StructuralEntry::new(
            TYPE_REFERENCE,
            forms
                .into_iter()
                .map(|(constructor, form)| {
                    ConstructorCodec::new(
                        EncodedConstructorId::new(TYPE_REFERENCE, constructor.index()),
                        vec![form.clone()],
                        form,
                        PositionalSignature::default(),
                    )
                })
                .collect(),
        )
    }

    fn seal_reference_table(
        forms: Vec<(ReferenceConstructor, structural_codec::StructuralForm)>,
    ) -> AddressedStructuralTable {
        AddressedStructuralTable::seal(
            StructuralRevision::new(2),
            TableIdentityPayload {
                core_universe: ENCODED_UNIVERSE,
                core_layout_identity: EncodedLayoutIdentity([0x6d; 32]),
                raw_profile_identity: RawProfileIdentity([1u8; 32]),
                committed_lexicon: DocumentTableAuthor::committed_lexicon(),
                leaf_codec_contracts: Vec::new(),
                entries: BTreeMap::from([(TYPE_REFERENCE, reference_entry(forms))]),
            },
        )
        .expect("the reference forms are provably disjoint")
    }

    #[test]
    fn full_grammar_seals_and_builtin_decode_ignores_constructor_order() {
        SchemaDocumentGrammar::build().expect("the complete document grammar seals");

        let mut author = DocumentTableAuthor::new();
        let ordered_forms = author.reference_forms().expect("author reference forms");
        let lexicon = author.into_lexicon();
        let ordered = seal_reference_table(ordered_forms.clone());
        let mut reversed_forms = ordered_forms;
        reversed_forms.reverse();
        let reversed = seal_reference_table(reversed_forms);
        let block = Recognizer::standard()
            .recognize("Integer")
            .expect("recognize builtin")
            .root_object_at(0)
            .expect("one builtin root")
            .clone();

        for table in [&ordered, &reversed] {
            let mut names = NameTable::new(IdentifierNamespace::Schema);
            let value = StructuralEvaluator::with_lexicon(table, &lexicon)
                .decode(TYPE_REFERENCE, &block, &mut names)
                .expect("decode builtin under either constructor order");
            let StructuralValue::Chosen {
                constructor,
                payload,
            } = value
            else {
                panic!("reference decode chooses a constructor");
            };
            assert_eq!(constructor, ReferenceConstructor::Integer.index());
            let StructuralValue::Atom(identifier) = payload.as_ref() else {
                panic!("integer literal carries its atom");
            };
            assert_eq!(
                names
                    .resolve(*identifier)
                    .expect("interned builtin")
                    .as_str(),
                "Integer"
            );
        }
    }
}
