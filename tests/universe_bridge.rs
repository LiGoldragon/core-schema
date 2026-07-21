//! The universe bridge: id allocation, Encoded-derived positional signatures, and the
//! signature-vs-Encoded validation that closes structural-codec's deferred deviation.

use core_schema::UniverseError;
use core_schema::fixture::{
    COMMIT_SEQUENCE, DATABASE_MARKER, DOCUMENTATION, FixtureFamily, INTEGER, STATE_DIGEST, SUMMARY,
    TEXT,
};

/// Every constructor's positional signature is DERIVED from the Encoded layout: a
/// newtype yields `[inner]`, the struct yields its three fields' referenced types,
/// and the delegate chain yields the wrapped type.
#[test]
fn signatures_are_derived_from_the_core_layout() {
    let family = FixtureFamily::build();
    let universe = family.universe();

    assert_eq!(
        universe
            .encoded_signature(COMMIT_SEQUENCE, 0)
            .unwrap()
            .fields(),
        &[INTEGER],
        "CommitSequence wraps Integer",
    );
    assert_eq!(
        universe
            .encoded_signature(STATE_DIGEST, 0)
            .unwrap()
            .fields(),
        &[INTEGER],
    );
    assert_eq!(
        universe.encoded_signature(SUMMARY, 0).unwrap().fields(),
        &[TEXT],
        "Summary wraps Text",
    );
    assert_eq!(
        universe
            .encoded_signature(DOCUMENTATION, 0)
            .unwrap()
            .fields(),
        &[SUMMARY],
        "Documentation wraps Summary",
    );
    assert_eq!(
        universe
            .encoded_signature(DATABASE_MARKER, 0)
            .unwrap()
            .fields(),
        &[COMMIT_SEQUENCE, STATE_DIGEST, STATE_DIGEST],
        "the struct's three fields' referenced types, in order",
    );
}

/// The authored standard table's every codec signature equals the Encoded field
/// signature — the deferred deviation, closed with a real Encoded layout to check.
#[test]
fn authored_table_agrees_with_the_core_layout() {
    let family = FixtureFamily::build();
    let table = family.standard_table().expect("seal standard table");
    family
        .universe()
        .validate_table(&table)
        .expect("every authored signature equals the Encoded field signature");
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
            encoded_type,
            constructor,
            authored,
            encoded,
        }) => {
            assert_eq!(encoded_type, COMMIT_SEQUENCE);
            assert_eq!(constructor, 0);
            assert!(authored.is_empty(), "the corrupted signature is empty");
            assert_eq!(
                encoded,
                vec![INTEGER],
                "the Encoded layout demands [Integer]"
            );
        }
        other => panic!("expected a loud SignatureMismatch, got {other:?}"),
    }
}
