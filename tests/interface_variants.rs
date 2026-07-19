//! The one interface-variant algebra is ordered alternatives with an optional
//! single typed payload. This is encoded data only; no unsettled source spelling
//! is authored here.

use core_schema::{CoreEnum, CoreReference, CoreVariant};
use name_table::{IdentifierNamespace, Name, NameTable};

#[test]
fn ordered_unit_and_payload_interface_variants_share_one_algebra() {
    let mut names = NameTable::new(IdentifierNamespace::Schema);
    let opened = names.intern(Name::new("Opened"));
    let delivered = names.intern(Name::new("Delivered"));
    let token = names.intern(Name::new("SubscriptionToken"));

    let interface = CoreEnum::new(
        names.intern(Name::new("Output")),
        vec![
            CoreVariant::new(opened, None),
            CoreVariant::new(delivered, Some(CoreReference::Plain(token))),
        ],
    );

    assert_eq!(interface.variants()[0].identifier(), opened);
    assert!(interface.variants()[0].payload().is_none());
    assert_eq!(interface.variants()[1].identifier(), delivered);
    assert!(matches!(
        interface.variants()[1].payload(),
        Some(CoreReference::Plain(identifier)) if *identifier == token
    ));
}
