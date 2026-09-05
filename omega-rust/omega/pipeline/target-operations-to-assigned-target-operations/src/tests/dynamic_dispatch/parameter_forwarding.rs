use crate::assign_registers;
use abstract_operations_to_target_operations::lower_to_target_operations;
use assigned_target_operations::AssignedOperation;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::MachineId;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use target::NativeTarget;
use target_operations::TargetOperation;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_psi_to_abstract_operations::lower_artifact_sections;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn multi_hop_target_plan(target: NativeTarget) -> target_operations::TargetOperationPlan {
    let source = r#"
        trait Measure { machine measure(&self) -> i32; }
        data Item { value: i32; }
        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 { transition { _ -> self.value } }
        }
        data Main { selected: Item; }
        machine Main::run(&self) {
            let erased: &dyn Measure = &self.selected as &dyn Item::Primary;
            let result: i32 = forward(erased);
        }
        machine forward(erased: &dyn Measure) -> i32 {
            let result: i32 = finish(erased);
            transition { _ -> result }
        }
        machine finish(erased: &dyn Measure) -> i32 {
            let result: i32 = erased.measure();
            transition { _ -> result }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = checked_trees_to_terminal_psi::lower_machine(&checked, "Main::run")
        .expect("lower multi-hop dynamic source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let abstract_plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified Terminal artifact");
    lower_to_target_operations(&abstract_plan, target)
        .expect("lower parameter-sourced forwarding to target operations")
}

fn multi_hop_unit_target_plan(target: NativeTarget) -> target_operations::TargetOperationPlan {
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
    let terminal = checked_trees_to_terminal_psi::lower_machine(&checked, "Main::run")
        .expect("lower multi-hop dynamic Unit source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let abstract_plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified Terminal artifact");
    lower_to_target_operations(&abstract_plan, target)
        .expect("lower parameter-sourced Unit forwarding to target operations")
}

#[test]
fn assigns_parameter_sourced_forwarding_without_reauthenticating_the_descriptor() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_plan = multi_hop_target_plan(target);
        let assigned = assign_registers(&target_plan)
            .expect("assign the parameter-sourced descriptor forwarding call");
        let forwarded = assigned
            .functions
            .iter()
            .filter_map(|function| match &function.operation {
                AssignedOperation::ReturnForwardedDynamicParameterScalarCall {
                    callee,
                    argument,
                    parameter_abi,
                    instance_destination,
                    table_destination,
                    function_call_plan,
                    callee_call_plan,
                    ..
                } => Some((
                    function,
                    callee,
                    argument,
                    parameter_abi,
                    instance_destination,
                    table_destination,
                    function_call_plan,
                    callee_call_plan,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [
            (
                function,
                callee,
                argument,
                parameter_abi,
                instance_destination,
                table_destination,
                function_plan,
                callee_plan,
            ),
        ] = forwarded.as_slice()
        else {
            panic!("one assigned parameter-sourced forwarding call expected")
        };
        assert_eq!(parameter_abi.parameter.owner, function.machine);
        assert_eq!(argument.target.owner, **callee);
        assert!(matches!(
            &argument.source,
            target_operations::AbstractDynamicDescriptorSource::Parameter(source)
                if source == &parameter_abi.parameter
        ));
        assert_eq!(parameter_abi.instance, **instance_destination);
        assert_eq!(parameter_abi.table, **table_destination);
        assert_eq!(function_plan, callee_plan);
        assert_ne!(parameter_abi.instance, parameter_abi.table);
    }
}

#[test]
fn assigns_parameter_sourced_unit_forwarding_without_a_result_carrier() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_plan = multi_hop_unit_target_plan(target);
        let assigned = assign_registers(&target_plan)
            .expect("assign the parameter-sourced Unit descriptor forwarding call");
        let forwarded = assigned
            .functions
            .iter()
            .filter_map(|function| match &function.operation {
                AssignedOperation::ForwardDynamicParameterUnitCall {
                    callee,
                    argument,
                    parameter_abi,
                    instance_destination,
                    table_destination,
                    function_call_plan,
                    callee_call_plan,
                    ..
                } => Some((
                    function,
                    callee,
                    argument,
                    parameter_abi,
                    instance_destination,
                    table_destination,
                    function_call_plan,
                    callee_call_plan,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [
            (
                function,
                callee,
                argument,
                parameter_abi,
                instance_destination,
                table_destination,
                function_plan,
                callee_plan,
            ),
        ] = forwarded.as_slice()
        else {
            panic!("one assigned parameter-sourced Unit forwarding call expected")
        };
        assert_eq!(parameter_abi.parameter.owner, function.machine);
        assert_eq!(argument.target.owner, **callee);
        assert!(matches!(
            &argument.source,
            target_operations::AbstractDynamicDescriptorSource::Parameter(source)
                if source == &parameter_abi.parameter
        ));
        assert_eq!(parameter_abi.instance, **instance_destination);
        assert_eq!(parameter_abi.table, **table_destination);
        assert_eq!(function_plan, callee_plan);
        assert!(function_plan.result.is_none());
        assert_ne!(parameter_abi.instance, parameter_abi.table);
    }
}

#[test]
fn rejects_parameter_sourced_unit_forwarding_target_drift_during_assignment() {
    let mut plan = multi_hop_unit_target_plan(NativeTarget::linux_x64());
    let (machine, operation) = plan
        .functions
        .iter_mut()
        .find_map(|function| {
            let TargetOperation::ForwardDynamicParameterUnitCall {
                psi_operation,
                argument,
                ..
            } = &mut function.operation
            else {
                return None;
            };
            argument.target.owner =
                MachineId::new(argument.target.owner.get() + 100).expect("distinct target owner");
            Some((function.machine, *psi_operation))
        })
        .expect("parameter-sourced Unit forwarding call");
    assert_eq!(
        assign_registers(&plan),
        Err(crate::AssignmentError::DynamicDescriptorAssignmentMismatch { machine, operation })
    );
}

#[test]
fn rejects_parameter_sourced_forwarding_target_drift_during_assignment() {
    let mut plan = multi_hop_target_plan(NativeTarget::linux_x64());
    let (machine, operation) = plan
        .functions
        .iter_mut()
        .find_map(|function| {
            let TargetOperation::ReturnForwardedDynamicParameterScalarCall {
                psi_operation,
                argument,
                ..
            } = &mut function.operation
            else {
                return None;
            };
            argument.target.owner =
                MachineId::new(argument.target.owner.get() + 100).expect("distinct target owner");
            Some((function.machine, *psi_operation))
        })
        .expect("parameter-sourced forwarding call");
    assert_eq!(
        assign_registers(&plan),
        Err(crate::AssignmentError::DynamicDescriptorAssignmentMismatch { machine, operation })
    );
}
