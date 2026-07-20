//! Codec-backed witnesses: these forms are emitted, recognized, decoded, and emitted
//! again by one sealed StructureTree that matches the installed document surface.

use core_schema::{
    EncodedReference, EncodedVariant, StreamingRelation,
    source_surface_candidates::SourceSurfaceCandidates,
};
use name_table::{IdentifierNamespace, Name, NameTable};

#[test]
fn interface_unit_and_payload_candidates_round_trip_through_one_structure_tree() {
    let candidates = SourceSurfaceCandidates::build().unwrap();
    let mut names = NameTable::new(IdentifierNamespace::Schema);
    let closed = names.intern(Name::new("Closed")).expect("allocate Closed");
    let opened = names.intern(Name::new("Opened")).expect("allocate Opened");
    let token = names
        .intern(Name::new("SubscriptionToken"))
        .expect("allocate SubscriptionToken");
    let encoded = vec![
        EncodedVariant::new(closed, None),
        EncodedVariant::new(opened, Some(EncodedReference::Plain(token))),
    ];

    let emitted = candidates.emit_interface(&encoded, &names).unwrap();
    let mut decoded_names = NameTable::new(IdentifierNamespace::Schema);
    let decoded = candidates
        .decode_interface(&emitted, &mut decoded_names)
        .unwrap();
    let re_emitted = candidates.emit_interface(&decoded, &decoded_names).unwrap();

    assert_eq!(decoded, encoded);
    assert_eq!(re_emitted, emitted);
}

#[test]
fn closed_streaming_relation_candidate_round_trips_through_one_structure_tree() {
    let candidates = SourceSurfaceCandidates::build().unwrap();
    let mut names = NameTable::new(IdentifierNamespace::Schema);
    let opening = names
        .intern(Name::new("OpenSubscription"))
        .expect("allocate OpenSubscription");
    let acknowledgement = names
        .intern(Name::new("SubscriptionOpened"))
        .expect("allocate SubscriptionOpened");
    let token = names
        .intern(Name::new("SubscriptionToken"))
        .expect("allocate SubscriptionToken");
    let event = names
        .intern(Name::new("IntentEvent"))
        .expect("allocate IntentEvent");
    let close_token = names
        .intern(Name::new("CloseSubscription"))
        .expect("allocate CloseSubscription");
    let encoded = vec![StreamingRelation::new(
        opening,
        acknowledgement,
        EncodedReference::Plain(token),
        EncodedReference::Plain(event),
        EncodedReference::Plain(close_token),
    )];

    let emitted = candidates
        .emit_streaming_relations(&encoded, &names)
        .unwrap();
    let mut decoded_names = NameTable::new(IdentifierNamespace::Schema);
    let decoded = candidates
        .decode_streaming_relations(&emitted, &mut decoded_names)
        .unwrap();
    let re_emitted = candidates
        .emit_streaming_relations(&decoded, &decoded_names)
        .unwrap();

    assert_eq!(decoded, encoded);
    assert_eq!(re_emitted, emitted);
}
