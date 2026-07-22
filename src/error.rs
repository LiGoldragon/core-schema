//! Typed errors at the crate boundary (thiserror; no anyhow). Each surface owns a
//! focused enum: Encoded identity, universe-bridge derivation and signature
//! validation, and the Textual round-trip.

use content_identity::ArchiveError;
use name_table::{Identifier, IdentifierNamespace, NameTableError};
use raw_discovery::RecognizeError;
use structural_codec::ids::{EncodedUniverseId, ScopedEncodedTypeId};
use structural_codec::{DecodeError, EncodeError, TableError};

use crate::reference::BuiltinReference;

/// A declaration attempts to reuse a name whose definition already belongs to the
/// textual interface. This failure is typed archive data, so a boundary can preserve
/// the declared identifier and the exact prior builtin without reducing either to
/// text.
#[derive(
    rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq, thiserror::Error,
)]
#[error("declaration {identifier} redefines builtin {builtin:?}")]
pub struct StructuralRedefinition {
    identifier: Identifier,
    builtin: BuiltinReference,
}

impl StructuralRedefinition {
    pub fn new(identifier: Identifier, builtin: BuiltinReference) -> Self {
        Self {
            identifier,
            builtin,
        }
    }

    pub fn identifier(&self) -> Identifier {
        self.identifier
    }

    pub fn builtin(&self) -> BuiltinReference {
        self.builtin
    }
}

/// Computing a stringless-Encoded value's content identity failed.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EncodedIdentityError {
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

/// A EncodedSchema relation or its schema-local identifiers did not meet the encoded
/// schema contract.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EncodedSchemaError {
    #[error("EncodedSchema requires Schema identifiers, not {0}")]
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

/// A failure at the validated EncodedSchema archive boundary.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EncodedSchemaLoadError {
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    #[error(transparent)]
    Schema(#[from] EncodedSchemaError),
}

/// The universe bridge — allocating type ids, deriving positional signatures from
/// the Encoded layout, or validating an authored structural table against that
/// layout — failed. `SignatureMismatch` is the loud failure the deferred
/// signature-vs-Encoded deviation is closed by: an authored codec signature that does
/// not equal the constructor's Encoded field signature.
#[derive(Debug, Clone, thiserror::Error)]
pub enum UniverseError {
    #[error(
        "reference {reference:?} uses scalar slot {slot:?}, but no member registered that slot"
    )]
    MissingScalarSlot {
        slot: crate::universe::ScalarSlot,
        reference: crate::reference::EncodedReference,
    },
    #[error("reference {reference:?} names {identifier}, which is absent from the NameTable")]
    ReferenceNameAbsent {
        identifier: Identifier,
        reference: crate::reference::EncodedReference,
    },
    #[error("reference {reference:?} names {identifier}, which has no registered universe member")]
    ReferenceTargetUnregistered {
        identifier: Identifier,
        reference: crate::reference::EncodedReference,
    },
    #[error("no universe type is registered under id {0:?}")]
    UnknownType(ScopedEncodedTypeId),
    #[error("two universe members use type id {0:?}")]
    DuplicateMemberIdentity(ScopedEncodedTypeId),
    #[error("universe member {member:?} belongs to {actual:?}, but this build seals {expected:?}")]
    UniverseScopeMismatch {
        expected: EncodedUniverseId,
        actual: EncodedUniverseId,
        member: ScopedEncodedTypeId,
    },
    #[error("two universe members use Schema identifier {0}")]
    DuplicateMemberName(Identifier),
    #[error(transparent)]
    Redefinition(#[from] StructuralRedefinition),
    #[error("two scalar primitive registrations fill the {0:?} slot")]
    DuplicateScalarSlot(crate::universe::ScalarSlot),
    #[error(
        "type {encoded_type:?} has {members} Encoded constructor(s), but the table entry has {codecs}"
    )]
    ConstructorCountMismatch {
        encoded_type: ScopedEncodedTypeId,
        members: usize,
        codecs: usize,
    },
    #[error(
        "constructor {constructor} of type {encoded_type:?}: authored signature {authored:?} does not equal the Encoded field signature {encoded:?}"
    )]
    SignatureMismatch {
        encoded_type: ScopedEncodedTypeId,
        constructor: u32,
        authored: Vec<ScopedEncodedTypeId>,
        encoded: Vec<ScopedEncodedTypeId>,
    },
    #[error("the structural table holds no entry for Encoded type {0:?}")]
    TableEntryAbsent(ScopedEncodedTypeId),
    #[error("the authority supplied {actual:?} as the NameTable home; EncodedSchema owns Schema")]
    WrongNameTableHome { actual: IdentifierNamespace },
    #[error("the authority supplied non-Schema identifier {0} for EncodedSchema")]
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

/// A Textual round-trip — recognizing schema text, decoding it into a EncodedSchema
/// value, or encoding a EncodedSchema value back to canonical text — failed.
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
