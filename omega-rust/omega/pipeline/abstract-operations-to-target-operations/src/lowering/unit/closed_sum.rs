//! Bounded structural-result inspection for the first native console-byte lane.

use super::super::scalar_abi::fixed_native_integer_shape;
use super::super::shared::*;
use super::boundary_call::lower_boundary_call;
use super::scalar_call::{KnownUnitInteger, insert_known_unit_integer};
use super::scalar_definitions::lower_integer_constant;
use target_operations::TargetBoundaryResult;

pub(super) fn has_bounded_shape(function: &AbstractFunction) -> bool {
    function.parameters.is_empty()
        && function.structural_parameters.is_empty()
        && function.block_entries.len() == 3
        && function.block_entries[0].block == function.entry
        && function.block_entries[0].parameters.is_empty()
        && function.block_entries[0].operation_offset == 0
        && function.block_entries[1].parameters.len() == 1
        && function.block_entries[1].operation_offset == 2
        && function.block_entries[2].parameters.is_empty()
        && function.block_entries[2].operation_offset == 6
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::BoundaryCall { .. },
                AbstractOperation::StructuralCase { .. },
                AbstractOperation::BoundaryCall { .. },
                AbstractOperation::IntegerConstant { .. },
                AbstractOperation::BoundaryCall { .. },
                AbstractOperation::ReturnUnit { .. },
                AbstractOperation::IntegerConstant { .. },
                AbstractOperation::BoundaryCall { .. },
                AbstractOperation::ReturnUnit { .. },
            ]
        )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderUnitCallEvidence,
    >,
    native_callbacks: &BTreeMap<OperationId, target_operations::TargetNativeCallbackArgument>,
    parameters: &[TargetStructuralParameter],
) -> Result<super::body::LoweredUnitBody, LoweringError> {
    if !has_bounded_shape(function) || !parameters.is_empty() {
        return Err(LoweringError::UnitFunctionNotStraightLine(function.machine));
    }

    let parameters_by_place = BTreeMap::new();
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let mut operations = Vec::with_capacity(10);
    let mut provenance = TerminalPsiProvenance::default();
    let mut integer_constants = BTreeMap::new();
    let mut scalar_values = BTreeMap::new();
    let mut nonreturning = false;

    lower_boundary_call(
        &function.operations[0],
        function,
        target,
        functions,
        structural_types,
        boundary_machines,
        settlements,
        installed_calls,
        native_callbacks,
        &parameters_by_place,
        &mut shape_cache,
        &mut active,
        &BTreeMap::new(),
        &integer_constants,
        &mut scalar_values,
        &mut operations,
        &mut provenance,
        &mut nonreturning,
    )?;
    if nonreturning {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let source_home = match operations.last() {
        Some(TargetUnitOperation::BoundarySettlement {
            result: TargetBoundaryResult::Structural(home),
            ..
        }) => home.clone(),
        _ => {
            return Err(LoweringError::UnsupportedOperationInUnitFunction(
                function.machine,
            ));
        }
    };
    let AbstractOperation::StructuralCase { source, cases } = &function.operations[1] else {
        unreachable!("bounded shape fixes the structural case")
    };
    if *source != source_home.result.place || cases.len() != 2 {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let declaration = structural_types
        .get(&source_home.result.structural_type)
        .copied()
        .ok_or(LoweringError::UnknownStructuralType(
            source_home.result.structural_type,
        ))?;
    let StructuralTypeShape::Sum {
        cases: declared_cases,
    } = &declaration.shape
    else {
        return Err(LoweringError::UnsupportedStructuralSum(declaration.id));
    };
    let source_layout =
        source_home
            .layout
            .sum()
            .ok_or(LoweringError::UnsupportedOperationInUnitFunction(
                function.machine,
            ))?;
    if declared_cases.len() != cases.len() || source_layout.cases.len() != cases.len() {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }

    let mut target_cases = Vec::with_capacity(cases.len());
    for (case_tag, (successor, declared_case)) in cases.iter().zip(declared_cases).enumerate() {
        if successor.case != declared_case.id
            || successor.trivial_affine_discards.as_slice() != [*source]
        {
            return Err(LoweringError::UnsupportedOperationInUnitFunction(
                function.machine,
            ));
        }
        let target_entry = function
            .block_entries
            .iter()
            .find(|entry| entry.block == successor.target)
            .ok_or(LoweringError::UnsupportedOperationInUnitFunction(
                function.machine,
            ))?;
        if target_entry.parameters.len() != successor.payloads.len() {
            return Err(LoweringError::UnsupportedOperationInUnitFunction(
                function.machine,
            ));
        }
        let relevant_fields = declared_case
            .fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .collect::<Vec<_>>();
        if relevant_fields.len() != source_layout.cases[case_tag].fields.len() {
            return Err(LoweringError::UnsupportedOperationInUnitFunction(
                function.machine,
            ));
        }
        let mut target_payloads = Vec::with_capacity(successor.payloads.len());
        for payload in &successor.payloads {
            let field_ordinal = relevant_fields
                .iter()
                .position(|field| field.id == payload.field)
                .ok_or(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ))?;
            let field = relevant_fields[field_ordinal];
            if field.field_type != StructuralFieldType::Scalar(payload.scalar_type) {
                return Err(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ));
            }
            let ScalarType::Integer(integer_type) = payload.scalar_type else {
                return Err(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ));
            };
            let shape = fixed_native_integer_shape(integer_type).ok_or(
                LoweringError::UnsupportedOperationInUnitFunction(function.machine),
            )?;
            let layout = source_layout.cases[case_tag].fields[field_ordinal];
            if layout.shape != shape {
                return Err(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ));
            }
            let home = TargetUnitScalarHomeRequirement {
                defining_operation: source_home.defining_operation,
                source_value: payload.parameter,
                scalar_type: payload.scalar_type,
                shape,
            };
            insert_known_unit_integer(
                &mut scalar_values,
                payload.parameter,
                KnownUnitInteger::Home(home),
            )?;
            target_payloads.push(target_operations::TargetUnitStructuralCasePayload {
                field: payload.field,
                field_byte_offset: u32::from(layout.byte_offset),
                home,
            });
        }
        let return_index = block_return_index(function, target_entry.operation_offset)?;
        let AbstractOperation::ReturnUnit {
            psi_edge: nominal_return_edge,
            cleanup_actions,
        } = &function.operations[return_index]
        else {
            unreachable!("block_return_index accepts only a Unit return")
        };
        if !cleanup_actions.is_empty() {
            return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
        }
        target_cases.push(target_operations::TargetUnitStructuralCaseSuccessor {
            psi_edge: successor.psi_edge,
            case: successor.case,
            case_tag: i32::try_from(case_tag)
                .map_err(|_| LoweringError::UnsupportedOperationInUnitFunction(function.machine))?,
            operation_ordinal: 0,
            nominal_return_edge: *nominal_return_edge,
            payloads: target_payloads,
        });
    }
    if target_cases
        .iter()
        .filter(|case| case.payloads.is_empty())
        .count()
        != 1
        || target_cases
            .iter()
            .map(|case| case.payloads.len())
            .sum::<usize>()
            != 1
    {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }

    let structural_case_ordinal = operations.len();
    operations.push(TargetUnitOperation::StructuralCase {
        source: source_home,
        cases: target_cases,
    });
    provenance
        .edges
        .extend(cases.iter().map(|case| case.psi_edge));

    for physical_index in 1..function.block_entries.len() {
        let entry = &function.block_entries[physical_index];
        let successor_index = cases
            .iter()
            .position(|successor| successor.target == entry.block)
            .ok_or(LoweringError::UnsupportedOperationInUnitFunction(
                function.machine,
            ))?;
        let arm_ordinal = u32::try_from(operations.len())
            .map_err(|_| LoweringError::UnsupportedOperationInUnitFunction(function.machine))?;
        let TargetUnitOperation::StructuralCase {
            cases: target_cases,
            ..
        } = &mut operations[structural_case_ordinal]
        else {
            unreachable!("the bounded structural case was just inserted")
        };
        target_cases[successor_index].operation_ordinal = arm_ordinal;

        let end = function
            .block_entries
            .get(physical_index + 1)
            .map_or(function.operations.len(), |next| next.operation_offset);
        lower_arm(
            function,
            target,
            functions,
            structural_types,
            boundary_machines,
            settlements,
            installed_calls,
            native_callbacks,
            &parameters_by_place,
            &mut shape_cache,
            &mut active,
            &mut integer_constants,
            &mut scalar_values,
            &mut operations,
            &mut provenance,
            entry.operation_offset,
            end,
            physical_index + 1 == function.block_entries.len(),
        )?;
    }

    Ok(super::body::LoweredUnitBody {
        operations,
        provenance,
    })
}

fn block_return_index(function: &AbstractFunction, start: usize) -> Result<usize, LoweringError> {
    let end = function
        .block_entries
        .iter()
        .find(|entry| entry.operation_offset > start)
        .map_or(function.operations.len(), |entry| entry.operation_offset);
    end.checked_sub(1)
        .filter(|index| {
            matches!(
                function.operations[*index],
                AbstractOperation::ReturnUnit { .. }
            )
        })
        .ok_or(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ))
}

#[allow(clippy::too_many_arguments)]
fn lower_arm(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderUnitCallEvidence,
    >,
    native_callbacks: &BTreeMap<OperationId, target_operations::TargetNativeCallbackArgument>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    integer_constants: &mut BTreeMap<ValueId, (OperationId, IntegerType, IntegerValue)>,
    scalar_values: &mut BTreeMap<ValueId, KnownUnitInteger>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
    start: usize,
    end: usize,
    final_arm: bool,
) -> Result<(), LoweringError> {
    let return_index =
        end.checked_sub(1)
            .ok_or(LoweringError::UnsupportedOperationInUnitFunction(
                function.machine,
            ))?;
    let AbstractOperation::ReturnUnit {
        psi_edge,
        cleanup_actions,
    } = &function.operations[return_index]
    else {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    };
    if !cleanup_actions.is_empty() {
        return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
    }
    let mut saw_nonreturning = false;
    for operation in &function.operations[start..return_index] {
        if saw_nonreturning {
            return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
        }
        match operation {
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type: ScalarType::Integer(scalar_type),
                value,
            } => lower_integer_constant(
                function.machine,
                *psi_operation,
                *result,
                *scalar_type,
                *value,
                false,
                integer_constants,
                scalar_values,
                operations,
                provenance,
            )?,
            AbstractOperation::BoundaryCall { .. } => {
                let mut nonreturning = false;
                lower_boundary_call(
                    operation,
                    function,
                    target,
                    functions,
                    structural_types,
                    boundary_machines,
                    settlements,
                    installed_calls,
                    native_callbacks,
                    parameters_by_place,
                    shape_cache,
                    active,
                    &BTreeMap::new(),
                    integer_constants,
                    scalar_values,
                    operations,
                    provenance,
                    &mut nonreturning,
                )?;
                saw_nonreturning = nonreturning;
            }
            _ => {
                return Err(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ));
            }
        }
    }
    if !saw_nonreturning {
        return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
    }
    if final_arm {
        operations.push(TargetUnitOperation::Return {
            psi_edge: *psi_edge,
            cleanup_actions: Vec::new(),
        });
    } else {
        operations.push(TargetUnitOperation::NonreturningTail {
            psi_edge: *psi_edge,
        });
    }
    provenance.edges.push(*psi_edge);
    Ok(())
}
