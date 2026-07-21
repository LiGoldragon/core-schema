//! `TextualSchema` is the reference instance of the shared `TextualForm` textual
//! interface. This witness proves its generalized `view` / `unview` reproduce schema's
//! own single-declaration `encode` / `decode` byte-for-byte and value-for-value, on both
//! the newtype and struct golden. The interface was generalized out of schema, so
//! schema's existing behavior proves the shared view fits without change.

use core_schema::SchemaLanguage;
use core_schema::TextualSchema;
use core_schema::declaration::EncodedType;
use core_schema::fixture::{COMMIT_SEQUENCE, DATABASE_MARKER};
use name_table::{IdentifierNamespace, NameTable};
use structural_codec::{Textual, TextualForm};

#[test]
fn view_and_unview_reproduce_encode_and_decode() {
    let goldens: [(_, &str); 2] = [
        (COMMIT_SEQUENCE, "CommitSequence.{ Integer }"),
        (
            DATABASE_MARKER,
            "DatabaseMarker.{ CommitSequence StateDigest StateDigest }",
        ),
    ];

    for (expected, source) in goldens {
        let textual = TextualSchema::fixture().expect("build textual schema");

        // The inherent single-declaration path (schema's own decode/encode).
        let mut inherent_names = NameTable::new(IdentifierNamespace::Schema);
        let decoded: EncodedType = textual
            .decode(expected, source, &mut inherent_names)
            .expect("inherent decode");
        let encoded: String = textual
            .encode(expected, &decoded, &mut inherent_names)
            .expect("inherent encode");

        // The shared textual interface uses the same structuretree and nametree.
        // Data crosses the boundary only as a `TextualForm<SchemaLanguage>` value.
        let mut textual_names = NameTable::new(IdentifierNamespace::Schema);
        let source_view: TextualForm<SchemaLanguage> = TextualForm::single(source.to_string());
        let unviewed: EncodedType = textual
            .unview(expected, &source_view, &mut textual_names)
            .expect("shared unview");
        let viewed_form: TextualForm<SchemaLanguage> = textual
            .view(expected, &unviewed, &mut textual_names)
            .expect("shared view");
        let viewed: String = viewed_form.sole_text().expect("sole view text").to_string();

        assert_eq!(decoded, unviewed, "unview reproduces decode for `{source}`");
        assert_eq!(encoded, viewed, "view reproduces encode for `{source}`");
        println!("witness `{source}` => shared textual view: {viewed}");
    }
}
