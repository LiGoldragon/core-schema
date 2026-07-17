//! The stringless by-kind type reference. Mirrors `schema-language`'s
//! `CoreReference` one-for-one: scalar leaves are structure, a `Plain` reference
//! names a declaration by [`Identifier`] (never a string), and every generic
//! application dispatches on its **kind and projection**, never on a head string
//! — the "generics lower by kind" ruling made concrete in the type.
//!
//! The projection enums are lifted verbatim from the ground truth so a future
//! convergence onto the release train is a rename, not a re-derivation:
//! [`SingleTypeReferenceProjection`] `{ Vector | Optional | ScopeOf }`,
//! [`MultiTypeReferenceProjection`] `{ Map }`, [`ValueReferenceProjection`]
//! `{ Bytes }`.

use name_table::{Identifier, Name, NameInterner, NameResolver, NameTableError};

/// A single-type generic application's lowering strategy, by kind.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum SingleTypeReferenceProjection {
    Vector,
    Optional,
    ScopeOf,
}

/// A multi-type generic application's lowering strategy, by kind.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultiTypeReferenceProjection {
    Map,
}

/// A value-carrying generic application's lowering strategy, by kind.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueReferenceProjection {
    Bytes,
}

/// A type at a reference position in the stringless substrate. Scalar leaves and
/// the value width are structure; `Plain` and each application dispatch by kind
/// and projection, never by a head string, keeping the substrate stringless.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
#[rkyv(
    serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator, __S::Error: rkyv::rancor::Source),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext, __C::Error: rkyv::rancor::Source)),
)]
pub enum CoreReference {
    String,
    Integer,
    Boolean,
    Bytes,
    Plain(Identifier),
    SingleTypeApplication {
        projection: SingleTypeReferenceProjection,
        #[rkyv(omit_bounds)]
        argument: Box<CoreReference>,
    },
    MultiTypeApplication {
        projection: MultiTypeReferenceProjection,
        #[rkyv(omit_bounds)]
        arguments: Vec<CoreReference>,
    },
    ValueApplication {
        projection: ValueReferenceProjection,
        value: u64,
    },
}

impl CoreReference {
    /// The `snake_case` field name this reference derives, used to decide whether a
    /// field's text name may be elided (the derived-name rule). It is the
    /// [`field_name`](Name::field_name) of the type name this reference presents in
    /// text ([`type_name`](Self::type_name)): the `snake_case` of a `Plain`
    /// reference's resolved name, or of a scalar leaf's own type spelling. Resolving
    /// a `Plain` name goes through the table, so a rename of the referenced type
    /// moves the derived name with no stored name data — exactly the ground-truth
    /// behaviour.
    ///
    /// Single home for the derived-name spelling. Because it delegates to
    /// [`type_name`](Self::type_name), the derived name always agrees with the type
    /// spelling the reference shows in text. The scalar cases previously carried
    /// hardcoded lowercase spellings; every one already matched the type-name route
    /// except the string leaf, whose text spelling is `Text` (not `String`), so an
    /// elided string field now derives `text` rather than `string`. LEAN: this
    /// reconciles the codec/lowering divergence toward the type-name spelling and is
    /// revisable by changing the string leaf's [`type_name`](Self::type_name).
    pub fn derived_field_name<Resolver: NameResolver + ?Sized>(
        &self,
        names: &Resolver,
    ) -> Result<String, NameTableError> {
        Ok(self.type_name(names)?.field_name())
    }

    /// This reference with every name identifier re-stamped from a `source` name
    /// space into a `canonical` one: a `Plain` reference's target name is resolved
    /// through `source` and re-interned into `canonical`, and each generic
    /// application re-stamps its argument(s) the same way. Scalar leaves and value
    /// applications carry no identifier, so they are returned unchanged. This is how
    /// the authority-provided universe path
    /// ([`CoreUniverse::from_assignment`](crate::universe::CoreUniverse::from_assignment))
    /// makes a `Plain` cross-reference's stored identifier a deterministic function of
    /// the canonical interning order rather than of the order the source parsed.
    pub fn restamp<Source, Canonical>(
        &self,
        source: &Source,
        canonical: &mut Canonical,
    ) -> Result<Self, NameTableError>
    where
        Source: NameResolver + ?Sized,
        Canonical: NameInterner + ?Sized,
    {
        Ok(match self {
            Self::String | Self::Integer | Self::Boolean | Self::Bytes => self.clone(),
            Self::Plain(identifier) => {
                let name = source.resolve(*identifier)?.clone();
                Self::Plain(canonical.intern(name))
            }
            Self::SingleTypeApplication {
                projection,
                argument,
            } => Self::SingleTypeApplication {
                projection: *projection,
                argument: Box::new(argument.restamp(source, canonical)?),
            },
            Self::MultiTypeApplication {
                projection,
                arguments,
            } => Self::MultiTypeApplication {
                projection: *projection,
                arguments: arguments
                    .iter()
                    .map(|argument| argument.restamp(source, canonical))
                    .collect::<Result<_, _>>()?,
            },
            Self::ValueApplication { projection, value } => Self::ValueApplication {
                projection: *projection,
                value: *value,
            },
        })
    }

    /// Classify a type name met at a reference position into a by-kind reference:
    /// a scalar keyword becomes the matching leaf, any other name a `Plain`
    /// reference carrying the identifier the name already interned to. This is the
    /// decode-side inverse of [`type_name`](Self::type_name).
    pub fn from_type_name(name: &Name, identifier: Identifier) -> Self {
        match name.as_str() {
            "Integer" => Self::Integer,
            "Text" => Self::String,
            "Boolean" => Self::Boolean,
            "Bytes" => Self::Bytes,
            _ => Self::Plain(identifier),
        }
    }

    /// The identifier of the type-name atom this reference presents in text: a
    /// `Plain` reference reuses its stored identifier; a scalar leaf interns its
    /// keyword. `None` for a generic application, which has no single type-name atom
    /// in this proof-of-concept.
    pub fn type_atom_identifier<Interner: NameInterner + ?Sized>(
        &self,
        interner: &mut Interner,
    ) -> Option<Identifier> {
        match self {
            Self::Plain(identifier) => Some(*identifier),
            Self::String => Some(interner.intern(Name::new("Text"))),
            Self::Integer => Some(interner.intern(Name::new("Integer"))),
            Self::Boolean => Some(interner.intern(Name::new("Boolean"))),
            Self::Bytes => Some(interner.intern(Name::new("Bytes"))),
            Self::SingleTypeApplication { .. }
            | Self::MultiTypeApplication { .. }
            | Self::ValueApplication { .. } => None,
        }
    }

    /// The `PascalCase` object name a scalar leaf presents in text (its type name),
    /// or the resolved name of a `Plain` reference. This is how a reference names
    /// its type at a use site — a `CommitSequence` atom, an `Integer` atom.
    pub fn type_name<Resolver: NameResolver + ?Sized>(
        &self,
        names: &Resolver,
    ) -> Result<Name, NameTableError> {
        Ok(match self {
            Self::String => Name::new("Text"),
            Self::Integer => Name::new("Integer"),
            Self::Boolean => Name::new("Boolean"),
            Self::Bytes => Name::new("Bytes"),
            Self::Plain(identifier) => names.resolve(*identifier)?.clone(),
            Self::SingleTypeApplication { argument, .. } => argument.type_name(names)?,
            Self::MultiTypeApplication { .. } => Name::new("Map"),
            Self::ValueApplication { .. } => Name::new("Bytes"),
        })
    }
}
