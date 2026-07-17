//! The identity keystone at the schema layer. A universe built from central-authority
//! assignments ([`CoreUniverse::from_assignment`]) is a deterministic function of the
//! assignment, never of the order an ingestion parsed its declarations: names are
//! interned in canonical assigned-id order and declarations registered in that order.
//! Two ingestions of one declared schema that received the same assignment therefore
//! build byte-identical Core values — the repair of the "same thing, re-ID'ed" defect
//! that parse-order interning caused. The stronger cross-process proof (the authority
//! binding identical identities to the same declared schema) lives at the sema-storage
//! layer; this proves the schema-side plumbing the authority feeds is order-blind.

use core_schema::declaration::CoreType;
use core_schema::{
    AssignedKind, AssignedMember, CoreNewtype, CoreReference, CoreUniverse, UniverseError,
};
use name_table::{Identifier, Name};
use structural_codec::ids::CoreUniverseId;

/// Two scalar newtypes — `Alpha` at local 0, `Beta` at local 1 — as an assignment, with
/// the input vector in the given order. The name→local mapping is held constant; only
/// the vector order varies, which the authority path must neutralise.
fn scalar_newtypes(order: [(&str, u32); 2]) -> Vec<AssignedMember> {
    order
        .into_iter()
        .map(|(name, local)| {
            AssignedMember::new(
                local,
                Name::new(name),
                // The newtype's placeholder identifier is re-stamped to the canonically
                // interned one by `from_assignment`; a scalar reference carries no
                // identifier, so the member's Core content is fixed by its assignment.
                AssignedKind::Declaration(CoreType::Newtype(CoreNewtype::new(
                    Identifier::new(0),
                    CoreReference::Integer,
                ))),
            )
        })
        .collect()
}

/// The universe is a function of the assignment alone: the same name→local mapping,
/// presented in opposite vector orders, yields the same universe id, the same canonical
/// name interning, and a byte-identical declared schema.
#[test]
fn authority_assignment_is_order_independent() {
    let universe = CoreUniverseId::new(42);
    let forward =
        CoreUniverse::from_assignment(universe, scalar_newtypes([("Alpha", 0), ("Beta", 1)]))
            .expect("build forward");
    let reverse =
        CoreUniverse::from_assignment(universe, scalar_newtypes([("Beta", 1), ("Alpha", 0)]))
            .expect("build reverse");

    assert_eq!(
        forward.universe(),
        reverse.universe(),
        "same minted universe"
    );

    // Names are interned in canonical assigned-id order, so `Alpha` (local 0) is always
    // identifier 0 and `Beta` (local 1) always identifier 1 — never the parse order.
    for built in [&forward, &reverse] {
        assert_eq!(
            built.names().resolve(Identifier::new(0)).unwrap().as_str(),
            "Alpha"
        );
        assert_eq!(
            built.names().resolve(Identifier::new(1)).unwrap().as_str(),
            "Beta"
        );
    }

    assert_eq!(
        forward
            .declared_schema()
            .content_identity()
            .expect("hash forward"),
        reverse
            .declared_schema()
            .content_identity()
            .expect("hash reverse"),
        "the authority path binds identical identities regardless of parse order",
    );
}

/// An identity names exactly one thing (law 2, at the schema layer): an assignment that
/// registers two members at the same local is rejected loudly rather than silently
/// collapsing them.
#[test]
fn duplicate_assigned_identity_is_rejected() {
    let universe = CoreUniverseId::new(7);
    let clash =
        CoreUniverse::from_assignment(universe, scalar_newtypes([("Alpha", 3), ("Beta", 3)]));
    assert!(matches!(
        clash,
        Err(UniverseError::DuplicateAssignedIdentity(3))
    ));
}
