use crate::phase_diagram::PhaseDiagramBuilder;
use omega_abstract_operations::{AbstractOperation, AbstractOperationPlan};
use omega_assigned_target_operations::{AssignedOperation, AssignedTargetOperationPlan};
use omega_control_flow::{
    ControlFlowPlan, MachineFlow, Operation, OperationKind, PlannedTransitionTarget, StateFlow,
    StateKey, TransitionFlow,
};
use omega_machine_bytes::EncodedMachinePlan;
use omega_machine_instructions::{MachineInstruction, MachineInstructionPlan};
use omega_object_file::{ObjectPlan, RelocationPlan, RelocationRecord, SectionKind, SymbolKind};
use omega_target_operations::{TargetOperation, TargetOperationPlan};
use psi_symbols::SymbolHandle;
use std::fmt::Debug;

const MAX_EMISSION_DETAIL_CHUNKS: usize = 16;
const MAX_EMISSION_DETAIL_LINES_PER_CHUNK: usize = 32;

pub fn abstract_operations_html(
    plan: &AbstractOperationPlan,
    control_flow: &ControlFlowPlan,
) -> String {
    let function_views = collect_state_function_views(
        "abstract block",
        plan.code.instructions.storage_slice(),
        |instruction| instruction.source_key,
        abstract_instruction_line,
    );
    build_backend_cfg_diagram("abstract_operations", control_flow, &function_views)
}

/// Checkable ENT3 implementation-evidence artifact sourced from the semantic
/// boundary root after machine emission. `enumeration_complete` remains false
/// until post-layout body/exit/veneer/thunk/leaf enumeration is wired;
/// consumers must not mistake retained fragments for a final certificate.
pub fn boundary_footprint_fragments_json(plan: &EncodedMachinePlan) -> String {
    fn push_string(output: &mut String, value: &str) {
        output.push('"');
        for character in value.chars() {
            match character {
                '"' => output.push_str("\\\""),
                '\\' => output.push_str("\\\\"),
                '\n' => output.push_str("\\n"),
                '\r' => output.push_str("\\r"),
                '\t' => output.push_str("\\t"),
                character => output.push(character),
            }
        }
        output.push('"');
    }

    fn push_evidence(
        output: &mut String,
        evidence: &omega_calling_conventions::StateFootprintEvidence,
    ) {
        output.push_str("{\"fingerprint\": \"0x");
        output.push_str(&format!("{:016x}", evidence.evidence_fingerprint()));
        output.push_str("\", \"machine_state_bits\": ");
        output.push_str(&evidence.machine_state().bits().to_string());
        output.push_str(", \"registers\": [");
        for (index, register) in evidence.registers().as_slice().iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            push_string(output, &format!("{register:?}"));
        }
        output.push_str("]}");
    }

    let footprints = &plan.semantics.boundaries.footprints;
    let composed = footprints.composed_evidence();
    let mut json = String::from(
        "{\n  \"evidence_stage\": \"encoded_machine\",\n  \"boundary_contract_fingerprint\": ",
    );
    if let Some(fingerprint) = footprints.boundary_contract_fingerprint {
        push_string(&mut json, &format!("0x{fingerprint:016x}"));
    } else {
        json.push_str("null");
    }
    json.push_str(",\n  \"enumeration_complete\": ");
    json.push_str(if footprints.enumeration_complete {
        "true"
    } else {
        "false"
    });
    json.push_str(",\n  \"composed\": ");
    push_evidence(&mut json, &composed);
    json.push_str(",\n  \"fragments\": [");
    for (index, fragment) in footprints.fragments.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\"origin\": ");
        push_string(
            &mut json,
            match fragment.origin {
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::EntryStorage => {
                    "entry_storage"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::EntrySliceDescriptor => {
                    "entry_slice_descriptor"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::ExitResultRegisters => {
                    "exit_result_registers"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::ExitIndirectResultCopy => {
                    "exit_indirect_result_copy"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyPlaceCopy => {
                    "compiler_body_place_copy"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyPlaceIntegerWrite => {
                    "compiler_body_place_integer_write"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyPlaceAddressWrite => {
                    "compiler_body_place_address_write"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyAtomicOperation => {
                    "compiler_body_atomic_operation"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyConstantHostResult => {
                    "compiler_body_constant_host_result"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscall => {
                    "compiler_body_outbound_syscall"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImport => {
                    "compiler_body_outbound_immediate_import"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImportResult => {
                    "compiler_body_outbound_immediate_import_result"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundFloatImportResult => {
                    "compiler_body_outbound_float_import_result"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDereferencedImportResult => {
                    "compiler_body_outbound_dereferenced_import_result"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImport => {
                    "compiler_body_outbound_data_import"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImportResult => {
                    "compiler_body_outbound_data_import_result"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImport => {
                    "compiler_body_outbound_authored_import"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImportResult => {
                    "compiler_body_outbound_authored_import_result"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImport => {
                    "compiler_body_outbound_authored_float_import"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImportResult => {
                    "compiler_body_outbound_authored_float_import_result"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImport => {
                    "compiler_body_outbound_authored_aggregate_import"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImportResult => {
                    "compiler_body_outbound_authored_aggregate_import_result"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateResult => {
                    "compiler_body_outbound_authored_aggregate_result"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundIndirectCall => {
                    "compiler_body_outbound_indirect_call"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundOpenCreateImport => {
                    "compiler_body_outbound_open_create_import"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteRead => {
                    "compiler_body_runtime_byte_read"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteWrite => {
                    "compiler_body_runtime_byte_write"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeLineRead => {
                    "compiler_body_runtime_line_read"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImport => {
                    "compiler_body_outbound_storage_import"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImportResult => {
                    "compiler_body_outbound_storage_import_result"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallDataArguments => {
                    "compiler_body_outbound_syscall_data_arguments"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResult => {
                    "compiler_body_outbound_syscall_result"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultDataArguments => {
                    "compiler_body_outbound_syscall_result_data_arguments"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultStorageArguments => {
                    "compiler_body_outbound_syscall_result_storage_arguments"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallStorageArguments => {
                    "compiler_body_outbound_syscall_storage_arguments"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecArgument => {
                    "compiler_body_outbound_syscall_timespec_argument"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecResult => {
                    "compiler_body_outbound_syscall_timespec_result"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyStorageBitFieldWrite => {
                    "compiler_body_storage_bit_field_write"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite => {
                    "compiler_body_place_bounded_buffer_write"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyPlaceStringWrite => {
                    "compiler_body_place_string_write"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireLiteralByteAppend => {
                    "compiler_body_wire_literal_byte_append"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintAppend => {
                    "compiler_body_wire_scalar_varint_append"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireTextBytesAppend => {
                    "compiler_body_wire_text_bytes_append"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarSliceAppend => {
                    "compiler_body_wire_scalar_slice_append"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintAppend => {
                    "compiler_body_wire_repeated_scalar_varint_append"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireExpectedByteRead => {
                    "compiler_body_wire_expected_byte_read"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintRead => {
                    "compiler_body_wire_scalar_varint_read"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireByteSliceRead => {
                    "compiler_body_wire_byte_slice_read"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedOpen => {
                    "compiler_body_wire_nested_open"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedClose => {
                    "compiler_body_wire_nested_close"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintRead => {
                    "compiler_body_wire_repeated_scalar_varint_read"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite => {
                    "compiler_body_text_assembly_write"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBinaryWrite => {
                    "compiler_body_place_binary_write"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyStorageConvertWrite => {
                    "compiler_body_storage_convert_write"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CallReturnMechanics => {
                    "call_return_mechanics"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::DispatchScaffold => {
                    "dispatch_scaffold"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::StaticGuardComparison => {
                    "static_guard_comparison"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::RuntimeTextGuardComparison => {
                    "runtime_text_guard_comparison"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::PlaceGuardComparison => {
                    "place_guard_comparison"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::RuntimeValueGuardComparison => {
                    "runtime_value_guard_comparison"
                }
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::CheckedAssemblyCatalog => {
                    "checked_assembly_catalog"
                }
            },
        );
        json.push_str(", \"evidence\": ");
        push_evidence(&mut json, &fragment.evidence);
        json.push('}');
    }
    if !footprints.fragments.is_empty() {
        json.push('\n');
        json.push_str("  ");
    }
    json.push_str("]\n}\n");
    json
}

#[cfg(test)]
mod boundary_footprint_tests {
    use super::*;
    use omega_abstract_operations::{BoundaryFootprintFragment, BoundaryFootprintFragmentOrigin};
    use omega_calling_conventions::{
        MachineRegister, MachineStateSet, RegisterSet, StateFootprintEvidence,
    };

    #[test]
    fn boundary_footprint_json_preserves_fragment_provenance_and_incomplete_status() {
        let mut plan = EncodedMachinePlan::default();
        plan.semantics
            .boundaries
            .footprints
            .boundary_contract_fingerprint = Some(0x1234);
        plan.semantics
            .boundaries
            .footprints
            .fragments
            .push(BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::EntryStorage,
                evidence: StateFootprintEvidence::new(
                    RegisterSet::new([MachineRegister::X86R15]),
                    MachineStateSet::empty(),
                ),
            });
        plan.semantics
            .boundaries
            .footprints
            .fragments
            .push(BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::StaticGuardComparison,
                evidence: StateFootprintEvidence::new(
                    RegisterSet::new([
                        MachineRegister::X86R10,
                        MachineRegister::X86R11,
                        MachineRegister::X86R15,
                    ]),
                    MachineStateSet::new([omega_calling_conventions::MachineState::Flags]),
                ),
            });
        plan.semantics
            .boundaries
            .footprints
            .fragments
            .push(BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::RuntimeTextGuardComparison,
                evidence: StateFootprintEvidence::new(
                    RegisterSet::new([
                        MachineRegister::X86Rax,
                        MachineRegister::X86Rcx,
                        MachineRegister::X86R15,
                    ]),
                    MachineStateSet::new([omega_calling_conventions::MachineState::Flags]),
                ),
            });
        plan.semantics
            .boundaries
            .footprints
            .fragments
            .push(BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::PlaceGuardComparison,
                evidence: StateFootprintEvidence::new(
                    RegisterSet::new([
                        MachineRegister::X86R10,
                        MachineRegister::X86R11,
                        MachineRegister::X86R14,
                        MachineRegister::X86R15,
                    ]),
                    MachineStateSet::new([omega_calling_conventions::MachineState::Flags]),
                ),
            });
        plan.semantics
            .boundaries
            .footprints
            .fragments
            .push(BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::RuntimeValueGuardComparison,
                evidence: StateFootprintEvidence::new(
                    RegisterSet::new([
                        MachineRegister::X86Rax,
                        MachineRegister::X86R10,
                        MachineRegister::X86R11,
                    ]),
                    MachineStateSet::new([omega_calling_conventions::MachineState::Flags]),
                ),
            });
        plan.semantics
            .boundaries
            .footprints
            .fragments
            .push(BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::DispatchScaffold,
                evidence: StateFootprintEvidence::new(
                    RegisterSet::new([MachineRegister::X86R12]),
                    MachineStateSet::new([omega_calling_conventions::MachineState::Flags]),
                ),
            });
        plan.semantics
            .boundaries
            .footprints
            .fragments
            .push(BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                evidence: StateFootprintEvidence::new(
                    RegisterSet::new([MachineRegister::X86Rsp]),
                    MachineStateSet::empty(),
                ),
            });
        plan.semantics
            .boundaries
            .footprints
            .fragments
            .push(BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::ExitIndirectResultCopy,
                evidence: StateFootprintEvidence::new(
                    RegisterSet::new([MachineRegister::X86R14]),
                    MachineStateSet::empty(),
                ),
            });
        plan.semantics
            .boundaries
            .footprints
            .fragments
            .push(BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::ExitResultRegisters,
                evidence: StateFootprintEvidence::new(
                    RegisterSet::new([MachineRegister::X86Rax]),
                    MachineStateSet::empty(),
                ),
            });
        plan.semantics
            .boundaries
            .footprints
            .fragments
            .push(BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::EntrySliceDescriptor,
                evidence: StateFootprintEvidence::new(
                    RegisterSet::new([MachineRegister::X86Rax]),
                    MachineStateSet::empty(),
                ),
            });

        let json = boundary_footprint_fragments_json(&plan);

        assert!(json.contains("\"evidence_stage\": \"encoded_machine\""));
        assert!(json.contains("\"boundary_contract_fingerprint\": \"0x0000000000001234\""));
        assert!(json.contains("\"enumeration_complete\": false"));
        assert!(json.contains("\"origin\": \"entry_storage\""));
        assert!(json.contains("\"origin\": \"entry_slice_descriptor\""));
        assert!(json.contains("\"origin\": \"exit_result_registers\""));
        assert!(json.contains("\"origin\": \"exit_indirect_result_copy\""));
        assert!(json.contains("\"origin\": \"call_return_mechanics\""));
        assert!(json.contains("\"origin\": \"dispatch_scaffold\""));
        assert!(json.contains("\"origin\": \"static_guard_comparison\""));
        assert!(json.contains("\"origin\": \"runtime_text_guard_comparison\""));
        assert!(json.contains("\"origin\": \"place_guard_comparison\""));
        assert!(json.contains("\"origin\": \"runtime_value_guard_comparison\""));
        assert!(json.contains("\"registers\": [\"X86R15\"]"));
        assert!(json.contains("\"fingerprint\": \"0x"));
    }
}

pub fn target_operations_html(
    plan: &TargetOperationPlan,
    control_flow: &ControlFlowPlan,
) -> String {
    let function_views = collect_state_function_views(
        "target block",
        plan.code.instructions.storage_slice(),
        |instruction| instruction.source_key,
        target_instruction_line,
    );
    build_backend_cfg_diagram("target_operations", control_flow, &function_views)
}

pub fn assigned_target_operations_html(
    plan: &AssignedTargetOperationPlan,
    control_flow: &ControlFlowPlan,
) -> String {
    let function_views = collect_state_function_views(
        "assigned block",
        plan.code.instructions.storage_slice(),
        |instruction| instruction.source_key,
        assigned_instruction_line,
    );
    build_backend_cfg_diagram("assigned_target_operations", control_flow, &function_views)
}

pub fn machine_instructions_html(
    plan: &MachineInstructionPlan,
    assigned_plan: &AssignedTargetOperationPlan,
    control_flow: &ControlFlowPlan,
) -> String {
    build_machine_instruction_diagram(plan, assigned_plan, control_flow)
}

pub fn emission_html(
    encoded_plan: &EncodedMachinePlan,
    machine_plan: &MachineInstructionPlan,
    assigned_plan: &AssignedTargetOperationPlan,
    control_flow: &ControlFlowPlan,
    object: &ObjectPlan,
    relocations: &RelocationPlan,
    native_disassembly: Option<&str>,
) -> String {
    let function_views = collect_emitted_function_views(
        encoded_plan,
        machine_plan,
        assigned_plan,
        object,
        relocations,
        native_disassembly,
    );
    build_emission_diagram(control_flow, &function_views)
}

fn build_emission_diagram(
    control_flow: &ControlFlowPlan,
    function_views: &[FunctionView],
) -> String {
    let mut diagram = PhaseDiagramBuilder::new("emission");
    let mut machine_nodes = Vec::new();
    let mut state_scope_nodes = Vec::new();
    let mut state_nodes = Vec::<(StateKey, String)>::new();
    let mut terminal_anchor_nodes = Vec::<(StateKey, String)>::new();

    for (machine_index, (_, machine)) in control_flow.machines.iter().enumerate() {
        let states = unique_machine_states(control_flow, machine)
            .into_iter()
            .filter(|state| function_view_by_key(function_views, state.key).is_some())
            .collect::<Vec<_>>();
        if states.is_empty() {
            continue;
        }
        let root_keys = backend_visual_root_keys(control_flow, &states);

        for root_key in &root_keys {
            let Some(root_state) = backend_state_by_key_in_slice(&states, *root_key) else {
                continue;
            };
            let machine_id = diagram.node(
                format!(
                    "machine_{}_{}",
                    machine.symbol.arena_index(),
                    root_key.state.arena_index()
                ),
                backend_machine_label(machine, root_state),
                "machine",
                machine_index + 1,
            );
            machine_nodes.push((machine.symbol, *root_key, machine_id));
        }

        for state in states.iter().copied() {
            let root_key = backend_root_key_for_state(control_flow, &states, &root_keys, state.key);
            let Some(machine_id) =
                backend_machine_id_for_root(&machine_nodes, machine.symbol, root_key)
            else {
                continue;
            };
            let function = function_view_by_key(function_views, state.key);
            let chunks = function.map(emission_chunks);
            let details = emission_state_details(control_flow, state, function, chunks.as_deref());
            let state_id = diagram.node(
                format!(
                    "state_{}_{}_{}",
                    state.key.machine.arena_index(),
                    state.key.state.arena_index(),
                    state.key.segment_index
                ),
                emission_state_label(control_flow, state, function, chunks.as_deref()),
                "state_block",
                machine_index + 1,
            );
            diagram.node_details(&state_id, details.clone());
            diagram.node_scoped_label(&state_id, details);
            diagram.containment_edge(machine_id, &state_id);
            state_nodes.push((state.key, state_id.clone()));
            state_scope_nodes.push((state.key, machine_id.to_owned()));
            terminal_anchor_nodes.push((state.key, state_id.clone()));
        }
    }

    for (_, machine) in control_flow.machines.iter() {
        for state in control_flow.states.span_or_empty(machine.states) {
            let Some(source_state_id) = state_node_id(&state_nodes, state.key) else {
                continue;
            };
            let source_anchor_id =
                state_node_id(&terminal_anchor_nodes, state.key).unwrap_or(source_state_id);
            let source_scope_id = backend_scope_id_for_state(&state_scope_nodes, state.key);

            for transition in control_flow.transitions.span_or_empty(state.transitions) {
                append_transition_edges(
                    &mut diagram,
                    control_flow,
                    &state_nodes,
                    state.key,
                    source_anchor_id,
                    transition,
                );
            }

            for operation in control_flow.operations.span_or_empty(state.operations) {
                if let Some(target_key) = operation_call_target_key(control_flow, operation) {
                    if let Some(target_id) = state_node_id(&state_nodes, target_key) {
                        let target_scope_id =
                            backend_scope_id_for_state(&state_scope_nodes, target_key);
                        if source_scope_id == target_scope_id {
                            diagram.edge(source_anchor_id, target_id, "call");
                        } else if let Some(scope_target_id) = target_scope_id {
                            append_backend_external_call_node(
                                &mut diagram,
                                state.key,
                                source_anchor_id,
                                operation,
                                scope_target_id,
                            );
                        }
                    }
                }
            }
        }
    }

    diagram.finish()
}

fn build_backend_cfg_diagram(
    title: &str,
    control_flow: &ControlFlowPlan,
    function_views: &[FunctionView],
) -> String {
    let mut diagram = PhaseDiagramBuilder::new(title);
    let mut machine_nodes = Vec::new();
    let mut state_scope_nodes = Vec::new();

    let mut state_nodes = Vec::<(StateKey, String)>::new();

    for (machine_index, (_, machine)) in control_flow.machines.iter().enumerate() {
        let states = unique_machine_states(control_flow, machine)
            .into_iter()
            .filter(|state| function_view_by_key(function_views, state.key).is_some())
            .collect::<Vec<_>>();
        if states.is_empty() {
            continue;
        }
        let root_keys = backend_visual_root_keys(control_flow, &states);

        for root_key in &root_keys {
            let Some(root_state) = backend_state_by_key_in_slice(&states, *root_key) else {
                continue;
            };
            let machine_id = diagram.node(
                format!(
                    "machine_{}_{}",
                    machine.symbol.arena_index(),
                    root_key.state.arena_index()
                ),
                backend_machine_label(machine, root_state),
                "machine",
                machine_index + 1,
            );
            machine_nodes.push((machine.symbol, *root_key, machine_id));
        }

        for state in states.iter().copied() {
            let root_key = backend_root_key_for_state(control_flow, &states, &root_keys, state.key);
            let Some(machine_id) =
                backend_machine_id_for_root(&machine_nodes, machine.symbol, root_key)
            else {
                continue;
            };
            let function = function_view_by_key(&function_views, state.key);
            let state_id = diagram.node(
                format!(
                    "state_{}_{}_{}",
                    state.key.machine.arena_index(),
                    state.key.state.arena_index(),
                    state.key.segment_index
                ),
                state_backend_label(control_flow, state, function),
                "state_block",
                machine_index + 1,
            );
            diagram.node_details(
                &state_id,
                state_backend_details(control_flow, state, function),
            );
            diagram.node_scoped_label(
                &state_id,
                state_backend_details(control_flow, state, function),
            );
            diagram.containment_edge(machine_id, &state_id);
            state_nodes.push((state.key, state_id.clone()));
            state_scope_nodes.push((state.key, machine_id.to_owned()));
        }
    }

    for (_, machine) in control_flow.machines.iter() {
        for state in control_flow.states.span_or_empty(machine.states) {
            let Some(source_state_id) = state_node_id(&state_nodes, state.key) else {
                continue;
            };
            let source_scope_id = backend_scope_id_for_state(&state_scope_nodes, state.key);

            for transition in control_flow.transitions.span_or_empty(state.transitions) {
                append_transition_edges(
                    &mut diagram,
                    control_flow,
                    &state_nodes,
                    state.key,
                    source_state_id,
                    transition,
                );
            }

            for operation in control_flow.operations.span_or_empty(state.operations) {
                if let Some(target_key) = operation_call_target_key(control_flow, operation) {
                    if let Some(target_id) = state_node_id(&state_nodes, target_key) {
                        let target_scope_id =
                            backend_scope_id_for_state(&state_scope_nodes, target_key);
                        if source_scope_id == target_scope_id {
                            diagram.edge(source_state_id, target_id, "call");
                        } else if let Some(scope_target_id) = target_scope_id {
                            append_backend_external_call_node(
                                &mut diagram,
                                state.key,
                                source_state_id,
                                operation,
                                scope_target_id,
                            );
                        }
                    }
                }
            }
        }
    }

    diagram.finish()
}

fn build_machine_instruction_diagram(
    plan: &MachineInstructionPlan,
    assigned_plan: &AssignedTargetOperationPlan,
    control_flow: &ControlFlowPlan,
) -> String {
    let mut diagram = PhaseDiagramBuilder::new("machine_instructions");
    let mut machine_nodes = Vec::new();
    let mut state_nodes = Vec::<(StateKey, String)>::new();
    let mut state_scope_nodes = Vec::<(StateKey, String)>::new();
    let function_views = collect_machine_function_views(plan, assigned_plan);

    for (machine_index, (_, machine)) in control_flow.machines.iter().enumerate() {
        let states = unique_machine_states(control_flow, machine)
            .into_iter()
            .filter(|state| function_view_by_key(&function_views, state.key).is_some())
            .collect::<Vec<_>>();
        if states.is_empty() {
            continue;
        }
        let root_keys = backend_visual_root_keys(control_flow, &states);

        for root_key in &root_keys {
            let Some(root_state) = backend_state_by_key_in_slice(&states, *root_key) else {
                continue;
            };
            let machine_id = diagram.node(
                format!(
                    "machine_{}_{}",
                    machine.symbol.arena_index(),
                    root_key.state.arena_index()
                ),
                backend_machine_label(machine, root_state),
                "machine",
                machine_index + 1,
            );
            machine_nodes.push((machine.symbol, *root_key, machine_id));
        }

        for state in states.iter().copied() {
            let root_key = backend_root_key_for_state(control_flow, &states, &root_keys, state.key);
            let Some(machine_id) =
                backend_machine_id_for_root(&machine_nodes, machine.symbol, root_key)
            else {
                continue;
            };
            let function = function_view_by_key(&function_views, state.key);
            let lines = function
                .map(|function| function.lines.clone())
                .unwrap_or_default();

            let block_title = function
                .map(|function| function.title.clone())
                .unwrap_or_else(|| "no lowered block".to_owned());
            let state_id = diagram.node(
                format!(
                    "state_{}_{}_{}",
                    state.key.machine.arena_index(),
                    state.key.state.arena_index(),
                    state.key.segment_index
                ),
                state_backend_label_from_parts(control_flow, state, &block_title, &[], &lines),
                "state_block",
                machine_index + 1,
            );
            diagram.node_details(
                &state_id,
                state_backend_details_from_parts(control_flow, state, &block_title, &[], &lines),
            );
            diagram.node_scoped_label(
                &state_id,
                state_backend_details_from_parts(control_flow, state, &block_title, &[], &lines),
            );
            diagram.containment_edge(machine_id, &state_id);
            state_nodes.push((state.key, state_id.clone()));
            state_scope_nodes.push((state.key, machine_id.to_owned()));
        }
    }

    for (_, machine) in control_flow.machines.iter() {
        for state in control_flow.states.span_or_empty(machine.states) {
            let Some(source_state_id) = state_node_id(&state_nodes, state.key) else {
                continue;
            };
            let source_scope_id = backend_scope_id_for_state(&state_scope_nodes, state.key);

            for transition in control_flow.transitions.span_or_empty(state.transitions) {
                append_transition_edges(
                    &mut diagram,
                    control_flow,
                    &state_nodes,
                    state.key,
                    source_state_id,
                    transition,
                );
            }

            for operation in control_flow.operations.span_or_empty(state.operations) {
                if let Some(target_key) = operation_call_target_key(control_flow, operation) {
                    if let Some(target_id) = state_node_id(&state_nodes, target_key) {
                        let target_scope_id =
                            backend_scope_id_for_state(&state_scope_nodes, target_key);
                        if source_scope_id == target_scope_id {
                            diagram.edge(source_state_id, target_id, "call");
                        } else if let Some(scope_target_id) = target_scope_id {
                            append_backend_external_call_node(
                                &mut diagram,
                                state.key,
                                source_state_id,
                                operation,
                                scope_target_id,
                            );
                        }
                    }
                }
            }
        }
    }

    diagram.finish()
}

struct FunctionView {
    source_key: StateKey,
    title: String,
    metadata_lines: Vec<String>,
    lines: Vec<String>,
    display_lines: Option<Vec<String>>,
    display_base_address: Option<u64>,
}
fn collect_state_function_views<Instruction>(
    title: &str,
    instructions: &[Instruction],
    instruction_source_key: impl Fn(&Instruction) -> StateKey,
    line: impl Fn(&Instruction) -> String,
) -> Vec<FunctionView> {
    let mut views = Vec::<FunctionView>::new();

    for (index, instruction) in instructions.iter().enumerate() {
        let source_key = instruction_source_key(instruction);
        let line = format!("{index:02} {}", line(instruction));
        if let Some(existing) = views.iter_mut().find(|view| view.source_key == source_key) {
            existing.lines.push(line);
        } else {
            views.push(FunctionView {
                source_key,
                title: title.to_owned(),
                metadata_lines: Vec::new(),
                lines: vec![line],
                display_lines: None,
                display_base_address: None,
            });
        }
    }

    views
}

fn collect_machine_function_views(
    plan: &MachineInstructionPlan,
    assigned_plan: &AssignedTargetOperationPlan,
) -> Vec<FunctionView> {
    let mut views = Vec::<FunctionView>::new();

    for (index, instruction) in plan.code.instructions.storage_slice().iter().enumerate() {
        let handle = psi_arena::Handle::from_arena_index(instruction.selected_instruction_index);
        if handle.arena_index() as usize >= assigned_plan.code.instructions.len() {
            continue;
        }
        let assigned_instruction = assigned_plan.code.instructions.get(handle);
        let source_key = assigned_instruction.source_key;
        let line = format!("{index:02} {}", machine_instruction_line(instruction));
        if let Some(existing) = views.iter_mut().find(|view| view.source_key == source_key) {
            existing.lines.push(line);
        } else {
            views.push(FunctionView {
                source_key,
                title: "machine block".to_owned(),
                metadata_lines: Vec::new(),
                lines: vec![line],
                display_lines: None,
                display_base_address: None,
            });
        }
    }

    views
}

fn collect_emitted_function_views(
    encoded_plan: &EncodedMachinePlan,
    machine_plan: &MachineInstructionPlan,
    assigned_plan: &AssignedTargetOperationPlan,
    object: &ObjectPlan,
    relocations: &RelocationPlan,
    native_disassembly: Option<&str>,
) -> Vec<FunctionView> {
    let encoded_instructions = encoded_plan.code.instructions.storage_slice();
    let machine_instructions = machine_plan.code.instructions.storage_slice();
    let mut current_offset = 0usize;
    let mut views = Vec::<FunctionView>::new();
    let mut first_offsets = Vec::<(StateKey, usize)>::new();
    let mut end_offsets = Vec::<(StateKey, usize)>::new();
    let native_disassembly_lines = native_disassembly.map(parse_disassembly_lines);
    let native_disassembly_base = native_disassembly_lines
        .as_ref()
        .and_then(|lines| disassembly_base_address(lines));

    for (machine_instruction, encoded_instruction) in
        machine_instructions.iter().zip(encoded_instructions.iter())
    {
        let selected_handle =
            psi_arena::Handle::from_arena_index(machine_instruction.selected_instruction_index);
        if !assigned_plan.code.instructions.is_valid(selected_handle) {
            let bytes = encoded_plan
                .code
                .bytes
                .span(encoded_instruction.bytes)
                .unwrap_or(&[]);
            current_offset += bytes.len();
            continue;
        }

        let source_key = assigned_plan
            .code
            .instructions
            .get(selected_handle)
            .source_key;
        let bytes = encoded_plan
            .code
            .bytes
            .span(encoded_instruction.bytes)
            .unwrap_or(&[]);
        let line = emitted_machine_instruction_line(
            current_offset,
            bytes,
            machine_instruction,
            relocations_for_selected_instruction(
                relocations,
                machine_instruction.selected_instruction_index,
            ),
            object,
        );
        if !first_offsets.iter().any(|(key, _)| *key == source_key) {
            first_offsets.push((source_key, current_offset));
        }
        if let Some((_, end_offset)) = end_offsets.iter_mut().find(|(key, _)| *key == source_key) {
            *end_offset = current_offset + bytes.len();
        } else {
            end_offsets.push((source_key, current_offset + bytes.len()));
        }

        if let Some(existing) = views.iter_mut().find(|view| view.source_key == source_key) {
            existing.lines.push(line);
        } else {
            views.push(FunctionView {
                source_key,
                title: "emitted block".to_owned(),
                metadata_lines: Vec::new(),
                lines: vec![line],
                display_lines: None,
                display_base_address: None,
            });
        }

        current_offset += bytes.len();
    }

    for view in &mut views {
        let Some((_, first_offset)) = first_offsets
            .iter()
            .find(|(key, _)| *key == view.source_key)
        else {
            continue;
        };
        let end_offset = end_offsets
            .iter()
            .find(|(key, _)| *key == view.source_key)
            .map(|(_, offset)| *offset)
            .unwrap_or(*first_offset);
        let byte_count = end_offset.saturating_sub(*first_offset);
        let relocation_count = relocations_in_range(relocations, *first_offset, end_offset);
        if let Some(symbol) = emitted_containing_function_symbol(object, *first_offset) {
            view.metadata_lines
                .push(format!("emitted inside: {symbol}"));
        }
        view.display_base_address = native_disassembly_base;
        view.metadata_lines.push(format!(
            "text +0x{first_offset:04x}..0x{end_offset:04x} bytes {byte_count} relocs {relocation_count}",
            first_offset = *first_offset,
        ));

        if let Some(disassembly) = native_disassembly_lines.as_deref() {
            let rendered = disassembly_lines_for_range(disassembly, *first_offset, end_offset);
            if !rendered.is_empty() {
                view.display_lines = Some(rendered);
            }
        }
    }

    views
}

fn state_backend_label(
    control_flow: &ControlFlowPlan,
    state: &StateFlow,
    function: Option<&FunctionView>,
) -> String {
    match function {
        Some(function) => state_backend_label_from_parts(
            control_flow,
            state,
            &function.title,
            &function.metadata_lines,
            &function.lines,
        ),
        None => state_backend_label_from_parts(control_flow, state, "no lowered block", &[], &[]),
    }
}

fn state_backend_label_from_parts(
    control_flow: &ControlFlowPlan,
    state: &StateFlow,
    title: &str,
    metadata_lines: &[String],
    lines_src: &[String],
) -> String {
    let mut lines = vec![
        format!(
            "{} [{}]",
            state_scoped_name(control_flow, state.key),
            state.key.segment_index
        ),
        title.to_owned(),
        state_flow_summary(control_flow, state),
    ];
    lines.extend(metadata_lines.iter().cloned());

    if lines_src.is_empty() {
        lines.push("no instructions".to_owned());
    } else {
        lines.extend(backend_instruction_summary(title, lines_src));
        lines.push(format!("instructions: {}", lines_src.len()));
        lines.push(String::new());
        lines.extend(backend_instruction_preview(lines_src));
    }

    lines.join("\n")
}

fn state_backend_details(
    control_flow: &ControlFlowPlan,
    state: &StateFlow,
    function: Option<&FunctionView>,
) -> String {
    match function {
        Some(function) => state_backend_details_from_parts(
            control_flow,
            state,
            &function.title,
            &function.metadata_lines,
            &function.lines,
        ),
        None => state_backend_details_from_parts(control_flow, state, "no lowered block", &[], &[]),
    }
}

fn state_backend_details_from_parts(
    control_flow: &ControlFlowPlan,
    state: &StateFlow,
    title: &str,
    metadata_lines: &[String],
    lines_src: &[String],
) -> String {
    let mut lines = vec![
        format!(
            "{} [{}]",
            state_scoped_name(control_flow, state.key),
            state.key.segment_index
        ),
        title.to_owned(),
        state_flow_summary(control_flow, state),
    ];
    lines.extend(metadata_lines.iter().cloned());

    let transition_summaries = backend_transition_summaries(control_flow, state);
    if !transition_summaries.is_empty() {
        lines.push("transitions".to_owned());
        lines.extend(transition_summaries);
    }

    let call_summaries = backend_call_summaries(control_flow, state);
    if !call_summaries.is_empty() {
        lines.push("calls".to_owned());
        lines.extend(call_summaries);
    }

    if !lines_src.is_empty() {
        lines.extend(backend_instruction_summary(title, lines_src));
        lines.push(format!("instructions: {}", lines_src.len()));
        let statement_count = distinct_statement_count(lines_src);
        if statement_count > 0 {
            lines.push(format!("source statements: {statement_count}"));
        }
        lines.push(String::new());
        lines.extend(group_lines_by_statement(lines_src));
    } else {
        lines.push("no instructions".to_owned());
    }

    lines.join("\n")
}

fn emission_state_label(
    control_flow: &ControlFlowPlan,
    state: &StateFlow,
    function: Option<&FunctionView>,
    chunks: Option<&[EmissionChunk]>,
) -> String {
    let mut lines = vec![
        format!(
            "{} [{}]",
            state_scoped_name(control_flow, state.key),
            state.key.segment_index
        ),
        function
            .map(|function| function.title.clone())
            .unwrap_or_else(|| "no emitted block".to_owned()),
        state_flow_summary(control_flow, state),
    ];

    if let Some(function) = function {
        let body_lines = emitted_display_lines(function);
        let chunk_count = chunks.map(|chunks| chunks.len()).unwrap_or(0);
        lines.extend(function.metadata_lines.iter().cloned());
        if let Some(call_summary) = compact_call_summary(control_flow, state)
            .or_else(|| compact_emitted_call_summary(function))
        {
            lines.push(call_summary);
        }
        lines.extend(backend_instruction_summary(
            &function.title,
            &function.lines,
        ));
        lines.push(format!("emitted chunks: {chunk_count}"));
        lines.push(format!("assembly lines: {}", body_lines.len()));
        if chunk_count > 1 {
            lines.push("scope to read full asm blocks".to_owned());
        }
        if !body_lines.is_empty() {
            lines.push(String::new());
            if chunk_count <= 1 {
                lines.extend(backend_instruction_preview(body_lines));
            } else if let Some(chunks) = chunks {
                lines.extend(emission_chunk_preview_lines(function, &chunks, 2));
            }
        }
    } else {
        lines.push("no instructions".to_owned());
    }

    lines.join("\n")
}

fn compact_call_summary(control_flow: &ControlFlowPlan, state: &StateFlow) -> Option<String> {
    let calls = backend_call_summaries(control_flow, state);
    if calls.is_empty() {
        return None;
    }
    const LIMIT: usize = 3;
    let mut parts = calls.into_iter().take(LIMIT).collect::<Vec<_>>();
    if control_flow
        .operations
        .span_or_empty(state.operations)
        .iter()
        .filter(|operation| operation_call_target_key(control_flow, operation).is_some())
        .count()
        > LIMIT
    {
        let remaining = control_flow
            .operations
            .span_or_empty(state.operations)
            .iter()
            .filter(|operation| operation_call_target_key(control_flow, operation).is_some())
            .count()
            - LIMIT;
        parts.push(format!("+{remaining} more"));
    }
    Some(format!("call targets: {}", parts.join(" | ")))
}

fn compact_emitted_call_summary(function: &FunctionView) -> Option<String> {
    let mut targets = Vec::<String>::new();
    for line in &function.lines {
        if !line.contains(": call ") {
            continue;
        }

        if let Some(relocations) = line.split("| reloc ").nth(1) {
            for note in relocations.split(", ") {
                let symbol = note
                    .split_whitespace()
                    .last()
                    .map(sanitize_emitted_call_target)
                    .filter(|target| !target.is_empty());
                if let Some(symbol) = symbol {
                    push_unique(&mut targets, symbol);
                }
            }
        }

        if targets.is_empty() {
            let fallback = line
                .split(": call ")
                .nth(1)
                .and_then(|rest| rest.split(" <- ").next())
                .map(str::trim)
                .filter(|target| !target.is_empty())
                .map(ToOwned::to_owned);
            if let Some(fallback) = fallback {
                push_unique(&mut targets, fallback);
            }
        }
    }

    if targets.is_empty() {
        for line in emitted_display_lines(function) {
            let Some(asm) = line.split(": ").nth(1) else {
                continue;
            };
            let mut parts = asm.split_whitespace();
            let Some(opcode) = parts.next() else {
                continue;
            };
            if !matches!(opcode, "bl" | "blr" | "call" | "callq") {
                continue;
            }
            let operand = parts.next().unwrap_or("?");
            push_unique(&mut targets, format!("{opcode} {operand}"));
        }
    }

    if targets.is_empty() {
        return None;
    }

    const LIMIT: usize = 3;
    let mut parts = targets.iter().take(LIMIT).cloned().collect::<Vec<_>>();
    if targets.len() > LIMIT {
        parts.push(format!("+{} more", targets.len() - LIMIT));
    }
    Some(format!("emitted calls: {}", parts.join(" | ")))
}

fn sanitize_emitted_call_target(symbol: &str) -> String {
    symbol.trim_start_matches('_').to_owned()
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn emission_state_details(
    control_flow: &ControlFlowPlan,
    state: &StateFlow,
    function: Option<&FunctionView>,
    chunks: Option<&[EmissionChunk]>,
) -> String {
    let mut lines = vec![
        format!(
            "{} [{}]",
            state_scoped_name(control_flow, state.key),
            state.key.segment_index
        ),
        function
            .map(|function| function.title.clone())
            .unwrap_or_else(|| "no emitted block".to_owned()),
        state_flow_summary(control_flow, state),
    ];

    if let Some(function) = function {
        lines.extend(function.metadata_lines.iter().cloned());
    }

    let transition_summaries = backend_transition_summaries(control_flow, state);
    if !transition_summaries.is_empty() {
        lines.push("transitions".to_owned());
        lines.extend(transition_summaries);
    }

    let call_summaries = backend_call_summaries(control_flow, state);
    if !call_summaries.is_empty() {
        lines.push("calls".to_owned());
        lines.extend(call_summaries);
    }

    if let Some(function) = function {
        let body_lines = emitted_display_lines(function);
        let chunks = chunks.unwrap_or(&[]);
        let chunk_count = chunks.len();
        lines.extend(backend_instruction_summary(
            &function.title,
            &function.lines,
        ));
        lines.push(format!("emitted chunks: {chunk_count}"));
        lines.push(format!("assembly lines: {}", body_lines.len()));
        lines.push(String::new());
        lines.extend(emission_chunk_body_lines(function, &chunks));
    } else {
        lines.push("no instructions".to_owned());
    }

    lines.join("\n")
}

fn state_flow_summary(control_flow: &ControlFlowPlan, state: &StateFlow) -> String {
    let call_count = control_flow
        .operations
        .span_or_empty(state.operations)
        .iter()
        .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .count();
    let transition_count = control_flow
        .transitions
        .span_or_empty(state.transitions)
        .len();
    format!("calls: {call_count} transitions: {transition_count}")
}

fn backend_transition_summaries(control_flow: &ControlFlowPlan, state: &StateFlow) -> Vec<String> {
    let mut lines = Vec::new();

    for (index, transition) in control_flow
        .transitions
        .span_or_empty(state.transitions)
        .iter()
        .enumerate()
    {
        if let Some(summary) =
            backend_transition_target_summary(control_flow, &transition.target, Some(state.key))
        {
            lines.push(format!("{index}. target -> {summary}"));
        }
        if let Some(summary) = backend_transition_target_summary(
            control_flow,
            &transition.continuation,
            Some(state.key),
        ) {
            lines.push(format!("{index}. continue -> {summary}"));
        }
    }

    lines
}

fn backend_call_summaries(control_flow: &ControlFlowPlan, state: &StateFlow) -> Vec<String> {
    let mut lines = Vec::new();

    for operation in control_flow.operations.span_or_empty(state.operations) {
        if let Some(target_key) = operation_call_target_key(control_flow, operation) {
            lines.push(format!(
                "#{} -> {}",
                operation.statement_index,
                state_scoped_name(control_flow, target_key)
            ));
        }
    }

    lines
}

fn distinct_statement_count(lines: &[String]) -> usize {
    let mut last = None::<usize>;
    let mut count = 0usize;
    for line in lines {
        let Some(statement) = statement_index_from_line(line) else {
            continue;
        };
        if last != Some(statement) {
            count += 1;
            last = Some(statement);
        }
    }
    count
}

fn group_lines_by_statement(lines: &[String]) -> Vec<String> {
    let mut grouped = Vec::new();
    let mut current_statement = None::<usize>;
    for line in lines {
        let statement = statement_index_from_line(line);
        if statement != current_statement {
            if !grouped.is_empty() {
                grouped.push(String::new());
            }
            if let Some(statement) = statement {
                grouped.push(format!("statement {statement}"));
            }
            current_statement = statement;
        }
        grouped.push(line.clone());
    }
    grouped
}

fn state_scoped_name(control_flow: &ControlFlowPlan, key: StateKey) -> String {
    let machine_name = control_flow
        .machine_by_symbol(key.machine)
        .map(|machine| machine.name.as_str())
        .unwrap_or("unknown_machine");
    let state_name = control_flow
        .state_by_key(key)
        .map(|state| state.name.as_str())
        .unwrap_or("unknown_state");
    format!("{machine_name}::{state_name}")
}

fn function_view_by_key(functions: &[FunctionView], key: StateKey) -> Option<&FunctionView> {
    functions.iter().find(|function| function.source_key == key)
}

fn backend_instruction_summary(title: &str, lines: &[String]) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }

    if title.starts_with("machine block") || title.starts_with("emitted block") {
        machine_block_summary(lines)
    } else {
        operation_block_summary(lines)
    }
}

fn backend_instruction_preview(lines: &[String]) -> Vec<String> {
    const PREVIEW_LINES: usize = 3;

    let preview_count = lines.len().min(PREVIEW_LINES);
    let mut preview = lines
        .iter()
        .take(preview_count)
        .cloned()
        .collect::<Vec<_>>();
    if lines.len() > PREVIEW_LINES {
        preview.push(format!(
            "... {} more in details",
            lines.len() - PREVIEW_LINES
        ));
    }
    preview
}

fn state_node_id(state_nodes: &[(StateKey, String)], key: StateKey) -> Option<&str> {
    state_nodes
        .iter()
        .find(|(state_key, _)| *state_key == key)
        .map(|(_, id)| id.as_str())
}

fn unique_machine_states<'plan>(
    plan: &'plan ControlFlowPlan,
    machine: &MachineFlow,
) -> Vec<&'plan StateFlow> {
    let mut states = Vec::new();
    for state in plan.states.span_or_empty(machine.states) {
        if states
            .iter()
            .any(|existing: &&StateFlow| existing.key == state.key)
        {
            continue;
        }
        states.push(state);
    }
    states
}

fn backend_visual_root_keys(plan: &ControlFlowPlan, states: &[&StateFlow]) -> Vec<StateKey> {
    let mut incoming = Vec::new();

    for state in states {
        for transition in plan.transitions.span_or_empty(state.transitions) {
            for target in [&transition.target, &transition.continuation] {
                if let Some(target_key) = backend_transition_target_key_in_states(states, target) {
                    if target_key != state.key && !incoming.contains(&target_key) {
                        incoming.push(target_key);
                    }
                }
            }
        }
    }

    let mut roots = states
        .iter()
        .filter(|state| !incoming.contains(&state.key))
        .map(|state| state.key)
        .collect::<Vec<_>>();
    if roots.is_empty() {
        if let Some(first) = states.first() {
            roots.push(first.key);
        }
    }
    roots
}

fn backend_root_key_for_state(
    plan: &ControlFlowPlan,
    states: &[&StateFlow],
    root_keys: &[StateKey],
    state_key: StateKey,
) -> StateKey {
    if root_keys.contains(&state_key) {
        return state_key;
    }

    for root_key in root_keys {
        if backend_reaches_state(plan, states, *root_key, state_key) {
            return *root_key;
        }
    }

    root_keys.first().copied().unwrap_or(state_key)
}

fn backend_reaches_state(
    plan: &ControlFlowPlan,
    states: &[&StateFlow],
    root_key: StateKey,
    target_key: StateKey,
) -> bool {
    let mut stack = vec![root_key];
    let mut visited = Vec::new();

    while let Some(key) = stack.pop() {
        if key == target_key {
            return true;
        }
        if visited.contains(&key) {
            continue;
        }
        visited.push(key);

        let Some(state) = backend_state_by_key_in_slice(states, key) else {
            continue;
        };
        for transition in plan.transitions.span_or_empty(state.transitions) {
            for target in [&transition.target, &transition.continuation] {
                if let Some(next_key) = backend_transition_target_key_in_states(states, target) {
                    stack.push(next_key);
                }
            }
        }
    }

    false
}

fn backend_transition_target_key_in_states(
    states: &[&StateFlow],
    target: &PlannedTransitionTarget,
) -> Option<StateKey> {
    match target {
        PlannedTransitionTarget::State { key, .. } => {
            states.iter().any(|state| state.key == *key).then_some(*key)
        }
        PlannedTransitionTarget::Nested { state_symbol, .. } => states
            .iter()
            .find(|state| state.key.state == *state_symbol)
            .map(|state| state.key),
        PlannedTransitionTarget::None
        | PlannedTransitionTarget::SelfTarget
        | PlannedTransitionTarget::Terminal => None,
    }
}

fn backend_state_by_key_in_slice<'states>(
    states: &'states [&StateFlow],
    key: StateKey,
) -> Option<&'states StateFlow> {
    states.iter().copied().find(|state| state.key == key)
}

fn backend_machine_label(machine: &MachineFlow, root_state: &StateFlow) -> String {
    format!(
        "{}\nentry slice: {} [{}]",
        machine.name.as_str(),
        root_state.name.as_str(),
        root_state.key.segment_index
    )
}

fn backend_machine_id_for_root(
    machine_nodes: &[(SymbolHandle, StateKey, String)],
    symbol: SymbolHandle,
    root_key: StateKey,
) -> Option<&str> {
    machine_nodes
        .iter()
        .find(|(machine_symbol, candidate_root_key, _)| {
            *machine_symbol == symbol && *candidate_root_key == root_key
        })
        .map(|node| node.2.as_str())
}

fn backend_scope_id_for_state(
    state_scope_nodes: &[(StateKey, String)],
    key: StateKey,
) -> Option<&str> {
    state_scope_nodes
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, id)| id.as_str())
}

fn append_backend_external_call_node(
    diagram: &mut PhaseDiagramBuilder,
    source_key: StateKey,
    source_id: &str,
    operation: &Operation,
    scope_target_id: &str,
) {
    let call_id = diagram.scoped_node(
        format!(
            "external_call_{}_{}_{}_{}",
            source_key.machine.arena_index(),
            source_key.state.arena_index(),
            source_key.segment_index,
            operation.statement_index
        ),
        format!(
            "external call\n{}\n\ndouble-click to scope target",
            backend_operation_label(operation)
        ),
        "external_call",
        source_key.machine.arena_index() as usize,
        scope_target_id,
    );
    diagram.edge(source_id, &call_id, "call");
    diagram.containment_edge(source_id, &call_id);
}

fn append_transition_edges(
    diagram: &mut PhaseDiagramBuilder,
    control_flow: &ControlFlowPlan,
    state_nodes: &[(StateKey, String)],
    source_key: StateKey,
    source_id: &str,
    transition: &TransitionFlow,
) {
    if let Some(target_key) =
        transition_target_key(control_flow, &transition.target, Some(source_key))
    {
        if let Some(target_id) = state_node_id(state_nodes, target_key) {
            let kind = if target_key == source_key {
                "transition_target_loopback"
            } else {
                "transition_target"
            };
            diagram.edge(source_id, target_id, kind);
        }
    }

    if let Some(target_key) =
        transition_target_key(control_flow, &transition.continuation, Some(source_key))
    {
        if let Some(target_id) = state_node_id(state_nodes, target_key) {
            let kind = if target_key == source_key {
                "transition_continuation_loopback"
            } else {
                "transition_continuation"
            };
            diagram.edge(source_id, target_id, kind);
        }
    }
}

fn transition_target_key(
    control_flow: &ControlFlowPlan,
    target: &PlannedTransitionTarget,
    source_key: Option<StateKey>,
) -> Option<StateKey> {
    match target {
        PlannedTransitionTarget::None | PlannedTransitionTarget::Terminal => None,
        PlannedTransitionTarget::SelfTarget => source_key,
        PlannedTransitionTarget::State { key, .. } => Some(*key),
        PlannedTransitionTarget::Nested {
            receiver_symbol,
            state_symbol,
            ..
        } => control_flow.state_key_by_symbols(*receiver_symbol, *state_symbol),
    }
}

fn backend_transition_target_summary(
    control_flow: &ControlFlowPlan,
    target: &PlannedTransitionTarget,
    source_key: Option<StateKey>,
) -> Option<String> {
    match target {
        PlannedTransitionTarget::Terminal => Some("terminal".to_owned()),
        PlannedTransitionTarget::SelfTarget => {
            source_key.map(|key| state_scoped_name(control_flow, key))
        }
        PlannedTransitionTarget::State { key, .. } => Some(state_scoped_name(control_flow, *key)),
        PlannedTransitionTarget::Nested {
            receiver_symbol,
            state_symbol,
            ..
        } => control_flow
            .state_key_by_symbols(*receiver_symbol, *state_symbol)
            .map(|key| state_scoped_name(control_flow, key)),
        PlannedTransitionTarget::None => None,
    }
}

fn operation_call_target_key(
    control_flow: &ControlFlowPlan,
    operation: &Operation,
) -> Option<StateKey> {
    let OperationKind::Call {
        receiver_symbol,
        target_symbol,
        has_receiver,
        ..
    } = operation.kind
    else {
        return None;
    };

    if has_receiver {
        return control_flow.state_key_by_symbols(receiver_symbol, target_symbol);
    }

    control_flow
        .states
        .iter()
        .find(|(_, state)| state.key.state == target_symbol)
        .map(|(_, state)| state.key)
}

fn backend_operation_label(operation: &Operation) -> String {
    match &operation.kind {
        OperationKind::Assignment => format!("#{} assignment", operation.statement_index),
        OperationKind::Call {
            has_receiver,
            receiver,
            target,
            ..
        } => {
            if *has_receiver {
                format!(
                    "#{} call {}.{}(...)",
                    operation.statement_index,
                    receiver.as_str(),
                    target.as_str()
                )
            } else {
                format!(
                    "#{} call {}(...)",
                    operation.statement_index,
                    target.as_str()
                )
            }
        }
        OperationKind::ConstantIntegerAssignment => {
            format!("#{} const-int assign", operation.statement_index)
        }
        OperationKind::Expression => format!("#{} expr", operation.statement_index),
        OperationKind::LocalData => format!("#{} local data", operation.statement_index),
        OperationKind::StaticAssignment => format!("#{} static assign", operation.statement_index),
    }
}

fn abstract_instruction_line(instruction: &AbstractOperation) -> String {
    format!(
        "{} @ statement {}",
        enum_variant_name(&instruction.kind),
        instruction.source_statement
    )
}

fn target_instruction_line(instruction: &TargetOperation) -> String {
    format!(
        "{} @ statement {}",
        enum_variant_name(&instruction.kind),
        instruction.source_statement
    )
}

fn assigned_instruction_line(instruction: &AssignedOperation) -> String {
    format!(
        "{} @ statement {}",
        enum_variant_name(&instruction.kind),
        instruction.source_statement
    )
}

fn machine_instruction_line(instruction: &MachineInstruction) -> String {
    let prefix = if machine_instruction_is_call(instruction) {
        "call"
    } else if machine_instruction_is_control(instruction) {
        "ctrl"
    } else {
        "data"
    };
    format!(
        "{prefix} {:?} <- {} #{}",
        instruction.kind,
        enum_variant_name(&instruction.source_kind),
        instruction.selected_instruction_index
    )
}

fn emitted_machine_instruction_line(
    offset: usize,
    bytes: &[u8],
    instruction: &MachineInstruction,
    relocations: Vec<&RelocationRecord>,
    object: &ObjectPlan,
) -> String {
    let bytes = if bytes.is_empty() {
        "--".to_owned()
    } else {
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut line = format!(
        "{offset:04x}: {} | {}",
        machine_instruction_line(instruction),
        bytes
    );
    if !relocations.is_empty() {
        let notes = relocations
            .into_iter()
            .map(|relocation| {
                format!(
                    "{} {}",
                    enum_variant_name(&relocation.kind),
                    emitted_symbol_name(object, relocation.symbol_handle)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        line.push_str(" | reloc ");
        line.push_str(&notes);
    }
    line
}

fn relocations_for_selected_instruction(
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
) -> Vec<&RelocationRecord> {
    relocations
        .record_set
        .records
        .iter()
        .filter(|(_, relocation)| {
            relocation.origin.selected_instruction_index() == Some(selected_instruction_index)
        })
        .map(|(_, relocation)| relocation)
        .collect()
}

fn relocations_in_range(relocations: &RelocationPlan, start: usize, end: usize) -> usize {
    relocations
        .record_set
        .records
        .iter()
        .filter(|(_, relocation)| {
            relocation.section == SectionKind::Text
                && relocation.offset >= start
                && relocation.offset < end
        })
        .count()
}

fn emitted_containing_function_symbol(object: &ObjectPlan, offset: usize) -> Option<String> {
    object
        .layout
        .symbols
        .iter()
        .find(|(_, symbol)| {
            symbol.kind == SymbolKind::Function
                && matches!(
                    symbol.section,
                    omega_object_file::SymbolSection::Section(SectionKind::Text)
                )
                && offset >= symbol.offset
                && offset < symbol.offset.saturating_add(symbol.size)
        })
        .map(|(_, symbol)| symbol.name.clone())
}

fn emitted_symbol_name(
    object: &ObjectPlan,
    symbol: omega_object_file::ObjectSymbolHandle,
) -> String {
    if object.layout.symbols.is_valid(symbol) {
        object.layout.symbols.get(symbol).name.clone()
    } else {
        "invalid".to_owned()
    }
}

fn emitted_display_lines(function: &FunctionView) -> &[String] {
    function.display_lines.as_deref().unwrap_or(&function.lines)
}

fn disassembly_lines_for_range(parsed: &[(u64, String)], start: usize, end: usize) -> Vec<String> {
    let Some(base_address) = parsed.first().map(|(address, _)| *address) else {
        return Vec::new();
    };
    let start_address = base_address.saturating_add(start as u64);
    let end_address = base_address.saturating_add(end as u64);
    parsed
        .iter()
        .filter(|(address, _)| *address >= start_address && *address < end_address)
        .map(|(address, asm)| format!("{:04x}: {}", address.saturating_sub(base_address), asm))
        .collect()
}

fn parse_disassembly_lines(disassembly: &str) -> Vec<(u64, String)> {
    disassembly
        .lines()
        .filter_map(parse_disassembly_line)
        .collect()
}

fn disassembly_base_address(parsed: &[(u64, String)]) -> Option<u64> {
    parsed.first().map(|(address, _)| *address)
}

fn parse_disassembly_line(line: &str) -> Option<(u64, String)> {
    let trimmed = line.trim_start();
    let hex_len = trimmed
        .chars()
        .take_while(|ch| ch.is_ascii_hexdigit())
        .count();
    if hex_len == 0 {
        return None;
    }

    let address = u64::from_str_radix(&trimmed[..hex_len], 16).ok()?;
    let remainder = trimmed[hex_len..]
        .trim_start_matches(|ch: char| ch == ':' || ch.is_whitespace())
        .trim();
    if remainder.is_empty() {
        return None;
    }

    let fields = remainder
        .split('\t')
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .collect::<Vec<_>>();
    let asm = if fields.is_empty() {
        remainder.trim().to_owned()
    } else if fields
        .first()
        .is_some_and(|field| looks_like_hex_byte_columns(field))
        && fields.len() >= 2
    {
        fields.last().unwrap_or(&remainder).to_string()
    } else {
        fields.join(" ")
    };
    if asm.is_empty() {
        return None;
    }

    Some((address, asm))
}

fn looks_like_hex_byte_columns(field: &str) -> bool {
    field
        .split_whitespace()
        .all(|part| part.len() == 2 && part.chars().all(|ch| ch.is_ascii_hexdigit()))
}

fn operation_block_summary(lines: &[String]) -> Vec<String> {
    let call_count = lines
        .iter()
        .filter(|line| matches!(operation_line_kind(line), OperationLineKind::Call))
        .count();
    let control_count = lines
        .iter()
        .filter(|line| {
            matches!(
                operation_line_kind(line),
                OperationLineKind::Control | OperationLineKind::Call
            )
        })
        .count();
    let terminator = lines
        .last()
        .map(|line| {
            let kind = operation_line_kind(line);
            if matches!(kind, OperationLineKind::Control | OperationLineKind::Call) {
                operation_line_head(line)
            } else {
                "fallthrough".to_owned()
            }
        })
        .unwrap_or_else(|| "none".to_owned());
    vec![
        format!("control: {control_count} calls: {call_count}"),
        format!("terminator: {terminator}"),
    ]
}

fn machine_block_summary(lines: &[String]) -> Vec<String> {
    let call_count = lines
        .iter()
        .filter(|line| matches!(machine_line_kind(line), MachineLineKind::Call))
        .count();
    let control_count = lines
        .iter()
        .filter(|line| !matches!(machine_line_kind(line), MachineLineKind::Data))
        .count();
    let terminator = lines
        .last()
        .map(|line| {
            if matches!(machine_line_kind(line), MachineLineKind::Data) {
                "fallthrough".to_owned()
            } else {
                machine_line_head(line)
            }
        })
        .unwrap_or_else(|| "none".to_owned());
    vec![
        format!("control: {control_count} calls: {call_count}"),
        format!("terminator: {terminator}"),
    ]
}

#[derive(Clone, Debug)]
struct EmissionChunk {
    index: usize,
    first_offset: usize,
    last_offset: usize,
    display_lines: Vec<String>,
    control_count: usize,
    call_count: usize,
    terminator: String,
}

fn emission_chunks(function: &FunctionView) -> Vec<EmissionChunk> {
    let display_lines = emitted_display_lines(function);
    let mut chunks = Vec::new();
    let mut current_lines = Vec::new();
    let mut first_offset = 0usize;
    let mut last_offset = 0usize;
    let mut control_count = 0usize;
    let mut call_count = 0usize;
    let mut terminator = "fallthrough".to_owned();

    for line in display_lines {
        if current_lines.is_empty() {
            first_offset = emitted_line_offset(line).unwrap_or(0);
            control_count = 0;
            call_count = 0;
            terminator = "fallthrough".to_owned();
        }

        last_offset = emitted_line_offset(line).unwrap_or(first_offset);
        current_lines.push(line.clone());
        let kind = asm_line_flow_kind(line);
        if matches!(kind, AsmLineFlowKind::Call) {
            call_count += 1;
        }
        if matches!(kind, AsmLineFlowKind::Terminator) {
            control_count += 1;
            terminator = asm_line_head(line);
            chunks.push(EmissionChunk {
                index: chunks.len(),
                first_offset,
                last_offset,
                display_lines: std::mem::take(&mut current_lines),
                control_count,
                call_count,
                terminator: terminator.clone(),
            });
        }
    }

    if !current_lines.is_empty() {
        chunks.push(EmissionChunk {
            index: chunks.len(),
            first_offset,
            last_offset,
            display_lines: current_lines,
            control_count,
            call_count,
            terminator,
        });
    }

    chunks
}

fn emission_chunk_preview_lines(
    function: &FunctionView,
    chunks: &[EmissionChunk],
    limit: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    for chunk in chunks.iter().take(limit) {
        lines.push(emission_chunk_header(function, chunks, chunk));
        lines.extend(chunk.display_lines.iter().take(3).cloned());
        if chunk.display_lines.len() > 3 {
            lines.push(format!(
                "... {} more lines in scoped block",
                chunk.display_lines.len() - 3
            ));
        }
        lines.push(String::new());
    }
    if chunks.len() > limit {
        lines.push(format!(
            "... {} more blocks in scoped view",
            chunks.len() - limit
        ));
    } else if matches!(lines.last(), Some(last) if last.is_empty()) {
        lines.pop();
    }
    lines
}

fn emission_chunk_body_lines(function: &FunctionView, chunks: &[EmissionChunk]) -> Vec<String> {
    let mut lines = Vec::new();
    for (index, chunk) in chunks.iter().take(MAX_EMISSION_DETAIL_CHUNKS).enumerate() {
        if index > 0 {
            lines.push(String::new());
        }
        lines.push(emission_chunk_header(function, chunks, chunk));
        lines.extend(
            chunk
                .display_lines
                .iter()
                .take(MAX_EMISSION_DETAIL_LINES_PER_CHUNK)
                .cloned(),
        );
        if chunk.display_lines.len() > MAX_EMISSION_DETAIL_LINES_PER_CHUNK {
            lines.push(format!(
                "... {} more assembly lines in B{}",
                chunk.display_lines.len() - MAX_EMISSION_DETAIL_LINES_PER_CHUNK,
                chunk.index
            ));
        }
    }

    if chunks.len() > MAX_EMISSION_DETAIL_CHUNKS {
        lines.push(String::new());
        lines.push(format!(
            "... {} more emitted blocks omitted from this report detail",
            chunks.len() - MAX_EMISSION_DETAIL_CHUNKS
        ));
    }
    lines
}

fn emission_chunk_header(
    function: &FunctionView,
    chunks: &[EmissionChunk],
    chunk: &EmissionChunk,
) -> String {
    format!(
        "B{} +0x{:04x}..0x{:04x}  control:{} calls:{}  terminator:{}  successors:{}",
        chunk.index,
        chunk.first_offset,
        chunk.last_offset,
        chunk.control_count,
        chunk.call_count,
        chunk.terminator,
        emission_chunk_successor_summary(function, chunks, chunk.index)
    )
}

fn emission_chunk_successor_summary(
    function: &FunctionView,
    chunks: &[EmissionChunk],
    index: usize,
) -> String {
    let successors = emission_chunk_successors(function, chunks, index);
    if successors.is_empty() {
        return "exit".to_owned();
    }
    successors
        .into_iter()
        .map(|successor| format!("B{successor}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn emission_chunk_successors(
    function: &FunctionView,
    chunks: &[EmissionChunk],
    index: usize,
) -> Vec<usize> {
    let Some(chunk) = chunks.get(index) else {
        return Vec::new();
    };
    let Some(last_line) = chunk.display_lines.last() else {
        return Vec::new();
    };
    let next_index = (index + 1 < chunks.len()).then_some(index + 1);
    match asm_terminator_kind(last_line) {
        AsmTerminatorKind::Return | AsmTerminatorKind::IndirectJump => Vec::new(),
        AsmTerminatorKind::UnconditionalBranch => {
            emission_local_branch_target(function, chunks, last_line)
                .into_iter()
                .collect()
        }
        AsmTerminatorKind::ConditionalBranch => {
            let mut successors = Vec::new();
            if let Some(target) = emission_local_branch_target(function, chunks, last_line) {
                successors.push(target);
            }
            if let Some(next_index) = next_index {
                successors.push(next_index);
            }
            successors.sort_unstable();
            successors.dedup();
            successors
        }
        AsmTerminatorKind::Fallthrough => next_index.into_iter().collect(),
    }
}

fn emission_local_branch_target(
    function: &FunctionView,
    chunks: &[EmissionChunk],
    line: &str,
) -> Option<usize> {
    let base = function.display_base_address?;
    let branch_target = branch_target_absolute_address(line)?;
    let local_target = usize::try_from(branch_target.checked_sub(base)?).ok()?;
    chunks
        .iter()
        .position(|chunk| chunk.first_offset == local_target)
}

fn branch_target_absolute_address(line: &str) -> Option<u64> {
    let marker = line.find("0x")?;
    let hex = line[marker + 2..]
        .chars()
        .take_while(|ch| ch.is_ascii_hexdigit())
        .collect::<String>();
    if hex.is_empty() {
        None
    } else {
        u64::from_str_radix(&hex, 16).ok()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AsmTerminatorKind {
    Fallthrough,
    ConditionalBranch,
    UnconditionalBranch,
    IndirectJump,
    Return,
}

fn asm_terminator_kind(line: &str) -> AsmTerminatorKind {
    let head = asm_line_head(line);
    match head.as_str() {
        "ret" | "retaa" | "retab" => AsmTerminatorKind::Return,
        "b" | "jmp" | "jmpq" => AsmTerminatorKind::UnconditionalBranch,
        "br" => AsmTerminatorKind::IndirectJump,
        head if head.starts_with("b.")
            || head.starts_with('j')
            || head.starts_with("cb")
            || head.starts_with("tb") =>
        {
            AsmTerminatorKind::ConditionalBranch
        }
        _ => AsmTerminatorKind::Fallthrough,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AsmLineFlowKind {
    Data,
    Call,
    Terminator,
}

fn asm_line_flow_kind(line: &str) -> AsmLineFlowKind {
    let head = asm_line_head(line);
    if matches!(head.as_str(), "bl" | "blr" | "call" | "callq") {
        AsmLineFlowKind::Call
    } else if head == "ret"
        || head == "retaa"
        || head == "retab"
        || head == "br"
        || head == "jmp"
        || head == "jmpq"
        || head == "b"
        || head.starts_with("b.")
        || head.starts_with('j')
        || head.starts_with("cb")
        || head.starts_with("tb")
    {
        AsmLineFlowKind::Terminator
    } else {
        AsmLineFlowKind::Data
    }
}

fn asm_line_head(line: &str) -> String {
    line.split_once(':')
        .map(|(_, rest)| rest.trim())
        .unwrap_or(line)
        .split_whitespace()
        .next()
        .unwrap_or("none")
        .to_owned()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MachineLineKind {
    Data,
    Call,
    Control,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OperationLineKind {
    Data,
    Call,
    Control,
}

fn machine_line_kind(line: &str) -> MachineLineKind {
    if line.contains(" call ") || line.contains(" call HostCallSequence ") {
        MachineLineKind::Call
    } else if line.contains(" ctrl ") || line.contains(" Dispatch") || line.contains(" Return") {
        MachineLineKind::Control
    } else {
        MachineLineKind::Data
    }
}

fn operation_line_kind(line: &str) -> OperationLineKind {
    let head = operation_line_head(line);
    if matches!(head.as_str(), "HostOperation" | "ReadRuntimeTextLine") {
        OperationLineKind::Call
    } else if head.starts_with("EnterDispatch")
        || head.starts_with("EvaluateDispatchGuard")
        || head.starts_with("SetDispatchState")
        || head.starts_with("LeaveDispatch")
        || head.starts_with("TerminateDispatch")
        || head.starts_with("LeaveFunction")
        || head.starts_with("CompareRuntime")
    {
        OperationLineKind::Control
    } else {
        OperationLineKind::Data
    }
}

fn operation_line_head(line: &str) -> String {
    let without_index = line.split_once(' ').map(|(_, rest)| rest).unwrap_or(line);
    without_index
        .split(" @ statement ")
        .next()
        .unwrap_or(without_index)
        .to_owned()
}

fn statement_index_from_line(line: &str) -> Option<usize> {
    let (_, suffix) = line.rsplit_once("@ statement ")?;
    suffix.trim().parse().ok()
}

fn machine_line_head(line: &str) -> String {
    let after_prefix = line.split_once(' ').map(|(_, rest)| rest).unwrap_or(line);
    after_prefix
        .split(" <- ")
        .next()
        .unwrap_or(after_prefix)
        .to_owned()
}

fn emitted_line_offset(line: &str) -> Option<usize> {
    let (prefix, _) = line.split_once(':')?;
    usize::from_str_radix(prefix.trim(), 16).ok()
}

fn machine_instruction_is_call(instruction: &MachineInstruction) -> bool {
    matches!(
        instruction.kind,
        omega_machine_instructions::MachineInstructionKind::HostCallSequence
    )
}

fn machine_instruction_is_control(instruction: &MachineInstruction) -> bool {
    matches!(
        instruction.kind,
        omega_machine_instructions::MachineInstructionKind::DispatchLoopEnter
            | omega_machine_instructions::MachineInstructionKind::DispatchCaseEnter
            | omega_machine_instructions::MachineInstructionKind::DispatchGuardCompareStatic
            | omega_machine_instructions::MachineInstructionKind::DispatchStateWrite
            | omega_machine_instructions::MachineInstructionKind::DispatchTerminate
            | omega_machine_instructions::MachineInstructionKind::DispatchCaseLeave
            | omega_machine_instructions::MachineInstructionKind::HostCallSequence
            | omega_machine_instructions::MachineInstructionKind::Return
    )
}

fn enum_variant_name(value: &impl Debug) -> String {
    let debug = format!("{value:?}");
    let end = debug.find([' ', '{', '(']).unwrap_or(debug.len());
    debug[..end].to_owned()
}
