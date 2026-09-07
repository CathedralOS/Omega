use crate::{AssignmentError, assign_registers};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use target::NativeTarget;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

const RANKED_COUNTDOWN_SOURCE: &str = r#"
    data Token { value: i32; }
    data Root {}

    machine Root::countdown(token: Token, remaining: u32)
    terminates by remaining -> Nat::Descending;
    {
        transition remaining > 0 {
            true -> countdown(token, remaining - 1)
            _ -> done(token)
        }
        state done(token: Token) {}
    }
"#;

const RANKED_RECEIVER_COUNTDOWN_SOURCE: &str = r#"
    data Token { value: i32; }
    data Root { token: Token; }

    machine Root::countdown(&mut self, remaining: u32)
    terminates by remaining -> Nat::Descending;
    {
        transition remaining > 0 {
            true -> countdown(remaining - 1)
            _ -> done()
        }
        state done(&mut self) {}
    }
"#;

fn ranked_target_from(
    source: &str,
    target: NativeTarget,
) -> target_operations::TargetOperationPlan {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::countdown")
        .expect("lower terminal countdown");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).expect("semantic");
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).expect("proof");
    let ranked =
        terminal_psi_to_abstract_operations::lower_artifact_sections_for_native_ranked_countdown(
            &semantic,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("admit native ranked countdown");
    abstract_operations_to_target_operations::lower_ranked_to_target_operations(&ranked, target)
        .expect("lower ranked target")
}

#[test]
fn ranked_owned_and_mutable_receiver_functions_require_the_common_selected_graph() {
    for source in [RANKED_COUNTDOWN_SOURCE, RANKED_RECEIVER_COUNTDOWN_SOURCE] {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let target_plan = ranked_target_from(source, target);
            assert_eq!(
                assign_registers(&target_plan),
                Err(AssignmentError::RequiresSelectedGraph(target_plan.entry))
            );
        }
    }
}
