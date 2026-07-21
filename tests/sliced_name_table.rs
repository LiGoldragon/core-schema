//! Core-schema consumes namespace-sliced name tables without flattening or
//! renumbering a completed foreign slice.

use name_table::{Identifier, IdentifierNamespace, Name, NameTable};

#[test]
fn composed_foreign_slice_retains_its_namespace_and_local_identifier() {
    let mut logos = NameTable::new(IdentifierNamespace::Logos);
    let foreign = logos
        .intern(Name::new("SharedToken"))
        .expect("one Logos name fits its namespace");
    assert_eq!(foreign, Identifier::Logos(0));

    let mut schema = NameTable::new(IdentifierNamespace::Schema);
    let home = schema
        .intern(Name::new("SchemaOnly"))
        .expect("schema home fits its namespace");
    assert_eq!(home, Identifier::Schema(0));
    let mut composed = schema
        .compose(&logos)
        .expect("compose completed Logos slice");

    assert_eq!(
        composed
            .resolve(foreign)
            .expect("foreign name resolves")
            .as_str(),
        "SharedToken"
    );
    assert_eq!(
        composed
            .intern(Name::new("SharedToken"))
            .expect("existing foreign name resolves without allocation"),
        foreign,
        "composition preserves the foreign namespace and local allocation",
    );
    assert_eq!(
        composed
            .resolve(home)
            .expect("schema home name resolves")
            .as_str(),
        "SchemaOnly",
        "composition retains the consumer's completed Schema home slice",
    );
}
