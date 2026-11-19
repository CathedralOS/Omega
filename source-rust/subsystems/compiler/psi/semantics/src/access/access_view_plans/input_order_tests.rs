use super::{AccessViewPlanError, validate_access_view_plans};
use semantic_vocabulary::MachineId;
use terminal_psi::{
    StructuralAccess, TerminalAccessViewInput, TerminalIdentityIndex, TerminalModule,
};

fn input(machine: u64, runtime_position: u32) -> TerminalAccessViewInput {
    let identity = TerminalIdentityIndex(0);
    TerminalAccessViewInput {
        machine: MachineId::new(machine).unwrap(),
        position: runtime_position,
        runtime_position,
        carrier_identity: identity,
        source_machine_identity: identity,
        source_state_identity: identity,
        source_parameter_identity: identity,
        access: StructuralAccess::SharedBorrow,
        binding_is_const: false,
        binding_is_mutable: false,
        view_identity: identity,
        policy_identity: identity,
        policy_plan_machine_identity: identity,
        schema_identity: identity,
        placement_report_fingerprint: 1,
        placement_commitment: [1; 32],
    }
}

fn order_refusal(inputs: Vec<TerminalAccessViewInput>) -> Option<AccessViewPlanError> {
    let mut module = TerminalModule::for_entry(MachineId::new(1).unwrap());
    module.access_view_inputs = inputs;
    validate_access_view_plans(&module)
        .err()
        .filter(|error| matches!(error, AccessViewPlanError::InputOutOfOrder { .. }))
}

// The rows name machines the module lacks, so ordered rows still fail
// later; this pins only the order rule from encoding.md.
#[test]
fn ordered_inputs_pass_the_order_rule() {
    assert_eq!(
        order_refusal(vec![input(1, 0), input(1, 2), input(2, 0)]),
        None
    );
}

#[test]
fn unordered_or_repeated_inputs_are_refused() {
    for (rows, machine, runtime_position) in [
        (vec![input(1, 2), input(1, 0)], 1, 0),
        (vec![input(2, 0), input(1, 3)], 1, 3),
        (vec![input(1, 1), input(1, 1)], 1, 1),
    ] {
        assert_eq!(
            order_refusal(rows),
            Some(AccessViewPlanError::InputOutOfOrder {
                machine: MachineId::new(machine).unwrap(),
                runtime_position,
            })
        );
    }
}
