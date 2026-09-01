use omega_psi_to_abstract_operations::lower_artifact_sections;
use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

use super::prelude::*;
use crate::{LoweringError, lower_to_target_operations};

fn abstract_plan() -> omega_abstract_operations::AbstractOperationPlan {
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
    lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified Terminal artifact")
}

#[test]
fn lowers_rebound_dynamic_versions_to_one_target_indirect_call() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = abstract_plan();
        let lowered = lower_to_target_operations(&source, target)
            .expect("target lowering retains rebound dynamic dispatch");
        let caller = lowered
            .functions
            .iter()
            .find(|function| function.machine == lowered.entry)
            .expect("entry caller");
        let TargetOperation::UnitBody(body) = &caller.operation else {
            panic!("dynamic caller must remain an attached Unit body")
        };
        let calls = body
            .operations
            .iter()
            .filter_map(|operation| match operation {
                TargetUnitOperation::DynamicScalarCall {
                    dynamic_dispatch,
                    initial_argument,
                    rebound_argument,
                    ..
                } => Some((dynamic_dispatch, initial_argument, rebound_argument)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(dynamic, initial, rebound)] = calls.as_slice() else {
            panic!("one target dynamic call expected: {body:#?}")
        };
        assert_eq!(initial.path, dynamic.initial.source.path);
        assert_eq!(rebound.path, dynamic.rebound.source.path);
        assert_ne!(initial.source_byte_offset, rebound.source_byte_offset);
        assert_eq!(initial.destination, rebound.destination);
        assert_eq!(dynamic.dispatch.realization, source.functions[1].machine);
    }
}

#[test]
fn rejects_reauthenticated_dynamic_descriptor_substitution() {
    let mut source = abstract_plan();
    let caller = source
        .functions
        .iter_mut()
        .find(|function| function.machine == source.entry)
        .expect("entry caller");
    let rejected_operation = caller
        .operations
        .iter_mut()
        .find_map(|operation| match operation {
            AbstractOperation::CallDynamicScalar {
                psi_operation,
                dynamic_dispatch,
                ..
            } => {
                dynamic_dispatch.dispatch.descriptor_ordinal += 1;
                Some(*psi_operation)
            }
            _ => None,
        })
        .expect("dynamic operation");
    assert_eq!(
        lower_to_target_operations(&source, NativeTarget::linux_x64()),
        Err(LoweringError::InvalidDynamicScalarDispatch {
            machine: source.entry,
            operation: rejected_operation,
        })
    );
}
