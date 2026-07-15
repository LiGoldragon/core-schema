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
    /// field's text name may be elided (the derived-name rule). For a `Plain`
    /// reference it is the `snake_case` of the referenced type's name; for a scalar
    /// leaf it is the scalar's own lowercase spelling. Resolving a `Plain` name goes
    /// through the table, so a rename of the referenced type moves the derived name
    /// with no stored name data — exactly the ground-truth behaviour.
    pub fn derived_field_name<Resolver: NameResolver + ?Sized>(
        &self,
        names: &Resolver,
    ) -> Result<String, NameTableError> {
        Ok(match self {
            Self::String => "string".to_owned(),
            Self::Integer => "integer".to_owned(),
            Self::Boolean => "boolean".to_owned(),
            Self::Bytes => "bytes".to_owned(),
            Self::Plain(identifier) => names.resolve(*identifier)?.field_name(),
            Self::SingleTypeApplication { argument, .. } => argument.derived_field_name(names)?,
            Self::MultiTypeApplication { .. } => "map".to_owned(),
            Self::ValueApplication { .. } => "bytes".to_owned(),
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
