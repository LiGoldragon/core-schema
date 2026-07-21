//! The stringless `CoreSchema` declaration family, modelled on `schema-language`'s
//! `CoreType { Struct | Enum | Newtype }`. Every name is an [`Identifier`] into the
//! [`NameTable`]; the declarations carry no strings, so a rename is a table-only
//! edit that never moves a Core value's content identity.
//!
//! [`NameTable`]: name_table::NameTable

use content_identity::{ContentHash, DomainSeparation, HashDomain, LayoutVersion};
use name_table::{Identifier, NameResolver, NameTableError};

use crate::error::{CoreIdentityError, CoreSchemaError, StreamingRelationReference};
use crate::reference::CoreReference;

/// The hash domain for stringless CoreSchema values, layout-version tagged. A
/// CoreSchema value's identity is blake3 over its stringless rkyv bytes under this
/// domain; the NameTable is not in the pre-image, so identity is rename-stable.
pub struct CoreSchemaDomain;

impl HashDomain for CoreSchemaDomain {
    fn separation() -> DomainSeparation {
        DomainSeparation::Contextual {
            context: "core-schema 2026 stringless core schema layer",
            // Layout 5: `StreamingRelation` is closed encoded protocol data on
            // CoreSchema. Layout 4 introduced namespace-variant `u16` identifiers;
            // both layout changes are intentional producer-to-consumer breaks. Old
            // schema packages are regenerated with their accompanying NameTable rather
            // than decoded as sliced identifiers. Layout 3 carried interface roles.
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
pub struct CoreSchema {
    declarations: Vec<CoreDeclaration>,
    streaming_relations: Vec<StreamingRelation>,
}

/// One reusable subscription protocol relation, entirely in encoded data.
///
/// The relation links an input opener and output acknowledgement by their ordered
/// interface-variant identifiers, then names the encoded references for its token,
/// event, and close-token values. A downstream signal projection generates the
/// existing streaming-frame topology from this relation; Spirit is not named here
/// and no source spelling is implied by this data model.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct StreamingRelation {
    opening_input_variant: Identifier,
    acknowledgement_output_variant: Identifier,
    token: CoreReference,
    event: CoreReference,
    close_token: CoreReference,
}

impl StreamingRelation {
    /// Construct one closed subscription protocol relation.
    pub fn new(
        opening_input_variant: Identifier,
        acknowledgement_output_variant: Identifier,
        token: CoreReference,
        event: CoreReference,
        close_token: CoreReference,
    ) -> Self {
        Self {
            opening_input_variant,
            acknowledgement_output_variant,
            token,
            event,
            close_token,
        }
    }

    pub fn opening_input_variant(&self) -> Identifier {
        self.opening_input_variant
    }

    pub fn acknowledgement_output_variant(&self) -> Identifier {
        self.acknowledgement_output_variant
    }

    pub fn token(&self) -> &CoreReference {
        &self.token
    }

    pub fn event(&self) -> &CoreReference {
        &self.event
    }

    pub fn close_token(&self) -> &CoreReference {
        &self.close_token
    }

    /// Validate this relation in its owning schema. A relation is valid only when
    /// its endpoints are variants of the role-correct interface enumerations and
    /// its encoded references resolve in that same schema.
    pub fn validate_in(&self, schema: &CoreSchema) -> Result<(), CoreSchemaError> {
        let input = schema
            .input()
            .ok_or(CoreSchemaError::MissingInputInterface)?;
        let CoreType::Enumeration(input) = input.value() else {
            return Err(CoreSchemaError::InterfaceRootNotEnumeration(
                DeclarationRole::InterfaceInput,
            ));
        };
        if !input
            .variants()
            .iter()
            .any(|variant| variant.identifier() == self.opening_input_variant)
        {
            return Err(CoreSchemaError::OpeningEndpointNotInputVariant(
                self.opening_input_variant,
            ));
        }

        let output = schema
            .output()
            .ok_or(CoreSchemaError::MissingOutputInterface)?;
        let CoreType::Enumeration(output) = output.value() else {
            return Err(CoreSchemaError::InterfaceRootNotEnumeration(
                DeclarationRole::InterfaceOutput,
            ));
        };
        if !output
            .variants()
            .iter()
            .any(|variant| variant.identifier() == self.acknowledgement_output_variant)
        {
            return Err(CoreSchemaError::AcknowledgementEndpointNotOutputVariant(
                self.acknowledgement_output_variant,
            ));
        }

        Self::validate_reference(schema, self.token(), StreamingRelationReference::Token)?;
        Self::validate_reference(schema, self.event(), StreamingRelationReference::Event)?;
        Self::validate_reference(
            schema,
            self.close_token(),
            StreamingRelationReference::CloseToken,
        )?;
        Ok(())
    }

    fn validate_reference(
        schema: &CoreSchema,
        reference: &CoreReference,
        part: StreamingRelationReference,
    ) -> Result<(), CoreSchemaError> {
        match reference {
            CoreReference::String
            | CoreReference::Integer
            | CoreReference::Boolean
            | CoreReference::Bytes
            | CoreReference::ValueApplication { .. } => Ok(()),
            CoreReference::Plain(identifier) => schema.declaration(*identifier).map(|_| ()).ok_or(
                CoreSchemaError::UnresolvedStreamingReference {
                    part,
                    identifier: *identifier,
                },
            ),
            CoreReference::SingleTypeApplication { argument, .. } => {
                Self::validate_reference(schema, argument, part)
            }
            CoreReference::MultiTypeApplication { arguments, .. } => arguments
                .iter()
                .try_for_each(|argument| Self::validate_reference(schema, argument, part)),
        }
    }
}

impl CoreSchema {
    /// A schema over the given declaration substrate, without streaming relations.
    pub fn new(declarations: Vec<CoreDeclaration>) -> Self {
        Self {
            declarations,
            streaming_relations: Vec::new(),
        }
    }

    /// Construct a schema with closed streaming protocol relations. The relation law
    /// is checked against this exact declaration substrate: opening endpoints are
    /// input-interface variants, acknowledgement endpoints are output-interface
    /// variants, and every encoded relation reference resolves here.
    pub fn with_streaming_relations(
        declarations: Vec<CoreDeclaration>,
        streaming_relations: Vec<StreamingRelation>,
    ) -> Result<Self, CoreSchemaError> {
        let schema = Self {
            declarations,
            streaming_relations,
        };
        schema.validate_streaming_relations()?;
        Ok(schema)
    }

    pub fn declarations(&self) -> &[CoreDeclaration] {
        &self.declarations
    }

    /// The reusable streaming protocol relations this schema declares, in order.
    pub fn streaming_relations(&self) -> &[StreamingRelation] {
        &self.streaming_relations
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

    fn declaration(&self, identifier: Identifier) -> Option<&CoreDeclaration> {
        self.declarations
            .iter()
            .find(|declaration| declaration.identifier() == identifier)
    }

    fn validate_streaming_relations(&self) -> Result<(), CoreSchemaError> {
        for relation in &self.streaming_relations {
            relation.validate_in(self)?;
        }
        Ok(())
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

    /// Lower a braced declaration body — the `Name.{ Field* }` form — into its
    /// canonical Core type. A single-field body lowers to a [`Newtype`](Self::Newtype)
    /// over that field's reference (the field name is dropped, exactly as a `Name.Ref`
    /// newtype carries none); any other arity is a [`Struct`](Self::Struct). This is
    /// the single home for the single-field-brace rule (psyche ruling 2026-07-17, bead
    /// `primary-56d1.36`), so the native document decode converges byte-for-byte onto
    /// the legacy lowering (`schema-language`'s `MacroExpansionStructBody::lower_type`,
    /// which collapses a one-field struct body to a newtype the same way).
    pub fn from_braced_body(identifier: Identifier, mut fields: Vec<CoreField>) -> Self {
        if fields.len() == 1 {
            let field = fields.remove(0);
            Self::Newtype(CoreNewtype::new(identifier, field.reference().clone()))
        } else {
            Self::Struct(CoreStruct::new(identifier, fields))
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
