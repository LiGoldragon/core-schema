//! Typed errors at the crate boundary (thiserror; no anyhow). Each surface owns a
//! focused enum: Core identity, universe-bridge derivation and signature
//! validation, and the Textual round-trip.

use content_identity::ArchiveError;
use name_table::NameTableError;
use raw_discovery::RecognizeError;
use structural_codec::ids::ScopedEncodedTypeId;
use structural_codec::{DecodeError, EncodeError, TableError};

/// Computing a stringless-Core value's content identity failed.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EncodedIdentityError {
    #[error(transparent)]
    Archive(#[from] ArchiveError),
}

/// The universe bridge — allocating type ids, deriving positional signatures from
/// the Core layout, or validating an authored structural table against that
/// layout — failed. `SignatureMismatch` is the loud failure the deferred
/// signature-vs-Core deviation is closed by: an authored codec signature that does
/// not equal the constructor's Core field signature.
#[derive(Debug, Clone, thiserror::Error)]
pub enum UniverseError {
    #[error("no universe type is allocated for the name identifier {0}")]
    UnresolvedName(name_table::Identifier),
    #[error("no universe type is registered under id {0:?}")]
    UnknownType(ScopedEncodedTypeId),
    #[error(
        "type {core_type:?} has {members} Core constructor(s), but the table entry has {codecs}"
    )]
    ConstructorCountMismatch {
        core_type: ScopedEncodedTypeId,
        members: usize,
        codecs: usize,
    },
    #[error(
        "constructor {constructor} of type {core_type:?}: authored signature {authored:?} does not equal the Core field signature {core:?}"
    )]
    SignatureMismatch {
        core_type: ScopedEncodedTypeId,
        constructor: u32,
        authored: Vec<ScopedEncodedTypeId>,
        core: Vec<ScopedEncodedTypeId>,
    },
    #[error("the structural table holds no entry for Core type {0:?}")]
    TableEntryAbsent(ScopedEncodedTypeId),
    #[error(
        "the authority assignment registers two members at the same local identity {0}; an identity names exactly one thing"
    )]
    DuplicateAssignedIdentity(u32),
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
