use omega_calling_conventions::{
    MachineState, MachineStateSet, PlanDiagnostic, RegisterSet, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan, validate_state_footprint,
};

mod assembly;
mod control;
mod exit;
mod guards;
mod inbound;
mod place_copies;
mod place_writes;
mod runtime_io;
mod runtime_values;
mod text;
mod wire;

pub use assembly::derive_boundary_checked_assembly_footprint;
pub use control::{
    derive_boundary_call_return_mechanics_footprint, derive_boundary_dispatch_scaffold_footprint,
};
pub use exit::{
    DerivedBoundaryExit, derive_boundary_exit, derive_boundary_exit_indirect_result_copy_footprint,
    derive_boundary_exit_result_register_footprint,
};
pub use guards::{
    derive_boundary_place_guard_footprint, derive_boundary_runtime_text_guard_footprint,
    derive_boundary_runtime_value_guard_footprint, derive_boundary_static_guard_footprint,
};
pub use inbound::{
    DerivedBoundaryEntryParameterStorage, DerivedBoundaryEntryStorage,
    derive_boundary_entry_slice_descriptor_footprint, derive_boundary_entry_storage,
    derive_boundary_entry_storage_writes,
};
pub use place_copies::derive_boundary_compiler_body_place_copy_footprint;
pub use place_writes::{
    derive_boundary_compiler_body_place_address_write_footprint,
    derive_boundary_compiler_body_place_binary_write_footprint,
    derive_boundary_compiler_body_place_integer_write_footprint,
    derive_boundary_compiler_body_storage_bit_field_write_footprint,
};
pub use runtime_io::{
    derive_boundary_compiler_body_runtime_byte_read_footprint,
    derive_boundary_compiler_body_runtime_byte_write_footprint,
    derive_boundary_compiler_body_runtime_line_read_footprint,
};
pub use runtime_values::{
    derive_boundary_compiler_body_atomic_footprint,
    derive_boundary_compiler_body_storage_convert_write_footprint,
};
pub use text::{
    derive_boundary_compiler_body_place_bounded_buffer_write_footprint,
    derive_boundary_compiler_body_place_string_write_footprint,
    derive_boundary_compiler_body_text_assembly_write_footprint,
};
pub use wire::{
    derive_boundary_compiler_body_wire_byte_slice_read_footprint,
    derive_boundary_compiler_body_wire_expected_byte_read_footprint,
    derive_boundary_compiler_body_wire_literal_byte_append_footprint,
    derive_boundary_compiler_body_wire_nested_close_footprint,
    derive_boundary_compiler_body_wire_nested_open_footprint,
    derive_boundary_compiler_body_wire_repeated_scalar_varint_append_footprint,
    derive_boundary_compiler_body_wire_repeated_scalar_varint_read_footprint,
    derive_boundary_compiler_body_wire_scalar_slice_append_footprint,
    derive_boundary_compiler_body_wire_scalar_varint_append_footprint,
    derive_boundary_compiler_body_wire_scalar_varint_read_footprint,
    derive_boundary_compiler_body_wire_text_bytes_append_footprint,
};

/// Derive the exact scratch footprint of per-target constant host results.
/// These rows materialize a value directly into runtime storage and never
/// cross a foreign-call boundary.
pub fn derive_boundary_compiler_body_constant_host_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::PlatformCallData;

    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    for instruction in instructions {
        let AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands: operand_span,
        } = &instruction.kind
        else {
            continue;
        };
        let Some((_, host_call)) = input.host_calls.calls.iter().find(|(_, host_call)| {
            host_call.source_key == instruction.source_key
                && host_call.statement_index == instruction.source_statement
        }) else {
            continue;
        };
        if !matches!(host_call.data, PlatformCallData::ConstantResult { .. }) {
            continue;
        }
        let Some(operation) = input
            .host_calls
            .operations
            .span(host_call.operations)
            .and_then(|operations| operations.get(usize::from(*operation_ordinal)))
        else {
            continue;
        };
        if !operation.operation_key.lowers_to_constant_result() {
            continue;
        }
        let Some(omega_abstract_operations::InstructionOperand {
            kind:
                InstructionOperandKind::RuntimeScalarInteger {
                    byte_offset,
                    byte_count,
                    ..
                },
        }) = operands
            .span(*operand_span)
            .and_then(|operands| operands.first())
        else {
            continue;
        };
        let clobbers = match architecture {
            omega_target::Architecture::X86_64 => omega_isa_x86_64::constant_host_result_clobbers(),
            omega_target::Architecture::Aarch64 => {
                omega_isa_aarch64::constant_host_result_clobbers(*byte_offset, *byte_count)
            }
        };
        registers.extend_from_slice(clobbers.as_slice());
    }
    let evidence =
        StateFootprintEvidence::new(RegisterSet::new(registers), MachineStateSet::empty());
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the semantic leaf footprint of simple outbound syscalls. The target
/// encoder is constrained by the same retained `CallPlan`; the supervisor may
/// realize any ordinary clobber admitted by that plan.
pub fn derive_boundary_compiler_body_outbound_syscall_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::{EntryControl, HostBindingMechanism};

    let mut registers = Vec::new();
    let mut has_syscall = false;
    for instruction in instructions {
        let AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands: operand_span,
        } = &instruction.kind
        else {
            continue;
        };
        let Some((_, host_call)) = input.host_calls.calls.iter().find(|(_, host_call)| {
            host_call.source_key == instruction.source_key
                && host_call.statement_index == instruction.source_statement
        }) else {
            continue;
        };
        let Some(operation) = input
            .host_calls
            .operations
            .span(host_call.operations)
            .and_then(|operations| operations.get(usize::from(*operation_ordinal)))
        else {
            continue;
        };
        let Some((_, binding)) = input
            .host_abi
            .bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation.operation_key)
        else {
            continue;
        };
        if !matches!(binding.mechanism, HostBindingMechanism::Syscall { .. })
            || operation.operation_key.uses_linux_timespec_result()
            || operation.operation_key.uses_linux_timespec_argument()
            || (binding.call_plan().result.is_some()
                && !operation.operation_key.discards_native_result())
            || !matches!(
                binding.call_plan().entry_control,
                EntryControl::SupervisorCall { .. }
            )
            || !operands.span(*operand_span).is_some_and(|operands| {
                !operands.is_empty()
                    && operands.iter().all(|operand| {
                        matches!(
                            operand.kind,
                            InstructionOperandKind::ImmediateInteger(_)
                                | InstructionOperandKind::ByteLength(_)
                        )
                    })
            })
        {
            continue;
        }
        has_syscall = true;
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
    }
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new(registers),
        if has_syscall {
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ])
        } else {
            MachineStateSet::empty()
        },
    );
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the footprint of ordinary built-in imports whose complete source
/// boundary is a non-empty list of immediate integer arguments and no result.
/// The foreign-control envelope is part of the same instruction program, so
/// its stack/control-state writes and AArch64 x16 scratch are retained here in
/// addition to the selected call plan's ordinary foreign-call clobbers.
pub fn derive_boundary_compiler_body_outbound_immediate_import_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::Immediate,
    )
}

/// Derive the companion no-result import class that loads one or more scalar
/// arguments from runtime storage. Exact storage relocations are retained at
/// machine emission; the semantic leaf is otherwise the same wrapped foreign
/// call ceiling as the immediate-only class.
pub fn derive_boundary_compiler_body_outbound_storage_import_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::Storage,
    )
}

/// Derive integer-result built-in imports whose actual arguments are all
/// immediate integers. The leading runtime scalar is the post-call result
/// store, not a wire argument.
pub fn derive_boundary_compiler_body_outbound_immediate_import_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::ImmediateResult,
    )
}

/// Derive built-in imports with one or more runtime float parameters and a
/// direct scalar result. Integer-returning rounding and float-returning math
/// operations share the same storage/control envelope.
pub fn derive_boundary_compiler_body_outbound_float_import_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::FloatResult,
    )
}

/// Derive the built-in errno accessor shape whose imported pointer result is
/// dereferenced once before its integer value is stored. The operation has no
/// wire arguments; its leading runtime scalar is solely the result store.
pub fn derive_boundary_compiler_body_outbound_dereferenced_import_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::DereferencedResult,
    )
}

/// Derive no-result built-in imports whose ordinary scalar parameter list
/// includes at least one compiler-owned static data address.
pub fn derive_boundary_compiler_body_outbound_data_import_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::Data,
    )
}

/// Derive direct-integer-result built-in imports whose ordinary scalar
/// parameter list includes at least one compiler-owned static data address.
pub fn derive_boundary_compiler_body_outbound_data_import_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::DataResult,
    )
}

/// Derive scalar source-authored imports whose retained canonical call plan is
/// the sole placement authority. This no-result subset accepts integer and
/// compiler-owned data-address parameters.
pub fn derive_boundary_compiler_body_outbound_authored_import_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::Authored,
    )
}

/// Derive the direct-integer-result companion to scalar source-authored
/// imports. The leading runtime scalar is the result root, never an argument.
pub fn derive_boundary_compiler_body_outbound_authored_import_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::AuthoredResult,
    )
}

/// Derive source-authored no-result imports with at least one runtime-float
/// parameter. Integer and static-data parameters may share the retained plan.
pub fn derive_boundary_compiler_body_outbound_authored_float_import_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::AuthoredFloat,
    )
}

/// Derive source-authored scalar imports with a float result or at least one
/// runtime-float parameter and a direct integer/float result.
pub fn derive_boundary_compiler_body_outbound_authored_float_import_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::AuthoredFloatResult,
    )
}

/// Derive source-authored no-result imports with at least one by-value
/// aggregate parameter. The retained plan owns direct, fragmented, stack, or
/// caller-copy placement for that one source operand.
pub fn derive_boundary_compiler_body_outbound_authored_aggregate_import_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::AuthoredAggregate,
    )
}

/// Derive source-authored imports with at least one by-value aggregate
/// parameter and one direct integer/float result.
pub fn derive_boundary_compiler_body_outbound_authored_aggregate_import_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::AuthoredAggregateResult,
    )
}

/// Derive source-authored imports whose result remains one aggregate storage
/// operand while the selected plan owns its direct fragments or hidden
/// destination pointer.
pub fn derive_boundary_compiler_body_outbound_authored_aggregate_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::AuthoredAggregateReturning,
    )
}

/// Derive Darwin's concrete variadic `open(path, flags, mode)` adapter. The
/// retained call plan owns the fixed/anonymous boundary and outgoing mode slot.
pub fn derive_boundary_compiler_body_outbound_open_create_import_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::OpenCreate,
    )
}

/// Derive integer-result built-in imports with one or more runtime-scalar
/// arguments. The leading runtime scalar remains the post-call result store;
/// only the trailing operands are wire arguments.
pub fn derive_boundary_compiler_body_outbound_storage_import_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::StorageResult,
    )
}

#[derive(Clone, Copy)]
enum DirectImportArgumentClass {
    Immediate,
    Storage,
    ImmediateResult,
    FloatResult,
    DereferencedResult,
    Data,
    DataResult,
    Authored,
    AuthoredResult,
    AuthoredFloat,
    AuthoredFloatResult,
    AuthoredAggregate,
    AuthoredAggregateResult,
    AuthoredAggregateReturning,
    OpenCreate,
    StorageResult,
}

fn is_runtime_aggregate_operand(kind: &omega_abstract_operations::InstructionOperandKind) -> bool {
    matches!(
        kind,
        omega_abstract_operations::InstructionOperandKind::RuntimeHomogeneousFloatAggregate { .. }
            | omega_abstract_operations::InstructionOperandKind::RuntimeSystemVAggregate { .. }
            | omega_abstract_operations::InstructionOperandKind::RuntimeSmallAggregate { .. }
            | omega_abstract_operations::InstructionOperandKind::RuntimeLargeAggregate { .. }
    )
}

fn derive_boundary_compiler_body_outbound_direct_import_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
    argument_class: DirectImportArgumentClass,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::{
        EntryControl, HostBindingMechanism, HostCapability, MachineRegister,
    };

    let mut registers = Vec::new();
    let mut has_import = false;
    for instruction in instructions {
        let AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands: operand_span,
        } = &instruction.kind
        else {
            continue;
        };
        let Some((_, host_call)) = input.host_calls.calls.iter().find(|(_, host_call)| {
            host_call.source_key == instruction.source_key
                && host_call.statement_index == instruction.source_statement
        }) else {
            continue;
        };
        let Some(operation) = input
            .host_calls
            .operations
            .span(host_call.operations)
            .and_then(|operations| operations.get(usize::from(*operation_ordinal)))
        else {
            continue;
        };
        let Some((_, binding)) = input
            .host_abi
            .bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation.operation_key)
        else {
            continue;
        };
        let Some(selected_operands) = operands.span(*operand_span) else {
            continue;
        };
        let uses_plan_driven_mixed_import_class = matches!(
            operation.operation_key.capability,
            HostCapability::Custom(_) | HostCapability::Unknown
        ) || matches!(
            (
                operation.operation_key.capability,
                operation.operation_key.operation,
            ),
            (
                HostCapability::ObjectiveC,
                omega_calling_conventions::HostOperation::MsgSendRect
                    | omega_calling_conventions::HostOperation::MsgSendImageSize
            ) | (
                HostCapability::CoreGraphics,
                omega_calling_conventions::HostOperation::RectMaxX
                    | omega_calling_conventions::HostOperation::RectMaxY
            ) | (
                HostCapability::Clock,
                omega_calling_conventions::HostOperation::SleepPoll
            )
        );
        if !matches!(binding.mechanism, HostBindingMechanism::Import { .. })
            || uses_plan_driven_mixed_import_class
                != matches!(
                argument_class,
                DirectImportArgumentClass::Authored
                    | DirectImportArgumentClass::AuthoredResult
                    | DirectImportArgumentClass::AuthoredFloat
                    | DirectImportArgumentClass::AuthoredFloatResult
                    | DirectImportArgumentClass::AuthoredAggregate
                    | DirectImportArgumentClass::AuthoredAggregateResult
                    | DirectImportArgumentClass::AuthoredAggregateReturning
            )
            || operation.operation_key.dereferences_result()
                != matches!(
                    argument_class,
                    DirectImportArgumentClass::DereferencedResult
                )
            || (input.target.architecture == omega_target::Architecture::Aarch64
                && matches!(
                    (
                        operation.operation_key.capability,
                        operation.operation_key.operation,
                    ),
                    (
                        HostCapability::Filesystem,
                        omega_calling_conventions::HostOperation::OpenCreate
                    )
                )
                && !matches!(argument_class, DirectImportArgumentClass::OpenCreate))
            || !matches!(binding.call_plan().entry_control, EntryControl::CallReturn)
            || selected_operands.is_empty()
            || match argument_class {
                DirectImportArgumentClass::Immediate => {
                    (binding.call_plan().result.is_some()
                        && !matches!(
                            operation.operation_key.operation,
                            omega_calling_conventions::HostOperation::GetStdHandle
                        ))
                        || binding.call_plan().parameters.len() != selected_operands.len()
                        || selected_operands.iter().any(|operand| {
                            !matches!(operand.kind, InstructionOperandKind::ImmediateInteger(_))
                        })
                }
                DirectImportArgumentClass::Storage => {
                    binding.call_plan().result.is_some()
                        || binding.call_plan().parameters.len() != selected_operands.len()
                        || !selected_operands.iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::ImmediateInteger(_)
                                    | InstructionOperandKind::RuntimeScalarInteger { .. }
                            )
                        })
                        || !selected_operands.iter().any(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::RuntimeScalarInteger { .. }
                            )
                        })
                }
                DirectImportArgumentClass::ImmediateResult => {
                    let win64_out_parameter = input.target.architecture
                        == omega_target::Architecture::X86_64
                        && operation.operation_key.capability == HostCapability::Clock
                        && matches!(
                            operation.operation_key.operation,
                            omega_calling_conventions::HostOperation::MonotonicTicks
                                | omega_calling_conventions::HostOperation::MonotonicTicksPerSecond
                                | omega_calling_conventions::HostOperation::WallClockRaw
                        );
                    !binding.call_plan().result.as_ref().is_some_and(|result| {
                        matches!(
                            result.shape.class,
                            omega_calling_conventions::ValueClass::Integer
                        )
                    }) || (!win64_out_parameter
                        && binding.call_plan().parameters.len() + 1 != selected_operands.len())
                        || (win64_out_parameter && selected_operands.len() != 1)
                        || !matches!(
                            selected_operands.first().map(|operand| &operand.kind),
                            Some(InstructionOperandKind::RuntimeScalarInteger { .. })
                        )
                        || selected_operands[1..].iter().any(|operand| {
                            !matches!(operand.kind, InstructionOperandKind::ImmediateInteger(_))
                        })
                }
                DirectImportArgumentClass::FloatResult => {
                    operation.operation_key.capability != HostCapability::Math
                        || !binding.call_plan().result.as_ref().is_some_and(|result| {
                            matches!(
                                result.shape.class,
                                omega_calling_conventions::ValueClass::Integer
                                    | omega_calling_conventions::ValueClass::Float
                            )
                        })
                        || binding.call_plan().parameters.len() + 1 != selected_operands.len()
                        || !matches!(
                            selected_operands.first().map(|operand| &operand.kind),
                            Some(InstructionOperandKind::RuntimeScalarInteger { .. })
                        )
                        || selected_operands[1..].is_empty()
                        || !selected_operands[1..].iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::RuntimeScalarFloat { .. }
                            )
                        })
                }
                DirectImportArgumentClass::DereferencedResult => {
                    !binding.call_plan().result.as_ref().is_some_and(|result| {
                        matches!(
                            result.shape.class,
                            omega_calling_conventions::ValueClass::Integer
                        )
                    }) || !binding.call_plan().parameters.is_empty()
                        || selected_operands.len() != 1
                        || !matches!(
                            selected_operands.first().map(|operand| &operand.kind),
                            Some(InstructionOperandKind::RuntimeScalarInteger { .. })
                        )
                }
                DirectImportArgumentClass::Data => {
                    let win64_composite_io = input.target.architecture
                        == omega_target::Architecture::X86_64
                        && matches!(
                            (
                                operation.operation_key.capability,
                                operation.operation_key.operation,
                            ),
                            (
                                HostCapability::Stdout | HostCapability::Stderr,
                                omega_calling_conventions::HostOperation::Write
                                    | omega_calling_conventions::HostOperation::WriteFile
                            ) | (
                                HostCapability::Stdin,
                                omega_calling_conventions::HostOperation::ReadFile
                            )
                        );
                    let discards_native_result = operation
                        .operation_key
                        .discards_native_result();
                    (binding.call_plan().result.is_some()
                        && !win64_composite_io
                        && !discards_native_result)
                        || (binding.call_plan().parameters.len() != selected_operands.len()
                            && !win64_composite_io)
                        || !selected_operands.iter().any(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::DataAddress { .. }
                                    | InstructionOperandKind::RuntimeStringPointer { .. }
                                    | InstructionOperandKind::RuntimeStringLength { .. }
                                    | InstructionOperandKind::RuntimePointeeStringPointer { .. }
                                    | InstructionOperandKind::RuntimePointeeStringLength { .. }
                                    | InstructionOperandKind::RuntimeStorageAddress { .. }
                            )
                        })
                        || !selected_operands.iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::ImmediateInteger(_)
                                    | InstructionOperandKind::ByteLength(_)
                                    | InstructionOperandKind::RuntimeScalarInteger { .. }
                                    | InstructionOperandKind::DataAddress { .. }
                                    | InstructionOperandKind::RuntimeStringPointer { .. }
                                    | InstructionOperandKind::RuntimeStringLength { .. }
                                    | InstructionOperandKind::RuntimePointeeStringPointer { .. }
                                    | InstructionOperandKind::RuntimePointeeStringLength { .. }
                                    | InstructionOperandKind::RuntimeStorageAddress { .. }
                            )
                        })
                }
                DirectImportArgumentClass::DataResult => {
                    !binding.call_plan().result.as_ref().is_some_and(|result| {
                        matches!(
                            result.shape.class,
                            omega_calling_conventions::ValueClass::Integer
                        )
                    }) || binding.call_plan().parameters.len() + 1 != selected_operands.len()
                        || !matches!(
                            selected_operands.first().map(|operand| &operand.kind),
                            Some(InstructionOperandKind::RuntimeScalarInteger { .. })
                        )
                        || !selected_operands[1..].iter().any(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::DataAddress { .. }
                                    | InstructionOperandKind::RuntimeStringPointer { .. }
                                    | InstructionOperandKind::RuntimeStringLength { .. }
                                    | InstructionOperandKind::RuntimePointeeStringPointer { .. }
                                    | InstructionOperandKind::RuntimePointeeStringLength { .. }
                                    | InstructionOperandKind::RuntimeStorageAddress { .. }
                            )
                        })
                        || !selected_operands[1..].iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::ImmediateInteger(_)
                                    | InstructionOperandKind::ByteLength(_)
                                    | InstructionOperandKind::RuntimeScalarInteger { .. }
                                    | InstructionOperandKind::DataAddress { .. }
                                    | InstructionOperandKind::RuntimeStringPointer { .. }
                                    | InstructionOperandKind::RuntimeStringLength { .. }
                                    | InstructionOperandKind::RuntimePointeeStringPointer { .. }
                                    | InstructionOperandKind::RuntimePointeeStringLength { .. }
                                    | InstructionOperandKind::RuntimeStorageAddress { .. }
                            )
                        })
                }
                DirectImportArgumentClass::Authored => {
                    (binding.call_plan().result.is_some()
                        && !operation.operation_key.discards_native_result())
                        || binding.call_plan().parameters.len() != selected_operands.len()
                        || !selected_operands.iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::ImmediateInteger(_)
                                    | InstructionOperandKind::RuntimeScalarInteger { .. }
                                    | InstructionOperandKind::DataAddress { .. }
                                    | InstructionOperandKind::RuntimeStorageAddress { .. }
                            )
                        })
                }
                DirectImportArgumentClass::AuthoredResult => {
                    !binding.call_plan().result.as_ref().is_some_and(|result| {
                        matches!(
                            result.shape.class,
                            omega_calling_conventions::ValueClass::Integer
                        )
                    }) || binding.call_plan().parameters.len() + 1 != selected_operands.len()
                        || !matches!(
                            selected_operands.first().map(|operand| &operand.kind),
                            Some(InstructionOperandKind::RuntimeScalarInteger { .. })
                        )
                        || !selected_operands[1..].iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::ImmediateInteger(_)
                                    | InstructionOperandKind::RuntimeScalarInteger { .. }
                                    | InstructionOperandKind::DataAddress { .. }
                                    | InstructionOperandKind::RuntimeStorageAddress { .. }
                            )
                        })
                }
                DirectImportArgumentClass::AuthoredFloat => {
                    binding.call_plan().result.is_some()
                        || binding.call_plan().parameters.len() != selected_operands.len()
                        || !selected_operands.iter().any(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::RuntimeScalarFloat { .. }
                            )
                        })
                        || !selected_operands.iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::ImmediateInteger(_)
                                    | InstructionOperandKind::RuntimeScalarInteger { .. }
                                    | InstructionOperandKind::RuntimeScalarFloat { .. }
                                    | InstructionOperandKind::DataAddress { .. }
                            )
                        })
                }
                DirectImportArgumentClass::AuthoredFloatResult => {
                    binding.call_plan().result.as_ref().map_or(true, |result| {
                        !matches!(
                            result.shape.class,
                            omega_calling_conventions::ValueClass::Integer
                                | omega_calling_conventions::ValueClass::Float
                            ) || binding.call_plan().parameters.len() + 1 != selected_operands.len()
                            || match result.shape.class {
                                omega_calling_conventions::ValueClass::Integer => !matches!(
                                    selected_operands.first().map(|operand| &operand.kind),
                                    Some(InstructionOperandKind::RuntimeScalarInteger { .. })
                                ),
                                omega_calling_conventions::ValueClass::Float => !matches!(
                                    selected_operands.first().map(|operand| &operand.kind),
                                    Some(
                                        InstructionOperandKind::RuntimeScalarInteger { .. }
                                            | InstructionOperandKind::RuntimeScalarFloat { .. }
                                    )
                                ),
                                _ => true,
                            }
                            || (!matches!(
                                result.shape.class,
                                omega_calling_conventions::ValueClass::Float
                            ) && !selected_operands[1..].iter().any(|operand| {
                                matches!(
                                    operand.kind,
                                    InstructionOperandKind::RuntimeScalarFloat { .. }
                                )
                            }))
                            || !selected_operands[1..].iter().all(|operand| {
                                matches!(
                                    operand.kind,
                                    InstructionOperandKind::ImmediateInteger(_)
                                        | InstructionOperandKind::RuntimeScalarInteger { .. }
                                        | InstructionOperandKind::RuntimeScalarFloat { .. }
                                        | InstructionOperandKind::DataAddress { .. }
                                )
                            })
                    })
                }
                DirectImportArgumentClass::AuthoredAggregate => {
                    binding.call_plan().result.is_some()
                        || binding.call_plan().parameters.len() != selected_operands.len()
                        || !selected_operands
                            .iter()
                            .any(|operand| is_runtime_aggregate_operand(&operand.kind))
                        || !selected_operands.iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::ImmediateInteger(_)
                                    | InstructionOperandKind::RuntimeScalarInteger { .. }
                                    | InstructionOperandKind::RuntimeScalarFloat { .. }
                                    | InstructionOperandKind::RuntimeHomogeneousFloatAggregate { .. }
                                    | InstructionOperandKind::RuntimeSystemVAggregate { .. }
                                    | InstructionOperandKind::RuntimeSmallAggregate { .. }
                                    | InstructionOperandKind::RuntimeLargeAggregate { .. }
                                    | InstructionOperandKind::DataAddress { .. }
                            )
                        })
                }
                DirectImportArgumentClass::AuthoredAggregateResult => {
                    binding.call_plan().result.as_ref().map_or(true, |result| {
                        !matches!(
                            result.shape.class,
                            omega_calling_conventions::ValueClass::Integer
                                | omega_calling_conventions::ValueClass::Float
                        ) || binding.call_plan().parameters.len() + 1 != selected_operands.len()
                            || match result.shape.class {
                                omega_calling_conventions::ValueClass::Integer => !matches!(
                                    selected_operands.first().map(|operand| &operand.kind),
                                    Some(InstructionOperandKind::RuntimeScalarInteger { .. })
                                ),
                                omega_calling_conventions::ValueClass::Float => !matches!(
                                    selected_operands.first().map(|operand| &operand.kind),
                                    Some(InstructionOperandKind::RuntimeScalarFloat { .. })
                                ),
                                _ => true,
                            }
                            || !selected_operands[1..]
                                .iter()
                                .any(|operand| is_runtime_aggregate_operand(&operand.kind))
                            || !selected_operands[1..].iter().all(|operand| {
                                matches!(
                                    operand.kind,
                                    InstructionOperandKind::ImmediateInteger(_)
                                        | InstructionOperandKind::RuntimeScalarInteger { .. }
                                        | InstructionOperandKind::RuntimeScalarFloat { .. }
                                        | InstructionOperandKind::RuntimeHomogeneousFloatAggregate { .. }
                                        | InstructionOperandKind::RuntimeSystemVAggregate { .. }
                                        | InstructionOperandKind::RuntimeSmallAggregate { .. }
                                        | InstructionOperandKind::RuntimeLargeAggregate { .. }
                                        | InstructionOperandKind::DataAddress { .. }
                                )
                            })
                    })
                }
                DirectImportArgumentClass::AuthoredAggregateReturning => {
                    binding.call_plan().result.is_none()
                        || binding.call_plan().parameters.len() + 1 != selected_operands.len()
                        || !selected_operands
                            .first()
                            .is_some_and(|operand| is_runtime_aggregate_operand(&operand.kind))
                        || !selected_operands[1..].iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::ImmediateInteger(_)
                                    | InstructionOperandKind::RuntimeScalarInteger { .. }
                                    | InstructionOperandKind::RuntimeScalarFloat { .. }
                                    | InstructionOperandKind::RuntimeHomogeneousFloatAggregate { .. }
                                    | InstructionOperandKind::RuntimeSystemVAggregate { .. }
                                    | InstructionOperandKind::RuntimeSmallAggregate { .. }
                                    | InstructionOperandKind::RuntimeLargeAggregate { .. }
                                    | InstructionOperandKind::DataAddress { .. }
                            )
                        })
                }
                DirectImportArgumentClass::OpenCreate => {
                    input.target.architecture != omega_target::Architecture::Aarch64
                        || !matches!(
                            (
                                operation.operation_key.capability,
                                operation.operation_key.operation,
                            ),
                            (
                                HostCapability::Filesystem,
                                omega_calling_conventions::HostOperation::OpenCreate
                            )
                        )
                        || !binding.call_plan().result.as_ref().is_some_and(|result| {
                            matches!(
                                result.shape.class,
                                omega_calling_conventions::ValueClass::Integer
                            )
                        })
                        || binding.call_plan().parameters.len() != 3
                        || !matches!(
                            selected_operands,
                            [
                                omega_abstract_operations::InstructionOperand {
                                    kind: InstructionOperandKind::RuntimeScalarInteger { .. }
                                },
                                omega_abstract_operations::InstructionOperand {
                                    kind: InstructionOperandKind::DataAddress { .. }
                                        | InstructionOperandKind::RuntimeStringPointer { .. }
                                        | InstructionOperandKind::RuntimePointeeStringPointer { .. }
                                        | InstructionOperandKind::RuntimeStorageAddress { .. }
                                },
                                omega_abstract_operations::InstructionOperand {
                                    kind: InstructionOperandKind::ImmediateInteger(_)
                                        | InstructionOperandKind::RuntimeScalarInteger { .. }
                                },
                                omega_abstract_operations::InstructionOperand {
                                    kind: InstructionOperandKind::ImmediateInteger(_)
                                },
                            ]
                        )
                }
                DirectImportArgumentClass::StorageResult => {
                    !binding.call_plan().result.as_ref().is_some_and(|result| {
                        matches!(
                            result.shape.class,
                            omega_calling_conventions::ValueClass::Integer
                        )
                    }) || binding.call_plan().parameters.len() + 1 != selected_operands.len()
                        || !matches!(
                            selected_operands.first().map(|operand| &operand.kind),
                            Some(InstructionOperandKind::RuntimeScalarInteger { .. })
                        )
                        || !selected_operands[1..].iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::ImmediateInteger(_)
                                    | InstructionOperandKind::RuntimeScalarInteger { .. }
                            )
                        })
                        || !selected_operands[1..].iter().any(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::RuntimeScalarInteger { .. }
                            )
                        })
                }
            }
        {
            continue;
        }
        has_import = true;
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
        match input.target.architecture {
            omega_target::Architecture::X86_64 => registers.push(MachineRegister::X86Rsp),
            omega_target::Architecture::Aarch64 => {
                registers.push(MachineRegister::Aarch64X(16));
                if matches!(
                    argument_class,
                    DirectImportArgumentClass::ImmediateResult
                        | DirectImportArgumentClass::FloatResult
                        | DirectImportArgumentClass::DereferencedResult
                        | DirectImportArgumentClass::DataResult
                        | DirectImportArgumentClass::AuthoredResult
                        | DirectImportArgumentClass::AuthoredFloatResult
                        | DirectImportArgumentClass::AuthoredAggregateResult
                        | DirectImportArgumentClass::OpenCreate
                        | DirectImportArgumentClass::StorageResult
                ) {
                    let result_range = selected_operands.first().and_then(|operand| match &operand
                        .kind
                    {
                        InstructionOperandKind::RuntimeScalarInteger {
                            byte_offset,
                            byte_count,
                            ..
                        }
                        | InstructionOperandKind::RuntimeScalarFloat {
                            byte_offset,
                            byte_count,
                            ..
                        } => Some((*byte_offset, *byte_count)),
                        _ => None,
                    });
                    if let Some((byte_offset, byte_count)) = result_range {
                        registers.extend_from_slice(
                            omega_isa_aarch64::constant_host_result_clobbers(
                                byte_offset,
                                byte_count,
                            )
                            .as_slice(),
                        );
                    }
                }
            }
        }
    }
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new(registers),
        if has_import {
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ])
        } else {
            MachineStateSet::empty()
        },
    );
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the compiler-owned footprint for indirect foreign calls whose
/// callee is loaded from a retained vtable or service-table mechanism. These
/// calls have no import relocation, but otherwise own the same call/return
/// machine-state envelope as direct imports.
pub fn derive_boundary_compiler_body_outbound_indirect_call_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::{EntryControl, HostBindingMechanism, MachineRegister};

    let mut registers = Vec::new();
    let mut has_call = false;
    for instruction in instructions {
        let AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands: operand_span,
        } = &instruction.kind
        else {
            continue;
        };
        let Some((_, host_call)) = input.host_calls.calls.iter().find(|(_, host_call)| {
            host_call.source_key == instruction.source_key
                && host_call.statement_index == instruction.source_statement
        }) else {
            continue;
        };
        let Some(operation) = input
            .host_calls
            .operations
            .span(host_call.operations)
            .and_then(|operations| operations.get(usize::from(*operation_ordinal)))
        else {
            continue;
        };
        let Some((_, binding)) = input
            .host_abi
            .bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation.operation_key)
        else {
            continue;
        };
        let Some(selected_operands) = operands.span(*operand_span) else {
            continue;
        };
        let dispatch_only = match binding.mechanism {
            HostBindingMechanism::VtableSlot { .. } | HostBindingMechanism::VtableField { .. } => 0,
            HostBindingMechanism::TableFunction { .. } => 1,
            _ => continue,
        };
        if !matches!(binding.call_plan().entry_control, EntryControl::CallReturn)
            || selected_operands.is_empty()
        {
            continue;
        }
        let parameter_count = binding.call_plan().parameters.len() + dispatch_only;
        let result_present = selected_operands.len() == parameter_count + 1;
        if selected_operands.len() != parameter_count && !result_present {
            continue;
        }
        has_call = true;
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
        match input.target.architecture {
            omega_target::Architecture::X86_64 => registers.push(MachineRegister::X86Rsp),
            omega_target::Architecture::Aarch64 => {
                registers.push(MachineRegister::Aarch64X(16));
                if result_present {
                    let result_range = selected_operands.first().and_then(|operand| match &operand
                        .kind
                    {
                        InstructionOperandKind::RuntimeScalarInteger {
                            byte_offset,
                            byte_count,
                            ..
                        }
                        | InstructionOperandKind::RuntimeScalarFloat {
                            byte_offset,
                            byte_count,
                            ..
                        } => Some((*byte_offset, *byte_count)),
                        _ => None,
                    });
                    if let Some((byte_offset, byte_count)) = result_range {
                        registers.extend_from_slice(
                            omega_isa_aarch64::constant_host_result_clobbers(
                                byte_offset,
                                byte_count,
                            )
                            .as_slice(),
                        );
                    }
                }
            }
        }
    }
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new(registers),
        if has_call {
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ])
        } else {
            MachineStateSet::empty()
        },
    );
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}

fn abstract_outbound_syscall_storage_argument_is_closed(
    _architecture: omega_target::Architecture,
    operand: &omega_abstract_operations::InstructionOperand,
) -> bool {
    use omega_abstract_operations::InstructionOperandKind;

    match operand.kind {
        InstructionOperandKind::RuntimeStringPointer { .. }
        | InstructionOperandKind::RuntimeStringLength { .. }
        | InstructionOperandKind::RuntimePointeeStringPointer { .. }
        | InstructionOperandKind::RuntimePointeeStringLength { .. }
        | InstructionOperandKind::RuntimeScalarInteger { .. }
        | InstructionOperandKind::RuntimeStorageAddress { .. } => true,
        _ => false,
    }
}

fn abstract_outbound_syscall_data_argument_is_closed(
    operand: &omega_abstract_operations::InstructionOperand,
) -> bool {
    matches!(
        operand.kind,
        omega_abstract_operations::InstructionOperandKind::DataAddress { .. }
    )
}

/// Derive no-result outbound syscall leaves that marshal one or more values,
/// descriptor fields, or addresses from runtime storage. Their marshallers use
/// only the normalized syscall plan's ordinary-clobber set; exact storage
/// relocations are retained later beside the encoded instruction.
pub fn derive_boundary_compiler_body_outbound_syscall_storage_arguments_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_relocatable_arguments_footprint(
        boundary,
        input,
        operands,
        instructions,
        false,
    )
}

/// Derive no-result outbound syscall leaves with at least one exact static
/// data-object address. Other parameters may be immediate or use the already
/// closed runtime-storage forms; the final validator retains both relocation
/// target classes independently.
pub fn derive_boundary_compiler_body_outbound_syscall_data_arguments_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_relocatable_arguments_footprint(
        boundary,
        input,
        operands,
        instructions,
        true,
    )
}

fn derive_boundary_compiler_body_outbound_syscall_relocatable_arguments_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
    requires_data_argument: bool,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::{
        EntryControl, HostBindingMechanism, HostOperation, HostOperationKey,
    };

    let mut registers = Vec::new();
    let mut has_syscall = false;
    for instruction in instructions {
        let Some((operation_key, operand_span)) = (match &instruction.kind {
            AbstractOperationKind::HostOperation {
                operation_ordinal,
                operands,
            } => input
                .host_calls
                .calls
                .iter()
                .find(|(_, host_call)| {
                    host_call.source_key == instruction.source_key
                        && host_call.statement_index == instruction.source_statement
                })
                .and_then(|(_, host_call)| {
                    input
                        .host_calls
                        .operations
                        .span(host_call.operations)
                        .and_then(|operations| operations.get(usize::from(*operation_ordinal)))
                })
                .map(|operation| (operation.operation_key, *operands)),
            AbstractOperationKind::WritePlatformNewline {
                capability,
                use_file_api,
                operands,
            } => Some((
                HostOperationKey::new(
                    *capability,
                    if *use_file_api {
                        HostOperation::WriteFile
                    } else {
                        HostOperation::Write
                    },
                ),
                *operands,
            )),
            _ => None,
        }) else {
            continue;
        };
        let Some((_, binding)) = input
            .host_abi
            .bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation_key)
        else {
            continue;
        };
        let Some(arguments) = operands.span(operand_span) else {
            continue;
        };
        let has_storage = arguments.iter().any(|operand| {
            abstract_outbound_syscall_storage_argument_is_closed(input.target.architecture, operand)
        });
        let has_data = arguments
            .iter()
            .any(abstract_outbound_syscall_data_argument_is_closed);
        if !matches!(binding.mechanism, HostBindingMechanism::Syscall { .. })
            || operation_key.uses_linux_timespec_result()
            || operation_key.uses_linux_timespec_argument()
            || (binding.call_plan().result.is_some() && !operation_key.discards_native_result())
            || binding.call_plan().parameters.len() != arguments.len()
            || !matches!(
                binding.call_plan().entry_control,
                EntryControl::SupervisorCall { .. }
            )
            || if requires_data_argument {
                !has_data
            } else {
                !has_storage || has_data
            }
            || !arguments.iter().all(|operand| {
                matches!(
                    operand.kind,
                    InstructionOperandKind::ImmediateInteger(_)
                        | InstructionOperandKind::ByteLength(_)
                ) || abstract_outbound_syscall_storage_argument_is_closed(
                    input.target.architecture,
                    operand,
                ) || abstract_outbound_syscall_data_argument_is_closed(operand)
            })
        {
            continue;
        }
        has_syscall = true;
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
    }
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new(registers),
        if has_syscall {
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ])
        } else {
            MachineStateSet::empty()
        },
    );
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the first result-bearing outbound syscall leaf. This deliberately
/// covers only a runtime-scalar destination followed by immediate/byte-length
/// parameters; relocatable parameters and composite adapters retain separate
/// footprint classes. AArch64's post-call store owns x16 and, for a large or
/// unscaled destination offset, x17 in addition to the syscall plan ceiling.
pub fn derive_boundary_compiler_body_outbound_syscall_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_result_footprint_for_arguments(
        boundary,
        input,
        operands,
        instructions,
        OutboundSyscallResultArgumentClass::Immediate,
    )
}

/// Derive result-bearing outbound syscalls whose ordinary parameters include
/// one or more of the closed runtime-storage forms. The plan still owns the
/// syscall marshaller; AArch64's post-call destination materializer contributes
/// its offset-sensitive x16/x17 scratch separately.
pub fn derive_boundary_compiler_body_outbound_syscall_result_storage_arguments_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_result_footprint_for_arguments(
        boundary,
        input,
        operands,
        instructions,
        OutboundSyscallResultArgumentClass::Storage,
    )
}

/// Derive result-bearing outbound syscall leaves with at least one exact
/// static data-object address and any otherwise-closed runtime-storage or
/// immediate parameters.
pub fn derive_boundary_compiler_body_outbound_syscall_result_data_arguments_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_result_footprint_for_arguments(
        boundary,
        input,
        operands,
        instructions,
        OutboundSyscallResultArgumentClass::Data,
    )
}

#[derive(Clone, Copy)]
enum OutboundSyscallResultArgumentClass {
    Immediate,
    Storage,
    Data,
}

#[derive(Clone, Copy)]
enum OutboundSyscallTimespecClass {
    Argument,
    Result,
}

/// Derive the Linux nanosleep adapter leaf. The concrete two-pointer syscall
/// plan owns the supervisor boundary while the compiler-owned request builder
/// additionally mutates balanced stack state and target-specific arithmetic
/// scratch.
pub fn derive_boundary_compiler_body_outbound_syscall_timespec_argument_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_timespec_footprint(
        boundary,
        input,
        operands,
        instructions,
        OutboundSyscallTimespecClass::Argument,
    )
}

/// Derive the Linux clock_gettime adapter leaf. Its private two-word result is
/// reduced to nanoseconds and stored into the semantic scalar destination.
pub fn derive_boundary_compiler_body_outbound_syscall_timespec_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_timespec_footprint(
        boundary,
        input,
        operands,
        instructions,
        OutboundSyscallTimespecClass::Result,
    )
}

fn derive_boundary_compiler_body_outbound_syscall_timespec_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
    class: OutboundSyscallTimespecClass,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::{EntryControl, HostBindingMechanism, MachineRegister};

    let mut registers = Vec::new();
    let mut has_syscall = false;
    for instruction in instructions {
        let AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands: operand_span,
        } = &instruction.kind
        else {
            continue;
        };
        let Some((_, host_call)) = input.host_calls.calls.iter().find(|(_, host_call)| {
            host_call.source_key == instruction.source_key
                && host_call.statement_index == instruction.source_statement
        }) else {
            continue;
        };
        let Some(operation) = input
            .host_calls
            .operations
            .span(host_call.operations)
            .and_then(|operations| operations.get(usize::from(*operation_ordinal)))
        else {
            continue;
        };
        let Some((_, binding)) = input
            .host_abi
            .bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation.operation_key)
        else {
            continue;
        };
        let Some(call_operands) = operands.span(*operand_span) else {
            continue;
        };
        let shape_matches = match (class, call_operands) {
            (
                OutboundSyscallTimespecClass::Result,
                [
                    omega_abstract_operations::InstructionOperand {
                        kind: InstructionOperandKind::RuntimeScalarInteger { byte_count: 8, .. },
                    },
                    omega_abstract_operations::InstructionOperand {
                        kind: InstructionOperandKind::ImmediateInteger(_),
                    },
                ],
            ) => true,
            (
                OutboundSyscallTimespecClass::Argument,
                [
                    omega_abstract_operations::InstructionOperand {
                        kind:
                            InstructionOperandKind::RuntimeScalarInteger {
                                byte_count: 4 | 8, ..
                            }
                            | InstructionOperandKind::ImmediateInteger(0..),
                    },
                ],
            ) => true,
            _ => false,
        };
        let operation_matches = match class {
            OutboundSyscallTimespecClass::Argument => {
                operation.operation_key.uses_linux_timespec_argument()
            }
            OutboundSyscallTimespecClass::Result => {
                operation.operation_key.uses_linux_timespec_result()
            }
        };
        if !operation_matches
            || !shape_matches
            || !matches!(binding.mechanism, HostBindingMechanism::Syscall { .. })
            || binding.call_plan().parameters.len() != 2
            || binding.call_plan().result.is_none()
            || !matches!(
                binding.call_plan().entry_control,
                EntryControl::SupervisorCall { .. }
            )
        {
            continue;
        }
        has_syscall = true;
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
        match (input.target.architecture, class, call_operands) {
            (omega_target::Architecture::X86_64, OutboundSyscallTimespecClass::Result, _) => {
                registers.push(MachineRegister::X86Rsp)
            }
            (omega_target::Architecture::X86_64, OutboundSyscallTimespecClass::Argument, _) => {
                registers.extend([MachineRegister::X86Rdx, MachineRegister::X86Rsp])
            }
            (
                omega_target::Architecture::Aarch64,
                OutboundSyscallTimespecClass::Result,
                [
                    omega_abstract_operations::InstructionOperand {
                        kind:
                            InstructionOperandKind::RuntimeScalarInteger {
                                byte_offset,
                                byte_count,
                                ..
                            },
                    },
                    _,
                ],
            ) => registers.extend_from_slice(
                omega_isa_aarch64::constant_host_result_clobbers(*byte_offset, *byte_count)
                    .as_slice(),
            ),
            _ => {}
        }
    }
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new(registers),
        if has_syscall {
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ])
        } else {
            MachineStateSet::empty()
        },
    );
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}

fn derive_boundary_compiler_body_outbound_syscall_result_footprint_for_arguments(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
    argument_class: OutboundSyscallResultArgumentClass,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::{EntryControl, HostBindingMechanism};

    let mut registers = Vec::new();
    let mut has_syscall = false;
    for instruction in instructions {
        let AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands: operand_span,
        } = &instruction.kind
        else {
            continue;
        };
        let Some((_, host_call)) = input.host_calls.calls.iter().find(|(_, host_call)| {
            host_call.source_key == instruction.source_key
                && host_call.statement_index == instruction.source_statement
        }) else {
            continue;
        };
        let Some(operation) = input
            .host_calls
            .operations
            .span(host_call.operations)
            .and_then(|operations| operations.get(usize::from(*operation_ordinal)))
        else {
            continue;
        };
        let Some((_, binding)) = input
            .host_abi
            .bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation.operation_key)
        else {
            continue;
        };
        let Some(call_operands) = operands.span(*operand_span) else {
            continue;
        };
        let Some((result, arguments)) = call_operands.split_first() else {
            continue;
        };
        let InstructionOperandKind::RuntimeScalarInteger {
            byte_offset,
            byte_count,
            ..
        } = &result.kind
        else {
            continue;
        };
        let has_storage_argument = arguments.iter().any(|operand| {
            abstract_outbound_syscall_storage_argument_is_closed(input.target.architecture, operand)
        });
        let has_data_argument = arguments
            .iter()
            .any(abstract_outbound_syscall_data_argument_is_closed);
        if !matches!(binding.mechanism, HostBindingMechanism::Syscall { .. })
            || operation.operation_key.uses_linux_timespec_result()
            || operation.operation_key.uses_linux_timespec_argument()
            || binding.call_plan().result.is_none()
            || operation.operation_key.discards_native_result()
            || binding.call_plan().parameters.len() != arguments.len()
            || !matches!(
                binding.call_plan().entry_control,
                EntryControl::SupervisorCall { .. }
            )
            || !match argument_class {
                OutboundSyscallResultArgumentClass::Immediate => {
                    !has_storage_argument && !has_data_argument
                }
                OutboundSyscallResultArgumentClass::Storage => {
                    has_storage_argument && !has_data_argument
                }
                OutboundSyscallResultArgumentClass::Data => has_data_argument,
            }
            || !arguments.iter().all(|operand| {
                matches!(
                    operand.kind,
                    InstructionOperandKind::ImmediateInteger(_)
                        | InstructionOperandKind::ByteLength(_)
                ) || abstract_outbound_syscall_storage_argument_is_closed(
                    input.target.architecture,
                    operand,
                ) || abstract_outbound_syscall_data_argument_is_closed(operand)
            })
        {
            continue;
        }
        has_syscall = true;
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
        if input.target.architecture == omega_target::Architecture::Aarch64 {
            registers.extend_from_slice(
                omega_isa_aarch64::constant_host_result_clobbers(*byte_offset, *byte_count)
                    .as_slice(),
            );
        }
    }
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new(registers),
        if has_syscall {
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ])
        } else {
            MachineStateSet::empty()
        },
    );
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_abstract_operations::SelectedInstructionKind;
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, MachineRegime, MachineRegister, MachineState, ValueLocation,
        ValueShape, evaluate_ordinary_boundary_entry_plan,
    };

    #[test]
    fn outbound_syscall_storage_arguments_close_over_runtime_address_shapes() {
        use omega_abstract_operations::{
            InstructionOperand, InstructionOperandKind, RuntimeStorageRegion,
        };

        let runtime_address = InstructionOperand {
            kind: InstructionOperandKind::RuntimeStorageAddress {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 64,
            },
        };
        let descriptor_length = InstructionOperand {
            kind: InstructionOperandKind::RuntimePointeeStringLength {
                region: RuntimeStorageRegion::Machine,
                byte_offset: 32,
            },
        };
        let bounded_small_offset = InstructionOperand {
            kind: InstructionOperandKind::RuntimeStringPointer {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 4087,
                is_bounded_buffer: true,
            },
        };
        let bounded_large_offset = InstructionOperand {
            kind: InstructionOperandKind::RuntimeStringPointer {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 4088,
                is_bounded_buffer: true,
            },
        };
        let data_address = InstructionOperand {
            kind: InstructionOperandKind::DataAddress {
                data: psi_arena::Handle::invalid(),
            },
        };

        for operand in [&runtime_address, &descriptor_length] {
            assert!(abstract_outbound_syscall_storage_argument_is_closed(
                omega_target::Architecture::X86_64,
                operand,
            ));
            assert!(abstract_outbound_syscall_storage_argument_is_closed(
                omega_target::Architecture::Aarch64,
                operand,
            ));
        }
        assert!(abstract_outbound_syscall_storage_argument_is_closed(
            omega_target::Architecture::Aarch64,
            &bounded_small_offset,
        ));
        assert!(abstract_outbound_syscall_storage_argument_is_closed(
            omega_target::Architecture::Aarch64,
            &bounded_large_offset,
        ));
        assert!(abstract_outbound_syscall_storage_argument_is_closed(
            omega_target::Architecture::X86_64,
            &bounded_large_offset,
        ));
        assert!(abstract_outbound_syscall_data_argument_is_closed(
            &data_address,
        ));
        assert!(!abstract_outbound_syscall_storage_argument_is_closed(
            omega_target::Architecture::X86_64,
            &data_address,
        ));
    }

    #[test]
    fn inbound_writes_consume_the_exact_selected_register() {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        };
        let mut boundary =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
                .expect("SysV boundary")
                .plan()
                .clone();
        let ValueLocation::Register { register, .. } =
            &mut boundary.call.parameters[0].locations[0]
        else {
            panic!("register parameter");
        };
        *register = MachineRegister::X86R10;

        let writes = derive_boundary_entry_storage_writes(
            &boundary,
            &[(24, ValueShape::integer(8, 8))],
            None,
            None,
        )
        .expect("selected inbound writes");

        assert_eq!(
            writes,
            vec![SelectedInstructionKind::WriteEntryArgumentRegister {
                register: MachineRegister::X86R10,
                byte_offset: 24,
                byte_size: 8,
            }]
        );
    }

    #[test]
    fn inbound_writes_capture_an_indirect_result_pointer() {
        let result = ValueShape::integer(24, 8);
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(result),
            },
        )
        .expect("SysV memory result");

        let writes =
            derive_boundary_entry_storage_writes(boundary.plan(), &[], Some(result), Some(96))
                .expect("hidden result pointer write");

        assert_eq!(
            writes,
            vec![SelectedInstructionKind::WriteEntryArgumentRegister {
                register: MachineRegister::X86Rdi,
                byte_offset: 96,
                byte_size: 8,
            }]
        );
    }

    #[test]
    fn inbound_writes_reject_a_state_invalid_plan() {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        };
        let mut boundary =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
                .expect("SysV boundary")
                .plan()
                .clone();
        boundary.state.initial_regime = MachineRegime::Aarch64A64 { exception_level: 0 };

        let error = derive_boundary_entry_storage_writes(
            &boundary,
            &[(0, ValueShape::integer(8, 8))],
            None,
            None,
        )
        .expect_err("architecture-mismatched state must fail closed");

        assert!(error.0.contains("different architectures"));
    }

    #[test]
    fn inbound_storage_carries_exact_x86_fragment_clobbers() {
        let parameters = vec![ValueShape::integer(8, 8); 7];
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: parameters.clone(),
                result: None,
            },
        )
        .expect("SysV boundary with one stack argument");
        let destinations = parameters
            .into_iter()
            .enumerate()
            .map(|(index, shape)| (index * 8, shape))
            .collect::<Vec<_>>();

        let derived = derive_boundary_entry_storage(boundary.plan(), &destinations, None, None)
            .expect("state-checked inbound storage");

        assert_eq!(derived.parameters.len(), 7);
        for (parameter_index, parameter) in derived.parameters.iter().enumerate() {
            assert_eq!(parameter.parameter_index, parameter_index);
            assert_eq!(parameter.destination_byte_offset, parameter_index * 8);
            assert_eq!(parameter.shape, ValueShape::integer(8, 8));
            assert_eq!(
                parameter.placement,
                boundary.plan().call.parameters[parameter_index]
            );
            assert_eq!(parameter.write_range, parameter_index..parameter_index + 1);
            assert_eq!(
                &derived.writes[parameter.write_range.clone()],
                &derived.writes[parameter_index..parameter_index + 1]
            );
        }
        assert_eq!(
            derived.footprint.registers().as_slice(),
            &[MachineRegister::X86R10, MachineRegister::X86R15]
        );
        assert_eq!(
            derived.footprint.machine_state(),
            MachineStateSet::new([MachineState::GeneralRegisters])
        );
    }

    #[test]
    fn inbound_storage_rejects_a_selected_register_destroyed_by_scratch() {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        };
        let mut boundary =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
                .expect("SysV boundary")
                .plan()
                .clone();
        let ValueLocation::Register { register, .. } =
            &mut boundary.call.parameters[0].locations[0]
        else {
            panic!("register parameter");
        };
        *register = MachineRegister::X86R15;

        let error =
            derive_boundary_entry_storage(&boundary, &[(0, ValueShape::integer(8, 8))], None, None)
                .expect_err("frame-base scratch cannot also carry an input");

        assert!(error.0.contains("before capturing it"));
        assert!(error.0.contains("X86R15"));
    }

    #[test]
    fn inbound_storage_tracks_aarch64_indirect_copy_scratch() {
        let parameter = ValueShape::integer(24, 8);
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: vec![parameter],
                result: None,
            },
        )
        .expect("AAPCS64 indirect boundary");

        let derived = derive_boundary_entry_storage(boundary.plan(), &[(0, parameter)], None, None)
            .expect("state-checked indirect copy");

        assert_eq!(
            derived.footprint.registers().as_slice(),
            &[MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17),]
        );
    }

    #[test]
    fn bytes_handoff_descriptor_footprint_comes_from_the_x86_encoder() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8); 4],
                result: None,
            },
        )
        .expect("Microsoft x64 bytes handoff");

        let evidence = derive_boundary_entry_slice_descriptor_footprint(&boundary)
            .expect("descriptor footprint");

        assert_eq!(
            evidence.registers().as_slice(),
            &[MachineRegister::X86Rax, MachineRegister::X86R15]
        );
    }

    #[test]
    fn bytes_handoff_descriptor_footprint_comes_from_the_aarch64_encoder() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8); 4],
                result: None,
            },
        )
        .expect("AAPCS64 bytes handoff");

        let evidence = derive_boundary_entry_slice_descriptor_footprint(&boundary)
            .expect("descriptor footprint");

        assert_eq!(
            evidence.registers().as_slice(),
            &[MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17),]
        );
    }

    #[test]
    fn call_return_mechanics_track_x86_stack_and_control_writes() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV call-return boundary");
        let instructions = [
            SelectedInstructionKind::EnterFunction,
            SelectedInstructionKind::LeaveFunction,
        ];

        let evidence = derive_boundary_call_return_mechanics_footprint(&boundary, &instructions)
            .expect("x86 call-return mechanics");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rbx,
                MachineRegister::X86Rsp,
                MachineRegister::X86Rbp,
                MachineRegister::X86Rsi,
                MachineRegister::X86Rdi,
                MachineRegister::X86R12,
                MachineRegister::X86R13,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
        assert!(evidence.machine_state().contains_all(MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
            MachineState::ControlState,
        ])));
    }

    #[test]
    fn call_return_mechanics_track_aarch64_frame_restore_and_control_writes() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 call-return boundary");
        let instructions = [
            SelectedInstructionKind::EnterFunction,
            SelectedInstructionKind::LeaveFunction,
        ];

        let evidence = derive_boundary_call_return_mechanics_footprint(&boundary, &instructions)
            .expect("AArch64 call-return mechanics");

        assert_eq!(
            evidence.registers().as_slice(),
            &[MachineRegister::Aarch64X(16)]
                .into_iter()
                .chain((19..=30).map(MachineRegister::Aarch64X))
                .collect::<Vec<_>>()
        );
        assert!(evidence.machine_state().contains_all(MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
            MachineState::ControlState,
        ])));
    }

    #[test]
    fn call_return_mechanics_reject_an_incomplete_selected_pair() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV call-return boundary");

        let error = derive_boundary_call_return_mechanics_footprint(
            &boundary,
            &[SelectedInstructionKind::EnterFunction],
        )
        .expect_err("missing return must reject");

        assert!(error.0.contains("exactly one function entry and return"));
    }

    fn dispatch_scaffold_instructions() -> [SelectedInstructionKind; 5] {
        [
            SelectedInstructionKind::EnterDispatchLoop {
                entry_dispatch_index: 0,
                terminal_dispatch_index: 2,
            },
            SelectedInstructionKind::EnterDispatchCase { dispatch_index: 0 },
            SelectedInstructionKind::SetDispatchState { dispatch_index: 1 },
            SelectedInstructionKind::LeaveDispatchCase,
            SelectedInstructionKind::LeaveDispatchLoop,
        ]
    }

    #[test]
    fn dispatch_scaffold_tracks_x86_state_register_and_flags() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV dispatch boundary");

        let evidence = derive_boundary_dispatch_scaffold_footprint(
            &boundary,
            &dispatch_scaffold_instructions(),
        )
        .expect("x86 dispatch scaffold");

        assert_eq!(evidence.registers().as_slice(), &[MachineRegister::X86R12]);
        assert!(
            evidence
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    #[test]
    fn dispatch_scaffold_tracks_aarch64_state_register_and_flags() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 dispatch boundary");

        let evidence = derive_boundary_dispatch_scaffold_footprint(
            &boundary,
            &dispatch_scaffold_instructions(),
        )
        .expect("AArch64 dispatch scaffold");

        assert_eq!(
            evidence.registers().as_slice(),
            &[MachineRegister::Aarch64X(28)]
        );
        assert!(
            evidence
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    #[test]
    fn dispatch_scaffold_rejects_an_incomplete_loop_pair() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV dispatch boundary");

        let error = derive_boundary_dispatch_scaffold_footprint(
            &boundary,
            &[SelectedInstructionKind::EnterDispatchLoop {
                entry_dispatch_index: 0,
                terminal_dispatch_index: 1,
            }],
        )
        .expect_err("missing loop leave must reject");

        assert!(error.0.contains("exactly one loop entry and leave"));
    }

    fn static_guard_instruction(is_float: bool, has_storage: bool) -> SelectedInstructionKind {
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: omega_abstract_operations::StateGuardLowering::CompareStaticValue,
            operator: omega_abstract_operations::StateGuardOperator::Equal,
            storage_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            byte_offset: 65_537,
            byte_size: 8,
            expected_value: 1,
            has_storage,
            is_float,
        }
    }

    #[test]
    fn static_guard_footprint_tracks_x86_integer_and_float_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV guard boundary");
        let instructions = [
            static_guard_instruction(false, true),
            static_guard_instruction(true, true),
            static_guard_instruction(true, false),
        ];

        let evidence = derive_boundary_static_guard_footprint(&boundary, &instructions)
            .expect("x86 static guard evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R15,
                MachineRegister::X86Xmm(0),
                MachineRegister::X86Xmm(1),
            ]
        );
        assert!(
            evidence
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    #[test]
    fn static_guard_footprint_tracks_aarch64_integer_and_float_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 guard boundary");
        let instructions = [
            static_guard_instruction(false, true),
            static_guard_instruction(true, true),
            static_guard_instruction(true, false),
        ];

        let evidence = derive_boundary_static_guard_footprint(&boundary, &instructions)
            .expect("AArch64 static guard evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(26),
                MachineRegister::Aarch64V(0),
                MachineRegister::Aarch64V(1),
            ]
        );
        assert!(
            evidence
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    #[test]
    fn storage_free_static_guard_contributes_no_footprint() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV guard boundary");

        let evidence = derive_boundary_static_guard_footprint(
            &boundary,
            &[static_guard_instruction(true, false)],
        )
        .expect("storage-free static guard evidence");

        assert!(evidence.registers().as_slice().is_empty());
        assert!(evidence.machine_state().is_empty());
    }

    fn runtime_text_guard_instructions() -> [SelectedInstructionKind; 2] {
        [
            SelectedInstructionKind::CompareRuntimeTextLiteral {
                buffer: omega_abstract_operations::AbstractDataObjectHandle::invalid(),
                literal: std::sync::Arc::from("omega"),
            },
            SelectedInstructionKind::CompareRuntimeTextStorage {
                buffer: omega_abstract_operations::AbstractDataObjectHandle::invalid(),
                source_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                source_offset: 65_537,
                operator: omega_abstract_operations::StateGuardOperator::Equal,
            },
        ]
    }

    #[test]
    fn runtime_text_guards_track_x86_literal_and_descriptor_loop_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV text-guard boundary");

        let evidence = derive_boundary_runtime_text_guard_footprint(
            &boundary,
            &runtime_text_guard_instructions(),
        )
        .expect("x86 runtime-text guard evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86Rcx,
                MachineRegister::X86R8,
                MachineRegister::X86R9,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
        assert!(
            evidence
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    #[test]
    fn runtime_text_guards_track_aarch64_literal_and_descriptor_loop_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 text-guard boundary");

        let evidence = derive_boundary_runtime_text_guard_footprint(
            &boundary,
            &runtime_text_guard_instructions(),
        )
        .expect("AArch64 runtime-text guard evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[14, 15, 16, 17, 19, 20, 21, 26].map(MachineRegister::Aarch64X)
        );
        assert!(
            evidence
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    fn place_guard_instructions() -> [SelectedInstructionKind; 2] {
        [
            SelectedInstructionKind::ComparePlaces {
                left: omega_abstract_operations::Place::at(
                    omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                    65_537,
                ),
                right: omega_abstract_operations::Place::at(
                    omega_abstract_operations::RuntimeStorageRegion::Machine,
                    131_073,
                ),
                byte_size: 8,
                operator: omega_abstract_operations::StateGuardOperator::Equal,
                is_float: true,
            },
            SelectedInstructionKind::ComparePlaceValue {
                place: omega_abstract_operations::Place::at(
                    omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                    40,
                ),
                byte_size: 8,
                expected_value: 7,
                operator: omega_abstract_operations::StateGuardOperator::Equal,
            },
        ]
    }

    #[test]
    fn place_guards_track_x86_walk_bases_values_and_float_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV place-guard boundary");

        let evidence =
            derive_boundary_place_guard_footprint(&boundary, &place_guard_instructions())
                .expect("x86 place-guard evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
                MachineRegister::X86Xmm(0),
                MachineRegister::X86Xmm(1),
            ]
        );
        assert!(
            evidence
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    #[test]
    fn place_guards_track_aarch64_large_offset_and_float_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 place-guard boundary");

        let evidence =
            derive_boundary_place_guard_footprint(&boundary, &place_guard_instructions())
                .expect("AArch64 place-guard evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(21),
                MachineRegister::Aarch64X(26),
                MachineRegister::Aarch64V(0),
                MachineRegister::Aarch64V(1),
            ]
        );
        assert!(
            evidence
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    fn runtime_value_guard_fixture() -> (
        psi_arena::Arena<omega_abstract_operations::AbstractValueOperand>,
        SelectedInstructionKind,
    ) {
        let mut operands = psi_arena::Arena::new();
        let left = operands.insert(omega_abstract_operations::ValueOperand::Storage {
            region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            byte_offset: 40,
            byte_size: 8,
        });
        let right = operands.insert(omega_abstract_operations::ValueOperand::Immediate(2));
        let binary = operands.insert(omega_abstract_operations::ValueOperand::Binary {
            left,
            operator: omega_abstract_operations::StateGuardOperator::AddTowardPositive,
            right,
            is_float: true,
            byte_width: 8,
            arithmetic_domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            operands_signed: false,
        });
        (
            operands,
            SelectedInstructionKind::CompareRuntimeValues {
                left: binary,
                right,
                byte_size: 8,
                operator: omega_abstract_operations::StateGuardOperator::Equal,
            },
        )
    }

    #[test]
    fn runtime_value_guards_track_x86_family_ceiling_and_nested_stack_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV runtime-value guard boundary");
        let (operands, instruction) = runtime_value_guard_fixture();

        let evidence =
            derive_boundary_runtime_value_guard_footprint(&boundary, &operands, &[instruction])
                .expect("x86 runtime-value guard evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86Rcx,
                MachineRegister::X86Rdx,
                MachineRegister::X86R8,
                MachineRegister::X86R9,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R15,
                MachineRegister::X86Xmm(0),
                MachineRegister::X86Xmm(1),
            ]
        );
        assert!(evidence.machine_state().contains_all(MachineStateSet::new([
            MachineState::Flags,
            MachineState::StackPointer,
            MachineState::ControlState,
        ])));
    }

    #[test]
    fn runtime_value_guards_track_aarch64_recursive_scratch_pool_ceiling() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 runtime-value guard boundary");
        let (operands, instruction) = runtime_value_guard_fixture();

        let evidence =
            derive_boundary_runtime_value_guard_footprint(&boundary, &operands, &[instruction])
                .expect("AArch64 runtime-value guard evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[9, 10, 11, 12, 13, 14, 15, 17, 19, 20, 21, 26]
                .map(MachineRegister::Aarch64X)
                .into_iter()
                .chain([MachineRegister::Aarch64V(0), MachineRegister::Aarch64V(1),])
                .collect::<Vec<_>>()
        );
        assert!(evidence.machine_state().contains_all(MachineStateSet::new([
            MachineState::Flags,
            MachineState::ControlState,
        ])));
        assert!(
            !evidence
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::StackPointer,]))
        );
    }

    #[test]
    fn exit_result_register_footprint_unions_x86_immediate_and_runtime_loads() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .expect("SysV result boundary");
        let instructions = [
            SelectedInstructionKind::WriteReturnRegisterInteger {
                register: MachineRegister::X86Rax,
                byte_size: 8,
                value: 1,
            },
            SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
                register: MachineRegister::X86Xmm(0),
                region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 24,
                byte_size: 8,
            },
        ];

        let evidence = derive_boundary_exit_result_register_footprint(&boundary, &instructions)
            .expect("x86 result evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R15,
                MachineRegister::X86Xmm(0),
            ]
        );
    }

    #[test]
    fn exit_result_register_footprint_tracks_aarch64_large_offset_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(ValueShape::float(8)),
            },
        )
        .expect("AAPCS64 result boundary");
        let instructions = [
            SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
                register: MachineRegister::Aarch64V(0),
                region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 4097,
                byte_size: 8,
            },
        ];

        let evidence = derive_boundary_exit_result_register_footprint(&boundary, &instructions)
            .expect("AArch64 result evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(26),
                MachineRegister::Aarch64V(0),
            ]
        );
    }

    fn indirect_result_copy_instruction(
        source_offset: usize,
        pointer_offset: usize,
        byte_count: usize,
    ) -> SelectedInstructionKind {
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            pointer_offset,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .expect("pointee target");
        SelectedInstructionKind::CopyPlaces {
            source: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                source_offset,
            ),
            target,
            byte_count,
            role: omega_abstract_operations::CopyPlacesRole::ExitIndirectResult,
        }
    }

    #[test]
    fn indirect_result_copy_footprint_tracks_x86_shared_base_scratch() {
        let result = ValueShape::integer(24, 8);
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(result),
            },
        )
        .expect("SysV indirect result");
        let instructions = [
            indirect_result_copy_instruction(64, 32, 24),
            indirect_result_copy_instruction(96, 40, 24),
        ];

        let evidence =
            derive_boundary_exit_indirect_result_copy_footprint(&boundary, 32, &instructions)
                .expect("x86 indirect-result evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
    }

    #[test]
    fn indirect_result_copy_footprint_tracks_aarch64_pointee_scratch() {
        let result = ValueShape::integer(24, 8);
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(result),
            },
        )
        .expect("AAPCS64 indirect result");
        let instructions = [indirect_result_copy_instruction(64, 32, 24)];

        let evidence =
            derive_boundary_exit_indirect_result_copy_footprint(&boundary, 32, &instructions)
                .expect("AArch64 indirect-result evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(20),
            ]
        );
    }

    #[test]
    fn ordinary_pointee_copy_does_not_acquire_indirect_result_footprint() {
        let result = ValueShape::integer(24, 8);
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(result),
            },
        )
        .expect("SysV indirect result");
        let mut instruction = indirect_result_copy_instruction(64, 32, 24);
        let SelectedInstructionKind::CopyPlaces { role, .. } = &mut instruction else {
            unreachable!("helper returns a place copy")
        };
        *role = omega_abstract_operations::CopyPlacesRole::Ordinary;

        let evidence =
            derive_boundary_exit_indirect_result_copy_footprint(&boundary, 32, [&instruction])
                .expect("ordinary copy remains valid outside boundary evidence");

        assert!(evidence.registers().as_slice().is_empty());
        assert!(evidence.machine_state().is_empty());
    }

    #[test]
    fn compiler_body_pointee_copy_footprint_requires_ordinary_role() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(ValueShape::integer(24, 8)),
            },
        )
        .expect("SysV boundary");
        let mut ordinary = indirect_result_copy_instruction(64, 32, 24);
        let SelectedInstructionKind::CopyPlaces { role, .. } = &mut ordinary else {
            unreachable!("helper returns a place copy")
        };
        *role = omega_abstract_operations::CopyPlacesRole::Ordinary;
        let exit = indirect_result_copy_instruction(64, 32, 24);

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&ordinary, &exit])
                .expect("ordinary pointee-copy evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
    }

    #[test]
    fn compiler_body_direct_copy_footprint_uses_exact_encoder_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                4096,
            ),
            target: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::Machine,
                32,
            ),
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary direct-copy evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(26),
            ]
        );
    }

    #[test]
    fn compiler_body_from_pointee_footprint_uses_exact_encoder_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            32,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(4096)))
        .expect("from-pointee source");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source,
            target: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::Machine,
                64,
            ),
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary from-pointee evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
            ]
        );
    }

    #[test]
    fn compiler_body_pointee_pair_footprint_uses_exact_encoder_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let pointee = |pointer_offset, field_offset| {
            omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                pointer_offset,
            )
            .with_step(omega_abstract_operations::PlaceStep::Deref)
            .and_then(|place| {
                place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                    field_offset,
                ))
            })
            .expect("frame-held pointee")
        };
        let instruction = SelectedInstructionKind::CopyPlaces {
            source: pointee(32, 4096),
            target: pointee(40, 0),
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary pointee-pair evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
            ]
        );
    }

    #[test]
    fn compiler_body_from_indexed_footprint_uses_exact_encoder_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            32,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset: 40,
                index_byte_size: 8,
                element_byte_size: 24,
            })
        })
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(16)))
        .expect("single indexed source");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source,
            target: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::Machine,
                64,
            ),
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary from-indexed evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(21),
                MachineRegister::Aarch64X(26),
            ]
        );
    }

    #[test]
    fn compiler_body_to_indexed_footprint_uses_exact_encoder_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            32,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset: 40,
                index_byte_size: 8,
                element_byte_size: 24,
            })
        })
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(16)))
        .expect("single indexed target");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                64,
            ),
            target,
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary to-indexed evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(21),
                MachineRegister::Aarch64X(26),
            ]
        );
    }

    #[test]
    fn compiler_body_indexed_to_pointee_footprint_uses_exact_encoder_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            32,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset: 40,
                index_byte_size: 8,
                element_byte_size: 24,
            })
        })
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(16)))
        .expect("single indexed source");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            64,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(8)))
        .expect("pointee target");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source,
            target,
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary indexed-to-pointee evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(21),
                MachineRegister::Aarch64X(26),
            ]
        );
    }

    #[test]
    fn compiler_body_frame_base_indexed_footprint_uses_exact_encoder_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            32,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 40,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(16)))
        .expect("frame-base-indexed source");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source,
            target: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                64,
            ),
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary frame-base-indexed evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(24),
                MachineRegister::Aarch64X(26),
            ]
        );
    }

    #[test]
    fn compiler_body_machine_indexed_footprint_uses_exact_encoder_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::Machine,
            32,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 40,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(16)))
        .expect("machine-indexed source");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source,
            target: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::Machine,
                64,
            ),
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary machine-indexed evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(26),
            ]
        );
    }

    #[test]
    fn compiler_body_to_machine_indexed_footprint_uses_exact_encoder_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::Machine,
            32,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 40,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(16)))
        .expect("machine-indexed target");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::Machine,
                64,
            ),
            target,
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary to-machine-indexed evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(26),
            ]
        );
    }

    #[test]
    fn compiler_body_frame_double_indexed_footprint_uses_both_index_scratches() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("System V boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            32,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 40,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset: 48,
                index_byte_size: 8,
                element_byte_size: 8,
            })
        })
        .expect("frame double-indexed source");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source,
            target: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                64,
            ),
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };
        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary frame-double-indexed evidence");
        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
    }

    #[test]
    fn compiler_body_machine_indexed_pair_reuses_one_x86_index_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("System V boundary");
        let indexed = |base_offset, index_offset| {
            omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::Machine,
                base_offset,
            )
            .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset,
                index_byte_size: 8,
                element_byte_size: 4,
            })
            .expect("machine indexed place")
        };
        let instruction = SelectedInstructionKind::CopyPlaces {
            source: indexed(32, 40),
            target: indexed(32, 48),
            byte_count: 4,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };
        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary machine-indexed-pair evidence");
        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R11,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
    }

    #[test]
    fn compiler_body_general_x86_copy_uses_materializer_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("System V boundary");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            32,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 64,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset: 72,
                index_byte_size: 8,
                element_byte_size: 8,
            })
        })
        .expect("frame double-indexed target");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            80,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 88,
            index_byte_size: 8,
            element_byte_size: 8,
        })
        .expect("indexed source keeps the pair in the general class");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source,
            target,
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };
        assert!(matches!(
            crate::classify_copy_places_shape(
                match &instruction {
                    SelectedInstructionKind::CopyPlaces { source, .. } => source,
                    _ => unreachable!(),
                },
                match &instruction {
                    SelectedInstructionKind::CopyPlaces { target, .. } => target,
                    _ => unreachable!(),
                },
            ),
            crate::CopyPlacesShape::General
        ));
        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary general place-copy evidence");
        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
    }

    #[test]
    fn compiler_body_direct_integer_write_tracks_large_aarch64_offset_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let instruction = SelectedInstructionKind::WritePlaceInteger {
            target: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                5000,
            ),
            value: 7,
            byte_size: 4,
        };
        let evidence =
            derive_boundary_compiler_body_place_integer_write_footprint(&boundary, [&instruction])
                .expect("ordinary direct integer-write evidence");
        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
            ]
        );
    }

    #[test]
    fn compiler_body_pointee_integer_write_tracks_large_aarch64_offset_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            5000,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(16)))
        .expect("frame-held pointee target");
        let instruction = SelectedInstructionKind::WritePlaceInteger {
            target,
            value: 7,
            byte_size: 4,
        };
        let evidence =
            derive_boundary_compiler_body_place_integer_write_footprint(&boundary, [&instruction])
                .expect("ordinary pointee integer-write evidence");
        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
            ]
        );
    }

    #[test]
    fn compiler_body_cross_region_frame_indexed_integer_write_tracks_aarch64_base() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            16,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::Machine,
                index_offset: 64,
                index_byte_size: 8,
                element_byte_size: 24,
            })
        })
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(8)))
        .expect("cross-region frame-indexed target");
        let instruction = SelectedInstructionKind::WritePlaceInteger {
            target,
            value: 7,
            byte_size: 4,
        };
        let evidence =
            derive_boundary_compiler_body_place_integer_write_footprint(&boundary, [&instruction])
                .expect("ordinary frame-indexed integer-write evidence");
        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(15),
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(21),
                MachineRegister::Aarch64X(26),
            ]
        );
    }

    #[test]
    fn compiler_body_cross_region_frame_base_indexed_integer_write_tracks_aarch64_base() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            16,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::Machine,
            index_offset: 64,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(8)))
        .expect("cross-region inline-frame target");
        let instruction = SelectedInstructionKind::WritePlaceInteger {
            target,
            value: 7,
            byte_size: 4,
        };
        let evidence =
            derive_boundary_compiler_body_place_integer_write_footprint(&boundary, [&instruction])
                .expect("ordinary inline-frame integer-write evidence");
        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(15),
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(26),
            ]
        );
    }

    #[test]
    fn compiler_body_x86_place_address_tracks_walk_indices_and_flags() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::Machine,
            16,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 32,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::Machine,
                index_offset: 48,
                index_byte_size: 8,
                element_byte_size: 8,
            })
        })
        .expect("double-indexed source");
        let instruction = SelectedInstructionKind::WritePlaceAddress {
            source,
            target_offset: 64,
        };

        let evidence =
            derive_boundary_compiler_body_place_address_write_footprint(&boundary, [&instruction])
                .expect("x86 place-address evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
        assert_eq!(
            evidence.machine_state(),
            MachineStateSet::new([MachineState::GeneralRegisters, MachineState::Flags])
        );
    }

    #[test]
    fn compiler_body_aarch64_place_address_tracks_machine_index_and_store_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::Machine,
            16,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 32,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .expect("machine-indexed source");
        let instruction = SelectedInstructionKind::WritePlaceAddress {
            source,
            target_offset: 3,
        };

        let evidence =
            derive_boundary_compiler_body_place_address_write_footprint(&boundary, [&instruction])
                .expect("aarch64 place-address evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(9),
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(26),
            ]
        );
        assert_eq!(
            evidence.machine_state(),
            MachineStateSet::new([MachineState::GeneralRegisters])
        );
    }

    #[test]
    fn compiler_body_aarch64_place_address_tracks_frame_double_index_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            16,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 32,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset: 40,
                index_byte_size: 8,
                element_byte_size: 8,
            })
        })
        .expect("frame-double-indexed source");
        let instruction = SelectedInstructionKind::WritePlaceAddress {
            source,
            target_offset: 64,
        };

        let evidence =
            derive_boundary_compiler_body_place_address_write_footprint(&boundary, [&instruction])
                .expect("aarch64 frame-double-indexed place-address evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(14),
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(26),
            ]
        );
        assert_eq!(
            evidence.machine_state(),
            MachineStateSet::new([MachineState::GeneralRegisters])
        );
    }

    #[test]
    fn compiler_body_aarch64_place_address_tracks_machine_double_index_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::Machine,
            16,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 32,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::Machine,
                index_offset: 40,
                index_byte_size: 8,
                element_byte_size: 8,
            })
        })
        .expect("machine-double-indexed source");
        let instruction = SelectedInstructionKind::WritePlaceAddress {
            source,
            target_offset: 64,
        };

        let evidence =
            derive_boundary_compiler_body_place_address_write_footprint(&boundary, [&instruction])
                .expect("aarch64 machine-double-indexed place-address evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(14),
                MachineRegister::Aarch64X(15),
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(26),
            ]
        );
        assert_eq!(
            evidence.machine_state(),
            MachineStateSet::new([MachineState::GeneralRegisters])
        );
    }

    #[test]
    fn compiler_body_general_x86_integer_write_uses_materializer_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV boundary");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            16,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::Machine,
            index_offset: 64,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .expect("cross-region inline frame target");
        assert_eq!(
            crate::classify_write_place_shape(&target),
            crate::WritePlaceShape::Unsupported
        );
        let instruction = SelectedInstructionKind::WritePlaceInteger {
            target,
            value: 7,
            byte_size: 4,
        };
        let evidence =
            derive_boundary_compiler_body_place_integer_write_footprint(&boundary, [&instruction])
                .expect("ordinary general integer-write evidence");
        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R11,
                MachineRegister::X86R15,
            ]
        );
    }

    #[test]
    fn compiler_body_general_x86_binary_write_uses_materializer_ceiling() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV boundary");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            16,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 64,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset: 72,
                index_byte_size: 8,
                element_byte_size: 8,
            })
        })
        .expect("frame double-indexed target");
        assert_eq!(
            crate::classify_write_place_shape(&target),
            crate::WritePlaceShape::Unsupported
        );

        let mut operands = psi_arena::Arena::new();
        let left = operands.insert(omega_abstract_operations::ValueOperand::Immediate(2));
        let right = operands.insert(omega_abstract_operations::ValueOperand::Immediate(3));
        let instruction = SelectedInstructionKind::WritePlaceBinary {
            target,
            byte_size: 4,
            left,
            operator: omega_abstract_operations::StateGuardOperator::Add,
            right,
            is_float: false,
            domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            target_signed: true,
        };
        let evidence = derive_boundary_compiler_body_place_binary_write_footprint(
            &boundary,
            &operands,
            [&instruction],
        )
        .expect("ordinary general binary-write evidence");
        assert_eq!(
            evidence.registers(),
            &omega_isa_x86_64::place_binary_write_register_write_ceiling()
        );
        assert_eq!(
            evidence.machine_state(),
            MachineStateSet::new([
                MachineState::GeneralRegisters,
                MachineState::VectorRegisters,
                MachineState::Flags,
                MachineState::StackPointer,
            ])
        );
    }

    #[test]
    fn compiler_body_general_x86_text_assembly_uses_materializer_ceiling() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV boundary");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            16,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::Machine,
            index_offset: 64,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::Machine,
                index_offset: 72,
                index_byte_size: 8,
                element_byte_size: 8,
            })
        })
        .expect("cross-region frame double-indexed target");
        assert_eq!(
            crate::classify_write_place_shape(&target),
            crate::WritePlaceShape::Unsupported
        );
        let instruction = SelectedInstructionKind::MaterializeTextBufferToPlace {
            buffer: psi_arena::Handle::invalid(),
            target,
        };
        let evidence =
            derive_boundary_compiler_body_text_assembly_write_footprint(&boundary, [&instruction])
                .expect("ordinary general text-assembly evidence");
        assert_eq!(
            evidence.registers(),
            &omega_isa_x86_64::place_text_buffer_materialize_register_writes()
        );
        assert!(evidence.machine_state().contains_all(MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::Flags,
        ])));
    }

    #[test]
    fn boundary_exit_consumes_the_exact_selected_result_register() {
        let result = ValueShape::integer(8, 8);
        let mut boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(result),
            },
        )
        .expect("SysV boundary")
        .plan()
        .clone();
        let ValueLocation::Register { register, .. } =
            &mut boundary.call.result.as_mut().expect("result").locations[0]
        else {
            panic!("register result");
        };
        *register = MachineRegister::X86R10;

        let exit = derive_boundary_exit(&boundary, &[], Some(result)).expect("boundary exit");

        assert_eq!(
            exit.control,
            omega_calling_conventions::EntryControl::CallReturn
        );
        assert_eq!(
            exit.result_locations,
            vec![ValueLocation::Register {
                register: MachineRegister::X86R10,
                value_byte_offset: 0,
                byte_size: 8,
            }]
        );
    }
}
