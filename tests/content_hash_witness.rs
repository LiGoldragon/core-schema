//! Cross-version golden-hash witness: an ABSOLUTE content-hash constant for a
//! representative CoreSchema value under the current layout version.
//!
//! Content identity is blake3 over a value's stringless rkyv bytes, and rkyv
//! archives an enum at a fixed size equal to its largest variant — so any change to
//! the archived representation of a type reachable from `CoreSchema` (a new or
//! larger variant, a reordered discriminant, a field-layout change) moves EVERY
//! value's hash, whether or not that value's own Rust source was touched. This
//! pinned absolute hash makes that class of change impossible to ship silently: it
//! fails this test loudly. Its sibling in `core-logos` exists because that exact
//! class of change once shipped there without a layout bump or a witness.
//!
//! If this test fails, the archived representation of a CoreSchema value changed.
//! That is a layout event, never a casual edit: bump `CoreSchemaDomain`'s
//! `LayoutVersion` in `src/declaration.rs`, document why the archived shape moved,
//! and update the constant below DELIBERATELY to the new hash. Do not "fix" the test
//! by pasting the new hash without bumping the layout version — that is the very
//! defect this witness exists to catch.

use content_identity::HashDomain;
use core_schema::declaration::{CoreNewtype, CoreType};
use core_schema::{CoreDeclaration, CoreReference, CoreSchema, CoreSchemaDomain};
use name_table::Identifier;

/// The content identity of a representative single-declaration schema — a public
/// `Newtype` over `Boolean` at a fixed identifier index — under the current
/// CoreSchema layout, as a lowercase hex blake3 digest. Pinned at layout 5, where
/// identifiers are namespace variants carrying `u16` locals and schemas carry
/// ordered streaming relations. The NameTable remains excluded from the pre-image.
const REPRESENTATIVE_SCHEMA_IDENTITY_LAYOUT_5: &str =
    "810b6fa336618ee9c229779edbe71a9b90497d8ebbf346fa7d16c58d9c74cc07";

/// The representative value the constant pins: a schema of one public declaration,
/// a `Newtype` wrapping `Boolean`. Built without a NameTable — the identifier is a
/// fixed index and the table is not part of the content-identity pre-image.
fn representative_schema() -> CoreSchema {
    CoreSchema::new(vec![CoreDeclaration::public(CoreType::Newtype(
        CoreNewtype::new(Identifier::Schema(0), CoreReference::Boolean),
    ))])
}

#[test]
fn representative_schema_identity_is_pinned_under_the_current_layout() {
    // The layout version this witness pins must be the one the domain currently
    // reports. If the domain moved to a new layout, the constant above is stale by
    // definition and must be re-derived deliberately.
    assert_eq!(
        CoreSchemaDomain::layout_version().value(),
        5,
        "the witnessed layout version moved; re-derive the pinned hash deliberately",
    );

    let identity = representative_schema()
        .content_identity()
        .expect("content identity");

    assert_eq!(
        identity.to_hexadecimal(),
        REPRESENTATIVE_SCHEMA_IDENTITY_LAYOUT_5,
        "the archived representation of a CoreSchema value changed — this is a layout \
         event: bump CoreSchemaDomain's LayoutVersion in src/declaration.rs, document \
         why the archived shape moved, and update this constant deliberately",
    );
}
