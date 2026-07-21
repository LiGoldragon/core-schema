//! The four conformance laws, re-proven with the real `CoreSchema` universe.

use core_schema::fixture::{COMMIT_SEQUENCE, DOCUMENTATION, FIELD, FLOAT, FixtureFamily};
use name_table::{IdentifierNamespace, Name, NameTable};
use raw_discovery::{Block, Delimiter, Recognizer};
use structural_codec::ids::ScopedCoreTypeId;
use structural_codec::table::AddressedStructuralTable;
use structural_codec::{CanonicalText, StructuralEvaluator};

fn recognize_single(source: &str) -> Block {
    Recognizer::standard()
        .recognize(source)
        .expect("valid schema text")
        .root_object_at(0)
        .expect("one root")
        .clone()
}

fn standard_table() -> AddressedStructuralTable {
    FixtureFamily::build()
        .standard_table()
        .expect("seal real-Core table")
}

#[test]
fn the_table_agrees_with_the_core_layout() {
    let family = FixtureFamily::build();
    let table = family.standard_table().expect("seal");
    family.universe().validate_table(&table).expect("agreement");
}

#[test]
fn law_one_round_trip_core() {
    let table = standard_table();
    let evaluator = StructuralEvaluator::new(&table);
    let cases: &[(ScopedCoreTypeId, &str)] = &[
        (COMMIT_SEQUENCE, "CommitSequence.{ Integer }"),
        (FIELD, "Integer"),
        (DOCUMENTATION, "alpha.beta.gamma"),
        (FLOAT, "-122.3"),
    ];
    for (expected, source) in cases {
        let block = recognize_single(source);
        let mut names = NameTable::new(IdentifierNamespace::Schema);
        let value = evaluator
            .decode(*expected, &block, &mut names)
            .unwrap_or_else(|error| panic!("decode {source}: {error}"));
        let re_encoded = evaluator
            .encode(*expected, &value, &names)
            .unwrap_or_else(|error| panic!("encode {source}: {error}"));
        let mut names_again = NameTable::new(IdentifierNamespace::Schema);
        let value_again = evaluator
            .decode(*expected, &re_encoded, &mut names_again)
            .unwrap_or_else(|error| panic!("re-decode {source}: {error}"));
        assert_eq!(value, value_again, "law 1 for {source}");
    }
}

#[test]
fn law_two_round_trip_canonical() {
    let table = standard_table();
    let evaluator = StructuralEvaluator::new(&table);
    let cases: &[(ScopedCoreTypeId, &str)] = &[
        (COMMIT_SEQUENCE, "CommitSequence.{ Integer }"),
        (FIELD, "Integer"),
        (DOCUMENTATION, "alpha.beta.gamma"),
        (FLOAT, "-122.3"),
    ];
    for (expected, source) in cases {
        let block = recognize_single(source);
        let mut names = NameTable::new(IdentifierNamespace::Schema);
        let value = evaluator
            .decode(*expected, &block, &mut names)
            .unwrap_or_else(|error| panic!("decode {source}: {error}"));
        let encoded = evaluator
            .encode(*expected, &value, &names)
            .unwrap_or_else(|error| panic!("encode {source}: {error}"));
        assert_eq!(
            encoded.canonical_text(),
            block.canonical_text(),
            "law 2 for {source}"
        );
    }
}

#[test]
fn law_three_interning_atomicity() {
    let table = standard_table();
    let evaluator = StructuralEvaluator::new(&table);
    let mut names = NameTable::new(IdentifierNamespace::Schema);
    names
        .intern(Name::new("PriorName"))
        .expect("test name fits its namespace");
    let bytes_before = names.to_archive_bytes().expect("before").as_ref().to_vec();
    let identity_before = names.identity().expect("identity before");
    let block = recognize_single("notADeclaration");
    assert!(
        evaluator
            .decode(COMMIT_SEQUENCE, &block, &mut names)
            .is_err()
    );
    let bytes_after = names.to_archive_bytes().expect("after").as_ref().to_vec();
    let identity_after = names.identity().expect("identity after");
    assert_eq!(bytes_before, bytes_after, "archived bytes unchanged");
    assert_eq!(
        identity_before, identity_after,
        "content identity unchanged"
    );
}

#[test]
fn law_four_identity_preserving_across_revisions() {
    let family = FixtureFamily::build();
    let table_old = family
        .table(Delimiter::Brace, b"lexicon-brace".to_vec(), 1)
        .expect("old table");
    let table_new = family
        .table(Delimiter::Parenthesis, b"lexicon-parenthesis".to_vec(), 2)
        .expect("new table");
    assert_ne!(table_old.identity(), table_new.identity());
    let evaluator_old = StructuralEvaluator::new(&table_old);
    let evaluator_new = StructuralEvaluator::new(&table_new);
    let block_old = recognize_single("CommitSequence.{ Integer }");
    let block_new = recognize_single("CommitSequence.( Integer )");
    let mut names_old = NameTable::new(IdentifierNamespace::Schema);
    let value_old = evaluator_old
        .decode(COMMIT_SEQUENCE, &block_old, &mut names_old)
        .expect("decode old text with old table");
    let mut names_new = NameTable::new(IdentifierNamespace::Schema);
    let value_new = evaluator_new
        .decode(COMMIT_SEQUENCE, &block_new, &mut names_new)
        .expect("decode new text with new table");
    assert_eq!(value_old, value_new, "the structural value never moved");
    assert_eq!(
        value_old.content_identity().expect("identity old"),
        value_new.content_identity().expect("identity new"),
        "the value's content identity never moved"
    );
    let re_encoded = evaluator_new
        .encode(COMMIT_SEQUENCE, &value_old, &names_old)
        .expect("encode old value with new table");
    assert_eq!(re_encoded.canonical_text(), block_new.canonical_text());
}
