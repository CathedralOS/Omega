use super::lower_machine;
use crate::TerminalMachineSelection;
use crate::expression_preparation::qualifications::PreparedScalarQualifications;
use crate::scalar_graph::scalar_graph_lowering::prepare_scalar_graph_machine;
use crate::scalar_graph::scalar_graph_module::build_scalar_graph_module;
use crate::terminal_identities::machine_id;
use checked_trees::types::PrimitiveType;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{OperationKind, Terminator};
use terminal_verifier::reconstruct_operation_obligations;
#[test]
fn unconditional_and_expression_getters_retain_the_same_borrowed_field() {
    for completion in ["self.value", "transition { _ -> (self.value) }"] {
        let checked = crate::front_end::checked_program(&format!(
            "data Record [copy] {{ value: i32; }}
             machine Record::read(&self) -> i32 {{ {completion} }}"
        ));
        let artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            terminal_production::TerminalMachineSelection::Name("Record::read"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("ordinary scalar completion publishes checked Terminal")
        .into_artifact();
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        assert!(
            module
                .machines
                .iter()
                .any(|machine| machine
                    .blocks
                    .iter()
                    .any(|block| block.operations.iter().any(|operation| matches!(
                        operation.kind,
                        OperationKind::IntegerStructuralField { .. }
                    ))))
        );
        let plan = &checked.facts.flow.terminal_unit_effects.machines[0];
        if completion.starts_with("transition") {
            assert!(matches!(
                plan.scalar_control
                    .as_ref()
                    .map(|control| &control.terminator),
                Some(checked_trees::CheckedScalarStateTerminator::Return { .. })
            ));
        } else {
            assert!(plan.scalar_result.is_some());
            assert!(plan.scalar_control.is_none());
        }
    }
}

#[test]
fn unconditional_scalar_return_rejects_forged_coordinates_type_and_missing_prefix() {
    let checked = crate::front_end::checked_program(
        "data Record [copy] { value: i32; }
         machine Record::read(&self) -> i32 {
             let observed: i32 = self.value;
             transition { _ -> (observed) }
         }",
    );
    lower_machine(&checked, TerminalMachineSelection::Name("Record::read"))
        .expect("retained prefix and final return");
    for corruption in 0..3 {
        let mut changed = checked.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.scalar_control.is_some())
            .unwrap();
        match corruption {
            0 => {
                plan.scalar_control.as_mut().unwrap().terminator =
                    checked_trees::CheckedScalarStateTerminator::Return {
                        statement_ordinal: 0,
                    }
            }
            1 => plan.scalar_control.as_mut().unwrap().primitive_type = PrimitiveType::Bool,
            2 => {
                plan.operations.remove(0);
            }
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Record::read")).is_err(),
            "unconditional return corruption {corruption} must reject"
        );
    }
}

#[test]
fn unconditional_scalar_return_cannot_replace_or_omit_an_authored_guard() {
    let checked = crate::front_end::checked_program(
        "data Record [copy] { value: i32; other: i32; }
         machine Record::read(&self, choose: bool) -> i32 {
             transition choose { true -> (self.value) false -> (self.other) }
         }",
    );
    lower_machine(&checked, TerminalMachineSelection::Name("Record::read"))
        .expect("both authored guarded returns");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.scalar_control.is_some())
        .unwrap();
    let (_, state) =
        crate::expression_preparation::source_custody::authored_state(&checked, plan.state)
            .unwrap();
    let statement_count = checked
        .statement_table
        .statements(state.statement_nodes)
        .len();
    // An arm may have a perfectly valid Return-role value. Neither selecting
    // that guard directly nor jumping to its final fallback preserves control.
    for statement_ordinal in [0, u32::try_from(statement_count - 1).unwrap()] {
        let mut changed = checked.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.scalar_control.is_some())
            .unwrap();
        plan.scalar_control.as_mut().unwrap().terminator =
            checked_trees::CheckedScalarStateTerminator::Return { statement_ordinal };
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Record::read")).is_err(),
            "forged return at {statement_ordinal} cannot discard the authored guard"
        );
    }
}

#[test]
fn guarded_division_obligation_retains_its_selected_arm_facts() {
    let checked = crate::front_end::checked_program(
        "machine value(denominator: u8) -> u8\nrequires 7u8 == 7u8\nensures 7u8 == 7u8\n{ transition (1 <= denominator) { true -> (7u8 / denominator) false -> 7 } }",
    );
    let graph = &checked.facts.flow.terminal_scalar_graphs.machines[0];
    let qualifications =
        PreparedScalarQualifications::prepare(&checked, &[graph.machine]).expect("qualifications");
    let prepared = prepare_scalar_graph_machine(&checked, &qualifications, graph.machine, graph)
        .expect("prepare");
    let lowered = build_scalar_graph_module(
        &prepared.states,
        &prepared.state_symbols,
        prepared.result_type,
        qualifications.catalog(),
        prepared.contract,
        prepared.crash_routes,
        prepared.identity_reshuffles,
        prepared.partition_compositions,
        machine_id(1),
        0,
        &[(graph.machine, machine_id(1))],
        &[(graph.machine, 1)],
        prepared.loop_plan.as_ref(),
    )
    .expect("module");
    let validated = terminal_verifier::validate_module(&lowered.semantic_module).expect("validate");
    let machine = &lowered.semantic_module.machines[0];
    let entry = machine
        .blocks
        .iter()
        .find(|block| block.id == machine.entry)
        .expect("entry");
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &entry.terminator
    else {
        panic!("selected branch")
    };
    let divisions: Vec<_> = machine
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(move |operation| match operation.kind {
                    OperationKind::ExactIntegerDivide { obligation, .. } => {
                        Some((block.id, obligation))
                    }
                    _ => None,
                })
        })
        .collect();
    assert_eq!(divisions.len(), 1);
    assert_eq!(divisions[0].0, when_true.target);
    assert_ne!(divisions[0].0, entry.id);
    assert_ne!(divisions[0].0, when_false.target);
    let context = validated.value_context(machine).expect("value context");
    let parameters = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect();
    let mut certificates = 0;
    for site in reconstruct_operation_obligations(&lowered.semantic_module).expect("obligations") {
        if site.canonical_certificate {
            certificates += 1;
            assert_eq!(site.obligation.id, divisions[0].1);
            assert!(site.semantic_axioms.contains(&site.obligation.proposition));
            let proof =
                crate::proofs::nonzero_divisor_certificate::produce_checked_canonical_integer_proof(
                    &context,
                    &site.obligation.proposition,
                    &machine.contract.requires,
                    &site.semantic_axioms,
                    &parameters,
                );
            assert!(proof.is_some(), "{site:#?}");
        }
    }
    assert_eq!(certificates, 1);
}
