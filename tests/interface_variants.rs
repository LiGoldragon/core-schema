//! Interface variants are ordered alternatives with an optional one typed payload.

use core_schema::{EncodedEnum, EncodedReference, EncodedVariant};
use name_table::{IdentifierNamespace, Name, NameTable};

#[test]
fn ordered_unit_and_payload_interface_variants_share_one_algebra() {
    let mut names = NameTable::new(IdentifierNamespace::Schema);
    let opened = names.intern(Name::new("Opened")).expect("allocate Opened");
    let delivered = names
        .intern(Name::new("Delivered"))
        .expect("allocate Delivered");
    let token = names
        .intern(Name::new("SubscriptionToken"))
        .expect("allocate SubscriptionToken");

    let interface = EncodedEnum::new(
        names.intern(Name::new("Output")).expect("allocate Output"),
        vec![
            EncodedVariant::new(opened, None),
            EncodedVariant::new(delivered, Some(EncodedReference::Plain(token))),
        ],
    );

    assert_eq!(interface.variants()[0].identifier(), opened);
    assert!(interface.variants()[0].payload().is_none());
    assert_eq!(interface.variants()[1].identifier(), delivered);
    assert!(matches!(
        interface.variants()[1].payload(),
        Some(EncodedReference::Plain(identifier)) if *identifier == token
    ));
}
