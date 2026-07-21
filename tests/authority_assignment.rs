//! Authority assignments preserve the complete supplied Schema NameTable and every
//! encoded identifier. The bridge never resolves a name merely to re-intern it into
//! another namespace.

use core_schema::declaration::{EncodedField, EncodedStruct, EncodedType};
use core_schema::{
    AssignedKind, AssignedMember, EncodedDeclaration, EncodedNewtype, EncodedReference,
    EncodedUniverse, EncodedUniverseBuilder, ScalarSlot, SingleTypeReferenceProjection,
    UniverseError,
};
use name_table::{Identifier, IdentifierNamespace, Name, NameTable};
use structural_codec::ids::{EncodedUniverseId, ScopedEncodedTypeId};

fn schema_table(names: &[&str]) -> (NameTable, Vec<Identifier>) {
    let mut table = NameTable::new(IdentifierNamespace::Schema);
    let identifiers = names
        .iter()
        .map(|name| table.intern(Name::new(*name)).expect("fixture fits"))
        .collect();
    (table, identifiers)
}

/// The authority boundary transfers its complete table and stored identifiers
/// verbatim. In particular, declarations, field names, and Plain targets are not
/// converted by resolving their spelling.
#[test]
fn authority_assignment_preserves_schema_identifiers_and_complete_table() {
    let (names, identifiers) = schema_table(&["Record", "label", "Target", "Integer"]);
    let [record, label, target, integer] = identifiers.as_slice() else {
        panic!("fixture identifiers")
    };
    let declaration = EncodedDeclaration::public(EncodedType::Struct(EncodedStruct::new(
        *record,
        vec![EncodedField::new(*label, EncodedReference::Plain(*target))],
    )));
    let target_declaration = EncodedDeclaration::public(EncodedType::Newtype(EncodedNewtype::new(
        *target,
        EncodedReference::Integer,
    )));

    let universe = EncodedUniverse::from_assignment(
        EncodedUniverseId::new(42),
        vec![
            AssignedMember::new(9, *target, AssignedKind::Declaration(target_declaration)),
            AssignedMember::new(3, *record, AssignedKind::Declaration(declaration.clone())),
            AssignedMember::new(
                1,
                *integer,
                AssignedKind::ScalarPrimitive(ScalarSlot::Integer),
            ),
        ],
        names,
    )
    .expect("Schema-home assignment is accepted");

    let stored = universe
        .encoded_type(structural_codec::ids::ScopedEncodedTypeId::new(
            EncodedUniverseId::new(42),
            3,
        ))
        .expect("record declaration");
    assert_eq!(
        stored,
        declaration.value(),
        "stored declaration is unmodified"
    );
    assert_eq!(
        universe.names().resolve(*target).unwrap().as_str(),
        "Target",
        "the supplied Schema home slice moved intact",
    );
}

/// A completed foreign slice remains borrowed by the moved Schema-home table; it is
/// not copied, flattened, or renumbered while the EncodedSchema member retains its own
/// Schema identifier.
#[test]
fn assignment_transfers_complete_composed_name_table() {
    let mut logos = NameTable::new(IdentifierNamespace::Logos);
    let foreign = logos.intern(Name::new("LogosToken")).expect("fixture fits");
    let mut schema = NameTable::new(IdentifierNamespace::Schema);
    let record = schema.intern(Name::new("Record")).expect("fixture fits");
    let integer = schema.intern(Name::new("Integer")).expect("fixture fits");
    let composed = schema.compose(&logos).expect("borrow Logos slice");

    let universe = EncodedUniverse::from_assignment(
        EncodedUniverseId::new(43),
        vec![
            AssignedMember::new(
                0,
                record,
                AssignedKind::Declaration(EncodedDeclaration::public(EncodedType::Newtype(
                    EncodedNewtype::new(record, EncodedReference::Integer),
                ))),
            ),
            AssignedMember::new(
                1,
                integer,
                AssignedKind::ScalarPrimitive(ScalarSlot::Integer),
            ),
        ],
        composed,
    )
    .expect("Schema member with borrowed foreign slice is valid");

    assert_eq!(
        universe.names().resolve(foreign).unwrap().as_str(),
        "LogosToken",
        "the complete borrowed Logos slice is retained"
    );
    assert_eq!(foreign, Identifier::Logos(0));
}

/// A Logos identifier remains Logos even in a composed Schema table. The authority
/// boundary rejects it rather than turning its spelling into a Schema identifier.
#[test]
fn logos_identifier_is_never_silently_converted_to_schema() {
    let mut logos = NameTable::new(IdentifierNamespace::Logos);
    let logos_record = logos.intern(Name::new("Record")).expect("fixture fits");
    assert_eq!(logos_record, Identifier::Logos(0));

    let schema = NameTable::new(IdentifierNamespace::Schema);
    let composed = schema.compose(&logos).expect("borrow Logos slice");
    let declaration = EncodedDeclaration::public(EncodedType::Newtype(EncodedNewtype::new(
        logos_record,
        EncodedReference::Integer,
    )));
    let error = EncodedUniverse::from_assignment(
        EncodedUniverseId::new(101),
        vec![AssignedMember::new(
            0,
            logos_record,
            AssignedKind::Declaration(declaration),
        )],
        composed,
    )
    .expect_err("foreign Encoded identifier is rejected");

    assert!(matches!(
        error,
        UniverseError::WrongSchemaIdentifier(Identifier::Logos(0))
    ));
}

#[test]
fn non_schema_name_table_home_is_rejected() {
    let mut logos = NameTable::new(IdentifierNamespace::Logos);
    let identifier = logos.intern(Name::new("Record")).expect("fixture fits");
    let error = EncodedUniverse::from_assignment(
        EncodedUniverseId::new(7),
        vec![AssignedMember::new(
            0,
            identifier,
            AssignedKind::LeafPrimitive,
        )],
        logos,
    )
    .expect_err("EncodedSchema owns a Schema-home table");
    assert!(matches!(
        error,
        UniverseError::WrongNameTableHome {
            actual: IdentifierNamespace::Logos
        }
    ));
}

#[test]
fn declaration_identifier_must_match_assigned_identifier() {
    let (names, identifiers) = schema_table(&["Assigned", "Stored"]);
    let error = EncodedUniverse::from_assignment(
        EncodedUniverseId::new(7),
        vec![AssignedMember::new(
            0,
            identifiers[0],
            AssignedKind::Declaration(EncodedDeclaration::public(EncodedType::Newtype(
                EncodedNewtype::new(identifiers[1], EncodedReference::Integer),
            ))),
        )],
        names,
    )
    .expect_err("mismatched authority and declaration identities are rejected");
    assert!(matches!(
        error,
        UniverseError::AssignedDeclarationIdentifierMismatch { .. }
    ));
}

#[test]
fn duplicate_assigned_identity_is_rejected() {
    let (names, identifiers) = schema_table(&["Alpha", "Beta"]);
    let clash = EncodedUniverse::from_assignment(
        EncodedUniverseId::new(7),
        vec![
            AssignedMember::new(3, identifiers[0], AssignedKind::LeafPrimitive),
            AssignedMember::new(3, identifiers[1], AssignedKind::LeafPrimitive),
        ],
        names,
    );
    assert!(matches!(
        clash,
        Err(UniverseError::DuplicateMemberIdentity(id)) if id == ScopedEncodedTypeId::new(EncodedUniverseId::new(7), 3)
    ));
}

/// The universal seal checks every builder path before any registry map exists:
/// NameTable home and resolution, Schema ownership, and both registry keys cannot
/// be bypassed by direct builder use.
#[test]
fn direct_builder_seal_rejects_wrong_home_foreign_unresolved_and_duplicate_members() {
    let wrong_home =
        EncodedUniverseBuilder::from_name_table(NameTable::new(IdentifierNamespace::Logos))
            .build(EncodedUniverseId::new(8));
    assert!(matches!(
        wrong_home,
        Err(UniverseError::WrongNameTableHome {
            actual: IdentifierNamespace::Logos
        })
    ));

    let mut foreign_builder = EncodedUniverseBuilder::new();
    foreign_builder.primitive_at(
        ScopedEncodedTypeId::new(EncodedUniverseId::new(8), 0),
        Identifier::Logos(0),
        ScalarSlot::Integer,
    );
    assert!(matches!(
        foreign_builder.build(EncodedUniverseId::new(8)),
        Err(UniverseError::WrongSchemaIdentifier(Identifier::Logos(0)))
    ));

    let mut unresolved_builder = EncodedUniverseBuilder::new();
    unresolved_builder.leaf_at(
        ScopedEncodedTypeId::new(EncodedUniverseId::new(8), 0),
        Identifier::Schema(99),
    );
    assert!(matches!(
        unresolved_builder.build(EncodedUniverseId::new(8)),
        Err(UniverseError::Names(_))
    ));

    let mut duplicate_id_builder = EncodedUniverseBuilder::new();
    let alpha = duplicate_id_builder.intern("Alpha").unwrap();
    let beta = duplicate_id_builder.intern("Beta").unwrap();
    let duplicate_id = ScopedEncodedTypeId::new(EncodedUniverseId::new(8), 0);
    duplicate_id_builder.leaf_at(duplicate_id, alpha);
    duplicate_id_builder.leaf_at(duplicate_id, beta);
    assert!(matches!(
        duplicate_id_builder.build(EncodedUniverseId::new(8)),
        Err(UniverseError::DuplicateMemberIdentity(id)) if id == duplicate_id
    ));

    let mut duplicate_name_builder = EncodedUniverseBuilder::new();
    let alpha = duplicate_name_builder.intern("Alpha").unwrap();
    duplicate_name_builder.leaf_at(
        ScopedEncodedTypeId::new(EncodedUniverseId::new(8), 0),
        alpha,
    );
    duplicate_name_builder.leaf_at(
        ScopedEncodedTypeId::new(EncodedUniverseId::new(8), 1),
        alpha,
    );
    assert!(matches!(
        duplicate_name_builder.build(EncodedUniverseId::new(8)),
        Err(UniverseError::DuplicateMemberName(name)) if name == alpha
    ));
}

#[test]
fn direct_builder_seal_rejects_member_from_another_universe() {
    let expected = EncodedUniverseId::new(12);
    let actual = EncodedUniverseId::new(13);
    let mut builder = EncodedUniverseBuilder::new();
    let alpha = builder.intern("Alpha").unwrap();
    let foreign_member = ScopedEncodedTypeId::new(actual, 0);
    builder.leaf_at(foreign_member, alpha);

    assert!(matches!(
        builder.build(expected),
        Err(UniverseError::UniverseScopeMismatch {
            expected: mismatch_expected,
            actual: mismatch_actual,
            member,
        }) if mismatch_expected == expected && mismatch_actual == actual && member == foreign_member
    ));
}

#[test]
fn direct_builder_seal_rejects_nested_plain_reference_from_another_universe() {
    let expected = EncodedUniverseId::new(14);
    let actual = EncodedUniverseId::new(15);
    let mut builder = EncodedUniverseBuilder::new();
    let record = builder.intern("Record").unwrap();
    let target = builder.intern("Target").unwrap();
    let foreign_target = ScopedEncodedTypeId::new(actual, 1);
    builder.declaration(
        ScopedEncodedTypeId::new(expected, 0),
        EncodedDeclaration::public(EncodedType::Newtype(EncodedNewtype::new(
            record,
            EncodedReference::SingleTypeApplication {
                projection: SingleTypeReferenceProjection::Optional,
                argument: Box::new(EncodedReference::Plain(target)),
            },
        ))),
    );
    builder.leaf_at(foreign_target, target);

    assert!(matches!(
        builder.build(expected),
        Err(UniverseError::UniverseScopeMismatch {
            expected: mismatch_expected,
            actual: mismatch_actual,
            member,
        }) if mismatch_expected == expected && mismatch_actual == actual && member == foreign_target
    ));
}

#[test]
fn direct_builder_seal_rejects_nested_scalar_reference_from_another_universe() {
    let expected = EncodedUniverseId::new(16);
    let actual = EncodedUniverseId::new(17);
    let mut builder = EncodedUniverseBuilder::new();
    let record = builder.intern("Record").unwrap();
    let integer = builder.intern("Integer").unwrap();
    let foreign_integer = ScopedEncodedTypeId::new(actual, 1);
    builder.declaration(
        ScopedEncodedTypeId::new(expected, 0),
        EncodedDeclaration::public(EncodedType::Newtype(EncodedNewtype::new(
            record,
            EncodedReference::SingleTypeApplication {
                projection: SingleTypeReferenceProjection::Optional,
                argument: Box::new(EncodedReference::Integer),
            },
        ))),
    );
    builder.primitive_at(foreign_integer, integer, ScalarSlot::Integer);

    assert!(matches!(
        builder.build(expected),
        Err(UniverseError::UniverseScopeMismatch {
            expected: mismatch_expected,
            actual: mismatch_actual,
            member,
        }) if mismatch_expected == expected && mismatch_actual == actual && member == foreign_integer
    ));
}

#[test]
fn direct_builder_seal_rejects_an_absent_scalar_slot() {
    let universe = EncodedUniverseId::new(18);
    let mut builder = EncodedUniverseBuilder::new();
    let record = builder.intern("Record").unwrap();
    builder.declaration(
        ScopedEncodedTypeId::new(universe, 0),
        EncodedDeclaration::public(EncodedType::Newtype(EncodedNewtype::new(
            record,
            EncodedReference::Integer,
        ))),
    );

    assert!(matches!(
        builder.build(universe),
        Err(UniverseError::MissingScalarSlot {
            slot: ScalarSlot::Integer,
            reference: EncodedReference::Integer,
        })
    ));
}

#[test]
fn direct_builder_seal_rejects_a_name_table_only_plain_target() {
    let universe = EncodedUniverseId::new(19);
    let mut builder = EncodedUniverseBuilder::new();
    let record = builder.intern("Record").unwrap();
    let target = builder.intern("TableOnlyTarget").unwrap();
    builder.declaration(
        ScopedEncodedTypeId::new(universe, 0),
        EncodedDeclaration::public(EncodedType::Newtype(EncodedNewtype::new(
            record,
            EncodedReference::Plain(target),
        ))),
    );

    assert!(matches!(
        builder.build(universe),
        Err(UniverseError::ReferenceTargetUnregistered {
            identifier,
            reference: EncodedReference::Plain(reference),
        }) if identifier == target && reference == target
    ));
}

#[test]
fn direct_builder_seal_rejects_nested_missing_scalar_and_member_references() {
    let universe = EncodedUniverseId::new(20);
    let mut scalar_builder = EncodedUniverseBuilder::new();
    let record = scalar_builder.intern("Record").unwrap();
    scalar_builder.declaration(
        ScopedEncodedTypeId::new(universe, 0),
        EncodedDeclaration::public(EncodedType::Newtype(EncodedNewtype::new(
            record,
            EncodedReference::SingleTypeApplication {
                projection: SingleTypeReferenceProjection::Optional,
                argument: Box::new(EncodedReference::Integer),
            },
        ))),
    );
    assert!(matches!(
        scalar_builder.build(universe),
        Err(UniverseError::MissingScalarSlot {
            slot: ScalarSlot::Integer,
            reference: EncodedReference::Integer,
        })
    ));

    let mut member_builder = EncodedUniverseBuilder::new();
    let record = member_builder.intern("Record").unwrap();
    let target = member_builder.intern("TableOnlyTarget").unwrap();
    member_builder.declaration(
        ScopedEncodedTypeId::new(universe, 0),
        EncodedDeclaration::public(EncodedType::Newtype(EncodedNewtype::new(
            record,
            EncodedReference::SingleTypeApplication {
                projection: SingleTypeReferenceProjection::Optional,
                argument: Box::new(EncodedReference::Plain(target)),
            },
        ))),
    );
    assert!(matches!(
        member_builder.build(universe),
        Err(UniverseError::ReferenceTargetUnregistered {
            identifier,
            reference: EncodedReference::Plain(reference),
        }) if identifier == target && reference == target
    ));
}

#[test]
fn direct_builder_seal_resolves_registered_scalar_and_plain_targets() {
    let universe_id = EncodedUniverseId::new(21);
    let mut builder = EncodedUniverseBuilder::new();
    let record = builder.intern("Record").unwrap();
    let target = builder.intern("Target").unwrap();
    let integer = builder.intern("Integer").unwrap();
    let target_id = ScopedEncodedTypeId::new(universe_id, 1);
    let integer_id = ScopedEncodedTypeId::new(universe_id, 2);
    builder.declaration(
        ScopedEncodedTypeId::new(universe_id, 0),
        EncodedDeclaration::public(EncodedType::Newtype(EncodedNewtype::new(
            record,
            EncodedReference::Plain(target),
        ))),
    );
    builder.declaration(
        target_id,
        EncodedDeclaration::public(EncodedType::Newtype(EncodedNewtype::new(
            target,
            EncodedReference::Integer,
        ))),
    );
    builder.primitive_at(integer_id, integer, ScalarSlot::Integer);

    let universe = builder
        .build(universe_id)
        .expect("registered scalar and member references satisfy the seal");
    assert_eq!(
        universe
            .resolve_reference(&EncodedReference::Plain(target))
            .unwrap(),
        target_id
    );
    assert_eq!(
        universe
            .resolve_reference(&EncodedReference::Integer)
            .unwrap(),
        integer_id
    );
}
