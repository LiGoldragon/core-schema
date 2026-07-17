//! The stringless `CoreSchema` declaration family, modelled on `schema-language`'s
//! `CoreType { Struct | Enum | Newtype }`. Every name is an [`Identifier`] into the
//! [`NameTable`]; the declarations carry no strings, so a rename is a table-only
//! edit that never moves a Core value's content identity.
//!
//! [`NameTable`]: name_table::NameTable

use content_identity::{ContentHash, DomainSeparation, HashDomain, LayoutVersion};
use name_table::{Identifier, NameResolver, NameTableError};

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
            // Layout 3: interface-root-ness is now carried by [`DeclarationRole`] on
            // each [`CoreDeclaration`] — the two protocol lines are ordinary
            // declarations tagged `InterfaceInput` / `InterfaceOutput`, no longer a
            // separate pair of interface slots (layout 2). A layout-2 value (slots)
            // and a layout-3 value (role-tagged declarations) hash under different
            // layout versions, as the storage-schema change demands.
            layout: LayoutVersion::new(3),
        }
    }
}

/// A loaded schema as a whole: one stringless declaration substrate in which the
/// document's two protocol lines live as ordinary declarations, distinguished by
/// their [`DeclarationRole`]. Names live in the accompanying `NameTable` produced
/// by the same decode.
///
/// The six-slot document layout (imports, input, output, types, generics, impls)
/// lands its `types` block and both interface brackets in the SAME
/// [`declarations`](Self::declarations) list: an `input` / `output` bracket becomes
/// a public enumeration declaration whose variants are the bracket's `Name.Payload`
/// bindings, tagged [`DeclarationRole::InterfaceInput`] /
/// [`DeclarationRole::InterfaceOutput`]; every `types` declaration is tagged
/// [`DeclarationRole::DataType`]. This is the SINGLE
/// representation of interface-root-ness shared by the native document decode and
/// legacy ingestion, and the marker downstream Nomos lowering reads to gate
/// interface-specific generation — the per-declaration lowering walk never sees the
/// interface roots unless they are declarations, so a marker on the declaration is
/// the only principled home. The imports, generics, and impls slots are not yet
/// modelled here; a document that carries content in them is rejected at decode
/// rather than silently dropped.
///
/// LEAN `interface-root-as-role`: interface roots are role-tagged declarations
/// rather than a separate interface-slot type. Trigger to revisit: the accepted
/// document-kind design review, which may reshape how interface roots and the
/// document kinds relate.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct CoreSchema {
    declarations: Vec<CoreDeclaration>,
}

impl CoreSchema {
    /// A schema over the given declaration substrate. Interface roots, when present,
    /// are the declarations carrying an interface [`DeclarationRole`].
    pub fn new(declarations: Vec<CoreDeclaration>) -> Self {
        Self { declarations }
    }

    pub fn declarations(&self) -> &[CoreDeclaration] {
        &self.declarations
    }

    /// The declarations that are ordinary data types — every declaration whose role
    /// is [`DeclarationRole::DataType`], the `types` block of the document layout.
    pub fn data_declarations(&self) -> impl Iterator<Item = &CoreDeclaration> {
        self.declarations
            .iter()
            .filter(|declaration| declaration.role() == DeclarationRole::DataType)
    }

    /// The document's input interface root — the declaration tagged
    /// [`DeclarationRole::InterfaceInput`], if the document carried one.
    pub fn input(&self) -> Option<&CoreDeclaration> {
        self.role_declaration(DeclarationRole::InterfaceInput)
    }

    /// The document's output interface root — the declaration tagged
    /// [`DeclarationRole::InterfaceOutput`], if the document carried one.
    pub fn output(&self) -> Option<&CoreDeclaration> {
        self.role_declaration(DeclarationRole::InterfaceOutput)
    }

    fn role_declaration(&self, role: DeclarationRole) -> Option<&CoreDeclaration> {
        self.declarations
            .iter()
            .find(|declaration| declaration.role() == role)
    }

    /// This schema's content identity, blake3 over its stringless rkyv bytes with
    /// the NameTable excluded by construction — a rename cannot move it.
    pub fn content_identity(&self) -> Result<ContentHash<CoreSchemaDomain>, CoreIdentityError> {
        Ok(ContentHash::of_core(self)?)
    }
}

/// Whether a declaration is an ordinary data type or one of the document's two
/// interface roots — the `input` / `output` protocol lines. This is the single
/// marker of interface-root-ness: the native document decode and legacy ingestion
/// both set it, and Nomos lowering reads it to gate interface-specific generation.
/// A closed typed record, never a boolean flag.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationRole {
    /// An ordinary type declaration — the document's `types` block.
    DataType,
    /// The `input` protocol line: the mail types a component accepts.
    InterfaceInput,
    /// The `output` protocol line: the mail types a component emits.
    InterfaceOutput,
}

impl DeclarationRole {
    /// The canonical declaration name an interface root carries — `Input` for the
    /// input line, `Output` for the output line, `None` for an ordinary data type.
    /// This is the position name `schema-language`'s legacy lowering assigns, so the
    /// native document decode and legacy ingestion mint the same interface-root name.
    pub fn interface_root_name(self) -> Option<&'static str> {
        match self {
            Self::DataType => None,
            Self::InterfaceInput => Some("Input"),
            Self::InterfaceOutput => Some("Output"),
        }
    }
}

/// One namespace declaration: a visibility, its [`DeclarationRole`], and the type it
/// declares. The declaration's identity is its value's type identifier (the
/// `Declaration`-name invariant of the ground truth: a declaration's name is always
/// its value's name).
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct CoreDeclaration {
    visibility: Visibility,
    role: DeclarationRole,
    value: CoreType,
}

impl CoreDeclaration {
    /// An ordinary data-type declaration ([`DeclarationRole::DataType`]).
    pub fn new(visibility: Visibility, value: CoreType) -> Self {
        Self {
            visibility,
            role: DeclarationRole::DataType,
            value,
        }
    }

    /// A public data-type declaration.
    pub fn public(value: CoreType) -> Self {
        Self::new(Visibility::Public, value)
    }

    /// A public interface-root declaration carrying its interface role. Interface
    /// roots are always public: they are a component's protocol surface.
    pub fn interface(role: DeclarationRole, value: CoreType) -> Self {
        Self {
            visibility: Visibility::Public,
            role,
            value,
        }
    }

    pub fn visibility(&self) -> Visibility {
        self.visibility
    }

    /// Whether this declaration is a data type or an interface root.
    pub fn role(&self) -> DeclarationRole {
        self.role
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

    /// This type with its own name identifier replaced, the rest of its shape kept.
    /// The authority-provided universe path
    /// ([`CoreUniverse::from_assignment`](crate::universe::CoreUniverse::from_assignment))
    /// uses it to re-stamp a declaration's name with the canonically-interned
    /// identifier, so the declaration's own identity is a deterministic function of the
    /// authority's assignment rather than of parse-order interning.
    pub fn with_identifier(&self, identifier: Identifier) -> Self {
        match self {
            Self::Newtype(newtype) => {
                Self::Newtype(CoreNewtype::new(identifier, newtype.reference().clone()))
            }
            Self::Struct(structure) => {
                Self::Struct(CoreStruct::new(identifier, structure.fields().to_vec()))
            }
            Self::Enumeration(enumeration) => {
                Self::Enumeration(CoreEnum::new(identifier, enumeration.variants().to_vec()))
            }
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

    /// Whether this field's stored name is exactly the one its reference derives —
    /// the single predicate that decides text-name elision. When true, the name may
    /// be elided in text (and re-derived on decode) or re-derived on Nomos lowering;
    /// when false the name is explicit and must be carried verbatim. This is the one
    /// home for the derive-versus-preserve decision shared by the textual codec
    /// ([`crate::textual`]) and Nomos field lowering, so the two never drift.
    pub fn name_is_derivable<Resolver: NameResolver + ?Sized>(
        &self,
        names: &Resolver,
    ) -> Result<bool, NameTableError> {
        let stored = names.resolve(self.identifier)?;
        Ok(stored.as_str() == self.reference.derived_field_name(names)?)
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
