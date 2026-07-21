//! The downstream half of conformance law 5: the derive's emitted structural entries
//! equal `core-schema`'s hand-authored entries, type for type.
//!
//! `structural-codec-derive` (upstream, in protos) lowers each `#[structural_form]`
//! authority into a `StructuralEntry`. The machinery-internal half of law 5 — that
//! the generated codec agrees with the trusted evaluator over the DERIVE's own table —
//! is proven upstream in `structural-codec-derive-fixtures`. But the derive is only
//! trustworthy if what it emits also equals what a schema author writes by hand. That
//! cross-check lives HERE, downstream, because only here do the two families resolve a
//! single `structural-codec` pin, so their `StructuralEntry` types unify and equality
//! is meaningful; holding it upstream would force two machinery versions into one
//! dependency graph.
//!
//! Drift — a stale constructor, a divergent form, a wrong signature — is a failure.
//! The `Field` meta-type is the pointed case: field names are illegal everywhere
//! (psyche ruling 2026-07-19), so both sides must carry exactly ONE constructor, the
//! bare elided `Type`.

use core_schema::fixture::{
    COMMIT_SEQUENCE, DATABASE_MARKER, DOCUMENTATION, FIELD, FLOAT, FixtureFamily, INTEGER,
    STATE_DIGEST, SUMMARY, TEXT,
};
use structural_codec::ids::ScopedEncodedTypeId;
use structural_codec_derive_fixtures::DerivedTable;

/// Every fixture type the two families share. The derived family and the authored
/// family cover exactly this universe.
const FIXTURE_TYPES: [ScopedEncodedTypeId; 9] = [
    INTEGER,
    FLOAT,
    TEXT,
    SUMMARY,
    DOCUMENTATION,
    COMMIT_SEQUENCE,
    STATE_DIGEST,
    DATABASE_MARKER,
    FIELD,
];

/// Type for type, the derive's emitted entry equals the hand-authored entry — the
/// same encoded type, the same constructors, the same forms and positional signatures.
#[test]
fn every_derived_entry_equals_the_authored_entry() {
    let authored = FixtureFamily::build()
        .standard_table()
        .expect("seal authored table");
    let derived = DerivedTable::of_fixture_family();
    let derived_entries = derived.entries();

    assert_eq!(
        derived_entries.len(),
        FIXTURE_TYPES.len(),
        "the derived family covers exactly the shared fixture universe",
    );

    for id in FIXTURE_TYPES {
        let authored_entry = authored
            .entry(id)
            .unwrap_or_else(|| panic!("authored table is missing {id:?}"));
        let derived_entry = derived_entries
            .get(&id)
            .unwrap_or_else(|| panic!("derived table is missing {id:?}"));
        assert_eq!(
            derived_entry, authored_entry,
            "derived entry diverged from the authored entry for {id:?}",
        );
    }
}

/// The `Field` meta-type, called out on its own: the ban leaves exactly one
/// constructor on both sides, and they are equal. An explicit `name.Type` constructor
/// on either side would be a field-name rendering and would fail here.
#[test]
fn the_field_meta_type_carries_one_elided_constructor_on_both_sides() {
    let authored = FixtureFamily::build()
        .standard_table()
        .expect("seal authored table");
    let derived = DerivedTable::of_fixture_family();

    let authored_field = authored.entry(FIELD).expect("authored Field entry");
    let derived_field = derived.entries().get(&FIELD).expect("derived Field entry");

    assert_eq!(
        authored_field.constructors.len(),
        1,
        "the authored Field carries a single elided constructor",
    );
    assert_eq!(
        derived_field.constructors.len(),
        1,
        "the derived Field carries a single elided constructor",
    );
    assert_eq!(
        derived_field, authored_field,
        "the derived Field entry equals the authored Field entry",
    );
}
