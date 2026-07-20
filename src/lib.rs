//! # core-schema
//!
//! The first REAL stringless encoded-schema layer of the next-generation NOTA family,
//! and the first REAL TextualForm pivot ([`TextualSchema`]).
//!
//! Slice one delivered four foundation crates — `content-identity`, `name-table`,
//! `raw-discovery`, `structural-codec` — with a synthetic fixture universe whose
//! ids keyed no real encoded-form layout. This crate makes that layer real:
//!
//! - **Stringless `EncodedSchema` value types** ([`declaration`], [`mod@reference`])
//!   modelled on `schema-language`'s `EncodedType { Struct | Enum | Newtype }`: every
//!   name is an [`Identifier`](name_table::Identifier) into the `NameTable`, and
//!   type references dispatch by kind and projection, never a head string. Content
//!   identity is blake3 over the stringless rkyv bytes with the NameTable excluded,
//!   so a rename is hash-stable by construction.
//! - **The universe bridge** ([`universe`]): a set of `EncodedSchema` declarations
//!   forms a `structural-codec` encoded universe — one [`ScopedEncodedTypeId`] per
//!   type, one constructor id per constructor, and each constructor's
//!   [`PositionalSignature`] derived from the encoded-form layout.
//!   [`EncodedUniverse::validate_table`] proves every authored codec signature equals
//!   the encoded-form field signature, and a mismatch fails loudly.
//! - **`TextualSchema`** ([`textual`]): real schema TEXT decodes — through
//!   raw-discovery and the trusted evaluator — into real `EncodedSchema` values with a
//!   real `NameTable`, and encodes back canonically. Field names are not authored;
//!   derived names exist only at the NameTable/emission boundary.
//!
//! This crate is greenfield by design. It models the proven encoded shapes of the
//! existing `schema-language`/`schema`/`schema-rust` repositories in the new
//! stringless discipline; convergence with those repositories happens later on the
//! release train and readapts to it. See `ARCHITECTURE.md`.
//!
//! [`ScopedEncodedTypeId`]: structural_codec::ids::ScopedEncodedTypeId
//! [`PositionalSignature`]: structural_codec::ids::PositionalSignature

pub mod declaration;
pub mod document;
pub mod error;
pub mod fixture;
pub mod manifest;
pub mod reference;
/// Codec-backed source-surface witnesses for the installed document grammar.
pub mod source_surface_candidates;
pub mod textual;
pub mod universe;

pub use declaration::{
    DeclarationRole, EncodedDeclaration, EncodedEnum, EncodedField, EncodedNewtype, EncodedSchema,
    EncodedSchemaDomain, EncodedStruct, EncodedType, EncodedVariant, StreamingRelation, Visibility,
};
pub use document::{
    DOCUMENT_SLOTS, DeclarationConstructor, InterfaceVariantConstructor, ReferenceConstructor,
    SchemaDocumentGrammar,
};
pub use error::{EncodedIdentityError, TextualError, UniverseError};
pub use fixture::FixtureFamily;
pub use manifest::{
    ManifestSchema, SchemaManifest, SchemaManifestError, SchemaManifestFile,
    SchemaManifestFileStructure, SchemaManifestStructure,
};
pub use reference::{
    EncodedReference, MultiTypeReferenceProjection, SingleTypeReferenceProjection,
    ValueReferenceProjection,
};
pub use textual::{SchemaLanguage, TextualSchema};
pub use universe::{
    AssignedKind, AssignedMember, ENCODED_UNIVERSE, EncodedUniverse, EncodedUniverseBuilder,
    MemberKind, ScalarSlot, UniverseType,
};

/// The universe identity a built [`EncodedUniverse`] scopes its type ids to, re-exported so
/// an authority-bound ingestion can map a minted universe (`signal-sema-storage`'s
/// `MintedUniverse`) onto the id [`EncodedUniverse::from_assignment`] builds in.
pub use structural_codec::ids::EncodedUniverseId;
