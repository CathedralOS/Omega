use super::{AddressReceiver, address_receiver, descriptor_capture};
use crate::checks::borrows::access_sources::stored_address;
use crate::flow::pointer_withholding_fixture::produce_builtin_range;

#[test]
fn builtin_range_opaque_slot_cannot_reuse_container_address() {
    let fixture = produce_builtin_range();
    let program = &fixture.program;
    let facts = &fixture.facts;
    let state = facts.flow.control.states.get(fixture.state);

    // A whole fresh range follows its performed formation to the owned array.
    // The empty call projection kept this original occurrence and its epoch.
    assert!(matches!(
        address_receiver(program, facts, fixture.whole),
        AddressReceiver::InlineValue,
    ));
    assert_eq!(descriptor_capture(program, facts, fixture.whole), None);
    let addresses = stored_address(program, facts, state, fixture.whole, true)
        .unwrap_or_else(|reason| panic!("formed range lost its owned backing: {reason}"));
    assert_eq!(addresses.len(), 1);
    assert_eq!(addresses[0].place.root, fixture.original_address.root);
    // This owned receiver supplies its enclosing storage, without a separate
    // projectable pointer-coordinate proof. The fixture independently checks
    // the range's exact 0..2 value geometry through project_call_place.
    assert!(addresses[0].place.segments.is_empty());
    assert_eq!(
        addresses[0].selected,
        checked_trees::CheckedBorrowSourceOccurrence::DirectStorage,
    );

    // The unchanged incoming element has no captured pointer. Its descriptor
    // boundary must stop resolution before the range's container address.
    match address_receiver(program, facts, fixture.projected) {
        AddressReceiver::Reference {
            capture,
            projection,
        } => {
            assert_eq!(capture, fixture.projected);
            assert!(projection.is_empty());
        }
        AddressReceiver::InlineValue | AddressReceiver::Unavailable => {
            panic!("an opaque stored pointer reused or lost its descriptor boundary");
        }
    }
    assert_eq!(
        descriptor_capture(program, facts, fixture.projected),
        Some(fixture.projected),
    );
    assert_eq!(
        stored_address(program, facts, state, fixture.projected, true).err(),
        Some("the completed storage address has no exact places"),
    );
}
