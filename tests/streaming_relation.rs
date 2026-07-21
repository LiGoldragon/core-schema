//! Streaming is one reusable encoded protocol relation, with no source spelling.

use core_schema::{CoreReference, CoreSchema, StreamingRelation};
use name_table::{IdentifierNamespace, Name, NameTable};

#[test]
fn streaming_relation_preserves_its_closed_encoded_links_in_order() {
    let mut names = NameTable::new(IdentifierNamespace::Schema);
    let open = names
        .intern(Name::new("OpenSubscription"))
        .expect("test relation fits its namespace");
    let acknowledged = names
        .intern(Name::new("SubscriptionOpened"))
        .expect("test relation fits its namespace");
    let token = names
        .intern(Name::new("SubscriptionToken"))
        .expect("test relation fits its namespace");
    let event = names
        .intern(Name::new("IntentEvent"))
        .expect("test relation fits its namespace");
    let close = names
        .intern(Name::new("CloseSubscription"))
        .expect("test relation fits its namespace");

    let relation = StreamingRelation::new(
        open,
        acknowledged,
        CoreReference::Plain(token),
        CoreReference::Plain(event),
        CoreReference::Plain(close),
    );
    let schema = CoreSchema::with_streaming_relations(Vec::new(), vec![relation]);
    let relation = &schema.streaming_relations()[0];

    assert_eq!(relation.opening_input_variant(), open);
    assert_eq!(relation.acknowledgement_output_variant(), acknowledged);
    assert!(matches!(relation.token(), CoreReference::Plain(identifier) if *identifier == token));
    assert!(matches!(relation.event(), CoreReference::Plain(identifier) if *identifier == event));
    assert!(
        matches!(relation.close_token(), CoreReference::Plain(identifier) if *identifier == close)
    );
}
