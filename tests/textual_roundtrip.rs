//! TextualSchema, the first real Textual form: real schema TEXT decodes into real
//! CoreSchema values with a real NameTable, and encodes back canonically. The
//! `Field` disjoint alternatives (elided derived name versus explicit `name.Type`)
//! work against the real Core layout.

use core_schema::ElisionLawError;
use core_schema::TextualError;
use core_schema::TextualSchema;
use core_schema::declaration::CoreType;
use core_schema::fixture::{COMMIT_SEQUENCE, DATABASE_MARKER};
use core_schema::reference::CoreReference;
use name_table::NameTable;
use raw_discovery::Recognizer;
use structural_codec::CanonicalText;

fn canonical(source: &str) -> String {
    Recognizer::standard()
        .recognize(source)
        .expect("recognize")
        .root_object_at(0)
        .expect("one root")
        .canonical_text()
}

fn name_table_rows(names: &NameTable) -> String {
    (0..names.len())
        .map(|index| {
            let identifier = name_table::Identifier::new(index as u32);
            format!(
                "  {index} -> {}",
                names.resolve(identifier).unwrap().as_str()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A newtype declaration decodes into a real `CoreNewtype` and encodes back to the
/// identical canonical text.
#[test]
fn newtype_declaration_round_trips() {
    let textual = TextualSchema::fixture().expect("build textual schema");
    let source = "CommitSequence.{ Integer }";
    let mut names = NameTable::new();

    let value = textual
        .decode(COMMIT_SEQUENCE, source, &mut names)
        .expect("decode CommitSequence");

    let CoreType::Newtype(newtype) = &value else {
        panic!("expected a newtype, got {value:?}");
    };
    assert_eq!(
        names.resolve(newtype.identifier()).unwrap().as_str(),
        "CommitSequence"
    );
    assert_eq!(newtype.reference(), &CoreReference::Integer);

    println!(
        "decoded {source}\n  => {value:?}\nNameTable:\n{}",
        name_table_rows(&names)
    );

    let re_encoded = textual
        .encode(COMMIT_SEQUENCE, &value, &mut names)
        .expect("encode CommitSequence");
    assert_eq!(re_encoded, canonical(source), "canonical text round-trips");
    println!("re-encoded => {re_encoded}");
}

/// The `DatabaseMarker` struct decodes into a real `CoreStruct` whose fields exercise
/// BOTH `Field` alternatives against the real Core layout: two names are elided
/// (derived from the type) and one is explicit (`secretDigest.StateDigest`). It
/// encodes back to the identical canonical text.
#[test]
fn struct_declaration_round_trips_with_both_field_alternatives() {
    let textual = TextualSchema::fixture().expect("build textual schema");
    let source = "DatabaseMarker.{ CommitSequence StateDigest secretDigest.StateDigest }";
    let mut names = NameTable::new();

    let value = textual
        .decode(DATABASE_MARKER, source, &mut names)
        .expect("decode DatabaseMarker");

    let CoreType::Struct(structure) = &value else {
        panic!("expected a struct, got {value:?}");
    };
    assert_eq!(
        names.resolve(structure.identifier()).unwrap().as_str(),
        "DatabaseMarker"
    );
    assert_eq!(structure.fields().len(), 3);

    let field_names: Vec<&str> = structure
        .fields()
        .iter()
        .map(|field| names.resolve(field.identifier()).unwrap().as_str())
        .collect();
    // Two elided (derived) names, one explicit.
    assert_eq!(
        field_names,
        vec!["commit_sequence", "state_digest", "secretDigest"]
    );

    // Every field references a declared type by identifier (Plain), never a string.
    for field in structure.fields() {
        assert!(matches!(field.reference(), CoreReference::Plain(_)));
    }

    println!(
        "decoded {source}\n  => {value:?}\nNameTable:\n{}",
        name_table_rows(&names)
    );

    let re_encoded = textual
        .encode(DATABASE_MARKER, &value, &mut names)
        .expect("encode DatabaseMarker");
    assert_eq!(re_encoded, canonical(source), "canonical text round-trips");
    println!("re-encoded => {re_encoded}");
}

/// The elision law: an explicit field name is legal ONLY where two or more fields
/// in the block share a type. Naming a uniquely-typed field explicitly — here
/// `foo.CommitSequence`, the block's only `CommitSequence` — is invalid syntax, so
/// decode must reject it with a typed [`ElisionLawError::SuperfluousName`] that
/// names the uniquely-typed type. The two shared `StateDigest` fields keep the
/// explicit name on `secretDigest` legal; only the uniquely-typed offender fails.
/// (psyche ruling, bead `primary-56d1.48`.)
#[test]
fn decode_rejects_explicit_name_on_uniquely_typed_field() {
    let textual = TextualSchema::fixture().expect("build textual schema");
    let source = "DatabaseMarker.{ foo.CommitSequence StateDigest secretDigest.StateDigest }";
    let mut names = NameTable::new();

    let error = textual
        .decode(DATABASE_MARKER, source, &mut names)
        .expect_err(
            "an explicit name on the uniquely-typed CommitSequence field is invalid syntax",
        );

    match error {
        TextualError::Elision(ElisionLawError::SuperfluousName {
            field_name,
            type_name,
        }) => {
            assert_eq!(field_name, "foo");
            assert_eq!(type_name, "CommitSequence");
        }
        other => panic!("expected an elision-law rejection, got {other:?}"),
    }
}
