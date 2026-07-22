//! The six-slot document layout: a whole spirit-min-shaped document decodes to a
//! full `EncodedSchema` — every type declaration, both enumerations, the `Vector`
//! reference projections, and both interface lines — and encodes back to stable
//! canonical text. Identifier binding through a central authority (content-hash
//! equality across front-ends) is a SEPARATE queued slice and is deliberately NOT
//! asserted here; this proves the native surface represents the accepted grammar.

use core_schema::declaration::EncodedType;
use core_schema::reference::{EncodedReference, SingleTypeReferenceProjection};
use core_schema::{
    BuiltinReference, EncodedDeclaration, TextualError, TextualSchema, UniverseError,
};
use name_table::{Identifier, IdentifierNamespace, NameTable};
use raw_discovery::Recognizer;
use structural_codec::CanonicalText;

/// The spirit-min schema in core-schema's native dialect: its shape verbatim — the
/// six root slots, the type declarations, both enumerations, the `Vector`
/// projections, and the two interface lines — with the string scalar spelled
/// `String`, its canonical spelling under the 2026-07-17 ruling ("Strings are
/// Strings"), exactly as spirit-min writes it.
const SPIRIT_MIN: &str = "\
{}
[Record.RecordPayload Observe.ObservePayload]
[RecordAccepted.RecordAcceptedPayload RecordsObserved.RecordsObservedPayload]
{
  RecordPayload.Entry
  ObservePayload.Query
  RecordAcceptedPayload.RecordIdentifier
  RecordsObservedPayload.RecordSet
  Topic.String
  Topics.Vector.Topic
  Description.String
  RecordIdentifier.Integer
  Entry.{ Topics Kind Description Magnitude }
  Query.{ Topic Kind }
  RecordSet.Vector.Entry
  Kind.[Decision Principle Correction Clarification Constraint]
  Magnitude.[Minimum VeryLow Low Medium High VeryHigh Maximum]
}
{}
{}";

fn text(names: &NameTable, identifier: Identifier) -> &str {
    names
        .resolve(identifier)
        .expect("interned identifier")
        .as_str()
}

fn declaration<'schema>(
    declarations: &'schema [EncodedDeclaration],
    names: &NameTable,
    wanted: &str,
) -> &'schema EncodedType {
    declarations
        .iter()
        .map(EncodedDeclaration::value)
        .find(|value| text(names, value.identifier()) == wanted)
        .unwrap_or_else(|| panic!("declaration {wanted} is present"))
}

/// Every builtin spelling is already a definition, not a user-declared type name.
/// The table-driven set is the complete current textual builtin lexicon.
#[test]
fn builtin_type_declarations_are_typed_redefinitions() {
    assert_eq!(
        BuiltinReference::ALL.len(),
        7,
        "the builtin lexicon is exhaustive"
    );

    for builtin in BuiltinReference::ALL {
        let document = format!(
            "{{}}\n[]\n[]\n{{\n  {}.[]\n}}\n{{}}\n{{}}",
            builtin.spelling()
        );
        let textual = TextualSchema::schema_document().expect("seal document grammar");
        let mut names = NameTable::new(IdentifierNamespace::Schema);
        let error = textual
            .decode_document(&document, &mut names)
            .expect_err("a builtin spelling cannot declare a user type");

        assert!(
            matches!(
                error,
                TextualError::Universe(UniverseError::Redefinition(redefinition))
                    if redefinition.builtin() == builtin
                        && text(&names, redefinition.identifier()) == builtin.spelling()
            ),
            "{} rejects as a typed builtin redefinition",
            builtin.spelling(),
        );
    }
}

/// The whole document decodes to the full declaration set, construct by construct:
/// every kind of newtype (plain, scalar, and both `Vector` projections), both
/// structs, both enumerations, and both interface lines.
#[test]
fn spirit_min_document_decodes_to_the_full_encoded_schema() {
    let textual = TextualSchema::schema_document().expect("build the document grammar");
    let mut names = NameTable::new(IdentifierNamespace::Schema);
    let schema = textual
        .decode_document(SPIRIT_MIN, &mut names)
        .expect("decode the whole document");

    // Thirteen data-type declarations plus the two interface roots.
    assert_eq!(
        schema.data_declarations().count(),
        13,
        "every type declaration decoded"
    );
    assert_eq!(
        schema.declarations().len(),
        15,
        "the substrate holds the data declarations and both interface roots"
    );
    let EncodedType::Enumeration(input_root) =
        schema.input().expect("an input interface root").value()
    else {
        panic!("the input interface root is an enumeration");
    };
    let EncodedType::Enumeration(output_root) =
        schema.output().expect("an output interface root").value()
    else {
        panic!("the output interface root is an enumeration");
    };
    assert_eq!(input_root.variants().len(), 2, "two input mail types");
    assert_eq!(output_root.variants().len(), 2, "two output mail types");
    assert_eq!(
        text(&names, schema.input().unwrap().identifier()),
        "Input",
        "the input root carries the canonical Input name"
    );
    assert_eq!(
        text(&names, schema.output().unwrap().identifier()),
        "Output",
        "the output root carries the canonical Output name"
    );

    // A newtype over a Plain declared type.
    let EncodedType::Newtype(record_payload) =
        declaration(schema.declarations(), &names, "RecordPayload")
    else {
        panic!("RecordPayload is a newtype");
    };
    assert!(
        matches!(record_payload.reference(), EncodedReference::Plain(id) if text(&names, *id) == "Entry"),
        "RecordPayload wraps Plain(Entry)",
    );

    // A newtype over the string scalar leaf.
    let EncodedType::Newtype(topic) = declaration(schema.declarations(), &names, "Topic") else {
        panic!("Topic is a newtype");
    };
    assert_eq!(
        topic.reference(),
        &EncodedReference::String,
        "Topic wraps the string leaf"
    );

    // A newtype over a single-type Vector projection of a Plain type.
    let EncodedType::Newtype(topics) = declaration(schema.declarations(), &names, "Topics") else {
        panic!("Topics is a newtype");
    };
    let EncodedReference::SingleTypeApplication {
        projection,
        argument,
    } = topics.reference()
    else {
        panic!(
            "Topics wraps a single-type application, got {:?}",
            topics.reference()
        );
    };
    assert_eq!(*projection, SingleTypeReferenceProjection::Vector);
    assert!(
        matches!(argument.as_ref(), EncodedReference::Plain(id) if text(&names, *id) == "Topic"),
        "Topics = Vector.Topic",
    );

    // A Vector projection over a struct type: RecordSet = Vector.Entry.
    let EncodedType::Newtype(record_set) = declaration(schema.declarations(), &names, "RecordSet")
    else {
        panic!("RecordSet is a newtype");
    };
    let EncodedReference::SingleTypeApplication {
        projection,
        argument,
    } = record_set.reference()
    else {
        panic!("RecordSet wraps a single-type application");
    };
    assert_eq!(*projection, SingleTypeReferenceProjection::Vector);
    assert!(
        matches!(argument.as_ref(), EncodedReference::Plain(id) if text(&names, *id) == "Entry")
    );

    // A struct: four fields whose elided names are derived from their types.
    let EncodedType::Struct(entry) = declaration(schema.declarations(), &names, "Entry") else {
        panic!("Entry is a struct");
    };
    let entry_fields: Vec<&str> = entry
        .fields()
        .iter()
        .map(|field| text(&names, field.identifier()))
        .collect();
    assert_eq!(
        entry_fields,
        vec!["topics", "kind", "description", "magnitude"]
    );

    // The two enumerations, with their unit variants in order.
    let EncodedType::Enumeration(kind) = declaration(schema.declarations(), &names, "Kind") else {
        panic!("Kind is an enumeration");
    };
    let kind_variants: Vec<&str> = kind
        .variants()
        .iter()
        .map(|variant| text(&names, variant.identifier()))
        .collect();
    assert_eq!(
        kind_variants,
        vec![
            "Decision",
            "Principle",
            "Correction",
            "Clarification",
            "Constraint"
        ],
    );
    assert!(
        kind.variants()
            .iter()
            .all(|variant| variant.payload().is_none()),
        "Kind's variants are unit variants",
    );

    let EncodedType::Enumeration(magnitude) =
        declaration(schema.declarations(), &names, "Magnitude")
    else {
        panic!("Magnitude is an enumeration");
    };
    assert_eq!(
        magnitude.variants().len(),
        7,
        "Magnitude has seven variants"
    );

    // The interface roots: each variant binds a mail-type name to a Plain payload.
    let record = &input_root.variants()[0];
    assert_eq!(text(&names, record.identifier()), "Record");
    assert!(
        matches!(record.payload(), Some(EncodedReference::Plain(id)) if text(&names, *id) == "RecordPayload"),
        "input binds Record.RecordPayload",
    );
    let record_accepted = &output_root.variants()[0];
    assert_eq!(text(&names, record_accepted.identifier()), "RecordAccepted");
    assert!(
        matches!(record_accepted.payload(), Some(EncodedReference::Plain(id)) if text(&names, *id) == "RecordAcceptedPayload"),
        "output binds RecordAccepted.RecordAcceptedPayload",
    );
}

/// Encode is a genuine inverse of decode: re-decoding the encoded document yields an
/// equal `EncodedSchema`, and every root slot's canonical text is stable against the
/// source.
#[test]
fn spirit_min_document_round_trips_to_stable_text() {
    let textual = TextualSchema::schema_document().expect("build the document grammar");

    let mut names = NameTable::new(IdentifierNamespace::Schema);
    let schema = textual
        .decode_document(SPIRIT_MIN, &mut names)
        .expect("decode the whole document");

    let encoded = textual
        .encode_document(&schema, &mut names)
        .expect("encode the whole document");

    let mut names_again = NameTable::new(IdentifierNamespace::Schema);
    let redecoded = textual
        .decode_document(&encoded, &mut names_again)
        .expect("re-decode the encoded document");
    assert_eq!(
        schema, redecoded,
        "the document round-trips to an equal EncodedSchema"
    );

    let source = Recognizer::standard()
        .recognize(SPIRIT_MIN)
        .expect("recognize the source");
    let round_tripped = Recognizer::standard()
        .recognize(&encoded)
        .expect("recognize the encoded document");
    assert_eq!(source.holds_root_objects(), 6);
    assert_eq!(round_tripped.holds_root_objects(), 6);
    for slot in 0..6 {
        assert_eq!(
            round_tripped.root_object_at(slot).unwrap().canonical_text(),
            source.root_object_at(slot).unwrap().canonical_text(),
            "slot {slot} canonical text is stable across the round trip",
        );
    }

    println!("encoded document:\n{encoded}");
}

/// The two 2026-07-17 rulings, native-side: (1) the string scalar is spelled `String`
/// — a `Name.String` newtype recognizes the string leaf, and an elided `String` field
/// derives the name `string`; (2) a single-field braced declaration `Name.{ Field }`
/// lowers to a NEWTYPE over that field's reference, dropping the field name, matching
/// the legacy front end.
const RULING_MIN: &str = "\
{}
[Ingest.Entry]
[Stored.Entry]
{
  Note.String
  Summary.{ Note }
  Entry.{ String Note }
}
{}
{}";

#[test]
fn string_scalar_and_single_field_brace_follow_the_rulings() {
    let textual = TextualSchema::schema_document().expect("build the document grammar");
    let mut names = NameTable::new(IdentifierNamespace::Schema);
    let schema = textual
        .decode_document(RULING_MIN, &mut names)
        .expect("decode the ruling document");

    // Ruling 1: `Note.String` is a newtype over the string SCALAR leaf, not a Plain
    // reference to a user type named `String`.
    let EncodedType::Newtype(note) = declaration(schema.declarations(), &names, "Note") else {
        panic!("Note is a newtype");
    };
    assert_eq!(
        note.reference(),
        &EncodedReference::String,
        "Note wraps the string scalar leaf",
    );

    // Ruling 2: `Summary.{ Note }` — a single-field braced body — lowers to a newtype
    // over the field's reference, the name `Note` dropped.
    let EncodedType::Newtype(summary) = declaration(schema.declarations(), &names, "Summary")
    else {
        panic!("Summary is a newtype (single-field brace collapses)");
    };
    assert!(
        matches!(summary.reference(), EncodedReference::Plain(id) if text(&names, *id) == "Note"),
        "Summary wraps Plain(Note), got {:?}",
        summary.reference(),
    );

    // Ruling 1, field position: an elided `String` field recognizes the scalar and
    // derives the name `string`.
    let EncodedType::Struct(entry) = declaration(schema.declarations(), &names, "Entry") else {
        panic!("Entry is a struct (two fields)");
    };
    let entry_fields: Vec<(&str, &EncodedReference)> = entry
        .fields()
        .iter()
        .map(|field| (text(&names, field.identifier()), field.reference()))
        .collect();
    assert_eq!(
        entry_fields[0].0, "string",
        "an elided String field derives `string`"
    );
    assert_eq!(entry_fields[0].1, &EncodedReference::String);
    assert_eq!(entry_fields[1].0, "note");
    assert!(
        matches!(entry_fields[1].1, EncodedReference::Plain(id) if text(&names, *id) == "Note")
    );
}
