use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_psi_to_abstract_operations::lower_artifact_sections;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

use super::prelude::*;
use crate::{LoweringError, lower_to_target_operations};

fn abstract_plan() -> abstract_operations::AbstractOperationPlan {
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
    let terminal = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::run")
        .expect("lower rebound dynamic source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified Terminal artifact")
}

fn multi_hop_forwarded_unit_plan() -> abstract_operations::AbstractOperationPlan {
    let source = r#"
        trait Touch { machine touch(&self); }
        data Item { value: i32; }
        Primary: Item satisfies Touch { machine touch(&self) {} }
        data Main { selected: Item; }
        machine Main::run(&self) {
            let erased: &dyn Touch = &self.selected as &dyn Item::Primary;
            forward(erased);
        }
        machine forward(erased: &dyn Touch) { finish(erased); }
        machine finish(erased: &dyn Touch) { erased.touch(); }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::run")
        .expect("lower multi-hop dynamic Unit source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified Terminal artifact")
}

fn dynamic_unit_plan() -> abstract_operations::AbstractOperationPlan {
    let source = r#"
        trait Touch {
            machine touch(&self);
        }

        data Item { value: i32; }

        Primary: Item satisfies Touch {
            machine touch(&self) {}
        }

        data Main {
            decoy: Item;
            selected: Item;
        }

        machine Main::run(&mut self) {
            let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Primary;
            erased.touch();
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::run")
        .expect("lower rebound dynamic Unit source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified Terminal artifact")
}

#[test]
fn lowers_rebound_dynamic_unit_without_a_scalar_result_carrier() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = dynamic_unit_plan();
        let lowered = lower_to_target_operations(&source, target)
            .expect("target lowering retains rebound dynamic Unit dispatch");
        let caller = lowered
            .functions
            .iter()
            .find(|function| function.machine == lowered.entry)
            .expect("entry caller");
        let TargetOperation::UnitBody(body) = &caller.operation else {
            panic!("dynamic Unit caller must remain an attached Unit body")
        };
        let calls = body
            .operations
            .iter()
            .filter_map(|operation| match operation {
                TargetUnitOperation::DynamicUnitCall {
                    dynamic_dispatch,
                    call_plan,
                    initial_argument,
                    rebound_argument,
                    ..
                } => Some((
                    dynamic_dispatch,
                    call_plan,
                    initial_argument,
                    rebound_argument,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(dynamic, call_plan, initial, rebound)] = calls.as_slice() else {
            panic!("one target dynamic Unit call expected: {body:#?}")
        };
        assert!(call_plan.result.is_none());
        assert_eq!(call_plan.parameters.len(), 1);
        assert_eq!(initial.path, dynamic.initial.source.path);
        assert_eq!(rebound.path, dynamic.rebound.source.path);
        assert_ne!(initial.source_byte_offset, rebound.source_byte_offset);
        assert_eq!(initial.destination, rebound.destination);
        assert!(
            dynamic
                .application
                .realization_callables
                .iter()
                .any(|callable| {
                    callable.machine == dynamic.dispatch.realization
                        && callable.result == terminal_psi::ClosedConformanceCallableResult::Unit
                })
        );
    }
}

#[test]
fn lowers_parameter_sourced_unit_forwarding_without_a_result_carrier() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = multi_hop_forwarded_unit_plan();
        let lowered = lower_to_target_operations(&source, target)
            .expect("target lowering retains the complete Unit forwarding chain");
        let forwarded = lowered
            .functions
            .iter()
            .filter_map(|function| match &function.operation {
                TargetOperation::ForwardDynamicParameterUnitCall {
                    callee,
                    argument,
                    parameter_abi,
                    function_call_plan,
                    callee_call_plan,
                    ..
                } => Some((
                    function,
                    callee,
                    argument,
                    parameter_abi,
                    function_call_plan,
                    callee_call_plan,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(function, callee, argument, parameter_abi, function_plan, callee_plan)] =
            forwarded.as_slice()
        else {
            panic!("one parameter-sourced Unit helper call expected: {lowered:#?}")
        };
        assert_eq!(parameter_abi.parameter.owner, function.machine);
        assert_eq!(argument.target.owner, **callee);
        assert!(matches!(
            &argument.source,
            abstract_operations::AbstractDynamicDescriptorSource::Parameter(source)
                if source == &parameter_abi.parameter
        ));
        assert!(function_plan.result.is_none());
        assert_eq!(function_plan.parameters.len(), 2);
        assert_eq!(function_plan, callee_plan);
        assert_eq!(parameter_abi.instance, function_plan.parameters[0]);
        assert_eq!(parameter_abi.table, function_plan.parameters[1]);
        assert!(lowered.functions.iter().any(|candidate| {
            candidate.machine == **callee
                && matches!(
                    candidate.operation,
                    TargetOperation::DynamicParameterUnitCall { .. }
                )
        }));
    }
}

#[test]
fn rejects_parameter_sourced_unit_forwarding_interface_drift() {
    let mut source = multi_hop_forwarded_unit_plan();
    let (machine, operation) = source
        .functions
        .iter_mut()
        .find_map(|function| {
            function.operations.iter_mut().find_map(|candidate| {
                let AbstractOperation::CallUnitWithDynamicArguments {
                    psi_operation,
                    dynamic_arguments,
                    ..
                } = candidate
                else {
                    return None;
                };
                let [argument] = dynamic_arguments.as_mut_slice() else {
                    return None;
                };
                let abstract_operations::AbstractDynamicDescriptorSource::Parameter(source) =
                    &mut argument.source
                else {
                    return None;
                };
                source.ordinal += 1;
                Some((function.machine, *psi_operation))
            })
        })
        .expect("parameter-sourced Unit forwarding call");
    assert_eq!(
        lower_to_target_operations(&source, NativeTarget::linux_x64()),
        Err(LoweringError::InvalidDynamicDispatch { machine, operation })
    );
}

#[test]
fn scalar_graph_rejects_dynamic_methods_requiring_unimplemented_field_observations() {
    let source = abstract_plan();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        assert!(matches!(
            lower_to_target_operations(&source, target),
            Err(LoweringError::UnsupportedControlFlow(_))
        ));
    }
}
