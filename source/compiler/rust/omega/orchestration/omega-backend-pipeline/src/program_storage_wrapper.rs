//! Exact second-pass insertion of the compiler-generated program-storage entry.

use omega_abstract_operations::{
    AbstractFunctionPlan, AbstractOperation, AbstractOperationKind, BoundaryFootprintFragmentOrigin,
};
use omega_backend_plan::BackendPlan;
use omega_calling_conventions::{CallSignature, MachineRegister, compose_state_footprints};
use omega_control_flow::MachineFunctionIdentity;
use omega_machine_emission::{MachineEmissionInput, emit_machine_bytes};
use omega_object_file_planning::{ObjectPlanningInput, build_object_plan};
use omega_relocations::{RelocationPlanningInput, build_relocation_plan};
use omega_runtime_storage::{runtime_frame_storage_alignment, runtime_frame_storage_size};
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryWrapperInsertion {
    pub wrapper_identity: MachineFunctionIdentity,
    pub wrapper_symbol: Arc<str>,
    pub continuation_identity: MachineFunctionIdentity,
}

/// Append one exact receiver-free wrapper and rebuild every representation
/// that owns code, object linkage, or relocation facts. The caller must still
/// independently replay the retained body template against these candidates
/// before publishing the resulting bridge or image.
pub fn insert_program_storage_entry_wrapper(
    plan: &mut BackendPlan,
    insertion: ProgramStorageEntryWrapperInsertion,
) -> Result<(), Diagnostic> {
    validate_insertion(plan, &insertion)?;
    let continuation_key = insertion
        .continuation_identity
        .source_key()
        .expect("validated source continuation identity");
    let kinds = exact_wrapper_kinds(insertion.continuation_identity);
    let mut abstract_operations = plan.abstract_operations.clone();
    let private_source_symbol =
        omega_object_file::private_function_symbol_name(insertion.continuation_identity)
            .ok_or_else(|| {
                Diagnostic::error("program-storage Source has no canonical private symbol")
            })?;
    let source_function_handle = abstract_operations
        .code
        .functions
        .iter()
        .find(|(_, function)| function.identity == insertion.continuation_identity)
        .map(|(handle, _)| handle)
        .expect("validated unique Source continuation");
    abstract_operations
        .code
        .functions
        .get_mut(source_function_handle)
        .symbol = Arc::from(private_source_symbol);
    let instructions =
        abstract_operations
            .code
            .instructions
            .insert_many(kinds.iter().cloned().map(|kind| AbstractOperation {
                kind,
                source_key: continuation_key,
                source_statement: 0,
            }));
    abstract_operations
        .code
        .functions
        .insert(AbstractFunctionPlan {
            symbol: Arc::clone(&insertion.wrapper_symbol),
            identity: insertion.wrapper_identity,
            instructions,
        });
    extend_call_return_footprint(
        &mut abstract_operations,
        plan.entry_boundary_plan.as_ref(),
        &kinds,
    )?;

    let target_operations =
        omega_abstract_operations_to_target_operations::build_target_operation_plan(
            plan.target,
            &plan.host_abi,
            &plan.host_calls,
            &abstract_operations,
        );
    let assigned_target_operations =
        omega_target_operations_to_assigned_target_operations::build_assigned_target_operations(
            &target_operations,
        );
    let machine_instructions =
        omega_assigned_target_operations_to_machine_instructions::build_machine_instructions(
            &assigned_target_operations,
        )?;
    let encoded_machine = emit_machine_bytes(MachineEmissionInput {
        target: plan.target,
        assigned_target_operations: &assigned_target_operations,
        machine_instructions: &machine_instructions,
        host_abi: &plan.host_abi,
        data: &plan.data,
        terminal_dispatch_index: plan.runtime_dispatch_loop.terminal_dispatch_index,
    })?;
    let object = build_object_plan(ObjectPlanningInput {
        target: plan.target,
        host_abi: &plan.host_abi,
        layouts: &plan.layouts,
        entry_machine_symbol: plan.entry_key.machine,
        entry_machine_name: plan.entry_machine_name(),
        entry_function_identity: insertion.wrapper_identity,
        encoded_machine: &encoded_machine,
        data: &plan.data,
        runtime_frame_size: runtime_frame_storage_size(&plan.runtime_storage),
        runtime_frame_alignment: runtime_frame_storage_alignment(&plan.runtime_storage),
    })?;
    let relocations = build_relocation_plan(RelocationPlanningInput {
        target: plan.target,
        instructions: &target_operations,
        assigned_target_operations: &assigned_target_operations,
        encoded_machine: &encoded_machine,
        data: &plan.data,
        object: &object,
        host_abi: &plan.host_abi,
        entry_machine_name: plan.entry_machine_name(),
    })?;

    plan.abstract_operations = abstract_operations;
    plan.target_operations = target_operations;
    plan.assigned_target_operations = assigned_target_operations;
    plan.machine_instructions = machine_instructions;
    plan.encoded_machine = encoded_machine;
    plan.object = object;
    plan.relocations = relocations;
    Ok(())
}

fn validate_insertion(
    plan: &BackendPlan,
    insertion: &ProgramStorageEntryWrapperInsertion,
) -> Result<(), Diagnostic> {
    if plan.target != omega_target::NativeTarget::uefi_x64()
        || insertion.wrapper_symbol.as_ref() != omega_object_file::entry_symbol_name(plan.target)
        || insertion
            .wrapper_identity
            .program_storage_entry_continuation()
            != insertion.continuation_identity.source_key()
    {
        return Err(Diagnostic::error(
            "program-storage wrapper insertion requires the exact UEFI x64 entry symbol and related Source identity",
        ));
    }
    let source_count = plan
        .abstract_operations
        .code
        .functions
        .iter()
        .filter(|(_, function)| function.identity == insertion.continuation_identity)
        .count();
    let wrapper_or_unrelated_symbol_exists =
        plan.abstract_operations
            .code
            .functions
            .iter()
            .any(|(_, function)| {
                function.identity == insertion.wrapper_identity
                    || (function.identity != insertion.continuation_identity
                        && function.symbol == insertion.wrapper_symbol)
            });
    if source_count != 1 || wrapper_or_unrelated_symbol_exists {
        return Err(Diagnostic::error(
            "program-storage wrapper insertion lost its unique Source continuation or collided with an existing function",
        ));
    }
    Ok(())
}

fn exact_wrapper_kinds(continuation: MachineFunctionIdentity) -> [AbstractOperationKind; 11] {
    [
        AbstractOperationKind::EnterFunction,
        AbstractOperationKind::ReserveOutgoingStackFrame { byte_count: 72 },
        AbstractOperationKind::CopyEntryIndirectU64ToOutgoingStack {
            source_register: MachineRegister::X86Rcx,
            source_byte_offset: 0,
            stack_byte_offset: 32,
        },
        AbstractOperationKind::CopyEntryIndirectU64ToOutgoingStack {
            source_register: MachineRegister::X86Rcx,
            source_byte_offset: 8,
            stack_byte_offset: 40,
        },
        AbstractOperationKind::CopyEntryIndirectU64ToOutgoingStack {
            source_register: MachineRegister::X86Rdx,
            source_byte_offset: 0,
            stack_byte_offset: 48,
        },
        AbstractOperationKind::CopyEntryIndirectU64ToOutgoingStack {
            source_register: MachineRegister::X86Rdx,
            source_byte_offset: 8,
            stack_byte_offset: 56,
        },
        AbstractOperationKind::LoadOutgoingStackAddress {
            register: MachineRegister::X86Rcx,
            stack_byte_offset: 32,
        },
        AbstractOperationKind::LoadOutgoingStackAddress {
            register: MachineRegister::X86Rdx,
            stack_byte_offset: 48,
        },
        AbstractOperationKind::CallInternalFunction {
            target: continuation,
        },
        AbstractOperationKind::ReleaseOutgoingStackFrame { byte_count: 72 },
        AbstractOperationKind::LeaveFunction,
    ]
}

fn extend_call_return_footprint(
    abstract_operations: &mut omega_abstract_operations::AbstractOperationPlan,
    entry_boundary_plan: Option<&omega_calling_conventions::BoundaryEntryPlan>,
    kinds: &[AbstractOperationKind; 11],
) -> Result<(), Diagnostic> {
    let boundary_plan = entry_boundary_plan.cloned().ok_or_else(|| {
        Diagnostic::error("program-storage wrapper insertion has no physical boundary plan")
    })?;
    let signature = CallSignature {
        parameters: boundary_plan
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .collect(),
        result: boundary_plan
            .call
            .result
            .as_ref()
            .map(|placement| placement.shape),
    };
    let boundary =
        omega_calling_conventions::validate_boundary_entry_plan(boundary_plan, &signature)
            .map_err(|error| Diagnostic::error(error.0))?;
    let wrapper = omega_instruction_selection::derive_boundary_call_return_mechanics_footprint(
        &boundary,
        kinds.iter(),
    )
    .map_err(|error| Diagnostic::error(error.0))?;
    let fragments = &mut abstract_operations
        .semantics
        .boundaries
        .footprints
        .fragments;
    let retained_indices = fragments
        .iter()
        .enumerate()
        .filter_map(|(index, fragment)| {
            (fragment.origin == BoundaryFootprintFragmentOrigin::CallReturnMechanics)
                .then_some(index)
        })
        .collect::<Vec<_>>();
    let [retained_index] = retained_indices.as_slice() else {
        return Err(Diagnostic::error(
            "program-storage wrapper insertion requires one exact retained CallReturn footprint",
        ));
    };
    let composed = compose_state_footprints([&fragments[*retained_index].evidence, &wrapper]);
    omega_calling_conventions::validate_call_return_mechanics_footprint(&boundary, &composed)
        .map_err(|error| Diagnostic::error(error.0))?;
    fragments[*retained_index].evidence = composed;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_backend_plan::{BackendArtifactRoots, BackendPlanPhaseTiming};
    use omega_control_flow::StateKey;
    use psi_arena::{Arena, HandleSpan};
    use psi_symbols::SymbolHandle;

    fn key() -> StateKey {
        StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            segment_index: 0,
        }
    }

    fn plan_without_boundary() -> BackendPlan {
        let target = omega_target::NativeTarget::uefi_x64();
        let continuation = MachineFunctionIdentity::source(key());
        let mut abstract_operations = omega_abstract_operations::AbstractOperationPlan::default();
        abstract_operations
            .code
            .functions
            .insert(AbstractFunctionPlan {
                symbol: Arc::from(omega_object_file::entry_symbol_name(target)),
                identity: continuation,
                instructions: HandleSpan::empty(),
            });
        BackendPlan {
            target_profile: omega_target::TargetProfile::UefiX64,
            target,
            artifacts: BackendArtifactRoots::empty_for_target(target),
            host_abi: Arc::new(omega_calling_conventions::build_host_abi_plan(target)),
            host_calls: Arc::new(Default::default()),
            state_calls: Arc::new(Default::default()),
            alias_flow: Default::default(),
            state_storage: Arc::new(Default::default()),
            state_values: Default::default(),
            abstract_data: Default::default(),
            data: Default::default(),
            abstract_operations,
            target_operations: Default::default(),
            assigned_target_operations: Default::default(),
            control_flow: Arc::new(Default::default()),
            runtime_flow: Arc::new(Default::default()),
            state_dispatch: Arc::new(Default::default()),
            state_guards: Arc::new(Default::default()),
            runtime_bodies: Arc::new(Default::default()),
            runtime_branching_calls: Default::default(),
            runtime_dispatch_loop: Default::default(),
            runtime_storage: Default::default(),
            runtime_text: Default::default(),
            layouts: Arc::new(omega_layout::LayoutPlan {
                data_layouts: Arena::new(),
                fields: Arena::new(),
                bit_fields: Vec::new(),
                stored_integers: Vec::new(),
                repeated_fields: Vec::new(),
                machine_layouts: Arena::new(),
                variants: Arena::new(),
                private_callback_demands: Vec::new(),
            }),
            entry_key: key(),
            entry_boundary_plan: None,
            callback_placements: Arc::from([]),
            callback_thunks: Arc::from([]),
            callback_private_relocations: Arc::from([]),
            callback_registrar_arguments: Arc::from([]),
            callback_registrar_destinations: Arc::from([]),
            receiver_bases: Vec::new(),
            state_contexts: Vec::new(),
            phase_timings: Arena::<BackendPlanPhaseTiming>::new(),
        }
    }

    #[test]
    fn failed_candidate_build_leaves_the_original_backend_plan_unchanged() {
        let mut plan = plan_without_boundary();
        let original = plan.clone();
        let continuation_identity = MachineFunctionIdentity::source(key());
        let wrapper_identity =
            MachineFunctionIdentity::program_storage_entry_wrapper(key()).unwrap();
        let wrapper_symbol = Arc::from(omega_object_file::entry_symbol_name(plan.target));
        let error = insert_program_storage_entry_wrapper(
            &mut plan,
            ProgramStorageEntryWrapperInsertion {
                wrapper_identity,
                wrapper_symbol,
                continuation_identity,
            },
        )
        .expect_err("missing physical boundary must reject after candidate mutation begins");
        assert!(error.message.contains("no physical boundary plan"));
        assert_eq!(plan, original);
    }
}
