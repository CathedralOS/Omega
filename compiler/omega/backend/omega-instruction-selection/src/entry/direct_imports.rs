//! Direct imported-call footprint derivation.

use omega_calling_conventions::{
    MachineState, MachineStateSet, PlanDiagnostic, RegisterSet, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan,
};

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
