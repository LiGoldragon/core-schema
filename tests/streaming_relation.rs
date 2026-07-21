//! Streaming relations are closed encoded protocol data whose construction checks
//! the schema relation law: input endpoint, output endpoint, and resolvable values.

use core_schema::{
    CoreDeclaration, CoreEnum, CoreNewtype, CoreReference, CoreSchema, CoreSchemaError, CoreType,
    CoreVariant, DeclarationRole, StreamingRelation, StreamingRelationReference,
};
use name_table::{Identifier, IdentifierNamespace, Name, NameTable};

struct RelationNames {
    input: Identifier,
    output: Identifier,
    open: Identifier,
    acknowledged: Identifier,
    arbitrary: Identifier,
    token: Identifier,
    event: Identifier,
    close: Identifier,
}

fn schema_parts() -> (RelationNames, Vec<CoreDeclaration>) {
    let mut names = NameTable::new(IdentifierNamespace::Schema);
    let identifier = |names: &mut NameTable, name| names.intern(Name::new(name)).unwrap();
    let input = identifier(&mut names, "Input");
    let output = identifier(&mut names, "Output");
    let open = identifier(&mut names, "OpenSubscription");
    let acknowledged = identifier(&mut names, "SubscriptionOpened");
    let arbitrary = identifier(&mut names, "ArbitraryType");
    let token = identifier(&mut names, "SubscriptionToken");
    let event = identifier(&mut names, "IntentEvent");
    let close = identifier(&mut names, "CloseSubscription");

    let declarations = vec![
        CoreDeclaration::interface(
            DeclarationRole::InterfaceInput,
            CoreType::Enumeration(CoreEnum::new(
                input,
                vec![CoreVariant::new(open, Some(CoreReference::Plain(token)))],
            )),
        ),
        CoreDeclaration::interface(
            DeclarationRole::InterfaceOutput,
            CoreType::Enumeration(CoreEnum::new(
                output,
                vec![CoreVariant::new(
                    acknowledged,
                    Some(CoreReference::Plain(event)),
                )],
            )),
        ),
        CoreDeclaration::public(CoreType::Newtype(CoreNewtype::new(
            arbitrary,
            CoreReference::Integer,
        ))),
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
    ];
    (
        RelationNames {
            input,
            output,
            open,
            acknowledged,
            arbitrary,
            token,
            event,
            close,
        },
        declarations,
    )
}

fn valid_relation(names: &RelationNames) -> StreamingRelation {
    StreamingRelation::new(
        names.open,
        names.acknowledged,
        CoreReference::Plain(names.token),
        CoreReference::Plain(names.event),
        CoreReference::Plain(names.close),
    )
}

#[test]
fn streaming_relation_preserves_valid_closed_encoded_links_in_order() {
    let (names, declarations) = schema_parts();
    let schema = CoreSchema::with_streaming_relations(declarations, vec![valid_relation(&names)])
        .expect("valid relation");
    let relation = &schema.streaming_relations()[0];

    assert_eq!(relation.opening_input_variant(), names.open);
    assert_eq!(
        relation.acknowledgement_output_variant(),
        names.acknowledged
    );
    assert!(
        matches!(relation.token(), CoreReference::Plain(identifier) if *identifier == names.token)
    );
    assert!(
        matches!(relation.event(), CoreReference::Plain(identifier) if *identifier == names.event)
    );
    assert!(
        matches!(relation.close_token(), CoreReference::Plain(identifier) if *identifier == names.close)
    );
    assert_eq!(schema.input().unwrap().identifier(), names.input);
    assert_eq!(schema.output().unwrap().identifier(), names.output);
}

#[test]
fn streaming_relation_rejects_swapped_endpoints() {
    let (names, declarations) = schema_parts();
    let error = CoreSchema::with_streaming_relations(
        declarations,
        vec![StreamingRelation::new(
            names.acknowledged,
            names.open,
            CoreReference::Plain(names.token),
            CoreReference::Plain(names.event),
            CoreReference::Plain(names.close),
        )],
    )
    .expect_err("an output variant cannot open an input relation");
    assert!(matches!(
        error,
        CoreSchemaError::OpeningEndpointNotInputVariant(identifier) if identifier == names.acknowledged
    ));

    let output_error = CoreSchema::with_streaming_relations(
        schema_parts().1,
        vec![StreamingRelation::new(
            names.open,
            names.open,
            CoreReference::Plain(names.token),
            CoreReference::Plain(names.event),
            CoreReference::Plain(names.close),
        )],
    )
    .expect_err("an input variant cannot acknowledge an output relation");
    assert!(matches!(
        output_error,
        CoreSchemaError::AcknowledgementEndpointNotOutputVariant(identifier) if identifier == names.open
    ));
}

#[test]
fn streaming_relation_rejects_arbitrary_endpoint() {
    let (names, declarations) = schema_parts();
    let error = CoreSchema::with_streaming_relations(
        declarations,
        vec![StreamingRelation::new(
            names.arbitrary,
            names.acknowledged,
            CoreReference::Plain(names.token),
            CoreReference::Plain(names.event),
            CoreReference::Plain(names.close),
        )],
    )
    .expect_err("data declaration is not an input interface variant");
    assert!(matches!(
        error,
        CoreSchemaError::OpeningEndpointNotInputVariant(identifier) if identifier == names.arbitrary
    ));
}

#[test]
fn streaming_relation_rejects_unresolved_endpoint_and_reference() {
    let (names, declarations) = schema_parts();
    let unresolved = Identifier::Schema(999);
    let endpoint_error = CoreSchema::with_streaming_relations(
        declarations.clone(),
        vec![StreamingRelation::new(
            unresolved,
            names.acknowledged,
            CoreReference::Plain(names.token),
            CoreReference::Plain(names.event),
            CoreReference::Plain(names.close),
        )],
    )
    .expect_err("unresolved endpoint is not an input-interface variant");
    assert!(matches!(
        endpoint_error,
        CoreSchemaError::OpeningEndpointNotInputVariant(identifier) if identifier == unresolved
    ));

    let reference_error = CoreSchema::with_streaming_relations(
        declarations,
        vec![StreamingRelation::new(
            names.open,
            names.acknowledged,
            CoreReference::Plain(unresolved),
            CoreReference::Plain(names.event),
            CoreReference::Plain(names.close),
        )],
    )
    .expect_err("unresolved relation reference is rejected");
    assert!(matches!(
        reference_error,
        CoreSchemaError::UnresolvedStreamingReference {
            part: StreamingRelationReference::Token,
            identifier,
        } if identifier == unresolved
    ));
}
