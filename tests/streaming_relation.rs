//! Streaming relations are closed encoded protocol data whose construction checks
//! the schema relation law: Schema-owned endpoints, role-correct interfaces, and
//! data-type value references.

use core_schema::{
    CoreDeclaration, CoreEnum, CoreNewtype, CoreReference, CoreSchema, CoreSchemaError, CoreType,
    CoreVariant, DeclarationRole, MultiTypeReferenceProjection, SingleTypeReferenceProjection,
    StreamingReferenceForm, StreamingRelation, StreamingRelationReference,
    ValueReferenceProjection,
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
    schema_parts_in(IdentifierNamespace::Schema)
}

fn schema_parts_in(namespace: IdentifierNamespace) -> (RelationNames, Vec<CoreDeclaration>) {
    let mut names = NameTable::new(namespace);
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

fn relation_with_value_reference(
    names: &RelationNames,
    part: StreamingRelationReference,
    reference: CoreReference,
) -> StreamingRelation {
    match part {
        StreamingRelationReference::Token => StreamingRelation::new(
            names.open,
            names.acknowledged,
            reference,
            CoreReference::Plain(names.event),
            CoreReference::Plain(names.close),
        ),
        StreamingRelationReference::Event => StreamingRelation::new(
            names.open,
            names.acknowledged,
            CoreReference::Plain(names.token),
            reference,
            CoreReference::Plain(names.close),
        ),
        StreamingRelationReference::CloseToken => StreamingRelation::new(
            names.open,
            names.acknowledged,
            CoreReference::Plain(names.token),
            CoreReference::Plain(names.event),
            reference,
        ),
    }
}

#[test]
fn streaming_relation_accepts_data_type_value_references() {
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

/// A graph may be internally consistent in the Logos namespace, but it is still
/// foreign to CoreSchema. Matching references must not launder its identifiers.
#[test]
fn streaming_relation_rejects_a_fully_matching_logos_graph() {
    let (names, declarations) = schema_parts_in(IdentifierNamespace::Logos);
    let error = CoreSchema::with_streaming_relations(declarations, vec![valid_relation(&names)])
        .expect_err("Logos identifiers are foreign to a CoreSchema relation boundary");

    assert!(matches!(
        error,
        CoreSchemaError::NonSchemaIdentifier(identifier) if identifier == names.open
    ));
}

/// Interface roots are relation topology, not values. Each value position rejects
/// a root with a typed role error rather than treating any declared identifier as a
/// valid reference.
/// Every non-Plain reference class is rejected at each streaming value position.
/// Generic applications deliberately fail as applications: their otherwise-valid
/// Plain arguments are not recursively accepted as relation values.
#[test]
fn streaming_relation_requires_plain_data_type_references_at_every_value_position() {
    let (names, declarations) = schema_parts();
    let cases = [
        (CoreReference::String, StreamingReferenceForm::Scalar),
        (CoreReference::Integer, StreamingReferenceForm::Scalar),
        (CoreReference::Boolean, StreamingReferenceForm::Scalar),
        (CoreReference::Bytes, StreamingReferenceForm::Scalar),
        (
            CoreReference::ValueApplication {
                projection: ValueReferenceProjection::Bytes,
                value: 32,
            },
            StreamingReferenceForm::BytesLength,
        ),
        (
            CoreReference::SingleTypeApplication {
                projection: SingleTypeReferenceProjection::Optional,
                argument: Box::new(CoreReference::Plain(names.token)),
            },
            StreamingReferenceForm::SingleTypeApplication,
        ),
        (
            CoreReference::MultiTypeApplication {
                projection: MultiTypeReferenceProjection::Map,
                arguments: vec![
                    CoreReference::Plain(names.token),
                    CoreReference::Plain(names.event),
                ],
            },
            StreamingReferenceForm::MultiTypeApplication,
        ),
    ];

    for part in [
        StreamingRelationReference::Token,
        StreamingRelationReference::Event,
        StreamingRelationReference::CloseToken,
    ] {
        for (reference, expected_form) in &cases {
            let error = CoreSchema::with_streaming_relations(
                declarations.clone(),
                vec![relation_with_value_reference(
                    &names,
                    part,
                    reference.clone(),
                )],
            )
            .expect_err("only Plain data-type identifiers may fill streaming value positions");
            assert!(matches!(
                error,
                CoreSchemaError::StreamingReferenceMustNameDataType { part: actual_part, form }
                    if actual_part == part && form == *expected_form
            ));
        }
    }
}

#[test]
fn streaming_relation_rejects_interface_roots_as_value_references() {
    let (names, declarations) = schema_parts();
    let cases = [
        (
            StreamingRelationReference::Token,
            names.input,
            DeclarationRole::InterfaceInput,
        ),
        (
            StreamingRelationReference::Event,
            names.output,
            DeclarationRole::InterfaceOutput,
        ),
        (
            StreamingRelationReference::CloseToken,
            names.input,
            DeclarationRole::InterfaceInput,
        ),
    ];

    for (part, identifier, expected_role) in cases {
        let error = CoreSchema::with_streaming_relations(
            declarations.clone(),
            vec![relation_with_value_reference(
                &names,
                part,
                CoreReference::Plain(identifier),
            )],
        )
        .expect_err("interface roots cannot supply relation values");
        assert!(matches!(
            error,
            CoreSchemaError::StreamingReferenceNotDataType {
                part: actual_part,
                identifier: actual_identifier,
                actual,
            } if actual_part == part && actual_identifier == identifier && actual == expected_role
        ));
    }
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
