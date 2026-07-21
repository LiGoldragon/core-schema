//! Authority assignments preserve the complete supplied Schema NameTable and every
//! encoded identifier. The bridge never resolves a name merely to re-intern it into
//! another namespace.

use core_schema::declaration::{CoreField, CoreStruct, CoreType};
use core_schema::{
    AssignedKind, AssignedMember, CoreDeclaration, CoreNewtype, CoreReference, CoreUniverse,
    UniverseError,
};
use name_table::{Identifier, IdentifierNamespace, Name, NameTable};
use structural_codec::ids::CoreUniverseId;

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
    let (names, identifiers) = schema_table(&["Record", "label", "Target"]);
    let [record, label, target] = identifiers.as_slice() else {
        panic!("fixture identifiers")
    };
    let declaration = CoreDeclaration::public(CoreType::Struct(CoreStruct::new(
        *record,
        vec![CoreField::new(*label, CoreReference::Plain(*target))],
    )));
    let target_declaration = CoreDeclaration::public(CoreType::Newtype(CoreNewtype::new(
        *target,
        CoreReference::Integer,
    )));

    let universe = CoreUniverse::from_assignment(
        CoreUniverseId::new(42),
        vec![
            AssignedMember::new(9, *target, AssignedKind::Declaration(target_declaration)),
            AssignedMember::new(3, *record, AssignedKind::Declaration(declaration.clone())),
        ],
        names,
    )
    .expect("Schema-home assignment is accepted");

    let stored = universe
        .core_type(structural_codec::ids::ScopedCoreTypeId::new(
            CoreUniverseId::new(42),
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
/// not copied, flattened, or renumbered while the CoreSchema member retains its own
/// Schema identifier.
#[test]
fn assignment_transfers_complete_composed_name_table() {
    let mut logos = NameTable::new(IdentifierNamespace::Logos);
    let foreign = logos.intern(Name::new("LogosToken")).expect("fixture fits");
    let mut schema = NameTable::new(IdentifierNamespace::Schema);
    let record = schema.intern(Name::new("Record")).expect("fixture fits");
    let composed = schema.compose(&logos).expect("borrow Logos slice");

    let universe = CoreUniverse::from_assignment(
        CoreUniverseId::new(43),
        vec![AssignedMember::new(
            0,
            record,
            AssignedKind::Declaration(CoreDeclaration::public(CoreType::Newtype(
                CoreNewtype::new(record, CoreReference::Integer),
            ))),
        )],
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
    let declaration = CoreDeclaration::public(CoreType::Newtype(CoreNewtype::new(
        logos_record,
        CoreReference::Integer,
    )));
    let error = CoreUniverse::from_assignment(
        CoreUniverseId::new(101),
        vec![AssignedMember::new(
            0,
            logos_record,
            AssignedKind::Declaration(declaration),
        )],
        composed,
    )
    .expect_err("foreign Core identifier is rejected");

    assert!(matches!(
        error,
        UniverseError::WrongSchemaIdentifier(Identifier::Logos(0))
    ));
}

#[test]
fn non_schema_name_table_home_is_rejected() {
    let mut logos = NameTable::new(IdentifierNamespace::Logos);
    let identifier = logos.intern(Name::new("Record")).expect("fixture fits");
    let error = CoreUniverse::from_assignment(
        CoreUniverseId::new(7),
        vec![AssignedMember::new(
            0,
            identifier,
            AssignedKind::LeafPrimitive,
        )],
        logos,
    )
    .expect_err("CoreSchema owns a Schema-home table");
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
    let error = CoreUniverse::from_assignment(
        CoreUniverseId::new(7),
        vec![AssignedMember::new(
            0,
            identifiers[0],
            AssignedKind::Declaration(CoreDeclaration::public(CoreType::Newtype(
                CoreNewtype::new(identifiers[1], CoreReference::Integer),
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
    let clash = CoreUniverse::from_assignment(
        CoreUniverseId::new(7),
        vec![
            AssignedMember::new(3, identifiers[0], AssignedKind::LeafPrimitive),
            AssignedMember::new(3, identifiers[1], AssignedKind::LeafPrimitive),
        ],
        names,
    );
    assert!(matches!(
        clash,
        Err(UniverseError::DuplicateAssignedIdentity(3))
    ));
}
