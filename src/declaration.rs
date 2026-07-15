//! The stringless `CoreSchema` declaration family, modelled on `schema-language`'s
//! `CoreType { Struct | Enum | Newtype }`. Every name is an [`Identifier`] into the
//! [`NameTable`]; the declarations carry no strings, so a rename is a table-only
//! edit that never moves a Core value's content identity.
//!
//! [`NameTable`]: name_table::NameTable

use content_identity::{ContentHash, DomainSeparation, HashDomain, LayoutVersion};
use name_table::Identifier;

use crate::error::CoreIdentityError;
use crate::reference::CoreReference;

/// The hash domain for stringless CoreSchema values, layout-version tagged. A
/// CoreSchema value's identity is blake3 over its stringless rkyv bytes under this
/// domain; the NameTable is not in the pre-image, so identity is rename-stable.
pub struct CoreSchemaDomain;

impl HashDomain for CoreSchemaDomain {
    fn separation() -> DomainSeparation {
        DomainSeparation::Contextual {
            context: "core-schema 2026 stringless core schema layer",
            layout: LayoutVersion::new(1),
        }
    }
}

/// A loaded schema as a whole: the stringless declaration substrate. Names live in
/// the accompanying `NameTable` produced by the same decode.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct CoreSchema {
    declarations: Vec<CoreDeclaration>,
}

impl CoreSchema {
    pub fn new(declarations: Vec<CoreDeclaration>) -> Self {
        Self { declarations }
    }

    pub fn declarations(&self) -> &[CoreDeclaration] {
        &self.declarations
    }

    /// This schema's content identity, blake3 over its stringless rkyv bytes with
    /// the NameTable excluded by construction — a rename cannot move it.
    pub fn content_identity(&self) -> Result<ContentHash<CoreSchemaDomain>, CoreIdentityError> {
        Ok(ContentHash::of_core(self)?)
    }
}

/// One namespace declaration: a visibility and the type it declares. The
/// declaration's identity is its value's type identifier (the `Declaration`-name
/// invariant of the ground truth: a declaration's name is always its value's name).
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct CoreDeclaration {
    visibility: Visibility,
    value: CoreType,
}

impl CoreDeclaration {
    pub fn new(visibility: Visibility, value: CoreType) -> Self {
        Self { visibility, value }
    }

    /// A public declaration.
    pub fn public(value: CoreType) -> Self {
        Self::new(Visibility::Public, value)
    }

    pub fn visibility(&self) -> Visibility {
        self.visibility
    }

    pub fn value(&self) -> &CoreType {
        &self.value
    }

    /// The declaration's identifier — carried by its value.
    pub fn identifier(&self) -> Identifier {
        self.value.identifier()
    }
}

/// Whether a declaration is exported. A closed typed record, never a boolean flag.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    Public,
    Private,
}

/// A declared type body, mirroring `schema-language`'s `CoreType`.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum CoreType {
    Newtype(CoreNewtype),
    Struct(CoreStruct),
    Enumeration(CoreEnum),
}

impl CoreType {
    /// The declared type's identifier.
    pub fn identifier(&self) -> Identifier {
        match self {
            Self::Newtype(newtype) => newtype.identifier(),
            Self::Struct(structure) => structure.identifier(),
            Self::Enumeration(enumeration) => enumeration.identifier(),
        }
    }

    /// How many Core constructors this type has: a product (newtype, struct) has
    /// one; a sum (enumeration) has one per variant.
    pub fn constructor_count(&self) -> usize {
        match self {
            Self::Newtype(_) | Self::Struct(_) => 1,
            Self::Enumeration(enumeration) => enumeration.variants().len(),
        }
    }
}

/// A newtype declaration: a single wrapped reference.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct CoreNewtype {
    identifier: Identifier,
    reference: CoreReference,
}

impl CoreNewtype {
    pub fn new(identifier: Identifier, reference: CoreReference) -> Self {
        Self {
            identifier,
            reference,
        }
    }

    pub fn identifier(&self) -> Identifier {
        self.identifier
    }

    pub fn reference(&self) -> &CoreReference {
        &self.reference
    }
}

/// A struct declaration: an ordered list of typed fields.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct CoreStruct {
    identifier: Identifier,
    fields: Vec<CoreField>,
}

impl CoreStruct {
    pub fn new(identifier: Identifier, fields: Vec<CoreField>) -> Self {
        Self { identifier, fields }
    }

    pub fn identifier(&self) -> Identifier {
        self.identifier
    }

    pub fn fields(&self) -> &[CoreField] {
        &self.fields
    }
}

/// A struct field: its own identifier (name) and the type it references. A field
/// whose name equals the `snake_case` of its reference elides that name in text;
/// the name is then derived on demand, never stored.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct CoreField {
    identifier: Identifier,
    reference: CoreReference,
}

impl CoreField {
    pub fn new(identifier: Identifier, reference: CoreReference) -> Self {
        Self {
            identifier,
            reference,
        }
    }

    pub fn identifier(&self) -> Identifier {
        self.identifier
    }

    pub fn reference(&self) -> &CoreReference {
        &self.reference
    }
}

/// An enumeration declaration: an ordered list of variants.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct CoreEnum {
    identifier: Identifier,
    variants: Vec<CoreVariant>,
}

impl CoreEnum {
    pub fn new(identifier: Identifier, variants: Vec<CoreVariant>) -> Self {
        Self {
            identifier,
            variants,
        }
    }

    pub fn identifier(&self) -> Identifier {
        self.identifier
    }

    pub fn variants(&self) -> &[CoreVariant] {
        &self.variants
    }
}

/// An enum variant: its identifier and an optional payload reference.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct CoreVariant {
    identifier: Identifier,
    payload: Option<CoreReference>,
}

impl CoreVariant {
    pub fn new(identifier: Identifier, payload: Option<CoreReference>) -> Self {
        Self {
            identifier,
            payload,
        }
    }

    pub fn identifier(&self) -> Identifier {
        self.identifier
    }

    pub fn payload(&self) -> Option<&CoreReference> {
        self.payload.as_ref()
    }
}
