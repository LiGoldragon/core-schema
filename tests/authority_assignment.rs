//! The identity keystone at the schema layer. A universe built from central-authority
//! assignments ([`CoreUniverse::from_assignment`]) is a deterministic function of the
//! assignment, never of the order an ingestion parsed its declarations: names are
//! interned in canonical assigned-id order and declarations registered in that order.
//! Two ingestions of one declared schema that received the same assignment therefore
//! build byte-identical Core values — the repair of the "same thing, re-ID'ed" defect
//! that parse-order interning caused. The stronger cross-process proof (the authority
//! binding identical identities to the same declared schema) lives at the sema-storage
//! layer; this proves the schema-side plumbing the authority feeds is order-blind.

use core_schema::declaration::{CoreField, CoreStruct, CoreType};
use core_schema::{
    AssignedKind, AssignedMember, CoreDeclaration, CoreNewtype, CoreReference, CoreUniverse,
    UniverseError,
};
use name_table::{Identifier, IdentifierNamespace, Name, NameTable};
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
                AssignedKind::Declaration(CoreDeclaration::public(CoreType::Newtype(
                    CoreNewtype::new(Identifier::Schema(0), CoreReference::Integer),
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
    // Scalar newtype references carry no name identifier, so the source name space is
    // never consulted for these members; an empty schema slice exercises the plumbing.
    let source = NameTable::new(IdentifierNamespace::Schema);
    let forward = CoreUniverse::from_assignment(
        universe,
        scalar_newtypes([("Alpha", 0), ("Beta", 1)]),
        &source,
    )
    .expect("build forward");
    let reverse = CoreUniverse::from_assignment(
        universe,
        scalar_newtypes([("Beta", 1), ("Alpha", 0)]),
        &source,
    )
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
            built
                .names()
                .resolve(Identifier::Schema(0))
                .unwrap()
                .as_str(),
            "Alpha"
        );
        assert_eq!(
            built
                .names()
                .resolve(Identifier::Schema(1))
                .unwrap()
                .as_str(),
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

/// A two-declaration schema whose declarations reference each other and carry an
/// explicit field name, built against a source name space interned in the given
/// order. `Record` (local 0) is a struct `{ label: Text, link: Target }`; `Target`
/// (local 1) is a newtype over `Integer`. Varying `interning_order` varies the
/// source identifiers every interior name (`label`, `link`, and the `Plain` target
/// of `link`) carries, standing in for two ingestions that parsed the same declared
/// schema in different orders. The name→local assignment is held constant.
fn cross_referencing_schema(interning_order: [&str; 4]) -> (NameTable, Vec<AssignedMember>) {
    let mut names = NameTable::new(IdentifierNamespace::Schema);
    for name in interning_order {
        names
            .intern(Name::new(name))
            .expect("test schema fits its namespace");
    }
    // Re-interning returns the already-assigned identifier (dedupe), so these read the
    // source identities the chosen order produced.
    let record = names
        .intern(Name::new("Record"))
        .expect("test schema fits its namespace");
    let label = names
        .intern(Name::new("label"))
        .expect("test schema fits its namespace");
    let link = names
        .intern(Name::new("link"))
        .expect("test schema fits its namespace");
    let target = names
        .intern(Name::new("Target"))
        .expect("test schema fits its namespace");

    let record_type = CoreType::Struct(CoreStruct::new(
        record,
        vec![
            CoreField::new(label, CoreReference::String),
            CoreField::new(link, CoreReference::Plain(target)),
        ],
    ));
    let target_type = CoreType::Newtype(CoreNewtype::new(target, CoreReference::Integer));

    let members = vec![
        AssignedMember::new(
            0,
            Name::new("Record"),
            AssignedKind::Declaration(CoreDeclaration::public(record_type)),
        ),
        AssignedMember::new(
            1,
            Name::new("Target"),
            AssignedKind::Declaration(CoreDeclaration::public(target_type)),
        ),
    ];
    (names, members)
}

/// The interior re-stamping keystone: field names and `Plain` cross-references are
/// re-stamped from the source name space into the canonical one, so a schema whose
/// declarations reference each other and carry explicit field names hashes
/// identically whatever order each ingestion interned. Two source name spaces that
/// assign the four names different identifiers, under one shared assignment, build
/// byte-identical Core values.
#[test]
fn interior_names_are_re_stamped_to_canonical_order() {
    let universe = CoreUniverseId::new(101);
    let (mut source_forward, members_forward) =
        cross_referencing_schema(["Record", "label", "link", "Target"]);
    let (mut source_reverse, members_reverse) =
        cross_referencing_schema(["Target", "link", "label", "Record"]);

    // The two source name spaces genuinely disagree on identifiers, so a build that
    // carried interior identifiers through verbatim would hash differently — the test
    // bites only because the re-stamping neutralises that.
    assert_ne!(
        source_forward
            .intern(Name::new("Target"))
            .expect("test schema fits its namespace"),
        source_reverse
            .intern(Name::new("Target"))
            .expect("test schema fits its namespace"),
        "the two parse orders must assign different source identifiers",
    );

    let forward = CoreUniverse::from_assignment(universe, members_forward, &source_forward)
        .expect("build forward");
    let reverse = CoreUniverse::from_assignment(universe, members_reverse, &source_reverse)
        .expect("build reverse");

    assert_eq!(
        forward
            .declared_schema()
            .content_identity()
            .expect("hash forward"),
        reverse
            .declared_schema()
            .content_identity()
            .expect("hash reverse"),
        "field names and Plain cross-references re-stamped to canonical order — the \
         built schema's bytes are a pure function of (assignment, declaration content)",
    );
}

/// An identity names exactly one thing (law 2, at the schema layer): an assignment that
/// registers two members at the same local is rejected loudly rather than silently
/// collapsing them.
#[test]
fn duplicate_assigned_identity_is_rejected() {
    let universe = CoreUniverseId::new(7);
    let source = NameTable::new(IdentifierNamespace::Schema);
    let clash = CoreUniverse::from_assignment(
        universe,
        scalar_newtypes([("Alpha", 3), ("Beta", 3)]),
        &source,
    );
    assert!(matches!(
        clash,
        Err(UniverseError::DuplicateAssignedIdentity(3))
    ));
}
