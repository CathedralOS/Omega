use super::super::super::*;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn dynamic_unit() -> optimization_unit::PsiOptimizationUnit {
    let source = r#"
        trait Measure {
            machine measure(&self) -> i32;
            machine alternate(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 {
                transition { _ -> self.value }
            }

            machine alternate(&self) -> i32 {
                transition { _ -> self.value }
            }
        }

        data Main {
            decoy: Item;
            selected: Item;
        }

        machine Main::run(&mut self) {
            let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Primary;
            let result: i32 = erased.measure();
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = checked_trees_to_terminal_psi::lower_machine(&checked, "Main::run")
        .expect("lower rebound dynamic source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let plan = terminal_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("lower verified Terminal artifact");
    optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("reconstruct dynamic optimization unit")
}

fn dynamic_dispatch(
    unit: &mut optimization_unit::PsiOptimizationUnit,
) -> &mut abstract_operations::AbstractReboundDynamicDispatch {
    unit.functions
        .iter_mut()
        .find(|function| function.machine == unit.entry)
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
        .find_map(|node| match &mut node.operation {
            AbstractOperation::CallDynamicScalar {
                dynamic_dispatch, ..
            } => Some(dynamic_dispatch),
            _ => None,
        })
        .expect("dynamic operation")
}

#[test]
fn rebound_dynamic_call_reconstructs_and_validates_exact_optimizer_custody() {
    let baseline = dynamic_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("verified rebound dynamic operation must survive optimizer reconstruction");

    let mut missing_unselected_row = baseline.clone();
    let dynamic = dynamic_dispatch(&mut missing_unselected_row);
    assert_eq!(dynamic.application.rows.len(), 2);
    let selected_requirement = dynamic.dispatch.public_requirement_identity.clone();
    dynamic
        .application
        .rows
        .retain(|row| row.public_requirement_identity == selected_requirement);
    missing_unselected_row.identity =
        recompute_psi_optimization_unit_identity(&missing_unselected_row);
    assert!(
        validate_psi_optimization_unit(&missing_unselected_row).is_err(),
        "removing an unselected table row must invalidate complete application custody"
    );

    let mut reordered_rows = baseline.clone();
    dynamic_dispatch(&mut reordered_rows)
        .application
        .rows
        .swap(0, 1);
    reordered_rows.identity = recompute_psi_optimization_unit_identity(&reordered_rows);
    assert!(
        validate_psi_optimization_unit(&reordered_rows).is_err(),
        "reordering the canonical table map must invalidate its Terminal commitment"
    );

    let mut owner_drift = baseline.clone();
    let dynamic = dynamic_dispatch(&mut owner_drift);
    dynamic.dispatch.owner = MachineId::new(dynamic.dispatch.owner.get() + 100).unwrap();
    owner_drift.identity = recompute_psi_optimization_unit_identity(&owner_drift);
    assert!(
        validate_psi_optimization_unit(&owner_drift).is_err(),
        "re-authenticating a substituted dynamic owner must not admit it"
    );
}
