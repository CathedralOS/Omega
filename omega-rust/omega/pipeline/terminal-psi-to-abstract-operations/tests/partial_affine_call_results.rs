//! Authored result projections retain exact cleanup through Omega admission.

use abstract_operations::AbstractOperation;
use optimization_unit_semantics::validate_psi_optimization_unit;
use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_psi::{StructuralPathSegment, TerminalAffineCleanupAction};
use terminal_psi_to_abstract_operations::{
    build_verified_psi_optimization_unit, lower_artifact_sections_for_optimization,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

#[test]
fn authored_call_result_cleanup_reaches_current_omega_ownership() {
    check_authored_call_result_cleanup(false, true, false);
}

#[test]
fn authored_boundary_result_cleanup_reaches_current_omega_ownership() {
    check_authored_call_result_cleanup(true, true, false);
}

#[test]
fn free_call_result_cleanup_reaches_current_omega_ownership() {
    check_authored_call_result_cleanup(false, false, false);
}

#[test]
fn free_boundary_result_cleanup_retains_service_reach_and_ownership() {
    check_authored_call_result_cleanup(true, false, false);
}

#[test]
fn anonymous_call_result_cleanup_retains_expression_owned_residuals() {
    for boundary in [false, true] {
        for attached in [false, true] {
            check_authored_call_result_cleanup(boundary, attached, true);
        }
    }
}

fn check_authored_call_result_cleanup(boundary: bool, attached: bool, anonymous: bool) {
    for (fields, calls, expected) in [
        (
            "left: Token; right: Token;",
            "Sink::take(result.right);",
            vec![vec![StructuralPathSegment::Field("left".into())]],
        ),
        (
            "left: Token; right: Token;",
            "Sink::take(result.right); Sink::take(result.left);",
            Vec::new(),
        ),
        (
            "row: [Token; 3]; tail: Token;",
            "Sink::take(result.row[1]);",
            vec![
                vec![StructuralPathSegment::Field("tail".into())],
                vec![
                    StructuralPathSegment::Field("row".into()),
                    StructuralPathSegment::FixedIndex(2),
                ],
                vec![
                    StructuralPathSegment::Field("row".into()),
                    StructuralPathSegment::FixedIndex(0),
                ],
            ],
        ),
    ] {
        if anonymous && calls.matches(';').count() != 1 {
            continue;
        }
        let mut source = format!(
            "data Token {{ value: u64; }}
             data Pair {{ {fields} }}
             data Sink {{}}
             machine Sink::take(token: Token) {{}}
             data Root {{}}
             machine Root::forward(value: Pair) -> Pair {{ value }}
             machine Root::enter(value: Pair) {{
                 let result: Pair = Root::forward(value);
                 {calls}
             }}"
        );
        if boundary {
            source = source
                .replace("data Token", "pub data Token")
                .replace("data Pair", "pub data Pair")
                .replace(
                    "machine Root::enter(value: Pair)",
                    "machine Root::enter() reaches Factory",
                )
                .replace("Root::forward(value);", "Factory::create();");
            source.push_str("boundary trait Factory { machine create() -> Pair reaches Factory; }");
        }
        if anonymous {
            let producer = if boundary {
                "Factory::create()"
            } else {
                "Root::forward(value)"
            };
            source = source
                .replace(&format!("let result: Pair = {producer};"), "")
                .replace("result.", &format!("{producer}."));
        }
        if !attached {
            source = source.replace("Root::", "").replace("Sink::", "");
        }
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let checked = lower_typed_trees(typed).expect("check");
        let entry_name = if attached { "Root::enter" } else { "enter" };
        let partial = &checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .machines;
        assert_eq!(partial.len(), 1);
        assert_eq!(
            partial[0].machine.attachment_type_identity.is_some(),
            attached
        );
        let mut malformed = checked.clone();
        malformed
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .machines[0]
            .machine
            .attachment_type_identity = if attached {
            None
        } else {
            Some("invented".into())
        };
        assert!(matches!(
            checked_trees_to_lowered_psi::lower_machine(&malformed, entry_name),
            Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(
                "partial affine Unit cleanup attachment disagrees with its signature"
            ))
        ));
        let terminal = checked_trees_to_lowered_psi::lower_machine(&checked, entry_name)
            .expect("lower authored cleanup");
        let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
        let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
        let input = lower_artifact_sections_for_optimization(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
        )
        .expect("verified Terminal enters Omega");
        let verified = build_verified_psi_optimization_unit(
            input,
            terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        )
        .expect("retain verified ownership evidence");
        validate_psi_optimization_unit(verified.unit())
            .unwrap_or_else(|error| panic!("{source}\n{error:?}"));
        let caller = verified
            .unit()
            .functions
            .iter()
            .find(|function| function.machine == terminal.semantic_module.entry)
            .expect("entry function");
        let nodes = &caller.blocks[0].nodes;
        let result = match &nodes[0].operation {
            AbstractOperation::CallStructural { result, .. } if !boundary => {
                assert_ne!(result.place, caller.structural_parameters[0].place);
                result
            }
            AbstractOperation::BoundaryCall { result, .. } if boundary => {
                assert!(caller.structural_parameters.is_empty());
                assert!(!caller.published_service_ceiling.is_empty());
                result.structural().expect("structural boundary result")
            }
            _ => panic!("leading producer retained"),
        };
        let AbstractOperation::ReturnUnit {
            cleanup_actions, ..
        } = &nodes.last().unwrap().operation
        else {
            panic!("Unit cleanup retained")
        };
        let actual = cleanup_actions
            .iter()
            .map(|action| {
                let TerminalAffineCleanupAction::DiscardResidual(discard) = action else {
                    panic!("no whole-root replacement for a partial result")
                };
                assert_eq!(discard.place, result.place);
                discard.path.clone()
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}
