use crate::assign_registers;
use omega_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_assigned_target_operations::{AssignedOperation, AssignedUnitOperation};
use omega_calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use omega_psi_to_abstract_operations::lower_artifact_sections;
use omega_target::NativeTarget;
use omega_target_operations::{
    TargetDynamicDescriptorParameterAbi, TargetFunction, TargetOperation, TargetOperationPlan,
    TerminalPsiProvenance,
};
use psi_core::{EdgeId, IntegerSign, IntegerType, MachineId, OperationId, ScalarType, ValueId};
use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{
    ClosedConformanceCallableResult, SemanticFingerprint, StructuralAccess,
    TerminalDynamicDescriptorParameter, TerminalDynamicRequirement, TerminalPsiIdentity,
    VocabularyMarker,
};
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

fn target_plan(target: NativeTarget) -> omega_target_operations::TargetOperationPlan {
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
    let abstract_plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified Terminal artifact");
    lower_to_target_operations(&abstract_plan, target)
        .expect("lower rebound dynamic call to target operations")
}

fn forwarded_target_plan(target: NativeTarget) -> omega_target_operations::TargetOperationPlan {
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
    let abstract_plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified Terminal artifact");
    lower_to_target_operations(&abstract_plan, target)
        .expect("lower caller and forwarded helper to target operations")
}

fn multi_hop_target_plan(target: NativeTarget) -> omega_target_operations::TargetOperationPlan {
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
    let terminal = psi_checked_trees_to_terminal::lower_machine(&checked, "Main::run")
        .expect("lower multi-hop dynamic source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let abstract_plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified Terminal artifact");
    lower_to_target_operations(&abstract_plan, target)
        .expect("lower parameter-sourced forwarding to target operations")
}

fn dynamic_unit_target_plan(target: NativeTarget) -> omega_target_operations::TargetOperationPlan {
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
    let abstract_plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified Terminal artifact");
    lower_to_target_operations(&abstract_plan, target)
        .expect("lower rebound dynamic Unit call to target operations")
}

#[test]
fn assigns_rebound_dynamic_unit_descriptor_without_a_result_home() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_plan = dynamic_unit_target_plan(target);
        let assigned = assign_registers(&target_plan).expect("assign dynamic Unit descriptor");
        let caller = assigned
            .functions
            .iter()
            .find(|function| function.machine == assigned.entry)
            .expect("entry caller");
        let AssignedOperation::UnitBody(body) = &caller.operation else {
            panic!("dynamic Unit caller must remain an attached Unit body")
        };
        let calls = body
            .operations
            .iter()
            .filter_map(|operation| match operation {
                AssignedUnitOperation::DynamicUnitCall {
                    dynamic_dispatch,
                    call_plan,
                    descriptor_abi,
                    descriptor_home_byte_offset,
                    initial_copy,
                    rebound_copy,
                    ..
                } => Some((
                    dynamic_dispatch,
                    call_plan,
                    descriptor_abi,
                    descriptor_home_byte_offset,
                    initial_copy,
                    rebound_copy,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(dynamic, call_plan, descriptor, descriptor_offset, initial, rebound)] =
            calls.as_slice()
        else {
            panic!("one assigned rebound Unit call expected: {body:#?}")
        };
        assert!(call_plan.result.is_none());
        assert_eq!(descriptor.instance_offset(), 0);
        assert_eq!(descriptor.table_offset(), 8);
        assert_eq!(descriptor.word_size(), 8);
        assert_eq!(descriptor.total_size(), 16);
        assert_eq!(descriptor.align(), 8);
        assert_eq!(**descriptor_offset % 8, 0);
        assert_eq!(initial.path, dynamic.initial.source.path);
        assert_eq!(rebound.path, dynamic.rebound.source.path);
        assert_ne!(initial.source_byte_offset, rebound.source_byte_offset);
        assert_eq!(initial.destination, rebound.destination);
    }
}

#[test]
fn rejects_reauthenticated_dynamic_unit_owner_substitution() {
    let mut plan = dynamic_unit_target_plan(NativeTarget::linux_x64());
    let caller = plan
        .functions
        .iter_mut()
        .find(|function| function.machine == plan.entry)
        .expect("entry caller");
    let TargetOperation::UnitBody(body) = &mut caller.operation else {
        panic!("dynamic Unit caller must remain an attached Unit body")
    };
    let rejected_operation = body
        .operations
        .iter_mut()
        .find_map(|operation| match operation {
            omega_target_operations::TargetUnitOperation::DynamicUnitCall {
                psi_operation,
                dynamic_dispatch,
                ..
            } => {
                dynamic_dispatch.dispatch.owner =
                    MachineId::new(dynamic_dispatch.dispatch.owner.get() + 100)
                        .expect("distinct machine");
                Some(*psi_operation)
            }
            _ => None,
        })
        .expect("dynamic Unit call");
    assert!(matches!(
        assign_registers(&plan),
        Err(crate::AssignmentError::DynamicUnitCallCustodyMismatch {
            operation: rejected,
            ..
        }) if rejected == rejected_operation
    ));
}

#[test]
fn assigns_forwarded_descriptor_registers_and_indirect_mechanism() {
    let target = NativeTarget::linux_x64();
    let pointer = ValueShape::integer(8, 8);
    let result = ValueShape::integer(4, 4);
    let function_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![pointer, pointer],
            result: Some(result),
        },
    )
    .unwrap();
    let dispatch_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![pointer],
            result: Some(result),
        },
    )
    .unwrap();
    let machine = MachineId::new(1).unwrap();
    let requirement = TerminalDynamicRequirement {
        slot: 0,
        declaring_trait_identity: "Measure".into(),
        public_requirement_identity: "Measure::measure".into(),
        result: ClosedConformanceCallableResult::I32,
    };
    let parameter = TerminalDynamicDescriptorParameter {
        owner: machine,
        ordinal: 0,
        source_position: 0,
        trait_identity: "Measure".into(),
        access: StructuralAccess::SharedBorrow,
        requirements: vec![requirement.clone()],
    };
    let plan = TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
        },
        target,
        entry: machine,
        functions: vec![TargetFunction {
            machine,
            attachment: None,
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: vec![OperationId::new(1).unwrap()],
                edges: vec![EdgeId::new(1).unwrap()],
            },
            operation: TargetOperation::ReturnDynamicParameterScalarCall {
                psi_edge: EdgeId::new(1).unwrap(),
                psi_operation: OperationId::new(1).unwrap(),
                source_value: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                ),
                parameter_abi: TargetDynamicDescriptorParameterAbi {
                    parameter,
                    instance: function_call_plan.parameters[0].clone(),
                    table: function_call_plan.parameters[1].clone(),
                },
                requirement,
                function_call_plan,
                dispatch_call_plan,
                table_slot_byte_offset: 0,
            },
        }],
    };
    let assigned = assign_registers(&plan).expect("assign forwarded descriptor call");
    let function = assigned.functions.first().expect("assigned helper");
    let omega_assigned_target_operations::AssignedOperation::ReturnDynamicParameterScalarCall {
        parameter_abi,
        mechanism,
        table_slot_byte_offset,
        ..
    } = &function.operation
    else {
        panic!("forwarded descriptor keeps its assigned role")
    };
    assert_eq!(
        parameter_abi.instance,
        omega_target_operations::MachineRegister::X86Rdi
    );
    assert_eq!(
        parameter_abi.table,
        omega_target_operations::MachineRegister::X86Rsi
    );
    assert_eq!(*table_slot_byte_offset, 0);
    assert_eq!(
        *mechanism,
        omega_assigned_target_operations::AssignedDynamicParameterCallMechanism::X86MemoryIndirect {
            table: omega_target_operations::MachineRegister::X86Rsi,
        }
    );
}

#[test]
fn assigns_rebound_descriptor_arguments_to_forwarded_call_registers() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_plan = forwarded_target_plan(target);
        let assigned = assign_registers(&target_plan)
            .expect("assign complete caller-to-forwarder descriptor path");
        let caller = assigned
            .functions
            .iter()
            .find(|function| function.machine == assigned.entry)
            .expect("entry caller");
        let AssignedOperation::UnitBody(body) = &caller.operation else {
            panic!("forwarding caller must remain an attached Unit body")
        };
        let calls = body
            .operations
            .iter()
            .filter_map(|operation| match operation {
                AssignedUnitOperation::StructuralScalarCallWithDynamicArguments {
                    callee,
                    call_plan,
                    copies,
                    dynamic_arguments,
                    ..
                } => Some((callee, call_plan, copies, dynamic_arguments)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(callee, call_plan, copies, dynamic_arguments)] = calls.as_slice() else {
            panic!("one assigned forwarded descriptor call expected: {body:#?}")
        };
        assert!(copies.is_empty());
        let [argument] = dynamic_arguments.as_slice() else {
            panic!("one assigned dynamic descriptor argument expected")
        };
        let expected_instance = match target.architecture {
            omega_target::Architecture::X86_64 => omega_target_operations::MachineRegister::X86Rdi,
            omega_target::Architecture::Aarch64 => {
                omega_target_operations::MachineRegister::Aarch64X(0)
            }
        };
        let expected_table = match target.architecture {
            omega_target::Architecture::X86_64 => omega_target_operations::MachineRegister::X86Rsi,
            omega_target::Architecture::Aarch64 => {
                omega_target_operations::MachineRegister::Aarch64X(1)
            }
        };
        assert_eq!(argument.instance.destination, expected_instance);
        assert_eq!(argument.table_destination, expected_table);
        assert_eq!(call_plan.parameters.len(), 2);
        assert!(
            target_plan
                .functions
                .iter()
                .any(|function| function.machine == **callee)
        );
    }
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
            omega_target_operations::AbstractDynamicDescriptorSource::Parameter(source)
                if source == &parameter_abi.parameter
        ));
        assert_eq!(parameter_abi.instance, **instance_destination);
        assert_eq!(parameter_abi.table, **table_destination);
        assert_eq!(function_plan, callee_plan);
        assert_ne!(parameter_abi.instance, parameter_abi.table);
    }
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

#[test]
fn rejects_reauthenticated_dynamic_owner_substitution() {
    let mut plan = target_plan(NativeTarget::linux_x64());
    let caller = plan
        .functions
        .iter_mut()
        .find(|function| function.machine == plan.entry)
        .expect("entry caller");
    let omega_target_operations::TargetOperation::UnitBody(body) = &mut caller.operation else {
        panic!("dynamic caller must remain an attached Unit body")
    };
    let rejected_operation = {
        let (psi_operation, dynamic) = body
            .operations
            .iter_mut()
            .find_map(|operation| match operation {
                omega_target_operations::TargetUnitOperation::DynamicScalarCall {
                    psi_operation,
                    dynamic_dispatch,
                    ..
                } => Some((*psi_operation, dynamic_dispatch)),
                _ => None,
            })
            .expect("dynamic call");
        dynamic.dispatch.owner =
            psi_core::MachineId::new(dynamic.dispatch.owner.get() + 100).expect("distinct machine");
        psi_operation
    };
    assert!(matches!(
        assign_registers(&plan),
        Err(crate::AssignmentError::DynamicScalarCallCustodyMismatch {
            operation: rejected,
            ..
        }) if rejected == rejected_operation
    ));
}

#[test]
fn assigns_canonical_descriptor_and_distinct_rebound_source() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_plan = target_plan(target);
        let assigned = assign_registers(&target_plan)
            .expect("assign rebound dynamic descriptor and result homes");
        let caller = assigned
            .functions
            .iter()
            .find(|function| function.machine == assigned.entry)
            .expect("entry caller");
        let AssignedOperation::UnitBody(body) = &caller.operation else {
            panic!("dynamic caller must remain an attached Unit body")
        };
        let calls = body
            .operations
            .iter()
            .filter_map(|operation| match operation {
                AssignedUnitOperation::DynamicScalarCall {
                    dynamic_dispatch,
                    result_home,
                    descriptor_abi,
                    descriptor_home_byte_offset,
                    initial_copy,
                    rebound_copy,
                    ..
                } => Some((
                    dynamic_dispatch,
                    result_home,
                    descriptor_abi,
                    descriptor_home_byte_offset,
                    initial_copy,
                    rebound_copy,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(dynamic, result, descriptor, descriptor_offset, initial, rebound)] = calls.as_slice()
        else {
            panic!("one assigned rebound dynamic call expected: {body:#?}")
        };

        assert_eq!(descriptor.instance_offset(), 0);
        assert_eq!(descriptor.table_offset(), 8);
        assert_eq!(descriptor.word_size(), 8);
        assert_eq!(descriptor.total_size(), 16);
        assert_eq!(descriptor.align(), 8);
        assert_eq!(**descriptor_offset % 8, 0);
        assert_eq!(result.byte_offset, **descriptor_offset + 16);
        assert_eq!(initial.path, dynamic.initial.source.path);
        assert_eq!(rebound.path, dynamic.rebound.source.path);
        assert_ne!(initial.source_byte_offset, rebound.source_byte_offset);
        assert_eq!(initial.destination, rebound.destination);
        assert_eq!(dynamic.application.rows.len(), 2);
        assert!(
            target_plan
                .functions
                .iter()
                .any(|function| function.machine == dynamic.dispatch.realization)
        );
    }
}

#[test]
fn rejects_dynamic_call_missing_an_unselected_table_row() {
    let mut plan = target_plan(NativeTarget::linux_x64());
    let caller = plan
        .functions
        .iter_mut()
        .find(|function| function.machine == plan.entry)
        .expect("entry caller");
    let omega_target_operations::TargetOperation::UnitBody(body) = &mut caller.operation else {
        panic!("dynamic caller must remain an attached Unit body")
    };
    let rejected_operation = body
        .operations
        .iter_mut()
        .find_map(|operation| match operation {
            omega_target_operations::TargetUnitOperation::DynamicScalarCall {
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
        .expect("dynamic call");
    assert!(matches!(
        assign_registers(&plan),
        Err(crate::AssignmentError::DynamicScalarCallCustodyMismatch {
            operation: rejected,
            ..
        }) if rejected == rejected_operation
    ));
}
