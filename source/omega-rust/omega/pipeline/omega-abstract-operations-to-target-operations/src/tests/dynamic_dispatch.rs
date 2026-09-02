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
    let terminal = psi_checked_trees_to_terminal::lower_machine(&checked, "Main::run")
        .expect("lower rebound dynamic source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified Terminal artifact")
}

fn forwarded_parameter_plan() -> omega_abstract_operations::AbstractOperationPlan {
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
            let result: i32 = forward(erased);
        }

        machine forward(erased: &dyn Measure) -> i32 {
            let result: i32 = erased.measure();
            transition { _ -> result }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = psi_checked_trees_to_terminal::lower_machine(&checked, "Main::run")
        .expect("lower forwarded dynamic source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified Terminal artifact")
}

fn dynamic_unit_plan() -> omega_abstract_operations::AbstractOperationPlan {
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
    let terminal = psi_checked_trees_to_terminal::lower_machine(&checked, "Main::run")
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
                        && callable.result == psi_terminal::ClosedConformanceCallableResult::Unit
                })
        );
    }
}

#[test]
fn lowers_forwarded_descriptor_to_two_word_entry_and_erased_slot_call() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = forwarded_parameter_plan();
        let helper = source
            .functions
            .iter()
            .find(|function| {
                function.operations.iter().any(|operation| {
                    matches!(
                        operation,
                        AbstractOperation::CallDynamicParameterScalar { .. }
                    )
                })
            })
            .expect("forward helper");
        let functions = source
            .functions
            .iter()
            .map(|function| (function.machine, function))
            .collect::<std::collections::BTreeMap<_, _>>();
        let structural_types = source
            .structural_types
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect::<std::collections::BTreeMap<_, _>>();
        let function = crate::lowering::lower_scalar_function_for_tests(
            helper,
            helper.result.scalar().expect("scalar helper result"),
            target,
            &functions,
            &structural_types,
            &std::collections::BTreeMap::new(),
        )
        .expect("target lowering selects the forwarded descriptor ABI");
        let TargetOperation::ReturnDynamicParameterScalarCall {
            parameter_abi,
            function_call_plan,
            dispatch_call_plan,
            table_slot_byte_offset,
            ..
        } = &function.operation
        else {
            panic!("forward helper must keep its role-specific target carrier")
        };
        assert_eq!(parameter_abi.parameter.owner, function.machine);
        assert_eq!(function_call_plan.parameters.len(), 2);
        assert_eq!(dispatch_call_plan.parameters.len(), 1);
        assert_eq!(*table_slot_byte_offset, 0);
        assert_eq!(parameter_abi.instance, function_call_plan.parameters[0]);
        assert_eq!(parameter_abi.table, function_call_plan.parameters[1]);
        assert_ne!(parameter_abi.instance, parameter_abi.table);
    }
}

#[test]
fn lowers_rebound_descriptor_into_forwarded_call_abi() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = forwarded_parameter_plan();
        let lowered = lower_to_target_operations(&source, target)
            .expect("lower complete caller-to-forwarder descriptor path");
        let caller = lowered
            .functions
            .iter()
            .find(|function| function.machine == lowered.entry)
            .expect("entry caller");
        let TargetOperation::UnitBody(body) = &caller.operation else {
            panic!("forwarding caller must remain an attached Unit body")
        };
        let calls = body
            .operations
            .iter()
            .filter_map(|operation| match operation {
                TargetUnitOperation::StructuralScalarCallWithDynamicArguments {
                    callee,
                    call_plan,
                    structural_arguments,
                    dynamic_arguments,
                    ..
                } => Some((callee, call_plan, structural_arguments, dynamic_arguments)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(callee, call_plan, structural_arguments, dynamic_arguments)] = calls.as_slice()
        else {
            panic!("one forwarded descriptor call expected: {body:#?}")
        };
        assert!(structural_arguments.is_empty());
        assert_eq!(call_plan.parameters.len(), 2);
        let [argument] = dynamic_arguments.as_slice() else {
            panic!("one dynamic descriptor argument expected")
        };
        assert_eq!(argument.instance.destination, call_plan.parameters[0]);
        assert_eq!(argument.table_destination, call_plan.parameters[1]);
        assert_ne!(argument.instance.destination, argument.table_destination);
        let omega_abstract_operations::AbstractDynamicDescriptorSource::Rebound {
            initial,
            rebound,
            application,
            ..
        } = &argument.custody.source
        else {
            panic!("caller must materialize its locally rebound descriptor")
        };
        assert_eq!(argument.instance.path, rebound.source.path);
        assert_ne!(initial.source.path, rebound.source.path);
        assert_eq!(
            application.trait_identity,
            argument.custody.target.trait_identity
        );
        assert!(
            lowered
                .functions
                .iter()
                .any(|function| function.machine == **callee)
        );
    }
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
        assert_eq!(dynamic.application.rows.len(), 2);
        assert!(
            source
                .functions
                .iter()
                .any(|function| function.machine == dynamic.dispatch.realization)
        );
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
        Err(LoweringError::InvalidDynamicDispatch {
            machine: source.entry,
            operation: rejected_operation,
        })
    );
}

#[test]
fn rejects_dynamic_call_missing_an_unselected_table_row() {
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
                let selected = dynamic_dispatch
                    .dispatch
                    .public_requirement_identity
                    .clone();
                dynamic_dispatch
                    .application
                    .rows
                    .retain(|row| row.public_requirement_identity == selected);
                Some(*psi_operation)
            }
            _ => None,
        })
        .expect("dynamic operation");
    assert_eq!(
        lower_to_target_operations(&source, NativeTarget::linux_x64()),
        Err(LoweringError::InvalidDynamicDispatch {
            machine: source.entry,
            operation: rejected_operation,
        })
    );
}
