use super::later_results::{SCALAR_HELPERS, encoded_locals};
use super::{
    AdmissionProfile, CheckedUnitEffectOperationPlan, StatementNode, decode_module,
    decode_proof_bundle, main_machine,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;

fn multiple_structural_source() -> String {
    format!(
        r#"
        {SCALAR_HELPERS}
        pub data Token {{ flag: bool; }}
        boundary trait Factory {{
            machine create(first: u16, second: u16, third: u16) -> Token reaches Factory;
        }}
        data Main {{}}
        machine Main::main() reaches Producer + Host + Factory {{
            let prefix: u16 = 5u16;
            let chosen: u16 = Producer::choose(identity16(prefix), 7u16, 11u16);
            Host::finish(chosen);
            let first: Token = Factory::create(identity16(chosen), identity16(prefix), 19u16);
            Host::finish(prefix);
            let between: u16 = chosen;
            let second: Token = Factory::create(identity16(between), identity16(chosen), 23u16);
            Host::finish(between);
        }}
    "#
    )
}

#[test]
fn later_structural_boundary_rejoins_each_authored_local_and_result_ordinal() {
    let original = crate::front_end::checked_program(&multiple_structural_source());
    let machine = main_machine(&original);
    let [state] = original.machine_states(machine) else {
        unreachable!()
    };
    let statements = original.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(first) = &statements[3] else {
        unreachable!()
    };
    let StatementNode::LocalData(second) = &statements[6] else {
        unreachable!()
    };
    let StatementNode::LocalData(prefix) = &statements[0] else {
        unreachable!()
    };
    for mutation in 0..8 {
        let mut changed = original.clone();
        if mutation < 4 {
            let plan = changed
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter_mut()
                .find(|plan| plan.machine == machine.symbol)
                .unwrap();
            let operation = plan.operations.iter_mut().find(|operation| matches!(operation,
                CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. } if result.binding_ordinal == 1)).unwrap();
            let CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                result,
                discard_result_on_return,
                source_site,
                ..
            } = operation
            else {
                unreachable!()
            };
            match mutation {
                0 => result.binding_ordinal = 0,
                1 => result.statement_index = 3,
                2 => *discard_result_on_return = false,
                3 => {
                    *source_site = Some(checked_trees::NominalMachineUseSite::Expression(
                        first.initial_value,
                    ))
                }
                _ => unreachable!(),
            }
        } else {
            let StatementNode::LocalData(local) = &mut changed
                .typed
                .statement_table
                .statements_mut(state.statement_nodes)[6]
            else {
                unreachable!()
            };
            assert_eq!(local.symbol, second.symbol);
            match mutation {
                4 => local.is_mutable = true,
                5 => local.symbol = symbols::SymbolHandle::invalid(),
                6 => local.type_reference = prefix.type_reference,
                7 => local.initial_value = first.initial_value,
                _ => unreachable!(),
            }
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Main::main")
            )
            .is_err(),
            "later structural result mutation {mutation}"
        );
    }
    let artifact = encoded_locals(
        &original,
        &["prefix", "chosen", "first", "between", "second"],
    );
    let mut module = decode_module(&artifact.0).unwrap();
    let entry = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let cleanup = entry
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            terminal_psi::Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } => Some(trivial_affine_discards),
            _ => None,
        })
        .unwrap();
    cleanup.swap(0, 1);
    assert!(
        terminal_verifier::verify_module(
            &module,
            &decode_proof_bundle(&artifact.1).unwrap(),
            &AdmissionProfile::default()
        )
        .is_err(),
        "result cleanup must remain in reverse production order"
    );
}
