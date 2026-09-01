use super::super::super::*;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

fn dynamic_unit() -> omega_optimization_unit::PsiOptimizationUnit {
    let source = r#"
        trait Measure {
            machine measure(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 {
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
    let terminal = psi_checked_trees_to_terminal::lower_machine(&checked, "Main::run")
        .expect("lower rebound dynamic source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let plan = omega_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("lower verified Terminal artifact");
    omega_optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("reconstruct dynamic optimization unit")
}

#[test]
fn rebound_dynamic_call_reconstructs_and_validates_exact_optimizer_custody() {
    let baseline = dynamic_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("verified rebound dynamic operation must survive optimizer reconstruction");

    let mut owner_drift = baseline.clone();
    let caller = owner_drift
        .functions
        .iter_mut()
        .find(|function| function.machine == owner_drift.entry)
        .unwrap();
    let dynamic = caller
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
        .find_map(|node| match &mut node.operation {
            AbstractOperation::CallDynamicScalar {
                dynamic_dispatch, ..
            } => Some(dynamic_dispatch),
            _ => None,
        })
        .expect("dynamic operation");
    dynamic.dispatch.owner = MachineId::new(dynamic.dispatch.owner.get() + 100).unwrap();
    owner_drift.identity = recompute_psi_optimization_unit_identity(&owner_drift);
    assert!(
        validate_psi_optimization_unit(&owner_drift).is_err(),
        "re-authenticating a substituted dynamic owner must not admit it"
    );
}
