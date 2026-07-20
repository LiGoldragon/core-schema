//! Reproducibly emit codec-backed witnesses for installed source forms.
//!
//! Run with `cargo run --example source_surface_candidates`. The printed source blocks
//! are not hand-authored examples: the experimental StructureTree emits them and the
//! executable decodes and re-emits each before printing.

use core_schema::{
    EncodedReference, EncodedVariant, StreamingRelation,
    source_surface_candidates::SourceSurfaceCandidates,
};
use name_table::{IdentifierNamespace, Name, NameTable};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let candidates = SourceSurfaceCandidates::build()?;

    let mut interface_names = NameTable::new(IdentifierNamespace::Schema);
    let closed = interface_names.intern(Name::new("Closed"))?;
    let opened = interface_names.intern(Name::new("Opened"))?;
    let token = interface_names.intern(Name::new("SubscriptionToken"))?;
    let variants = vec![
        EncodedVariant::new(closed, None),
        EncodedVariant::new(opened, Some(EncodedReference::Plain(token))),
    ];
    let interface = candidates.emit_interface(&variants, &interface_names)?;
    let mut decoded_interface_names = NameTable::new(IdentifierNamespace::Schema);
    let decoded_variants = candidates.decode_interface(&interface, &mut decoded_interface_names)?;
    if candidates.emit_interface(&decoded_variants, &decoded_interface_names)? != interface {
        return Err("interface candidate failed round-trip".into());
    }

    let mut streaming_names = NameTable::new(IdentifierNamespace::Schema);
    let opening = streaming_names.intern(Name::new("OpenSubscription"))?;
    let acknowledgement = streaming_names.intern(Name::new("SubscriptionOpened"))?;
    let streaming_token = streaming_names.intern(Name::new("SubscriptionToken"))?;
    let event = streaming_names.intern(Name::new("IntentEvent"))?;
    let close_token = streaming_names.intern(Name::new("CloseSubscription"))?;
    let relations = vec![StreamingRelation::new(
        opening,
        acknowledgement,
        EncodedReference::Plain(streaming_token),
        EncodedReference::Plain(event),
        EncodedReference::Plain(close_token),
    )];
    let streaming = candidates.emit_streaming_relations(&relations, &streaming_names)?;
    let mut decoded_streaming_names = NameTable::new(IdentifierNamespace::Schema);
    let decoded_relations =
        candidates.decode_streaming_relations(&streaming, &mut decoded_streaming_names)?;
    if candidates.emit_streaming_relations(&decoded_relations, &decoded_streaming_names)?
        != streaming
    {
        return Err("streaming candidate failed round-trip".into());
    }

    println!("# Codec-backed source candidates\n");
    println!("These blocks were emitted and round-tripped by `SourceSurfaceCandidates`.\n");
    println!("## Interface alternatives\n\n```\n{interface}\n```\n");
    println!("## Closed streaming relations\n\n```\n{streaming}\n```");
    Ok(())
}
