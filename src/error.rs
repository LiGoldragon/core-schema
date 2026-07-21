//! Typed errors at the crate boundary (thiserror; no anyhow). Each surface owns a
//! focused enum: Core identity, universe-bridge derivation and signature
//! validation, and the Textual round-trip.

use content_identity::ArchiveError;
use name_table::{Identifier, IdentifierNamespace, NameTableError};
use raw_discovery::RecognizeError;
use structural_codec::ids::ScopedCoreTypeId;
use structural_codec::{DecodeError, EncodeError, TableError};

/// Computing a stringless-Core value's content identity failed.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CoreIdentityError {
    #[error(transparent)]
    Archive(#[from] ArchiveError),
}

/// Which encoded reference in a streaming relation failed validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamingRelationReference {
    Token,
    Event,
    CloseToken,
}

impl std::fmt::Display for StreamingRelationReference {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Token => formatter.write_str("token"),
            Self::Event => formatter.write_str("event"),
            Self::CloseToken => formatter.write_str("close-token"),
        }
    }
}

/// The non-plain reference form rejected at a streaming value position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamingReferenceForm {
    Scalar,
    BytesLength,
    SingleTypeApplication,
    MultiTypeApplication,
}

impl std::fmt::Display for StreamingReferenceForm {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scalar => formatter.write_str("scalar"),
            Self::BytesLength => formatter.write_str("Bytes length"),
            Self::SingleTypeApplication => formatter.write_str("single-type generic application"),
            Self::MultiTypeApplication => formatter.write_str("multi-type generic application"),
        }
    }
}

/// A CoreSchema relation or its schema-local identifiers did not meet the encoded
/// schema contract.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CoreSchemaError {
    #[error("CoreSchema requires Schema identifiers, not {0}")]
    NonSchemaIdentifier(Identifier),
    #[error("a streaming relation requires an input interface enumeration")]
    MissingInputInterface,
    #[error("a streaming relation requires an output interface enumeration")]
    MissingOutputInterface,
    #[error("the {0:?} interface root must be an enumeration")]
    InterfaceRootNotEnumeration(crate::declaration::DeclarationRole),
    #[error("streaming opening endpoint {0} is not an input-interface variant")]
    OpeningEndpointNotInputVariant(Identifier),
    #[error("streaming acknowledgement endpoint {0} is not an output-interface variant")]
    AcknowledgementEndpointNotOutputVariant(Identifier),
    #[error("streaming {part} reference {identifier} does not resolve in this schema")]
    UnresolvedStreamingReference {
        part: StreamingRelationReference,
        identifier: Identifier,
    },
    #[error(
        "streaming {part} reference {identifier} must name a data-type declaration, not {actual:?}"
    )]
    StreamingReferenceNotDataType {
        part: StreamingRelationReference,
        identifier: Identifier,
        actual: crate::declaration::DeclarationRole,
    },
    #[error("streaming {part} reference must name a data-type declaration, not a {form} reference")]
    StreamingReferenceMustNameDataType {
        part: StreamingRelationReference,
        form: StreamingReferenceForm,
    },
}

/// A failure at the validated CoreSchema archive boundary.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CoreSchemaLoadError {
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    #[error(transparent)]
    Schema(#[from] CoreSchemaError),
}

/// The universe bridge — allocating type ids, deriving positional signatures from
/// the Core layout, or validating an authored structural table against that
/// layout — failed. `SignatureMismatch` is the loud failure the deferred
/// signature-vs-Core deviation is closed by: an authored codec signature that does
/// not equal the constructor's Core field signature.
#[derive(Debug, Clone, thiserror::Error)]
pub enum UniverseError {
    #[error("no universe type is allocated for the name identifier {0}")]
    UnresolvedName(Identifier),
    #[error("no universe type is registered under id {0:?}")]
    UnknownType(ScopedCoreTypeId),
    #[error("two universe members use type id {0:?}")]
    DuplicateMemberIdentity(ScopedCoreTypeId),
    #[error("two universe members use Schema identifier {0}")]
    DuplicateMemberName(Identifier),
    #[error("two scalar primitive registrations fill the {0:?} slot")]
    DuplicateScalarSlot(crate::universe::ScalarSlot),
    #[error(
        "type {core_type:?} has {members} Core constructor(s), but the table entry has {codecs}"
    )]
    ConstructorCountMismatch {
        core_type: ScopedCoreTypeId,
        members: usize,
        codecs: usize,
    },
    #[error(
        "constructor {constructor} of type {core_type:?}: authored signature {authored:?} does not equal the Core field signature {core:?}"
    )]
    SignatureMismatch {
        core_type: ScopedCoreTypeId,
        constructor: u32,
        authored: Vec<ScopedCoreTypeId>,
        core: Vec<ScopedCoreTypeId>,
    },
    #[error("the structural table holds no entry for Core type {0:?}")]
    TableEntryAbsent(ScopedCoreTypeId),
    #[error(
        "the authority assignment registers two members at the same local identity {0}; an identity names exactly one thing"
    )]
    DuplicateAssignedIdentity(u32),
    #[error("the authority supplied {actual:?} as the NameTable home; CoreSchema owns Schema")]
    WrongNameTableHome { actual: IdentifierNamespace },
    #[error("the authority supplied non-Schema identifier {0} for CoreSchema")]
    WrongSchemaIdentifier(Identifier),
    #[error(
        "the authority member identifier {assigned} does not equal declaration identifier {declared}"
    )]
    AssignedDeclarationIdentifierMismatch {
        assigned: Identifier,
        declared: Identifier,
    },
    #[error(
        "a by-kind type application ({0}) has no allocated universe type in this proof-of-concept universe"
    )]
    UnsupportedApplication(&'static str),
    #[error(transparent)]
    Table(#[from] TableError),
    #[error(transparent)]
    Names(#[from] NameTableError),
}

/// A Textual round-trip — recognizing schema text, decoding it into a CoreSchema
/// value, or encoding a CoreSchema value back to canonical text — failed.
#[derive(Debug, Clone, thiserror::Error)]
pub enum TextualError {
    #[error("the source held no root object to decode")]
    EmptySource,
    #[error(transparent)]
    Recognize(#[from] RecognizeError),
    #[error(transparent)]
    Decode(#[from] DecodeError),
    #[error(transparent)]
    Encode(#[from] EncodeError),
    #[error(transparent)]
    SingleChunk(#[from] structural_codec::error::SingleChunkRequired),
    #[error(transparent)]
    Names(#[from] NameTableError),
    #[error(transparent)]
    Universe(#[from] UniverseError),
    #[error("the decoded structural value did not fit the expected {0} shape at reification")]
    ReifyShape(&'static str),
    #[error("reification met an unknown type name {0:?} that is not a universe type")]
    ReifyUnknownType(String),
    #[error("the document held {0} root slots, but the six-slot layout requires exactly 6")]
    DocumentArity(usize),
    #[error(
        "the document's {0} slot is not the expected shape (a non-empty imports/generics/impls slot is not yet modelled)"
    )]
    DocumentSlot(&'static str),
    #[error("the schema carries no {0} interface root to encode into its protocol-line slot")]
    MissingInterfaceRoot(&'static str),
}
