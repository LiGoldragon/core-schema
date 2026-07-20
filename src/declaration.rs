//! The stringless `EncodedSchema` declaration family, modelled on `schema-language`'s
//! `EncodedType { Struct | Enum | Newtype }`. Every name is an [`Identifier`] into the
//! [`NameTable`]; the declarations carry no strings, so a rename is a table-only
//! edit that never moves a Core value's content identity.
//!
//! [`NameTable`]: name_table::NameTable

use content_identity::{ContentHash, DomainSeparation, HashDomain, LayoutVersion};
use name_table::{Identifier, NameInterner, NameResolver, NameTableError};

use crate::error::EncodedIdentityError;
use crate::reference::EncodedReference;

/// The hash domain for stringless EncodedSchema values, layout-version tagged. A
/// EncodedSchema value's identity is blake3 over its stringless rkyv bytes under this
/// domain; the NameTable is not in the pre-image, so identity is rename-stable.
pub struct EncodedSchemaDomain;

impl HashDomain for EncodedSchemaDomain {
    fn separation() -> DomainSeparation {
        DomainSeparation::Contextual {
            context: "core-schema 2026 stringless core schema layer",
            // Layout 5 adds a closed `StreamingRelation` family to the encoded
            // schema. Layout 4 adopted namespace-variant `u16` identifiers. Both are
            // deliberate producer-to-consumer archive breaks: old schema packages
            // are regenerated with their accompanying NameTable rather than decoded
            // as sliced identifiers. Layout 3 introduced interface roles.
            layout: LayoutVersion::new(5),
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
pub struct EncodedSchema {
    declarations: Vec<EncodedDeclaration>,
    streaming_relations: Vec<StreamingRelation>,
}

/// One reusable subscription protocol relation, entirely in encoded data.
///
/// The relation links an input opener and output acknowledgement by their ordered
/// interface-variant identifiers, then names the encoded references for its token,
/// event, and close-token values. A downstream signal projection generates the
/// streaming-frame topology from this relation; no component-specific path or
/// source spelling is implied by this data model.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct StreamingRelation {
    opening_input_variant: Identifier,
    acknowledgement_output_variant: Identifier,
    token: EncodedReference,
    event: EncodedReference,
    close_token: EncodedReference,
}

impl StreamingRelation {
    /// Construct one closed subscription protocol relation.
    pub fn new(
        opening_input_variant: Identifier,
        acknowledgement_output_variant: Identifier,
        token: EncodedReference,
        event: EncodedReference,
        close_token: EncodedReference,
    ) -> Self {
        Self {
            opening_input_variant,
            acknowledgement_output_variant,
            token,
            event,
            close_token,
        }
    }

    /// The input interface variant that opens the subscription.
    pub fn opening_input_variant(&self) -> Identifier {
        self.opening_input_variant
    }

    /// The output interface variant that acknowledges opening.
    pub fn acknowledgement_output_variant(&self) -> Identifier {
        self.acknowledgement_output_variant
    }

    /// The typed subscription token carried by the relation.
    pub fn token(&self) -> &EncodedReference {
        &self.token
    }

    /// The typed event carried after subscription activation.
    pub fn event(&self) -> &EncodedReference {
        &self.event
    }

    /// The typed token accepted by the relation's close operation.
    pub fn close_token(&self) -> &EncodedReference {
        &self.close_token
    }
}

impl EncodedSchema {
    /// A schema over declarations without streaming relations.
    pub fn new(declarations: Vec<EncodedDeclaration>) -> Self {
        Self::with_streaming_relations(declarations, Vec::new())
    }

    /// A schema over declarations and closed streaming protocol relations.
    pub fn with_streaming_relations(
        declarations: Vec<EncodedDeclaration>,
        streaming_relations: Vec<StreamingRelation>,
    ) -> Self {
        Self {
            declarations,
            streaming_relations,
        }
    }

    pub fn declarations(&self) -> &[EncodedDeclaration] {
        &self.declarations
    }

    /// The reusable streaming protocol relations this schema declares, in order.
    pub fn streaming_relations(&self) -> &[StreamingRelation] {
        &self.streaming_relations
    }

    /// The declarations that are ordinary data types — every declaration whose role
    /// is [`DeclarationRole::DataType`], the `types` block of the document layout.
    pub fn data_declarations(&self) -> impl Iterator<Item = &EncodedDeclaration> {
        self.declarations
            .iter()
            .filter(|declaration| declaration.role() == DeclarationRole::DataType)
    }

    /// The document's input interface root — the declaration tagged
    /// [`DeclarationRole::InterfaceInput`], if the document carried one.
    pub fn input(&self) -> Option<&EncodedDeclaration> {
        self.role_declaration(DeclarationRole::InterfaceInput)
    }

    /// The document's output interface root — the declaration tagged
    /// [`DeclarationRole::InterfaceOutput`], if the document carried one.
    pub fn output(&self) -> Option<&EncodedDeclaration> {
        self.role_declaration(DeclarationRole::InterfaceOutput)
    }

    fn role_declaration(&self, role: DeclarationRole) -> Option<&EncodedDeclaration> {
        self.declarations
            .iter()
            .find(|declaration| declaration.role() == role)
    }

    /// This schema's content identity, blake3 over its stringless rkyv bytes with
    /// the NameTable excluded by construction — a rename cannot move it.
    pub fn content_identity(
        &self,
    ) -> Result<ContentHash<EncodedSchemaDomain>, EncodedIdentityError> {
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
pub struct EncodedDeclaration {
    visibility: Visibility,
    role: DeclarationRole,
    value: EncodedType,
}

impl EncodedDeclaration {
    /// An ordinary data-type declaration ([`DeclarationRole::DataType`]).
    pub fn new(visibility: Visibility, value: EncodedType) -> Self {
        Self {
            visibility,
            role: DeclarationRole::DataType,
            value,
        }
    }

    /// A public data-type declaration.
    pub fn public(value: EncodedType) -> Self {
        Self::new(Visibility::Public, value)
    }

    /// A public interface-root declaration carrying its interface role. Interface
    /// roots are always public: they are a component's protocol surface.
    pub fn interface(role: DeclarationRole, value: EncodedType) -> Self {
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

    pub fn value(&self) -> &EncodedType {
        &self.value
    }

    /// The declaration's identifier — carried by its value.
    pub fn identifier(&self) -> Identifier {
        self.value.identifier()
    }

    /// This declaration re-stamped into a canonical name space — its visibility and
    /// [`DeclarationRole`] preserved, its value's own name replaced with the
    /// already-canonically-interned `own`, and every interior name re-stamped through
    /// `source` into `canonical` ([`EncodedType::restamp`]). The authority-provided
    /// universe path ([`EncodedUniverse::from_assignment`](crate::universe::EncodedUniverse::from_assignment))
    /// uses it so an ingested declaration keeps its role and visibility while its
    /// stored identifiers become a deterministic function of the canonical order.
    pub fn restamp<Source, Canonical>(
        &self,
        own: Identifier,
        source: &Source,
        canonical: &mut Canonical,
    ) -> Result<Self, NameTableError>
    where
        Source: NameResolver + ?Sized,
        Canonical: NameInterner + ?Sized,
    {
        Ok(Self {
            visibility: self.visibility,
            role: self.role,
            value: self.value.restamp(own, source, canonical)?,
        })
    }
}

/// Whether a declaration is exported. A closed typed record, never a boolean flag.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    Public,
    Private,
}

/// A declared type body, mirroring `schema-language`'s `EncodedType`.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum EncodedType {
    Newtype(EncodedNewtype),
    Struct(EncodedStruct),
    Enumeration(EncodedEnum),
}

impl EncodedType {
    /// The declared type's identifier.
    pub fn identifier(&self) -> Identifier {
        match self {
            Self::Newtype(newtype) => newtype.identifier(),
            Self::Struct(structure) => structure.identifier(),
            Self::Enumeration(enumeration) => enumeration.identifier(),
        }
    }

    /// Lower a braced declaration body — the `Name.{ Field* }` form — into its
    /// canonical Core type. A single-field body lowers to a [`Newtype`](Self::Newtype)
    /// over that field's reference (the field name is dropped, exactly as a `Name.Ref`
    /// newtype carries none); any other arity is a [`Struct`](Self::Struct). This is
    /// the single home for the single-field-brace rule (psyche ruling 2026-07-17, bead
    /// `primary-56d1.36`), so the native document decode converges byte-for-byte onto
    /// the legacy lowering (`schema-language`'s `MacroExpansionStructBody::lower_type`,
    /// which collapses a one-field struct body to a newtype the same way).
    pub fn from_braced_body(identifier: Identifier, mut fields: Vec<EncodedField>) -> Self {
        if fields.len() == 1 {
            let field = fields.remove(0);
            Self::Newtype(EncodedNewtype::new(identifier, field.reference().clone()))
        } else {
            Self::Struct(EncodedStruct::new(identifier, fields))
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

    /// This type re-stamped into a canonical name space: its own name identifier
    /// replaced with the already-canonically-interned `own`, and EVERY interior name
    /// — field names, variant names, and the target of each `Plain` cross-reference —
    /// resolved through the `source` name space and re-interned into `canonical`.
    /// The authority-provided universe path
    /// ([`EncodedUniverse::from_assignment`](crate::universe::EncodedUniverse::from_assignment))
    /// uses it so the built declaration's every stored identifier is a deterministic
    /// function of the canonical interning order (the authority's assignment plus a
    /// fixed positional walk), never of the order the source parsed. Without the
    /// interior re-stamping, schemas whose declarations reference each other or carry
    /// explicit field names still hashed differently across parse orders.
    pub fn restamp<Source, Canonical>(
        &self,
        own: Identifier,
        source: &Source,
        canonical: &mut Canonical,
    ) -> Result<Self, NameTableError>
    where
        Source: NameResolver + ?Sized,
        Canonical: NameInterner + ?Sized,
    {
        Ok(match self {
            Self::Newtype(newtype) => Self::Newtype(EncodedNewtype::new(
                own,
                newtype.reference().restamp(source, canonical)?,
            )),
            Self::Struct(structure) => {
                let fields = structure
                    .fields()
                    .iter()
                    .map(|field| field.restamp(source, canonical))
                    .collect::<Result<_, _>>()?;
                Self::Struct(EncodedStruct::new(own, fields))
            }
            Self::Enumeration(enumeration) => {
                let variants = enumeration
                    .variants()
                    .iter()
                    .map(|variant| variant.restamp(source, canonical))
                    .collect::<Result<_, _>>()?;
                Self::Enumeration(EncodedEnum::new(own, variants))
            }
        })
    }
}

/// A newtype declaration: a single wrapped reference.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct EncodedNewtype {
    identifier: Identifier,
    reference: EncodedReference,
}

impl EncodedNewtype {
    pub fn new(identifier: Identifier, reference: EncodedReference) -> Self {
        Self {
            identifier,
            reference,
        }
    }

    pub fn identifier(&self) -> Identifier {
        self.identifier
    }

    pub fn reference(&self) -> &EncodedReference {
        &self.reference
    }
}

/// A struct declaration: an ordered list of typed fields.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct EncodedStruct {
    identifier: Identifier,
    fields: Vec<EncodedField>,
}

impl EncodedStruct {
    pub fn new(identifier: Identifier, fields: Vec<EncodedField>) -> Self {
        Self { identifier, fields }
    }

    pub fn identifier(&self) -> Identifier {
        self.identifier
    }

    pub fn fields(&self) -> &[EncodedField] {
        &self.fields
    }
}

/// A struct field: its own identifier (name) and the type it references. A field
/// whose name equals the `snake_case` of its reference elides that name in text;
/// the name is then derived on demand, never stored.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct EncodedField {
    identifier: Identifier,
    reference: EncodedReference,
}

impl EncodedField {
    pub fn new(identifier: Identifier, reference: EncodedReference) -> Self {
        Self {
            identifier,
            reference,
        }
    }

    pub fn identifier(&self) -> Identifier {
        self.identifier
    }

    pub fn reference(&self) -> &EncodedReference {
        &self.reference
    }

    /// This field re-stamped into a canonical name space: its own name resolved
    /// through `source` and re-interned into `canonical`, and its reference
    /// re-stamped the same way. Part of the authority-provided canonicalisation
    /// ([`EncodedType::restamp`]).
    pub fn restamp<Source, Canonical>(
        &self,
        source: &Source,
        canonical: &mut Canonical,
    ) -> Result<Self, NameTableError>
    where
        Source: NameResolver + ?Sized,
        Canonical: NameInterner + ?Sized,
    {
        let name = source.resolve(self.identifier)?.clone();
        let identifier = canonical.intern(name);
        Ok(Self::new(
            identifier,
            self.reference.restamp(source, canonical)?,
        ))
    }

    /// Whether this field's stored name is exactly the one its reference derives.
    /// Field names are illegal in every Protos surface, so the textual codec never
    /// consults this — a decoded field always carries its type-derived name. It
    /// remains the home for Nomos field lowering's derive-versus-preserve decision:
    /// when true the name is re-derived from the type, when false a programmatically
    /// constructed Core carries it verbatim.
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
pub struct EncodedEnum {
    identifier: Identifier,
    variants: Vec<EncodedVariant>,
}

impl EncodedEnum {
    pub fn new(identifier: Identifier, variants: Vec<EncodedVariant>) -> Self {
        Self {
            identifier,
            variants,
        }
    }

    pub fn identifier(&self) -> Identifier {
        self.identifier
    }

    pub fn variants(&self) -> &[EncodedVariant] {
        &self.variants
    }
}

/// An enum variant: its identifier and an optional payload reference.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct EncodedVariant {
    identifier: Identifier,
    payload: Option<EncodedReference>,
}

impl EncodedVariant {
    pub fn new(identifier: Identifier, payload: Option<EncodedReference>) -> Self {
        Self {
            identifier,
            payload,
        }
    }

    pub fn identifier(&self) -> Identifier {
        self.identifier
    }

    pub fn payload(&self) -> Option<&EncodedReference> {
        self.payload.as_ref()
    }

    /// This variant re-stamped into a canonical name space: its own name resolved
    /// through `source` and re-interned into `canonical`, and its optional payload
    /// reference re-stamped the same way. Part of the authority-provided
    /// canonicalisation ([`EncodedType::restamp`]).
    pub fn restamp<Source, Canonical>(
        &self,
        source: &Source,
        canonical: &mut Canonical,
    ) -> Result<Self, NameTableError>
    where
        Source: NameResolver + ?Sized,
        Canonical: NameInterner + ?Sized,
    {
        let name = source.resolve(self.identifier)?.clone();
        let identifier = canonical.intern(name);
        let payload = match &self.payload {
            Some(reference) => Some(reference.restamp(source, canonical)?),
            None => None,
        };
        Ok(Self::new(identifier, payload))
    }
}
