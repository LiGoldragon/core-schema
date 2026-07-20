//! Cross-version golden-hash witness: an ABSOLUTE content-hash constant for a
//! representative EncodedSchema value under the current layout version.
//!
//! Content identity is blake3 over a value's stringless rkyv bytes, and rkyv
//! archives an enum at a fixed size equal to its largest variant — so any change to
//! the archived representation of a type reachable from `EncodedSchema` (a new or
//! larger variant, a reordered discriminant, a field-layout change) moves EVERY
//! value's hash, whether or not that value's own Rust source was touched. This
//! pinned absolute hash makes that class of change impossible to ship silently: it
//! fails this test loudly. Its sibling in `core-logos` exists because that exact
//! class of change once shipped there without a layout bump or a witness.
//!
//! If this test fails, the archived representation of a EncodedSchema value changed.
//! That is a layout event, never a casual edit: bump `EncodedSchemaDomain`'s
//! `LayoutVersion` in `src/declaration.rs`, document why the archived shape moved,
//! and update the constant below DELIBERATELY to the new hash. Do not "fix" the test
//! by pasting the new hash without bumping the layout version — that is the very
//! defect this witness exists to catch.

use content_identity::HashDomain;
use core_schema::declaration::{EncodedNewtype, EncodedType};
use core_schema::{EncodedDeclaration, EncodedReference, EncodedSchema, EncodedSchemaDomain};
use name_table::Identifier;

/// The content identity of a representative single-declaration schema — a public
/// `Newtype` over `Boolean` at a fixed identifier index — under the current
/// EncodedSchema layout, as a lowercase hex blake3 digest. Pinned at layout 5,
/// which adds closed streaming relations after the namespace-sliced identifier
/// layout at version 4.
/// The value is fully deterministic: the identifier is a fixed index and the
/// NameTable is excluded from the pre-image by construction.
const REPRESENTATIVE_SCHEMA_IDENTITY_LAYOUT_5: &str =
    "810b6fa336618ee9c229779edbe71a9b90497d8ebbf346fa7d16c58d9c74cc07";

/// The representative value the constant pins: a schema of one public declaration,
/// a `Newtype` wrapping `Boolean`. Built without a NameTable — the identifier is a
/// fixed index and the table is not part of the content-identity pre-image.
fn representative_schema() -> EncodedSchema {
    EncodedSchema::new(vec![EncodedDeclaration::public(EncodedType::Newtype(
        EncodedNewtype::new(Identifier::Schema(0), EncodedReference::Boolean),
    ))])
}

#[test]
fn representative_schema_identity_is_pinned_under_the_current_layout() {
    let identity = representative_schema()
        .content_identity()
        .expect("content identity");

    // The layout version this witness pins must be the one the domain currently
    // reports. If the domain moved to a new layout, the constant above is stale by
    // definition and must be re-derived deliberately.
    assert_eq!(
        EncodedSchemaDomain::layout_version().value(),
        5,
        "the witnessed layout version moved; re-derive the pinned hash deliberately",
    );

    assert_eq!(
        identity.to_hexadecimal(),
        REPRESENTATIVE_SCHEMA_IDENTITY_LAYOUT_5,
        "the archived representation of a EncodedSchema value changed — this is a layout \
         event: bump EncodedSchemaDomain's LayoutVersion in src/declaration.rs, document \
         why the archived shape moved, and update this constant deliberately",
    );
}
