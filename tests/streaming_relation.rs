//! Streaming is one reusable encoded protocol relation, with no source spelling.

use core_schema::{EncodedReference, EncodedSchema, StreamingRelation};
use name_table::{IdentifierNamespace, Name, NameTable};

#[test]
fn streaming_relation_preserves_its_closed_encoded_links_in_order() {
    let mut names = NameTable::new(IdentifierNamespace::Schema);
    let open = names.intern(Name::new("OpenSubscription"));
    let acknowledged = names.intern(Name::new("SubscriptionOpened"));
    let token = names.intern(Name::new("SubscriptionToken"));
    let event = names.intern(Name::new("IntentEvent"));
    let close = names.intern(Name::new("CloseSubscription"));

    let relation = StreamingRelation::new(
        open,
        acknowledged,
        EncodedReference::Plain(token),
        EncodedReference::Plain(event),
        EncodedReference::Plain(close),
    );
    let schema = EncodedSchema::with_streaming_relations(Vec::new(), vec![relation]);
    let relation = &schema.streaming_relations()[0];

    assert_eq!(relation.opening_input_variant(), open);
    assert_eq!(relation.acknowledgement_output_variant(), acknowledged);
    assert!(
        matches!(relation.token(), EncodedReference::Plain(identifier) if *identifier == token)
    );
    assert!(
        matches!(relation.event(), EncodedReference::Plain(identifier) if *identifier == event)
    );
    assert!(
        matches!(relation.close_token(), EncodedReference::Plain(identifier) if *identifier == close)
    );
}
