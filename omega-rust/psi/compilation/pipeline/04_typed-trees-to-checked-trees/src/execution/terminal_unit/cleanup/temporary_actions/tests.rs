use super::value_call_context;
use checked_trees::{
    CheckFacts, CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedComposedUnitControlTerminatorPlan, CheckedControlResultPlan, CheckedStructuralAccess,
    CheckedUnitStructuralParameterPlan,
};
use symbols::SymbolHandle;

fn body(state: SymbolHandle) -> CheckedComposedUnitControlStatePlan {
    CheckedComposedUnitControlStatePlan {
        state,
        structural_parameters: vec![CheckedUnitStructuralParameterPlan {
            position: 0,
            type_identity: Default::default(),
            multiplicity: language_semantics::Multiplicity::Affine,
            access: CheckedStructuralAccess::Owned,
            primitive_referent: None,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            binding_carrier: None,
        }],
        scalar_parameters: Vec::new(),
        parameter_storage: Vec::new(),
        erased_scalar_parameters: Vec::new(),
        erased_proof_parameters: Vec::new(),
        requires: Vec::new(),
        entry_claims: Vec::new(),
        operations: Vec::new(),
        place_loans: Vec::new(),
        terminator: CheckedComposedUnitControlTerminatorPlan::ReturnUnit,
        terminator_statement_ordinal: 0,
        statement_count: 0,
        selected_exit: None,
        normal_exit_cleanups: Default::default(),
    }
}

fn graph(
    owner: SymbolHandle,
    body: CheckedComposedUnitControlStatePlan,
) -> CheckedComposedUnitControlMachinePlan {
    CheckedComposedUnitControlMachinePlan {
        machine: owner,
        result: CheckedControlResultPlan::Unit,
        result_reference_sources: Vec::new(),
        natural_ranks: Vec::new(),
        preserving_rank_edges: Vec::new(),
        attachment_type_identity: None,
        provider_attachment_requirements: Vec::new(),
        body_qualifications: Vec::new(),
        contract_report_fingerprint: 0,
        contract_commitment: checked_trees::MachineContractCommitment::from_digest([0; 32]),
        contract_service_reach: Default::default(),
        service_reach: Default::default(),
        states: vec![body],
    }
}

#[test]
fn fused_body_uses_its_own_parameter_roster_not_the_graph_entry_owner() {
    let owner = SymbolHandle::from_arena_index(1);
    let state = SymbolHandle::from_arena_index(2);
    let mut facts = CheckFacts::default();
    facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .push(graph(owner, body(state)));
    let context = value_call_context(
        &facts,
        &facts.flow.terminal_unit_effects.cleanup_control,
        state,
    )
    .expect("the source body survives inside another machine's graph");
    assert_eq!(context.parameters.len(), 1);
    assert_eq!(context.parameters[0].position, 0);
    assert!(context.release_owned.is_empty());
}

#[test]
fn another_body_cannot_supply_a_missing_value_call_namespace() {
    let mut facts = CheckFacts::default();
    facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .push(graph(
            SymbolHandle::from_arena_index(1),
            body(SymbolHandle::from_arena_index(2)),
        ));
    assert!(matches!(
        value_call_context(
            &facts,
            &facts.flow.terminal_unit_effects.cleanup_control,
            SymbolHandle::from_arena_index(3)
        ),
        Err("missing executable parameter catalog")
    ));
}

#[test]
fn repeated_fused_bodies_must_agree_on_the_shared_value_namespace() {
    let state = SymbolHandle::from_arena_index(3);
    let mut facts = CheckFacts::default();
    let plans = &mut facts.flow.terminal_unit_effects.composed_machines;
    plans.push(graph(SymbolHandle::from_arena_index(1), body(state)));
    plans.push(graph(SymbolHandle::from_arena_index(2), body(state)));
    assert!(
        value_call_context(
            &facts,
            &facts.flow.terminal_unit_effects.cleanup_control,
            state
        )
        .is_ok()
    );
    facts.flow.terminal_unit_effects.composed_machines[1].states[0].structural_parameters[0]
        .position = 1;
    assert!(matches!(
        value_call_context(
            &facts,
            &facts.flow.terminal_unit_effects.cleanup_control,
            state
        ),
        Err("surviving bodies disagree on the value-call operand namespace")
    ));
}
