//! Ordinary control graphs; available definitions belong to dominating blocks.
use super::shared::*;
use super::unit::scalar_call::KnownUnitInteger;
pub(super) mod aggregate_results;
mod borrowed_calls;
mod byte_write;
mod dominance;
mod observations;
mod operations;
mod primitive_storage;
mod records;
pub(super) mod scalar_arrays;
mod scalar_sources;
mod structural_case;
mod terminator;
mod transfers;
use operations::lower_operation;
use target_operations::{TargetControlBlock, TargetControlGraph, TargetScalarBlockParameter};
use terminator::lower_terminator;

#[derive(Clone)]
struct LiveDefinitions {
    // Dominating definitions only. Abstract-unit validation owns edge liveness.
    structural_homes: BTreeMap<PlaceId, target_operations::TargetStructuralHomeRequirement>,
    nonreturning: bool,
    stored_descriptors: BTreeSet<OperationId>,
    integers: BTreeMap<ValueId, KnownUnitInteger>,
    booleans: BTreeMap<ValueId, (OperationId, bool)>,
    scalar_homes: BTreeMap<ValueId, TargetUnitScalarHomeRequirement>,
    ieee_float_constants: BTreeMap<ValueId, (OperationId, semantic_vocabulary::IeeeFloatValue)>,
    scalar_block_parameters: BTreeMap<ValueId, target_operations::TargetScalarBlockValue>,
    views: BTreeMap<PlaceId, (OperationId, StructuralTypeId)>,
    block_views: BTreeSet<PlaceId>,
    owned_arrivals: BTreeSet<PlaceId>,
    lengths: BTreeMap<ValueId, PlaceId>,
}

pub(super) fn lower(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &StructuralTypeLookup<'_>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderCallEvidence,
    >,
    scalar_abis: &BTreeMap<MachineId, ScalarFunctionAbi>,
    native_callbacks: &BTreeMap<OperationId, target_operations::TargetNativeCallbackArgument>,
) -> Result<TargetFunction, LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    if function.block_entries.is_empty() {
        return Err(invalid());
    }
    let unobserved_owned = super::unobserved_owned::accepts(function, structural_types);
    // Invocation borrows keep their original places throughout the graph.
    // Shared signature preparation owns referent layout and ABI placement;
    // only transferred block descriptors need the narrower edge classifier.
    if !matches!(
        function.result,
        AbstractFunctionResult::Unit
            | AbstractFunctionResult::Scalar(_)
            | AbstractFunctionResult::Structural(_)
    ) || (!unobserved_owned
        && !function.structural_parameters.iter().all(|parameter| {
            parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
                && ((parameter.access != StructuralAccess::Owned
                    && matches!(
                        parameter.multiplicity,
                        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                    ))
                    || (parameter.access == StructuralAccess::Owned
                        && (parameter.multiplicity == StructuralMultiplicity::Affine
                            || scalar_arrays::is_owned_parameter(parameter, structural_types)
                            || (parameter.multiplicity == StructuralMultiplicity::Unrestricted
                                && structural_types
                                    .get(&parameter.structural_type)
                                    .is_some_and(|declaration| {
                                        matches!(
                                            declaration.shape,
                                            terminal_psi::StructuralTypeShape::Sum { .. }
                                        )
                                    })))
                        && !parameter.is_self))
        }))
        || !function.entry_claims.is_empty()
    {
        return Err(invalid());
    }
    let prepared =
        super::function_signature::prepare_function_signature(function, target, structural_types)?;
    let parameters_by_place = super::function_signature::parameters_by_place(&prepared.parameters);
    let mut definitions = BTreeSet::new();
    for parameter in &function.parameters {
        if !definitions.insert(parameter.value) {
            return Err(LoweringError::DuplicateValue(parameter.value));
        }
    }
    transfers::validate_parameters(function, &mut definitions)?;
    for operation in &function.operations {
        let result = match operation {
            AbstractOperation::IntegerConstant { result, .. }
            | AbstractOperation::BooleanConstant { result, .. }
            | AbstractOperation::BooleanStructuralField { result, .. }
            | AbstractOperation::IeeeFloatConstant { result, .. }
            | AbstractOperation::IeeeFloatCompare { result, .. }
            | AbstractOperation::IntegerWiden { result, .. }
            | AbstractOperation::IntegerExactCast { result, .. }
            | AbstractOperation::IntegerEqual { result, .. }
            | AbstractOperation::IntegerLessThan { result, .. }
            | AbstractOperation::IntegerLessOrEqual { result, .. }
            | AbstractOperation::BooleanNot { result, .. }
            | AbstractOperation::BooleanEqual { result, .. } => Some(*result),
            AbstractOperation::ExactIntegerAdd { result, .. }
            | AbstractOperation::WrappingIntegerAdd { result, .. }
            | AbstractOperation::SaturatingIntegerAdd { result, .. }
            | AbstractOperation::WrappingIntegerSubtract { result, .. }
            | AbstractOperation::SaturatingIntegerSubtract { result, .. }
            | AbstractOperation::WrappingIntegerMultiply { result, .. }
            | AbstractOperation::ExactIntegerMultiply { result, .. }
            | AbstractOperation::SaturatingIntegerMultiply { result, .. }
            | AbstractOperation::ExactIntegerDivide { result, .. }
            | AbstractOperation::ExactIntegerRemainder { result, .. }
            | AbstractOperation::WrappingIntegerDivide { result, .. }
            | AbstractOperation::WrappingIntegerRemainder { result, .. }
            | AbstractOperation::SaturatingIntegerDivide { result, .. }
            | AbstractOperation::SaturatingIntegerRemainder { result, .. }
            | AbstractOperation::IntegerBitwiseAnd { result, .. }
            | AbstractOperation::IntegerBitwiseOr { result, .. }
            | AbstractOperation::IntegerBitwiseXor { result, .. }
            | AbstractOperation::IntegerBitwiseNot { result, .. }
            | AbstractOperation::WrappingIntegerShiftLeft { result, .. }
            | AbstractOperation::WrappingIntegerShiftRight { result, .. }
            | AbstractOperation::ExactIntegerShiftLeft { result, .. }
            | AbstractOperation::ExactIntegerShiftRight { result, .. }
            | AbstractOperation::ExactIntegerSubtract { result, .. }
            | AbstractOperation::Call { result, .. } => Some(*result),
            AbstractOperation::ByteSequenceLength { result, .. }
            | AbstractOperation::ByteSequenceRead { result, .. }
            | AbstractOperation::PrimitiveScalarRead { result, .. }
            | AbstractOperation::IntegerStructuralField { result, .. }
            | AbstractOperation::StructuralCaseMembership { result, .. }
            | AbstractOperation::CallStructuralScalar { result, .. }
            | AbstractOperation::CallDynamicScalar { result, .. }
            | AbstractOperation::CallStoredDynamicScalar { result, .. }
            | AbstractOperation::CallStructuralScalarWithDynamicArguments { result, .. } => {
                Some(result.value)
            }
            _ => None,
        };
        if let Some(result) = result
            && !definitions.insert(result)
        {
            return Err(LoweringError::DuplicateValue(result));
        }
    }
    let mut places = function
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<BTreeSet<_>>();
    if places.len() != function.structural_parameters.len() {
        return Err(invalid());
    }
    for entry in &function.block_entries {
        for (position, parameter) in entry.structural_parameters.iter().enumerate() {
            if entry.block == function.entry
                || parameter.position as usize != position
                || (!unobserved_owned
                    && !super::scalar::byte_views::is_byte_parameter(parameter, structural_types)
                    && !super::unobserved_owned::parameter(parameter))
                || !places.insert(parameter.place)
            {
                return Err(invalid());
            }
        }
    }
    for operation in &function.operations {
        let established = match operation {
            AbstractOperation::EstablishPrimitiveLocal { result, .. }
            | AbstractOperation::EstablishScalarArray { result, .. }
            | AbstractOperation::EstablishRecord { result, .. }
            | AbstractOperation::EstablishScalarCase { result, .. }
            | AbstractOperation::CallStructural { result, .. }
            | AbstractOperation::ByteSequenceSubslice { result, .. }
            | AbstractOperation::BoundaryCall {
                result: abstract_operations::AbstractBoundaryResult::Structural(result),
                ..
            } => Some(result.place),
            _ => None,
        };
        if established.is_some_and(|place| !places.insert(place)) {
            return Err(invalid());
        }
    }
    let entries = &function.block_entries;
    if entries.is_empty()
        || entries[0].operation_offset != 0
        || entries.iter().any(|entry| {
            entry.block == function.entry
                && !entry.parameters.is_empty()
                && entry.parameters != function.parameters
        })
    {
        return Err(invalid());
    }
    let mut ranges = Vec::new();
    let mut incoming = vec![Vec::new(); entries.len()];
    let mut outgoing = vec![Vec::new(); entries.len()];
    let mut entry_position = None;
    for (position, entry) in entries.iter().enumerate() {
        if entries[..position]
            .iter()
            .any(|earlier| earlier.block == entry.block)
        {
            return Err(invalid());
        }
        if entry.block == function.entry {
            entry_position = Some(position);
        }
        let end = entries
            .get(position + 1)
            .map_or(function.operations.len(), |next| next.operation_offset);
        if entry.operation_offset >= end || end > function.operations.len() {
            return Err(invalid());
        }
        ranges.push(entry.operation_offset..end);
        let targets = match &function.operations[end - 1] {
            AbstractOperation::ReturnStructural { .. } => Vec::new(),
            AbstractOperation::Return {
                cleanup_actions, ..
            }
            | AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } if cleanup_actions
                .iter()
                .all(|action| matches!(action, TerminalAffineCleanupAction::DiscardRoot(_))) =>
            {
                Vec::new()
            }
            AbstractOperation::Jump {
                target,
                residual_affine_discards,
                ..
            } if residual_affine_discards.is_empty() => {
                vec![*target]
            }
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => {
                vec![when_true.target, when_false.target]
            }
            AbstractOperation::StructuralCase { cases, .. } => {
                cases.iter().map(|case| case.target).collect()
            }
            _ => return Err(invalid()),
        };
        for target_block in targets {
            let target_position = entries
                .iter()
                .position(|candidate| candidate.block == target_block)
                .ok_or_else(invalid)?;
            if !outgoing[position].contains(&target_position) {
                outgoing[position].push(target_position);
                incoming[target_position].push(position);
            }
        }
    }
    let entry_position = entry_position.ok_or_else(invalid)?;
    if !incoming[entry_position].is_empty() {
        return Err(invalid());
    }
    let schedule = dominance::schedule(&incoming, &outgoing, entry_position).ok_or_else(invalid)?;
    let mut live_exits: Vec<Option<LiveDefinitions>> = vec![None; entries.len()];
    let mut lowered = vec![None; entries.len()];
    let mut block_provenance = vec![TerminalPsiProvenance::default(); entries.len()];
    let initial = LiveDefinitions {
        structural_homes: BTreeMap::new(),
        nonreturning: false,
        stored_descriptors: BTreeSet::new(),
        integers: super::function_signature::integer_parameters(
            function.machine,
            &prepared.scalar_parameters,
        )?,
        booleans: BTreeMap::new(),
        scalar_homes: BTreeMap::new(),
        ieee_float_constants: BTreeMap::new(),
        scalar_block_parameters: BTreeMap::new(),
        views: BTreeMap::new(),
        block_views: BTreeSet::new(),
        owned_arrivals: BTreeSet::new(),
        lengths: BTreeMap::new(),
    };
    for (position, dominator) in schedule {
        let mut live = match dominator {
            None => initial.clone(),
            Some(dominator) => live_exits[dominator].as_ref().ok_or_else(invalid)?.clone(),
        };
        live.nonreturning = false;
        if position != entry_position {
            transfers::enter(&entries[position], &mut live);
            live.block_views.extend(
                entries[position]
                    .structural_parameters
                    .iter()
                    .filter(|parameter| {
                        matches!(
                            parameter.access,
                            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                        )
                    })
                    .map(|parameter| parameter.place),
            );
            if !unobserved_owned {
                for parameter in entries[position]
                    .structural_parameters
                    .iter()
                    .filter(|parameter| parameter.access == StructuralAccess::Owned)
                {
                    let home = aggregate_results::block_home(
                        entries[position].block,
                        parameter,
                        structural_types,
                    )?;
                    if live
                        .structural_homes
                        .insert(parameter.place, home)
                        .is_some()
                    {
                        return Err(invalid());
                    }
                }
            }
            live.owned_arrivals.extend(
                entries[position]
                    .structural_parameters
                    .iter()
                    .filter(|parameter| parameter.access == StructuralAccess::Owned)
                    .map(|parameter| parameter.place),
            );
        }
        let range = ranges[position].clone();
        let mut operations = Vec::new();
        let provenance = &mut block_provenance[position];
        for operation in &function.operations[range.start..range.end - 1] {
            lower_operation(
                operation,
                function,
                target,
                functions,
                structural_types,
                boundary_machines,
                settlements,
                installed_calls,
                scalar_abis,
                native_callbacks,
                &prepared,
                &parameters_by_place,
                &mut live,
                &mut operations,
                provenance,
            )?;
        }
        transfers::validate_successors(&function.operations[range.end - 1], function, &live)?;
        let terminator = lower_terminator(
            &function.operations[range.end - 1],
            function,
            &prepared,
            &live,
            structural_types,
            provenance,
        )?;
        lowered[position] = Some(TargetControlBlock {
            block: entries[position].block,
            structural_parameters: entries[position].structural_parameters.clone(),
            parameters: entries[position]
                .parameters
                .iter()
                .map(|parameter| TargetScalarBlockParameter {
                    value: parameter.value,
                    scalar_type: parameter.scalar_type,
                })
                .collect(),
            operations,
            terminator,
        });
        live_exits[position] = Some(live);
    }
    let blocks = lowered
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(invalid)?;
    let mut provenance = TerminalPsiProvenance::default();
    for block in block_provenance {
        provenance.operations.extend(block.operations);
        provenance.edges.extend(block.edges);
    }
    Ok(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        scalar_abi: None,
        mixed_structural_scalar_abi: None,
        provenance,
        graph: TargetControlGraph {
            structural_types: structural_types.catalog().clone(),
            call_plan: prepared.call_plan,
            scalar_parameters: prepared.scalar_parameters,
            parameters: prepared.parameters,
            entry: function.entry,
            blocks,
        },
    })
}
