use super::*;
use terminal_psi::{
    BoundaryMachineResult, TerminalDynamicInterface, TerminalDynamicSlot, TerminalDynamicTable,
    TerminalIdentityTableBuilder,
};

#[test]
fn dynamic_call_follows_the_invoked_slot_or_complete_finite_roster_only() {
    let mut identities = TerminalIdentityTableBuilder::default();
    let interface_identity = identities.intern("Shape");
    let first_requirement = identities.intern("Shape::first");
    let unrelated = identities.intern("Shape::other");
    let mut module = TerminalModule::declarations();
    let mut slots = Vec::new();
    for slot in 0..3 {
        slots.push(TerminalDynamicSlot {
            slot,
            declaring_trait_identity: interface_identity,
            requirement_identity: if slot < 2 {
                first_requirement
            } else {
                unrelated
            },
            family_tuple: vec![slot.to_string()],
            scalar_parameters: Vec::new(),
            structural_parameters: Vec::new(),
            entry_claim_paths: Vec::new(),
            result: BoundaryMachineResult::Unit,
            result_claim_paths: Vec::new(),
            result_reference_regions: Vec::new(),
        });
    }
    module
        .dynamic_dispatch
        .interfaces
        .push(TerminalDynamicInterface {
            id: 0,
            identity: interface_identity,
            slots,
        });
    module.dynamic_dispatch.tables.push(TerminalDynamicTable {
        id: 0,
        interface: 0,
        implementation_identity: identities.intern("Selected"),
        rows: (1..=3).map(MachineId::new).collect(),
    });
    module.identities = identities.build();
    let calls = SynchronousCallGraph::new(&module, None).unwrap();
    let mut pending = Vec::new();
    calls
        .append_dynamic_calls(0, 0, false, &mut pending)
        .unwrap();
    assert_eq!(
        pending,
        vec![TerminalCallable::Machine(MachineId::new(1).unwrap())]
    );
    pending.clear();
    calls
        .append_dynamic_calls(0, 0, true, &mut pending)
        .unwrap();
    assert_eq!(
        pending,
        vec![
            TerminalCallable::Machine(MachineId::new(1).unwrap()),
            TerminalCallable::Machine(MachineId::new(2).unwrap()),
        ]
    );
    assert!(
        calls
            .append_dynamic_calls(0, 3, false, &mut pending)
            .is_err()
    );
    assert!(
        calls
            .append_dynamic_calls(1, 0, false, &mut pending)
            .is_err()
    );
}
