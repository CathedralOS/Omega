use omega_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_psi_to_abstract_operations::lower_artifact_sections;
use omega_target::{Architecture, NativeTarget};
use omega_target_operations_to_assigned_target_operations::assign_registers;
use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

fn assigned_plan(target: NativeTarget) -> omega_assigned_target_operations::AssignedOperationPlan {
    assigned_scalar_plan_from_source(
        target,
        r#"
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
            let result: i32 = forward(erased);
        }

        machine forward(erased: &dyn Measure) -> i32 {
            let result: i32 = erased.measure();
            transition { _ -> result }
        }
    "#,
    )
}

fn assigned_stored_plan(
    target: NativeTarget,
) -> omega_assigned_target_operations::AssignedOperationPlan {
    assigned_unit_plan_from_source(
        target,
        r#"
        trait Measure { machine measure(&self) -> bool; }
        data Item [copy] { value: bool; }
        Primary: Item satisfies Measure {
            machine measure(&self) -> bool { transition { _ -> self.value } }
        }
        data Holder<'item> { handler: &'item dyn Measure; }
        data Main [copy] { item: Item; }
        machine Main::run<'item>(&self) {
            let erased: &'item dyn Measure = &self.item as &dyn Item::Primary;
            let holder: Holder<'item> = Holder { handler: erased };
            let result: bool = holder.handler.measure();
        }
    "#,
    )
}

#[test]
fn emits_stored_descriptor_establishment_and_later_reload() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let assigned = assigned_stored_plan(target);
        let emitted = crate::emit_machine_code(&assigned).expect("stored descriptor emission");
        let caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("entry caller");
        let [call] = caller.stored_dynamic_calls.as_slice() else {
            panic!("one stored descriptor call expected: {caller:#?}")
        };
        let establishment = &call.establishment;
        assert_ne!(establishment.psi_operation, call.psi_operation);
        assert_eq!(call.dynamic_dispatch.stored, establishment.stored);
        assert_eq!(
            establishment.descriptor_abi,
            omega_machine_code::DynamicTraitDescriptorAbiRecord {
                instance_byte_offset: 0,
                table_byte_offset: 8,
                word_byte_size: 8,
                total_byte_size: 16,
                byte_alignment: 8,
            }
        );
        assert_eq!(
            establishment.instance.selection_ordinal,
            establishment.stored.selection.ordinal
        );
        assert_eq!(call.argument.source, establishment.instance.source.source);
        assert_eq!(
            call.argument.source_home_byte_offset,
            establishment.descriptor_home_byte_offset
        );
        assert_eq!(
            call.result.home.byte_offset,
            establishment.descriptor_home_byte_offset + 16
        );
        assert!(establishment.operation_ordinal < call.operation_ordinal);
        assert!(establishment.byte_count > 0);
        assert!(call.byte_count > 0);
        assert_eq!(call.selected_table_byte_offset, 0);
        match (target.architecture, establishment.table_address.encoding) {
            (
                Architecture::X86_64,
                omega_machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                    relocation_offset,
                },
            ) => {
                assert_eq!(
                    &caller.bytes[call.indirect_call_offset
                        ..call.indirect_call_offset + call.indirect_call_byte_count],
                    &[0x41, 0xff, 0xd3]
                );
                assert!(relocation_offset > establishment.table_address.code_offset);
            }
            (
                Architecture::Aarch64,
                omega_machine_code::DynamicTableAddressEncoding::Aarch64PageAddress {
                    page_relocation_offset,
                    page_offset_relocation_offset,
                },
            ) => {
                assert_eq!(
                    u32::from_le_bytes(
                        caller.bytes[call.indirect_call_offset..call.indirect_call_offset + 4]
                            .try_into()
                            .unwrap()
                    ),
                    0xd63f_0120
                );
                assert_eq!(page_offset_relocation_offset, page_relocation_offset + 4);
            }
            evidence => panic!("unexpected stored descriptor evidence: {evidence:?}"),
        }
    }
}

#[test]
fn rejects_stored_descriptor_home_substitution_before_emission() {
    let target = NativeTarget::linux_x64();
    let assigned = assigned_stored_plan(target);

    let mut bad_store = assigned.clone();
    let entry = bad_store.entry;
    let store_operation = bad_store
        .functions
        .iter_mut()
        .find(|function| function.machine == entry)
        .and_then(|function| match &mut function.operation {
            omega_assigned_target_operations::AssignedOperation::UnitBody(body) => body
                .operations
                .iter_mut()
                .find_map(|operation| match operation {
                    omega_assigned_target_operations::AssignedUnitOperation::StoreDynamicDescriptor {
                        psi_operation,
                        descriptor_home_byte_offset,
                        ..
                    } => {
                        *descriptor_home_byte_offset += 8;
                        Some(*psi_operation)
                    }
                    _ => None,
                }),
            _ => None,
        })
        .expect("stored descriptor establishment");
    assert_eq!(
        crate::emit_machine_code(&bad_store),
        Err(crate::EmissionError::InvalidStoredDynamicDescriptorCustody(
            store_operation
        ))
    );

    let mut bad_call = assigned;
    let entry = bad_call.entry;
    let call_operation = bad_call
        .functions
        .iter_mut()
        .find(|function| function.machine == entry)
        .and_then(|function| match &mut function.operation {
            omega_assigned_target_operations::AssignedOperation::UnitBody(body) => body
                .operations
                .iter_mut()
                .find_map(|operation| match operation {
                    omega_assigned_target_operations::AssignedUnitOperation::StoredDynamicScalarCall {
                        psi_operation,
                        descriptor_home_byte_offset,
                        ..
                    } => {
                        *descriptor_home_byte_offset += 8;
                        Some(*psi_operation)
                    }
                    _ => None,
                }),
            _ => None,
        })
        .expect("stored descriptor reload");
    assert_eq!(
        crate::emit_machine_code(&bad_call),
        Err(crate::EmissionError::InvalidStoredDynamicCallCustody(
            call_operation
        ))
    );
}

fn assigned_direct_plan(
    target: NativeTarget,
) -> omega_assigned_target_operations::AssignedOperationPlan {
    assigned_scalar_plan_from_source(
        target,
        r#"
        trait Measure { machine measure(&self) -> i32; }
        data Item { value: i32; }
        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 { transition { _ -> self.value } }
        }
        data Main { selected: Item; }
        machine Main::run(&mut self) {
            let erased: &dyn Measure = &self.selected as &dyn Item::Primary;
            let result: i32 = forward(erased);
        }
        machine forward(erased: &dyn Measure) -> i32 {
            let result: i32 = erased.measure();
            transition { _ -> result }
        }
    "#,
    )
}

fn assigned_multi_hop_plan(
    target: NativeTarget,
) -> omega_assigned_target_operations::AssignedOperationPlan {
    assigned_scalar_plan_from_source(
        target,
        r#"
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
    "#,
    )
}

fn assigned_multi_hop_unit_plan(
    target: NativeTarget,
) -> omega_assigned_target_operations::AssignedOperationPlan {
    assigned_unit_plan_from_source(
        target,
        r#"
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
    "#,
    )
}

pub(super) fn assigned_scalar_plan_from_source(
    target: NativeTarget,
    source: &str,
) -> omega_assigned_target_operations::AssignedOperationPlan {
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = psi_checked_trees_to_terminal::lower_machine(&checked, "Main::run")
        .expect("lower forwarded descriptor source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let abstract_plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("admit Terminal artifact");
    let target_plan = lower_to_target_operations(&abstract_plan, target)
        .expect("lower caller and helper to target operations");
    assign_registers(&target_plan).expect("assign forwarded descriptor ABI")
}

fn assigned_unit_plan(
    target: NativeTarget,
) -> omega_assigned_target_operations::AssignedOperationPlan {
    assigned_unit_plan_from_source(
        target,
        r#"
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
            forward(erased);
        }

        machine forward(erased: &dyn Touch) {
            erased.touch();
        }
    "#,
    )
}

fn assigned_direct_unit_plan(
    target: NativeTarget,
) -> omega_assigned_target_operations::AssignedOperationPlan {
    assigned_unit_plan_from_source(
        target,
        r#"
        trait Touch { machine touch(&self); }
        data Item { value: i32; }
        Primary: Item satisfies Touch { machine touch(&self) {} }
        data Main { selected: Item; }
        machine Main::run(&mut self) {
            let erased: &dyn Touch = &self.selected as &dyn Item::Primary;
            forward(erased);
        }
        machine forward(erased: &dyn Touch) { erased.touch(); }
    "#,
    )
}

fn assigned_unit_plan_from_source(
    target: NativeTarget,
    source: &str,
) -> omega_assigned_target_operations::AssignedOperationPlan {
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = psi_checked_trees_to_terminal::lower_machine(&checked, "Main::run")
        .expect("lower forwarded Unit descriptor source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let abstract_plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("admit Terminal artifact");
    let target_plan = lower_to_target_operations(&abstract_plan, target)
        .expect("lower result-less caller and helper to target operations");
    assign_registers(&target_plan).expect("assign result-less forwarded descriptor ABI")
}

#[test]
fn emits_direct_selection_scalar_forwarding_with_a_durable_result_home() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let assigned = assigned_direct_plan(target);
        let emitted = crate::emit_machine_code(&assigned)
            .expect("emit direct-selection scalar caller and helper");
        let caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("entry caller");
        let [call] = caller.forwarded_dynamic_descriptor_calls.as_slice() else {
            panic!("one direct-selection scalar call expected: {caller:#?}")
        };
        let result = call.result.as_ref().expect("scalar forwarded result");
        let semantic_result = call.semantic_result.expect("scalar semantic result");
        assert_eq!(call.call_plan.result.as_ref(), Some(&result.source));
        assert_eq!(caller.unit_scalar_homes, [result.home]);
        assert_eq!(result.home.source_value, semantic_result.value);
        let [argument] = call.dynamic_arguments.as_slice() else {
            panic!("one direct-selection descriptor argument expected")
        };
        assert!(matches!(
            argument.custody.source,
            omega_target_operations::AbstractDynamicDescriptorSource::Selection { .. }
        ));
        assert_eq!(argument.adapters.len(), 1);
        assert_eq!(
            argument.adapters[0].result,
            psi_terminal::ClosedConformanceCallableResult::I32
        );

        let helper = emitted
            .functions
            .iter()
            .find(|function| function.machine == call.callee)
            .expect("direct-selection scalar helper");
        let [parameter_call] = helper.dynamic_parameter_calls.as_slice() else {
            panic!("one scalar parameter-slot call expected: {helper:#?}")
        };
        assert!(parameter_call.source_value.is_some());
        assert_eq!(
            parameter_call.scalar_type,
            Some(psi_core::ScalarType::Integer(
                psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32).unwrap(),
            ))
        );
    }
}

#[test]
fn emits_parameter_sourced_forwarding_as_a_direct_helper_call() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let assigned = assigned_multi_hop_plan(target);
        let emitted = crate::emit_machine_code(&assigned)
            .expect("emit parameter-sourced descriptor helper chain");
        let helpers = emitted
            .functions
            .iter()
            .filter(|function| !function.forwarded_dynamic_parameter_calls.is_empty())
            .collect::<Vec<_>>();
        let [helper] = helpers.as_slice() else {
            panic!("one parameter-forwarding helper expected: {emitted:#?}")
        };
        let [call] = helper.forwarded_dynamic_parameter_calls.as_slice() else {
            unreachable!()
        };
        assert!(matches!(
            &call.argument.source,
            omega_target_operations::AbstractDynamicDescriptorSource::Parameter(source)
                if source == &call.parameter
        ));
        assert_eq!(call.argument.target.owner, call.callee);
        assert_eq!(call.function_call_plan, call.callee_call_plan);
        assert_eq!(call.instance, call.instance_destination);
        assert_eq!(call.table, call.table_destination);
        assert_eq!(call.operation_ordinal, 0);
        assert_eq!(call.code_offset, 0);
        let omega_machine_code::ForwardedDynamicParameterCallStackEvidence::Scalar(call_stack) =
            &call.call_stack
        else {
            panic!("scalar forwarding requires scalar call-stack custody")
        };
        assert_eq!(
            call.byte_count,
            call.direct_call_offset
                + call.direct_call_byte_count
                + match target.architecture {
                    Architecture::X86_64 => call_stack
                        .outbound
                        .map_or(0, |stack| stack.release_byte_count),
                    Architecture::Aarch64 => 8,
                }
        );
        assert!(helper.scalar_stack.is_some());
        let [relocation] = helper.internal_calls.as_slice() else {
            panic!("one direct helper relocation expected")
        };
        assert_eq!(relocation.target, call.callee);
        assert_eq!(relocation.scalar_stack.as_ref(), Some(call_stack));
        assert_eq!(
            relocation.offset,
            match target.architecture {
                Architecture::X86_64 => call.direct_call_offset + 1,
                Architecture::Aarch64 => call.direct_call_offset,
            }
        );
        let final_helper = emitted
            .functions
            .iter()
            .find(|function| function.machine == call.callee)
            .expect("final dynamic dispatch helper");
        assert_eq!(final_helper.dynamic_parameter_calls.len(), 1);
    }
}

#[test]
fn emits_parameter_sourced_unit_forwarding_as_a_result_less_direct_call() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let assigned = assigned_multi_hop_unit_plan(target);
        let emitted = crate::emit_machine_code(&assigned)
            .expect("emit parameter-sourced Unit descriptor helper chain");
        let helpers = emitted
            .functions
            .iter()
            .filter(|function| !function.forwarded_dynamic_parameter_calls.is_empty())
            .collect::<Vec<_>>();
        let [helper] = helpers.as_slice() else {
            panic!("one Unit parameter-forwarding helper expected: {emitted:#?}")
        };
        let [call] = helper.forwarded_dynamic_parameter_calls.as_slice() else {
            unreachable!()
        };
        assert!(call.source_value.is_none());
        assert!(call.scalar_type.is_none());
        assert!(matches!(
            &call.argument.source,
            omega_target_operations::AbstractDynamicDescriptorSource::Parameter(source)
                if source == &call.parameter
        ));
        assert_eq!(call.function_call_plan, call.callee_call_plan);
        assert!(call.function_call_plan.result.is_none());
        assert_eq!(call.instance, call.instance_destination);
        assert_eq!(call.table, call.table_destination);
        let omega_machine_code::ForwardedDynamicParameterCallStackEvidence::Unit(call_stack) =
            &call.call_stack
        else {
            panic!("Unit forwarding requires Unit call-stack custody")
        };
        assert!(helper.scalar_stack.is_none());
        assert!(helper.unit_stack.is_some());
        let [relocation] = helper.internal_calls.as_slice() else {
            panic!("one direct Unit helper relocation expected")
        };
        assert_eq!(relocation.target, call.callee);
        assert_eq!(relocation.unit_stack.as_ref(), Some(call_stack));
        assert!(relocation.scalar_stack.is_none());
        let final_helper = emitted
            .functions
            .iter()
            .find(|function| function.machine == call.callee)
            .expect("final Unit dynamic dispatch helper");
        let [dispatch] = final_helper.dynamic_parameter_calls.as_slice() else {
            panic!("one final Unit parameter-slot dispatch expected")
        };
        assert!(dispatch.source_value.is_none());
        assert!(dispatch.scalar_type.is_none());
    }
}

#[test]
fn rejects_parameter_sourced_forwarding_register_drift_before_emission() {
    let mut assigned = assigned_multi_hop_plan(NativeTarget::linux_x64());
    let rejected = assigned
        .functions
        .iter_mut()
        .find_map(|function| {
            let omega_assigned_target_operations::AssignedOperation::ReturnForwardedDynamicParameterScalarCall {
                psi_operation,
                instance_destination,
                table_destination,
                ..
            } = &mut function.operation
            else {
                return None;
            };
            *table_destination = *instance_destination;
            Some(*psi_operation)
        })
        .expect("parameter-sourced forwarding helper");
    assert_eq!(
        crate::emit_machine_code(&assigned),
        Err(crate::EmissionError::InvalidDynamicDescriptorCallCustody(
            rejected
        ))
    );
}

#[test]
fn rejects_parameter_sourced_unit_forwarding_register_drift_before_emission() {
    let mut assigned = assigned_multi_hop_unit_plan(NativeTarget::linux_x64());
    let rejected = assigned
        .functions
        .iter_mut()
        .find_map(|function| {
            let omega_assigned_target_operations::AssignedOperation::ForwardDynamicParameterUnitCall {
                psi_operation,
                instance_destination,
                table_destination,
                ..
            } = &mut function.operation
            else {
                return None;
            };
            *table_destination = *instance_destination;
            Some(*psi_operation)
        })
        .expect("parameter-sourced Unit forwarding helper");
    assert_eq!(
        crate::emit_machine_code(&assigned),
        Err(crate::EmissionError::InvalidDynamicDescriptorCallCustody(
            rejected
        ))
    );
}

#[test]
fn emits_result_less_direct_selection_forwarding_without_rebound_evidence() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let assigned = assigned_direct_unit_plan(target);
        let emitted = crate::emit_machine_code(&assigned)
            .expect("emit direct-selection caller and descriptor helper");
        let caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("entry caller");
        let [call] = caller.forwarded_dynamic_descriptor_calls.as_slice() else {
            panic!("one direct-selection forwarded descriptor call expected: {caller:#?}")
        };
        assert!(call.semantic_result.is_none());
        assert!(call.result.is_none());
        let [argument] = call.dynamic_arguments.as_slice() else {
            panic!("one direct-selection descriptor argument expected")
        };
        assert!(matches!(
            argument.custody.source,
            omega_target_operations::AbstractDynamicDescriptorSource::Selection { .. }
        ));
        assert_eq!(argument.adapters.len(), 1);
        assert_eq!(
            argument.adapters[0].result,
            psi_terminal::ClosedConformanceCallableResult::Unit
        );

        let helper = emitted
            .functions
            .iter()
            .find(|function| function.machine == call.callee)
            .expect("direct-selection forwarding helper");
        let [parameter_call] = helper.dynamic_parameter_calls.as_slice() else {
            panic!("one result-less parameter-slot call expected: {helper:#?}")
        };
        assert!(parameter_call.source_value.is_none());
        assert!(parameter_call.scalar_type.is_none());
    }
}

#[test]
fn emits_result_less_forwarded_descriptor_and_helper_slot_calls() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let assigned = assigned_unit_plan(target);
        let emitted = crate::emit_machine_code(&assigned)
            .expect("emit result-less caller and descriptor helper");
        let caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("entry caller");
        let [call] = caller.forwarded_dynamic_descriptor_calls.as_slice() else {
            panic!("one result-less forwarded descriptor call expected: {caller:#?}")
        };
        assert!(call.semantic_result.is_none());
        assert!(call.result.is_none());
        assert!(call.call_plan.result.is_none());
        assert!(caller.unit_scalar_homes.is_empty());
        let [argument] = call.dynamic_arguments.as_slice() else {
            panic!("one forwarded descriptor argument expected")
        };
        assert_eq!(argument.adapters.len(), 1);
        assert_eq!(
            argument.adapters[0].result,
            psi_terminal::ClosedConformanceCallableResult::Unit
        );
        assert!(argument.adapters[0].erased_call_plan.result.is_none());
        assert!(argument.adapters[0].realization_call_plan.result.is_none());

        let helper = emitted
            .functions
            .iter()
            .find(|function| function.machine == call.callee)
            .expect("forward helper");
        let [parameter_call] = helper.dynamic_parameter_calls.as_slice() else {
            panic!("one result-less parameter-slot call expected: {helper:#?}")
        };
        assert!(parameter_call.source_value.is_none());
        assert!(parameter_call.scalar_type.is_none());
        assert_eq!(
            parameter_call.requirement.result,
            psi_terminal::ClosedConformanceCallableResult::Unit
        );
        assert!(parameter_call.function_call_plan.result.is_none());
        assert!(parameter_call.dispatch_call_plan.result.is_none());
    }
}

#[test]
fn emits_forwarded_descriptor_materializations_and_direct_helper_call() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let assigned = assigned_plan(target);
        let emitted = crate::emit_machine_code(&assigned)
            .expect("emit caller and descriptor-dispatch helper");
        let caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("entry caller");
        let [call] = caller.forwarded_dynamic_descriptor_calls.as_slice() else {
            panic!("one caller-side descriptor call expected: {caller:#?}")
        };
        let [argument] = call.dynamic_arguments.as_slice() else {
            panic!("one descriptor argument expected")
        };
        assert_eq!(call.call_plan.parameters.len(), 2);
        let result = call.result.as_ref().expect("scalar forwarded result");
        let semantic_result = call.semantic_result.expect("scalar semantic result");
        assert_eq!(call.call_plan.result.as_ref(), Some(&result.source));
        assert_eq!(caller.unit_scalar_homes, [result.home]);
        assert_eq!(result.home.defining_operation, call.psi_operation);
        assert_eq!(result.home.source_value, semantic_result.value);
        let result_end = result.code_offset + result.byte_count;
        assert_eq!(result_end, call.code_offset + call.byte_count);
        let call_end = call.direct_call_offset + call.direct_call_byte_count;
        assert_eq!(
            result.code_offset,
            call.unit_stack.outbound.map_or(call_end, |outbound| {
                outbound.release_offset + outbound.release_byte_count
            })
        );
        assert_eq!(argument.instance.destination, call.call_plan.parameters[0]);
        assert_eq!(argument.adapters.len(), 2);
        for (row_index, adapter) in argument.adapters.iter().enumerate() {
            assert_eq!(adapter.identity.row_index, row_index as u32);
            assert_eq!(
                adapter.identity.application,
                match &argument.custody.source {
                    omega_target_operations::AbstractDynamicDescriptorSource::Rebound {
                        application,
                        ..
                    } => application.commitment,
                    _ => panic!("caller-local descriptor expected"),
                }
            );
            assert_eq!(adapter.erased_call_plan.parameters.len(), 1);
            assert_eq!(adapter.realization_call_plan.parameters.len(), 1);
            assert_eq!(
                adapter.direct_call_byte_count,
                if target.architecture == Architecture::X86_64 {
                    5
                } else {
                    4
                }
            );
            assert_eq!(
                adapter.return_byte_count,
                if target.architecture == Architecture::X86_64 {
                    1
                } else {
                    4
                }
            );
            match target.architecture {
                Architecture::X86_64 => {
                    assert_eq!(adapter.bytes[adapter.direct_call_offset], 0xe8);
                    assert_eq!(adapter.bytes[adapter.return_offset], 0xc3);
                }
                Architecture::Aarch64 => {
                    assert_eq!(
                        u32::from_le_bytes(
                            adapter.bytes
                                [adapter.direct_call_offset..adapter.direct_call_offset + 4]
                                .try_into()
                                .unwrap()
                        ),
                        0x9400_0000,
                    );
                    assert_eq!(
                        u32::from_le_bytes(
                            adapter.bytes[adapter.return_offset..adapter.return_offset + 4]
                                .try_into()
                                .unwrap()
                        ),
                        0xd65f_03c0,
                    );
                }
            }
        }
        assert_eq!(
            call.direct_call_byte_count,
            if target.architecture == Architecture::X86_64 {
                5
            } else {
                4
            }
        );
        assert!(call.direct_call_offset > argument.table_address.code_offset);
        assert!(
            caller
                .internal_calls
                .iter()
                .any(|relocation| relocation.target == call.callee)
        );
        match (
            target.architecture,
            argument.table_address.encoding,
            argument.instance_destination,
            argument.table_destination,
        ) {
            (
                Architecture::X86_64,
                omega_machine_code::DynamicTableAddressEncoding::X86_64Relative32 { .. },
                omega_target_operations::MachineRegister::X86Rdi,
                omega_target_operations::MachineRegister::X86Rsi,
            ) => assert_eq!(caller.bytes[call.direct_call_offset], 0xe8),
            (
                Architecture::Aarch64,
                omega_machine_code::DynamicTableAddressEncoding::Aarch64PageAddress { .. },
                omega_target_operations::MachineRegister::Aarch64X(0),
                omega_target_operations::MachineRegister::Aarch64X(1),
            ) => assert_eq!(
                u32::from_le_bytes(
                    caller.bytes[call.direct_call_offset..call.direct_call_offset + 4]
                        .try_into()
                        .unwrap()
                ) & 0xfc00_0000,
                0x9400_0000
            ),
            other => panic!("unexpected target-specific descriptor evidence: {other:?}"),
        }

        let helper = emitted
            .functions
            .iter()
            .find(|function| function.machine == call.callee)
            .expect("forwarded helper");
        assert_eq!(helper.dynamic_parameter_calls.len(), 1);
    }
}
