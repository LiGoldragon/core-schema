//! # core-schema
//!
//! The first REAL stringless Core schema layer of the next-generation NOTA family,
//! and the first REAL Textual form ([`TextualSchema`]).
//!
//! Slice one delivered four foundation crates — `content-identity`, `name-table`,
//! `raw-discovery`, `structural-codec` — with a synthetic fixture universe whose
//! ids keyed no real Core layout. This crate makes that layer real:
//!
//! - **Stringless `CoreSchema` value types** ([`declaration`], [`mod@reference`])
//!   modelled on `schema-language`'s `CoreType { Struct | Enum | Newtype }`: every
//!   name is an [`Identifier`](name_table::Identifier) into the `NameTable`, and
//!   type references dispatch by kind and projection, never a head string. Content
//!   identity is blake3 over the stringless rkyv bytes with the NameTable excluded,
//!   so a rename is hash-stable by construction.
//! - **The universe bridge** ([`universe`]): a set of `CoreSchema` declarations
//!   forms a `structural-codec` Core universe — one [`ScopedCoreTypeId`] per type,
//!   one constructor id per constructor, and each constructor's
//!   [`PositionalSignature`] DERIVED from the Core layout. This closes
//!   `structural-codec`'s deferred signature-vs-Core deviation:
//!   [`CoreUniverse::validate_table`] proves every authored codec signature equals
//!   the Core field signature, and a mismatch fails loudly.
//! - **`TextualSchema`** ([`textual`]): real schema TEXT decodes — through
//!   raw-discovery and the trusted evaluator — into real `CoreSchema` values with a
//!   real `NameTable`, and encodes back canonically. The derived-name rule (a field
//!   name elided when it equals the `snake_case` of its type) works against the real
//!   Core layout.
//!
//! This crate is greenfield by design. It models the proven Core shapes of the
//! existing `schema-language`/`schema`/`schema-rust` repositories in the new
//! stringless discipline; convergence with those repositories happens later on the
//! release train and readapts to it. See `ARCHITECTURE.md`.
//!
//! [`ScopedCoreTypeId`]: structural_codec::ids::ScopedCoreTypeId
//! [`PositionalSignature`]: structural_codec::ids::PositionalSignature

pub mod declaration;
pub mod document;
pub mod error;
pub mod fixture;
pub mod reference;
pub mod textual;
pub mod universe;

pub use declaration::{
    CoreDeclaration, CoreEnum, CoreField, CoreNewtype, CoreSchema, CoreSchemaDomain, CoreStruct,
    CoreType, CoreVariant, DeclarationRole, Visibility,
};
pub use document::{
    DOCUMENT_SLOTS, DeclarationConstructor, ReferenceConstructor, SchemaDocumentGrammar,
};
pub use error::{CoreIdentityError, ElisionLawError, TextualError, UniverseError};
pub use fixture::FixtureFamily;
pub use reference::{
    CoreReference, MultiTypeReferenceProjection, SingleTypeReferenceProjection,
    ValueReferenceProjection,
};
pub use textual::TextualSchema;
pub use universe::{
    AssignedKind, AssignedMember, CORE_UNIVERSE, CoreUniverse, CoreUniverseBuilder, MemberKind,
    ScalarSlot, UniverseType,
};

/// The universe identity a built [`CoreUniverse`] scopes its type ids to, re-exported so
/// an authority-bound ingestion can map a minted universe (`signal-sema-storage`'s
/// `MintedUniverse`) onto the id [`CoreUniverse::from_assignment`] builds in.
pub use structural_codec::ids::CoreUniverseId;
