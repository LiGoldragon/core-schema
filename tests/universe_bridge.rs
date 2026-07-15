//! The universe bridge: id allocation, Core-derived positional signatures, and the
//! signature-vs-Core validation that closes structural-codec's deferred deviation.

use core_schema::UniverseError;
use core_schema::fixture::{
    COMMIT_SEQUENCE, DATABASE_MARKER, DOCUMENTATION, FixtureFamily, INTEGER, STATE_DIGEST, SUMMARY,
    TEXT,
};

/// Every constructor's positional signature is DERIVED from the Core layout: a
/// newtype yields `[inner]`, the struct yields its three fields' referenced types,
/// and the delegate chain yields the wrapped type.
#[test]
fn signatures_are_derived_from_the_core_layout() {
    let family = FixtureFamily::build();
    let universe = family.universe();

    assert_eq!(
        universe
            .core_signature(COMMIT_SEQUENCE, 0)
            .unwrap()
            .fields(),
        &[INTEGER],
        "CommitSequence wraps Integer",
    );
    assert_eq!(
        universe.core_signature(STATE_DIGEST, 0).unwrap().fields(),
        &[INTEGER],
    );
    assert_eq!(
        universe.core_signature(SUMMARY, 0).unwrap().fields(),
        &[TEXT],
        "Summary wraps Text",
    );
    assert_eq!(
        universe.core_signature(DOCUMENTATION, 0).unwrap().fields(),
        &[SUMMARY],
        "Documentation wraps Summary",
    );
    assert_eq!(
        universe
            .core_signature(DATABASE_MARKER, 0)
            .unwrap()
            .fields(),
        &[COMMIT_SEQUENCE, STATE_DIGEST, STATE_DIGEST],
        "the struct's three fields' referenced types, in order",
    );
}

/// The authored standard table's every codec signature equals the Core field
/// signature — the deferred deviation, closed with a real Core layout to check.
#[test]
fn authored_table_agrees_with_the_core_layout() {
    let family = FixtureFamily::build();
    let table = family.standard_table().expect("seal standard table");
    family
        .universe()
        .validate_table(&table)
        .expect("every authored signature equals the Core field signature");
}

/// A mismatched table fails validation LOUDLY: the negative control corrupts
/// CommitSequence's signature and the guard rejects it with a typed mismatch that
/// names the type, the constructor, and both signatures.
#[test]
fn a_mismatched_table_fails_validation_loudly() {
    let family = FixtureFamily::build();
    let corrupted = family.corrupted_table().expect("seal corrupted table");

    match family.universe().validate_table(&corrupted) {
        Err(UniverseError::SignatureMismatch {
            core_type,
            constructor,
            authored,
            core,
        }) => {
            assert_eq!(core_type, COMMIT_SEQUENCE);
            assert_eq!(constructor, 0);
            assert!(authored.is_empty(), "the corrupted signature is empty");
            assert_eq!(core, vec![INTEGER], "the Core layout demands [Integer]");
        }
        other => panic!("expected a loud SignatureMismatch, got {other:?}"),
    }
}
