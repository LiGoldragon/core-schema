//! CoreSchema owns its archive API and rejects semantic invalidity after rkyv
//! validation; callers cannot deserialize the domain type directly.

use core_schema::{
    CoreDeclaration, CoreEnum, CoreNewtype, CoreReference, CoreSchema, CoreType, CoreVariant,
    DeclarationRole, StreamingRelation,
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
    let schema = CoreSchema::with_streaming_relations(
        vec![
            CoreDeclaration::interface(
                DeclarationRole::InterfaceInput,
                CoreType::Enumeration(CoreEnum::new(input, vec![CoreVariant::new(open, None)])),
            ),
            CoreDeclaration::interface(
                DeclarationRole::InterfaceOutput,
                CoreType::Enumeration(CoreEnum::new(
                    output,
                    vec![CoreVariant::new(acknowledged, None)],
                )),
            ),
            CoreDeclaration::public(CoreType::Newtype(CoreNewtype::new(
                token,
                CoreReference::Integer,
            ))),
            CoreDeclaration::public(CoreType::Newtype(CoreNewtype::new(
                event,
                CoreReference::Integer,
            ))),
            CoreDeclaration::public(CoreType::Newtype(CoreNewtype::new(
                close,
                CoreReference::Integer,
            ))),
        ],
        vec![StreamingRelation::new(
            open,
            acknowledged,
            CoreReference::Plain(token),
            CoreReference::Plain(event),
            CoreReference::Plain(close),
        )],
    )
    .expect("fixture is semantically valid");

    let bytes = schema.to_archive_bytes().expect("archive schema");
    let loaded = CoreSchema::from_archive_bytes(&bytes).expect("load schema");

    assert_eq!(loaded, schema);
    assert_eq!(
        loaded.content_identity().unwrap(),
        schema.content_identity().unwrap()
    );
}
