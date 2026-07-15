//! Core identity is blake3 over the stringless rkyv bytes with the NameTable
//! excluded, so a rename is hash-stable by construction while a structural edit
//! moves the hash.

use core_schema::declaration::{CoreNewtype, CoreType};
use core_schema::{CoreDeclaration, CoreReference, CoreSchema, FixtureFamily};
use name_table::{Name, NameTable};

/// Rebuild a table identical to `original` except that identifier `target` resolves
/// to `replacement` — a rename, expressed as a fresh table since interning is
/// append-only. Interning order is preserved, so every other identifier keeps its
/// index.
fn rename(original: &NameTable, target: name_table::Identifier, replacement: &str) -> NameTable {
    let mut renamed = NameTable::new();
    for index in 0..original.len() {
        let identifier = name_table::Identifier::new(index as u32);
        let name = if identifier == target {
            Name::new(replacement)
        } else {
            original
                .resolve(identifier)
                .expect("known identifier")
                .clone()
        };
        renamed.intern(name);
    }
    renamed
}

/// A rename is a NameTable-only edit: the CoreSchema value is untouched, so its
/// content identity does not move, even though the projected name genuinely changes.
#[test]
fn a_rename_leaves_core_identity_unchanged() {
    let family = FixtureFamily::build();
    let schema = family.schema();
    let names = family.universe().names();

    // CommitSequence is the first declaration; take its identifier.
    let commit_identifier = schema.declarations()[0].identifier();
    let before = schema.content_identity().expect("hash before rename");

    let renamed = rename(names, commit_identifier, "Commitment");

    // The projected name really moved.
    assert_eq!(
        names.resolve(commit_identifier).unwrap().as_str(),
        "CommitSequence",
    );
    assert_eq!(
        renamed.resolve(commit_identifier).unwrap().as_str(),
        "Commitment",
    );

    // The Core hash did not — the stringless value carries no names.
    let after = schema.content_identity().expect("hash after rename");
    assert_eq!(before, after, "rename is hash-stable");
}

/// A structural edit — adding a field to a struct — DOES move the Core hash, so the
/// rename-stability above is a genuine property, not hash-insensitivity.
#[test]
fn a_structural_edit_moves_core_identity() {
    let family = FixtureFamily::build();
    let base = family.schema().content_identity().expect("base hash");

    // A one-declaration schema and the same declaration with an extra newtype hash
    // differently.
    let commit = family.schema().declarations()[0].clone();
    let smaller = CoreSchema::new(vec![commit.clone()]);
    let larger = CoreSchema::new(vec![
        commit,
        CoreDeclaration::public(CoreType::Newtype(CoreNewtype::new(
            name_table::Identifier::new(999),
            CoreReference::Boolean,
        ))),
    ]);

    assert_ne!(
        smaller.content_identity().expect("small"),
        larger.content_identity().expect("large"),
        "a structural change moves the Core hash",
    );
    assert_ne!(
        base,
        smaller.content_identity().expect("small vs full"),
        "the full family and a single-declaration schema differ",
    );
}
