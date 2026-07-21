//! EncodedSchema owns its archive API and rejects semantic invalidity after rkyv
//! validation; callers cannot deserialize the domain type directly.

use core_schema::{
    DeclarationRole, EncodedDeclaration, EncodedEnum, EncodedNewtype, EncodedReference,
    EncodedSchema, EncodedType, EncodedVariant, StreamingRelation,
};
use name_table::Identifier;

#[test]
fn validated_archive_round_trips_a_streaming_schema() {
    let input = Identifier::Schema(0);
    let output = Identifier::Schema(1);
    let open = Identifier::Schema(2);
    let acknowledged = Identifier::Schema(3);
    let token = Identifier::Schema(4);
    let event = Identifier::Schema(5);
    let close = Identifier::Schema(6);
    let schema = EncodedSchema::with_streaming_relations(
        vec![
            EncodedDeclaration::interface(
                DeclarationRole::InterfaceInput,
                EncodedType::Enumeration(EncodedEnum::new(
                    input,
                    vec![EncodedVariant::new(open, None)],
                )),
            ),
            EncodedDeclaration::interface(
                DeclarationRole::InterfaceOutput,
                EncodedType::Enumeration(EncodedEnum::new(
                    output,
                    vec![EncodedVariant::new(acknowledged, None)],
                )),
            ),
            EncodedDeclaration::public(EncodedType::Newtype(EncodedNewtype::new(
                token,
                EncodedReference::Integer,
            ))),
            EncodedDeclaration::public(EncodedType::Newtype(EncodedNewtype::new(
                event,
                EncodedReference::Integer,
            ))),
            EncodedDeclaration::public(EncodedType::Newtype(EncodedNewtype::new(
                close,
                EncodedReference::Integer,
            ))),
        ],
        vec![StreamingRelation::new(
            open,
            acknowledged,
            EncodedReference::Plain(token),
            EncodedReference::Plain(event),
            EncodedReference::Plain(close),
        )],
    )
    .expect("fixture is semantically valid");

    let bytes = schema.to_archive_bytes().expect("archive schema");
    let loaded = EncodedSchema::from_archive_bytes(&bytes).expect("load schema");

    assert_eq!(loaded, schema);
    assert_eq!(
        loaded.content_identity().unwrap(),
        schema.content_identity().unwrap()
    );
}
