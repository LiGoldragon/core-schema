//! The stringless `EncodedSchema` declaration family, modelled on `schema-language`'s
//! `EncodedType { Struct | Enum | Newtype }`. Every name is an [`Identifier`] into the
//! [`NameTable`]; the declarations carry no strings, so a rename is a table-only
//! edit that never moves a Encoded value's content identity.
//!
//! [`NameTable`]: name_table::NameTable

use content_identity::{ContentHash, DomainSeparation, HashDomain, LayoutVersion, PortableArchive};
use name_table::{Identifier, NameResolver, NameTableError};

use crate::error::{
    EncodedIdentityError, EncodedSchemaError, EncodedSchemaLoadError, StreamingReferenceForm,
    StreamingRelationReference,
};
use crate::reference::EncodedReference;

/// The hash domain for stringless EncodedSchema values, layout-version tagged. A
/// EncodedSchema value's identity is blake3 over its stringless rkyv bytes under this
/// domain; the NameTable is not in the pre-image, so identity is rename-stable.
pub struct EncodedSchemaDomain;

impl HashDomain for EncodedSchemaDomain {
    fn separation() -> DomainSeparation {
        DomainSeparation::Contextual {
            context: "core-schema 2026 stringless core schema layer",
            // Layout 5: `StreamingRelation` is closed encoded protocol data on
            // EncodedSchema. The validated archive DTO intentionally preserves this
            // exact canonical field layout. Layout 4 introduced namespace-variant `u16` identifiers;
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedSchema {
    declarations: Vec<EncodedDeclaration>,
    streaming_relations: Vec<StreamingRelation>,
}

/// The private wire representation of [`EncodedSchema`]. Keeping rkyv on this DTO
/// makes archive bytes a validated boundary rather than a second public EncodedSchema
/// constructor. Its fields deliberately match EncodedSchema's canonical layout so
/// content identity remains over the same stringless data.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
struct EncodedSchemaArchive {
    declarations: Vec<EncodedDeclaration>,
    streaming_relations: Vec<StreamingRelation>,
}

impl From<&EncodedSchema> for EncodedSchemaArchive {
    fn from(schema: &EncodedSchema) -> Self {
        Self {
            declarations: schema.declarations.clone(),
            streaming_relations: schema.streaming_relations.clone(),
        }
    }
}

impl TryFrom<EncodedSchemaArchive> for EncodedSchema {
    type Error = EncodedSchemaError;

    fn try_from(archive: EncodedSchemaArchive) -> Result<Self, Self::Error> {
        Self::with_streaming_relations(archive.declarations, archive.streaming_relations)
    }
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

    pub fn opening_input_variant(&self) -> Identifier {
        self.opening_input_variant
    }

    pub fn acknowledgement_output_variant(&self) -> Identifier {
        self.acknowledgement_output_variant
    }

    pub fn token(&self) -> &EncodedReference {
        &self.token
    }

    pub fn event(&self) -> &EncodedReference {
        &self.event
    }

    pub fn close_token(&self) -> &EncodedReference {
        &self.close_token
    }

    /// Validate this relation in its owning schema. A relation is valid only when
    /// every carried identifier belongs to the Schema namespace, its endpoints are
    /// variants of the role-correct interface enumerations, and its encoded value
    /// references name data-type declarations in that same schema.
    pub fn validate_in(&self, schema: &EncodedSchema) -> Result<(), EncodedSchemaError> {
        schema.require_schema_identifier(self.opening_input_variant)?;
        schema.require_schema_identifier(self.acknowledgement_output_variant)?;

        let input = schema
            .input()
            .ok_or(EncodedSchemaError::MissingInputInterface)?;
        let EncodedType::Enumeration(input) = input.value() else {
            return Err(EncodedSchemaError::InterfaceRootNotEnumeration(
                DeclarationRole::InterfaceInput,
            ));
        };
        if !input
            .variants()
            .iter()
            .any(|variant| variant.identifier() == self.opening_input_variant)
        {
            return Err(EncodedSchemaError::OpeningEndpointNotInputVariant(
                self.opening_input_variant,
            ));
        }

        let output = schema
            .output()
            .ok_or(EncodedSchemaError::MissingOutputInterface)?;
        let EncodedType::Enumeration(output) = output.value() else {
            return Err(EncodedSchemaError::InterfaceRootNotEnumeration(
                DeclarationRole::InterfaceOutput,
            ));
        };
        if !output
            .variants()
            .iter()
            .any(|variant| variant.identifier() == self.acknowledgement_output_variant)
        {
            return Err(EncodedSchemaError::AcknowledgementEndpointNotOutputVariant(
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
        schema: &EncodedSchema,
        reference: &EncodedReference,
        part: StreamingRelationReference,
    ) -> Result<(), EncodedSchemaError> {
        match reference {
            EncodedReference::Plain(identifier) => {
                schema.streaming_data_type(*identifier, part).map(|_| ())
            }
            EncodedReference::String
            | EncodedReference::Integer
            | EncodedReference::Boolean
            | EncodedReference::Bytes => {
                Err(EncodedSchemaError::StreamingReferenceMustNameDataType {
                    part,
                    form: StreamingReferenceForm::Scalar,
                })
            }
            EncodedReference::ValueApplication { .. } => {
                Err(EncodedSchemaError::StreamingReferenceMustNameDataType {
                    part,
                    form: StreamingReferenceForm::BytesLength,
                })
            }
            EncodedReference::SingleTypeApplication { .. } => {
                Err(EncodedSchemaError::StreamingReferenceMustNameDataType {
                    part,
                    form: StreamingReferenceForm::SingleTypeApplication,
                })
            }
            EncodedReference::MultiTypeApplication { .. } => {
                Err(EncodedSchemaError::StreamingReferenceMustNameDataType {
                    part,
                    form: StreamingReferenceForm::MultiTypeApplication,
                })
            }
        }
    }
}

impl EncodedSchema {
    /// A schema over the given declaration substrate, without streaming relations.
    pub fn new(declarations: Vec<EncodedDeclaration>) -> Self {
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
        declarations: Vec<EncodedDeclaration>,
        streaming_relations: Vec<StreamingRelation>,
    ) -> Result<Self, EncodedSchemaError> {
        let schema = Self {
            declarations,
            streaming_relations,
        };
        schema.validate_streaming_relations()?;
        Ok(schema)
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

    fn declaration(&self, identifier: Identifier) -> Option<&EncodedDeclaration> {
        self.declarations
            .iter()
            .find(|declaration| declaration.identifier() == identifier)
    }

    /// The central role boundary for encoded streaming value references. A relation
    /// value is a declared data type, never an interface root merely because that
    /// root happens to carry the same schema-local identifier.
    fn streaming_data_type(
        &self,
        identifier: Identifier,
        part: StreamingRelationReference,
    ) -> Result<&EncodedDeclaration, EncodedSchemaError> {
        self.require_schema_identifier(identifier)?;
        let declaration = self
            .declaration(identifier)
            .ok_or(EncodedSchemaError::UnresolvedStreamingReference { part, identifier })?;
        if declaration.role() != DeclarationRole::DataType {
            return Err(EncodedSchemaError::StreamingReferenceNotDataType {
                part,
                identifier,
                actual: declaration.role(),
            });
        }
        Ok(declaration)
    }

    /// EncodedSchema is Schema-owned data. Relation boundaries must not accept a
    /// foreign namespace identifier simply because another declaration matches it.
    fn require_schema_identifier(&self, identifier: Identifier) -> Result<(), EncodedSchemaError> {
        if matches!(identifier, Identifier::Schema(_)) {
            Ok(())
        } else {
            Err(EncodedSchemaError::NonSchemaIdentifier(identifier))
        }
    }

    fn validate_streaming_relations(&self) -> Result<(), EncodedSchemaError> {
        for relation in &self.streaming_relations {
            relation.validate_in(self)?;
        }
        Ok(())
    }

    /// Archive this schema through its private wire DTO. The domain type itself
    /// intentionally has no raw rkyv surface, so every load must pass semantic
    /// relation validation.
    pub fn to_archive_bytes(&self) -> Result<rkyv::util::AlignedVec, EncodedSchemaLoadError> {
        Ok(EncodedSchemaArchive::from(self).to_archive_bytes()?)
    }

    /// Load and validate a EncodedSchema archive. Archive corruption and a valid rkyv
    /// payload that violates the EncodedSchema relation law are distinct typed errors.
    pub fn from_archive_bytes(bytes: &[u8]) -> Result<Self, EncodedSchemaLoadError> {
        let archive = EncodedSchemaArchive::from_archive_bytes(bytes)?;
        Ok(Self::try_from(archive)?)
    }

    /// This schema's content identity, blake3 over the private DTO's canonical
    /// stringless rkyv bytes with the NameTable excluded by construction — a rename
    /// cannot move it.
    pub fn content_identity(
        &self,
    ) -> Result<ContentHash<EncodedSchemaDomain>, EncodedIdentityError> {
        Ok(ContentHash::of_core(&EncodedSchemaArchive::from(self))?)
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
    /// canonical Encoded type. A single-field body lowers to a [`Newtype`](Self::Newtype)
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

    /// How many Encoded constructors this type has: a product (newtype, struct) has
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

    /// Whether this field's stored name is exactly the one its reference derives.
    /// Field names are illegal in every Protos surface, so the textual codec never
    /// consults this — a decoded field always carries its type-derived name. It
    /// remains the home for Nomos field lowering's derive-versus-preserve decision:
    /// when true the name is re-derived from the type, when false a programmatically
    /// constructed Encoded carries it verbatim.
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
}

#[cfg(test)]
mod archive_tests {
    use content_identity::PortableArchive;
    use name_table::Identifier;

    use super::{
        DeclarationRole, EncodedDeclaration, EncodedEnum, EncodedNewtype, EncodedReference,
        EncodedSchema, EncodedSchemaArchive, EncodedType, EncodedVariant, StreamingRelation,
    };
    use crate::error::{EncodedSchemaError, EncodedSchemaLoadError};

    #[test]
    fn serialized_invalid_dto_is_rejected_at_the_semantic_archive_boundary() {
        let input = Identifier::Schema(0);
        let output = Identifier::Schema(1);
        let open = Identifier::Schema(2);
        let acknowledged = Identifier::Schema(3);
        let token = Identifier::Schema(4);
        let event = Identifier::Schema(5);
        let close = Identifier::Schema(6);
        let archive = EncodedSchemaArchive {
            declarations: vec![
                EncodedDeclaration::interface(
                    DeclarationRole::InterfaceInput,
                    EncodedType::Enumeration(EncodedEnum::new(
                        input,
                        vec![EncodedVariant::new(open, None)],
                    )),
                ),
                EncodedDeclaration::interface(
                    DeclarationRole::InterfaceOutput,
                    EncodedType::Enumeration(EncodedEnum::new(
                        output,
                        vec![EncodedVariant::new(acknowledged, None)],
                    )),
                ),
                EncodedDeclaration::public(EncodedType::Newtype(EncodedNewtype::new(
                    token,
                    EncodedReference::Integer,
                ))),
                EncodedDeclaration::public(EncodedType::Newtype(EncodedNewtype::new(
                    event,
                    EncodedReference::Integer,
                ))),
                EncodedDeclaration::public(EncodedType::Newtype(EncodedNewtype::new(
                    close,
                    EncodedReference::Integer,
                ))),
            ],
            streaming_relations: vec![StreamingRelation::new(
                open,
                acknowledged,
                EncodedReference::Integer,
                EncodedReference::Plain(event),
                EncodedReference::Plain(close),
            )],
        };
        let bytes = archive.to_archive_bytes().expect("serialize crafted DTO");

        assert!(matches!(
            EncodedSchema::from_archive_bytes(&bytes),
            Err(EncodedSchemaLoadError::Schema(
                EncodedSchemaError::StreamingReferenceMustNameDataType { .. }
            ))
        ));
    }
}
