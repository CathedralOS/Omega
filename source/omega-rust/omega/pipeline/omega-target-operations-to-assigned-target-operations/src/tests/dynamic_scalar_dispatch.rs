use crate::assign_registers;
use omega_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_assigned_target_operations::{AssignedOperation, AssignedUnitOperation};
use omega_psi_to_abstract_operations::lower_artifact_sections;
use omega_target::NativeTarget;
use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
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
