/*
We deterministically project an `AbstractOperationPlan` into a bare
`PsiOptimizationUnit` seed and its per-function `PsiOptimizationFunction` graphs.
We check that machine and block IDs are strictly increasing, slice the flat
operation stream into basic blocks, and extract each node's scalar definitions,
operand uses, control-flow successor edges, provenance sites, and ownership
events in a single pass. This seed carries no admission authority until verified
Terminal module catalogs and frontier facts are attached.
*/

use super::{
    AbstractFunction, AbstractOperation, AbstractOperationPlan, AbstractSuccessor, Arc, BTreeSet,
    BlockId, EffectLink, MachineId, ObligationId, OperationId, OptimizationBlock, OptimizationEdge,
    OptimizationFact, OptimizationNode, OptimizationUnitIdentity, OwnershipEvent, PlaceId,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, ScalarType,
    StructuralPlaceDeclaration, StructuralPlaceKind, TerminalRootServiceReach, ValueDefinition,
    ValueDefinitionSite, ValueId, ValueUse, WorkScheduleIdentity, WorkSettlement,
    recompute_psi_optimization_unit_identity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationUnitBuildError {
    MissingBlocks(MachineId),
    FirstBlockDoesNotStartAtZero(MachineId),
    InvalidBlockOffset { machine: MachineId, offset: usize },
    UnorderedBlock(MachineId, BlockId),
    UnorderedMachine(MachineId),
    NodeIndexOverflow(MachineId),
    ParameterIndexOverflow(MachineId),
}

impl std::fmt::Display for OptimizationUnitBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot construct canonical Psi optimization unit: {self:?}"
        )
    }
}

impl std::error::Error for OptimizationUnitBuildError {}

pub fn reconstruct_psi_optimization_unit_seed(
    plan: &AbstractOperationPlan,
    work_schedule: WorkScheduleIdentity,
) -> Result<PsiOptimizationUnit, OptimizationUnitBuildError> {
    let functions = plan
        .functions
        .iter()
        .map(reconstruct_psi_optimization_function_seed)
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(pair) = functions
        .windows(2)
        .find(|pair| pair[0].machine >= pair[1].machine)
    {
        return Err(OptimizationUnitBuildError::UnorderedMachine(
            pair[1].machine,
        ));
    }
    let mut unit = PsiOptimizationUnit {
        identity: OptimizationUnitIdentity::from_canonical_bytes(b"pending canonical content"),
        psi: plan.psi,
        work_schedule,
        entry: plan.entry,
        structural_types: plan.structural_types.clone(),
        structural_domains: Arc::new([]),
        services: Arc::new([]),
        root_service_reach: TerminalRootServiceReach::default(),
        boundary_machines: plan.boundary_machines.clone(),
        provider_candidates: plan.provider_candidates.clone(),
        dynamic_dispatch: plan.dynamic_dispatch.clone(),
        callables: plan.callables.clone(),
        accepted_obligation_facts: Vec::new(),
        ownership_frontier_facts: Vec::new(),
        pruned_machines: Vec::new(),
        identities: plan.identities.clone(),
        runtime_evidence: None,
        functions,
    };
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    Ok(unit)
}

pub fn reconstruct_psi_optimization_function_seed(
    function: &AbstractFunction,
) -> Result<PsiOptimizationFunction, OptimizationUnitBuildError> {
    if function.block_entries.is_empty() {
        return Err(OptimizationUnitBuildError::MissingBlocks(function.machine));
    }
    if function.block_entries[0].operation_offset != 0 {
        return Err(OptimizationUnitBuildError::FirstBlockDoesNotStartAtZero(
            function.machine,
        ));
    }
    let mut previous_block = None;
    for entry in &function.block_entries {
        if entry.operation_offset > function.operations.len() {
            return Err(OptimizationUnitBuildError::InvalidBlockOffset {
                machine: function.machine,
                offset: entry.operation_offset,
            });
        }
        if previous_block.is_some_and(|previous| previous >= entry.block) {
            return Err(OptimizationUnitBuildError::UnorderedBlock(
                function.machine,
                entry.block,
            ));
        }
        previous_block = Some(entry.block);
    }

    let parameters = function
        .parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| {
            Ok(ValueDefinition {
                value: parameter.value,
                scalar_type: parameter.scalar_type,
                site: ValueDefinitionSite::FunctionParameter(u32::try_from(position).map_err(
                    |_| OptimizationUnitBuildError::ParameterIndexOverflow(function.machine),
                )?),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut facts = Vec::new();
    let mut structural_places = function
        .structural_parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
            },
        })
        .chain(
            function
                .result
                .structural()
                .map(|result| StructuralPlaceDeclaration {
                    id: result.place,
                    kind: StructuralPlaceKind::Result,
                }),
        )
        .collect::<Vec<_>>();
    let mut declared_places = function
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .chain(function.entry_claims.iter().map(|claim| claim.input))
        .chain(function.result.structural().map(|result| result.place))
        .collect::<BTreeSet<_>>();
    let mut effect_token = 0u64;
    for entry in &function.block_entries {
        for parameter in &entry.structural_parameters {
            structural_places.push(StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::BlockParameter {
                    block: entry.block,
                    position: parameter.position,
                },
            });
            declared_places.insert(parameter.place);
        }
    }
    let mut blocks = Vec::with_capacity(function.block_entries.len());
    for (block_index, entry) in function.block_entries.iter().enumerate() {
        let end = function
            .block_entries
            .get(block_index + 1)
            .map_or(function.operations.len(), |next| next.operation_offset);
        if end < entry.operation_offset {
            return Err(OptimizationUnitBuildError::InvalidBlockOffset {
                machine: function.machine,
                offset: end,
            });
        }
        let block_parameter_rows = entry
            .parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| {
                Ok(ValueDefinition {
                    value: parameter.value,
                    scalar_type: parameter.scalar_type,
                    site: ValueDefinitionSite::BlockParameter {
                        block: entry.block,
                        position: u32::try_from(position).map_err(|_| {
                            OptimizationUnitBuildError::ParameterIndexOverflow(function.machine)
                        })?,
                    },
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut nodes = Vec::with_capacity(end - entry.operation_offset);
        for (local_index, operation) in function.operations[entry.operation_offset..end]
            .iter()
            .enumerate()
        {
            let node = u32::try_from(local_index)
                .map_err(|_| OptimizationUnitBuildError::NodeIndexOverflow(function.machine))?;
            let provenance = operation_node_provenance(operation);
            let work = provenance
                .iter()
                .copied()
                .map(|site| WorkSettlement { site, units: 1 })
                .collect();
            let definitions = operation_definition(operation)
                .into_iter()
                .map(|(value, scalar_type)| ValueDefinition {
                    value,
                    scalar_type,
                    site: ValueDefinitionSite::Node {
                        block: entry.block,
                        node,
                    },
                })
                .collect();
            let uses = operation_uses(operation)
                .into_iter()
                .map(|value| ValueUse {
                    value,
                    block: entry.block,
                    node,
                })
                .collect();
            collect_places(operation, &mut declared_places);
            collect_operation_structural_places(operation, &mut structural_places);
            collect_fact(operation, &mut facts);
            let ownership = operation_ownership(operation);
            let successors = operation_edges(operation);
            nodes.push(OptimizationNode {
                operation: operation.clone(),
                provenance,
                work,
                effect: EffectLink {
                    input: effect_token,
                    output: effect_token + 1,
                },
                definitions,
                uses,
                successors,
                ownership,
            });
            effect_token += 1;
        }
        blocks.push(OptimizationBlock {
            structural_parameters: entry.structural_parameters.clone(),
            id: entry.block,
            parameters: block_parameter_rows,
            nodes,
        });
    }

    Ok(PsiOptimizationFunction {
        machine: function.machine,
        attachment: function.attachment,
        nominal_cleanup_subject: function.nominal_cleanup_subject,
        entry: function.entry,
        parameters,
        structural_parameters: function.structural_parameters.clone(),
        structural_places,
        result: function.result.clone(),
        declared_places,
        entry_claim_declarations: function.entry_claims.clone(),
        block_entry_claims: function.block_entry_claims.clone(),
        content_entry_claims: Vec::new(),
        verified_contract: None,
        evidence_contract_lanes: Vec::new(),
        entry_claims: function
            .entry_claims
            .iter()
            .map(|claim| claim.claim)
            .collect(),
        published_service_ceiling: function.published_service_ceiling.clone(),
        declared_service_reach: Vec::new(),
        facts,
        blocks,
    })
}

pub(super) fn operation_edges(operation: &AbstractOperation) -> Vec<OptimizationEdge> {
    use AbstractOperation as O;
    match operation {
        O::Jump {
            psi_edge,
            target,
            bindings,
            structural_bindings,
            cleanup_actions,
        } => vec![OptimizationEdge {
            structural_bindings: structural_bindings.clone(),
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
            cleanup_actions: cleanup_actions.clone(),
            provenance: vec![PsiProvenance::Edge(*psi_edge)],
            work: vec![WorkSettlement {
                site: PsiProvenance::Edge(*psi_edge),
                units: 1,
            }],
        }],
        O::Conditional {
            when_true,
            when_false,
            ..
        } => vec![successor_edge(when_true), successor_edge(when_false)],
        O::StructuralCase { cases, .. } => cases
            .iter()
            .map(|case| OptimizationEdge {
                psi_edge: case.psi_edge,
                target: case.target,
                bindings: Vec::new(),
                structural_bindings: case.structural_bindings.clone(),
                cleanup_actions: case.cleanup_actions.clone(),
                provenance: vec![PsiProvenance::Edge(case.psi_edge)],
                work: vec![WorkSettlement {
                    site: PsiProvenance::Edge(case.psi_edge),
                    units: 1,
                }],
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn successor_edge(successor: &AbstractSuccessor) -> OptimizationEdge {
    OptimizationEdge {
        structural_bindings: successor.structural_bindings.clone(),
        psi_edge: successor.psi_edge,
        target: successor.target,
        bindings: successor.bindings.clone(),
        cleanup_actions: successor.cleanup_actions.clone(),
        provenance: vec![PsiProvenance::Edge(successor.psi_edge)],
        work: vec![WorkSettlement {
            site: PsiProvenance::Edge(successor.psi_edge),
            units: 1,
        }],
    }
}

use abstract_operations::element_view_read_write_pair_pattern;

pub(super) fn collect_fact(operation: &AbstractOperation, facts: &mut Vec<OptimizationFact>) {
    if let Some((obligation, support)) = operation_obligation(operation) {
        facts.push(OptimizationFact::OperationObligationReference {
            obligation,
            support,
        });
    }
    if let Some(support) = operation.psi_operation() {
        facts.extend(
            operation
                .runtime_indices()
                .into_iter()
                .map(
                    |(_, obligation)| OptimizationFact::OperationObligationReference {
                        obligation,
                        support,
                    },
                ),
        );
    }
    match operation {
        abstract_operations::establish_record_or_case_pattern!(psi_operation, fields, ..) => {
            facts.extend(fields.iter().filter_map(|initializer| {
                let terminal_psi::RecordFieldValue::Scalar {
                    range_obligation, ..
                } = &initializer.value
                else {
                    return None;
                };
                range_obligation.map(
                    |obligation| OptimizationFact::OperationObligationReference {
                        obligation,
                        support: *psi_operation,
                    },
                )
            }));
        }

        AbstractOperation::BooleanConstant {
            psi_operation,
            result,
            value,
        } => facts.push(OptimizationFact::BooleanConstant {
            value: *result,
            constant: *value,
            support: *psi_operation,
        }),
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            value,
            ..
        } => facts.push(OptimizationFact::IntegerConstant {
            value: *result,
            constant: *value,
            support: *psi_operation,
        }),
        _ => {}
    }
}

fn operation_obligation(operation: &AbstractOperation) -> Option<(ObligationId, OperationId)> {
    use AbstractOperation as O;
    match operation {
        O::EstablishPrimitiveLocal {
            psi_operation,
            range_obligation: Some(obligation),
            ..
        }
        | O::WriteOnlyPrimitiveStore {
            psi_operation,
            range_obligation: Some(obligation),
            ..
        }
        | O::StructuralScalarFieldStore {
            psi_operation,
            range_obligation: Some(obligation),
            ..
        }
        | O::AtomicEvent {
            psi_operation,
            resident_obligation: Some(obligation),
            ..
        }
        | O::StructuralByteViewFieldStore {
            psi_operation,
            predicate_obligation: Some(obligation),
            ..
        }
        | O::EstablishByteSequenceConcatenation {
            psi_operation,
            capacity_obligation: obligation,
            ..
        }
        | O::ByteSequenceSubslice {
            psi_operation,
            obligation,
            ..
        }
        | O::ByteSequenceWrite {
            psi_operation,
            obligation,
            ..
        }
        | abstract_operations::byte_sequence_store_pair_pattern!(psi_operation, obligation, ..)
        | O::StructuralByteSequenceFieldByteStore {
            psi_operation,
            obligation,
            ..
        }
        | abstract_operations::byte_read_or_field_read_pattern!(psi_operation, obligation, ..)
        | O::ElementViewSubslice {
            psi_operation,
            obligation,
            ..
        }
        | O::EstablishElementView {
            psi_operation,
            region:
                terminal_psi::ElementViewRegion::RepresentationBytes { obligation, .. }
                | terminal_psi::ElementViewRegion::WholeRepresentation { obligation, .. },
            ..
        }
        | element_view_read_write_pair_pattern!(psi_operation, obligation, ..)
        | O::IntegerExactCast {
            psi_operation,
            obligation,
            ..
        } => Some((*obligation, *psi_operation)),
        O::IntegerShift {
            psi_operation,
            policy,
            ..
        } => policy
            .obligation()
            .map(|obligation| (obligation, *psi_operation)),
        O::IntegerArithmetic {
            psi_operation,
            arithmetic,
            ..
        } => arithmetic
            .obligation()
            .map(|obligation| (obligation, *psi_operation)),
        _ => None,
    }
}

pub(super) fn operation_node_provenance(operation: &AbstractOperation) -> Vec<PsiProvenance> {
    use AbstractOperation as O;
    let site = match operation {
        abstract_operations::jump_or_conditional_pattern!() | O::StructuralCase { .. } => {
            return Vec::new();
        }
        abstract_operations::return_or_return_unit_pattern!(psi_edge, ..)
        | O::InvokeNoNormal { psi_edge, .. }
        | O::ExternalCompletion { psi_edge, .. }
        | O::Crash { psi_edge, .. } => PsiProvenance::Edge(*psi_edge),
        O::ReturnStructural {
            psi_edge,
            trivial_affine_locals,
            ..
        } => {
            return std::iter::once(PsiProvenance::Edge(*psi_edge))
                .chain(
                    trivial_affine_locals
                        .iter()
                        .map(|(operation, _, _)| PsiProvenance::Operation(*operation)),
                )
                .collect();
        }
        _ => PsiProvenance::Operation(
            operation
                .psi_operation()
                .expect("non-control operation names its authored psi operation"),
        ),
    };
    vec![site]
}

use abstract_operations::AbstractStructuralCallResult;
use abstract_operations::binding_arguments;

pub fn operation_definition(operation: &AbstractOperation) -> Option<(ValueId, ScalarType)> {
    use AbstractOperation as O;
    match operation {
        O::RegisterAccess { result, .. } => result.map(|result| (result.value, result.scalar_type)),
        O::ActivateTaskRegister { result, .. }
        | O::ReadTaskRegister { result, .. }
        | O::ReloadSegmentDescriptors { result, .. }
        | O::ReadSegmentSelector { result, .. } => Some((result.value, result.scalar_type)),
        O::Call {
            result,
            scalar_type,
            ..
        }
        | O::IntegerConstant {
            result,
            scalar_type,
            ..
        } => Some((*result, *scalar_type)),
        O::IeeeFloatConstant { result, value, .. } => {
            Some((*result, ScalarType::IeeeFloat(value.format())))
        }
        O::NearestIeeeFloatFusedMultiplyAdd { result, format, .. }
        | O::FloatResultPolicy { result, format, .. } => {
            Some((*result, ScalarType::IeeeFloat(*format)))
        }
        O::CallStructural {
            result: AbstractStructuralCallResult::Scalar(result),
            ..
        }
        | O::SelectImplementation { result, .. }
        | O::SelectMachine { result, .. }
        | abstract_operations::primitive_storage_observation_pattern!(result, ..)
        | abstract_operations::byte_read_predicate_equal_pattern!(result, ..)
        | O::ByteSequenceLength { result, .. }
        | O::ElementViewLength { result, .. }
        | O::ElementViewRead { result, .. }
        | abstract_operations::byte_len_or_field_read_pattern!(result, ..)
        | O::StructuralScalarFieldRead { result, .. } => Some((result.value, result.scalar_type)),
        O::BoundaryCall {
            result: abstract_operations::AbstractBoundaryResult::Scalar(result),
            ..
        } => Some((result.value, result.scalar_type)),
        O::BooleanConstant { result, .. }
        | abstract_operations::boolean_not_or_equal_pair_pattern!(result, ..)
        | O::IeeeFloatCompare { result, .. }
        | abstract_operations::integer_equal_less_pair_pattern!(result, ..)
        | O::IntegerLessOrEqual { result, .. } => Some((*result, ScalarType::Boolean)),
        abstract_operations::bitwise_not_and_pair_pattern!(result, scalar_type, ..)
        | abstract_operations::bitwise_or_xor_pair_pattern!(result, scalar_type, ..)
        | O::IntegerArithmetic {
            result,
            scalar_type,
            ..
        }
        | O::TrappingInteger {
            result,
            scalar_type,
            ..
        } => Some((*result, ScalarType::Integer(*scalar_type))),
        abstract_operations::integer_widen_or_exact_cast_pattern!(result, target_type, ..) => {
            Some((*result, ScalarType::Integer(*target_type)))
        }
        O::IntegerShift {
            result, value_type, ..
        } => Some((*result, ScalarType::Integer(*value_type))),
        O::ScalarRepresentationView {
            result,
            target_type,
            ..
        } => Some((*result, *target_type)),
        O::AtomicEvent { event, .. } => {
            use abstract_operations::AbstractAtomicEvent as E;
            match event {
                E::Load { result, .. } => Some((result.value, result.scalar_type)),
                E::ReadModifyWrite { prior, .. }
                | E::Swap { prior, .. }
                | E::CompareExchange {
                    observed: prior, ..
                } => Some((prior.value, prior.scalar_type)),
                E::Store { .. } | E::CompareExchangeOnce { .. } | E::Fence { .. } => None,
            }
        }
        O::EstablishActivationClaim { .. }
        | O::ReleaseActivationClaim { .. }
        | O::SplitActivationClaim { .. }
        | O::MergeActivationClaim { .. }
        | O::EstablishNativeCodeLayout { .. }
        | O::GlobalDescriptorTable { .. }
        | O::EstablishScalarArray { .. }
        | O::EstablishStructuralArray { .. }
        | O::EstablishPrimitiveLocal { .. }
        | O::PrimitiveLocalStore { .. }
        | O::ByteSequenceSubslice { .. }
        | O::ByteSequenceWrite { .. }
        | O::StructuralByteSequenceFieldByteStore { .. }
        | O::ByteSequenceStore { .. }
        | O::StructuralByteSequenceFieldStore { .. }
        | O::StructuralByteViewFieldStore { .. }
        | O::EstablishElementView { .. }
        | O::ElementViewWrite { .. }
        | O::ElementViewSubslice { .. }
        | O::WriteOnlyPrimitiveStore { .. }
        | O::StructuralScalarFieldStore { .. }
        | O::MoveStructuralField { .. }
        | O::MoveStructuralValue { .. }
        | O::TransformContent { .. }
        | O::StructuralLeafCopy { .. }
        | O::StoreStructuralField { .. }
        | O::EstablishCase { .. }
        | O::EstablishByteSequenceConcatenation { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishZeroedStructural { .. }
        | O::EstablishStructuralLocal { .. }
        | O::EstablishRecord { .. }
        | O::PortWrite { .. }
        | O::Jump { .. }
        | O::Conditional { .. }
        | O::StructuralCase { .. }
        | O::Return { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::EstablishAccessReference { .. }
        | O::EstablishReference { .. }
        | O::ReleaseReference { .. }
        | O::InvokeNoNormal { .. }
        | O::ExternalCompletion { .. }
        | O::Crash { .. }
        | O::CallStructural {
            result:
                AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Structural { .. },
            ..
        }
        | O::BoundaryCall {
            result:
                abstract_operations::AbstractBoundaryResult::Unit
                | abstract_operations::AbstractBoundaryResult::Structural(_),
            ..
        } => None,
    }
}

pub fn operation_uses(operation: &AbstractOperation) -> Vec<ValueId> {
    let mut uses = direct_uses(operation);
    uses.extend(
        operation
            .runtime_indices()
            .into_iter()
            .map(|(index, _)| index),
    );
    uses
}

fn direct_uses(operation: &AbstractOperation) -> Vec<ValueId> {
    use AbstractOperation as O;
    match operation {
        O::EstablishActivationClaim { committed, .. }
        | O::SplitActivationClaim {
            committed_at: committed,
            ..
        } => vec![*committed],
        O::ReleaseActivationClaim { .. } | O::MergeActivationClaim { .. } => Vec::new(),
        O::RegisterAccess { event, .. } => event.inputs().copied().collect(),
        O::ActivateTaskRegister { selector, .. } | O::ReloadSegmentDescriptors { selector, .. } => {
            vec![*selector]
        }
        O::ReadTaskRegister { .. } | O::ReadSegmentSelector { .. } => Vec::new(),
        abstract_operations::establish_case_or_record_fields_pattern!(fields) => fields
            .iter()
            .filter_map(|initializer| match &initializer.value {
                terminal_psi::RecordFieldValue::Scalar { value, .. } => Some(*value),
                terminal_psi::RecordFieldValue::Structural(_) => None,
            })
            .collect(),
        O::EstablishStructuralArray { .. } => Vec::new(),
        O::EstablishScalarArray { elements, .. } => elements.clone(),
        O::EstablishReference { storage, .. } => {
            storage.iter().map(|view| view.byte_offset).collect()
        }
        abstract_operations::byte_read_or_field_read_pattern!(index, length, ..) => {
            vec![*index, *length]
        }
        O::ElementViewRead { index, length, .. } => vec![*index, *length],
        O::EstablishByteSequenceConcatenation { left, right, .. } => {
            vec![left.length(), right.length()]
        }
        O::ByteSequencePredicate { operand, .. } => vec![operand.length()],
        O::ByteSequenceEqual { left, right, .. } => {
            vec![left.length(), right.length()]
        }
        abstract_operations::byte_sequence_store_pair_pattern!(length, ..) => vec![*length],
        abstract_operations::byte_sequence_write_pair_pattern!(index, value, length, ..)
        | O::ElementViewWrite {
            index,
            value,
            length,
            ..
        } => {
            vec![*index, *value, *length]
        }
        O::ByteSequenceSubslice {
            start, end, length, ..
        } => vec![*start, *end, *length],
        O::ElementViewSubslice {
            start, end, length, ..
        } => vec![*start, *end, *length],
        O::EstablishElementView {
            region: terminal_psi::ElementViewRegion::WholeRepresentation { source_length, .. },
            ..
        } => vec![*source_length],
        O::EstablishElementView {
            region: terminal_psi::ElementViewRegion::RepresentationBytes { start, end, .. },
            ..
        } => vec![*start, *end],
        O::CallStructural {
            callee, arguments, ..
        } => callee
            .selector()
            .into_iter()
            .chain(arguments.iter().copied())
            .collect(),
        O::Call { arguments, .. }
        | O::BoundaryCall { arguments, .. }
        | O::InvokeNoNormal { arguments, .. }
        | O::ExternalCompletion { arguments, .. } => arguments.clone(),
        abstract_operations::primitive_local_triple_pattern!(value, ..)
        | O::StructuralScalarFieldStore { value, .. } => {
            vec![value.value]
        }
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | abstract_operations::integer_widen_or_exact_cast_pattern!(operand, ..)
        | O::ScalarRepresentationView { operand, .. } => vec![*operand],
        O::BooleanEqual { left, right, .. }
        | abstract_operations::integer_equal_less_pair_pattern!(left, right, ..)
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | abstract_operations::bitwise_or_xor_pair_pattern!(left, right, ..)
        | O::IntegerArithmetic { left, right, .. } => vec![*left, *right],
        O::TrappingInteger { operation, .. } => operation.operands(),
        O::FloatResultPolicy { policy, .. } => policy.operands(),
        O::IeeeFloatCompare { left, right, .. } => vec![*left, *right],
        O::NearestIeeeFloatFusedMultiplyAdd {
            left,
            right,
            addend,
            ..
        } => vec![*left, *right, *addend],
        O::IntegerShift { value, count, .. } => vec![*value, *count],
        O::Jump { bindings, .. } => binding_arguments(bindings).collect(),
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => std::iter::once(*condition)
            .chain(binding_arguments(&when_true.bindings))
            .chain(binding_arguments(&when_false.bindings))
            .collect(),
        O::Return { value, .. } => vec![*value],
        O::AtomicEvent { event, .. } => {
            use abstract_operations::AbstractAtomicEvent as E;
            match event {
                E::Store { value, .. } | E::Swap { value, .. } => vec![*value],
                E::ReadModifyWrite { operand, .. } => vec![*operand],
                E::CompareExchange {
                    expected,
                    replacement,
                    ..
                }
                | E::CompareExchangeOnce {
                    expected,
                    replacement,
                    ..
                } => vec![*expected, *replacement],
                E::Load { .. } | E::Fence { .. } => Vec::new(),
            }
        }
        O::EstablishNativeCodeLayout { .. }
        | O::GlobalDescriptorTable { .. }
        | O::StorageAddress { .. }
        | O::PrimitiveScalarRead { .. }
        | O::StructuralCaseMembership { .. }
        | O::StructuralByteViewFieldStore { .. }
        | O::StructuralByteSequenceFieldLength { .. }
        | O::ByteSequenceLength { .. }
        | O::ElementViewLength { .. }
        | O::MoveStructuralField { .. }
        | O::MoveStructuralValue { .. }
        | O::TransformContent { .. }
        | O::StructuralLeafCopy { .. }
        | O::StoreStructuralField { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishZeroedStructural { .. }
        | O::EstablishStructuralLocal { .. }
        | O::SelectImplementation { .. }
        | O::SelectMachine { .. }
        | O::PortWrite { .. }
        | O::IntegerConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::BooleanConstant { .. }
        | O::StructuralScalarFieldRead { .. }
        | O::StructuralCase { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::EstablishAccessReference { .. }
        | O::ReleaseReference { .. }
        | O::Crash { .. }
        | O::EstablishElementView {
            region: terminal_psi::ElementViewRegion::WholeCollection,
            ..
        } => Vec::new(),
    }
}

pub(super) fn collect_places(operation: &AbstractOperation, places: &mut BTreeSet<PlaceId>) {
    use AbstractOperation as O;
    match operation {
        O::EstablishByteSequenceConcatenation {
            result,
            left,
            right,
            ..
        } => {
            places.extend([result.place, left.source(), right.source()]);
        }
        O::EstablishStructuralArray {
            result, elements, ..
        } => {
            places.insert(result.place);
            places.extend(elements.iter().map(|argument| argument.place));
        }
        abstract_operations::establish_record_or_case_pattern!(result, fields, ..) => {
            places.insert(result.place);
            places.extend(
                fields
                    .iter()
                    .filter_map(|initializer| match &initializer.value {
                        terminal_psi::RecordFieldValue::Structural(argument) => {
                            Some(argument.place)
                        }
                        terminal_psi::RecordFieldValue::Scalar { .. } => None,
                    }),
            );
        }
        O::Jump {
            structural_bindings,
            ..
        } => {
            places.extend(
                structural_bindings
                    .iter()
                    .flat_map(|binding| [binding.parameter, binding.argument.place]),
            );
        }
        O::Conditional {
            when_true,
            when_false,
            ..
        } => {
            places.extend(
                when_true
                    .structural_bindings
                    .iter()
                    .chain(&when_false.structural_bindings)
                    .flat_map(|binding| [binding.parameter, binding.argument.place]),
            );
        }
        O::ByteSequenceSubslice { source, result, .. } => {
            places.insert(source.place);
            places.insert(result.place);
        }
        O::ElementViewSubslice { source, result, .. } => {
            places.insert(source.place);
            places.insert(result.place);
        }
        O::EstablishElementView { result, source, .. } => {
            places.insert(result.place);
            places.insert(source.place);
        }
        O::PrimitiveLocalStore { destination, .. } => {
            places.insert(*destination);
        }
        O::WriteOnlyPrimitiveStore { destination, .. } => {
            places.insert(destination.place());
        }
        O::StructuralScalarFieldStore { destination, .. } => {
            places.insert(destination.place());
        }
        abstract_operations::byte_literal_establish_pair_pattern!(place, ..) => {
            places.insert(place.id);
        }
        O::EstablishActivationClaim { result, .. }
        | O::EstablishNativeCodeLayout { result, .. }
        | O::EstablishPrimitiveLocal { result, .. }
        | O::EstablishZeroedStructural { result, .. }
        | O::EstablishStructuralLocal { result, .. }
        | O::EstablishScalarArray { result, .. }
        | O::CallStructural {
            result: AbstractStructuralCallResult::Structural { result, .. },
            ..
        }
        | O::BoundaryCall {
            result: abstract_operations::AbstractBoundaryResult::Structural(result),
            ..
        } => {
            places.insert(result.place);
        }
        O::TransformContent {
            source,
            parents,
            result,
            ..
        } => {
            places.insert(result.place);
            places.insert(source.place);
            places.extend(parents.iter().map(|parent| parent.place));
        }
        O::StructuralLeafCopy { source, result, .. } => {
            places.insert(*source);
            places.insert(result.place);
        }
        O::EstablishReference { result, source, .. }
        | O::MoveStructuralValue { result, source, .. }
        | O::EstablishAccessReference { result, source, .. } => {
            places.insert(result.place);
            places.insert(source.place);
        }
        O::ElementViewLength { source, .. } | O::ByteSequenceLength { source, .. } => {
            places.insert(source.place);
        }
        O::ReleaseActivationClaim { source, .. } => {
            places.insert(source.place);
        }
        O::ReleaseReference { source, .. } => {
            places.insert(*source);
        }
        O::MoveStructuralField { source, result, .. } => {
            places.insert(source.place());
            places.insert(result.place);
        }
        O::StoreStructuralField {
            destination, value, ..
        } => {
            places.insert(destination.place());
            places.insert(value.place);
        }
        abstract_operations::byte_sequence_store_pair_pattern!(destination, source, ..) => {
            places.insert(*destination);
            places.insert(*source);
        }
        abstract_operations::primitive_storage_observation_pattern!(source, ..)
        | O::ByteSequenceRead { source, .. }
        | abstract_operations::byte_sequence_write_pair_pattern!(destination: source, ..)
        | O::ElementViewRead { source, .. }
        | abstract_operations::byte_len_or_field_read_pattern!(source, ..)
        | O::StructuralScalarFieldRead { source, .. }
        | O::ReturnStructural { source, .. } => {
            places.insert(*source);
        }
        O::StructuralCase { source, cases, .. } => {
            places.insert(*source);
            places.extend(
                cases
                    .iter()
                    .flat_map(|case| &case.structural_bindings)
                    .flat_map(|binding| [binding.parameter, binding.argument.place]),
            );
        }
        O::AtomicEvent { event, .. } => {
            if let Some(place) = event.place() {
                places.insert(place);
            }
            if let abstract_operations::AbstractAtomicEvent::CompareExchangeOnce {
                outcome, ..
            } = event
            {
                places.insert(outcome.place);
            }
        }
        _ => {}
    }
}

pub(super) fn collect_operation_structural_places(
    operation: &AbstractOperation,
    structural_places: &mut Vec<StructuralPlaceDeclaration>,
) {
    match operation {
        abstract_operations::establish_or_subslice_set_pattern!(psi_operation, result, ..)
        | abstract_operations::establish_record_or_case_pattern!(psi_operation, result, ..)
        | AbstractOperation::EstablishNativeCodeLayout {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::EstablishZeroedStructural {
            psi_operation,
            result,
        }
        | AbstractOperation::EstablishStructuralLocal {
            psi_operation,
            result,
        }
        | AbstractOperation::EstablishReference {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::EstablishAccessReference {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::MoveStructuralField {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::CallStructural {
            psi_operation,
            result: AbstractStructuralCallResult::Structural { result, .. },
            ..
        }
        | AbstractOperation::StructuralLeafCopy {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::MoveStructuralValue {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::TransformContent {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::BoundaryCall {
            psi_operation,
            result: abstract_operations::AbstractBoundaryResult::Structural(result),
            ..
        } => structural_places.push(StructuralPlaceDeclaration {
            id: result.place,
            kind: StructuralPlaceKind::OperationResult {
                producer: *psi_operation,
                structural_type: result.structural_type,
            },
        }),
        abstract_operations::byte_literal_establish_pair_pattern!(place, ..) => {
            structural_places.push(*place);
        }
        AbstractOperation::AtomicEvent {
            psi_operation,
            event: abstract_operations::AbstractAtomicEvent::CompareExchangeOnce { outcome, .. },
            ..
        } => structural_places.push(StructuralPlaceDeclaration {
            id: outcome.place,
            kind: StructuralPlaceKind::OperationResult {
                producer: *psi_operation,
                structural_type: outcome.structural_type,
            },
        }),
        _ => {}
    }
}

pub fn operation_ownership(operation: &AbstractOperation) -> Vec<OwnershipEvent> {
    use AbstractOperation as O;
    match operation {
        O::CallStructural {
            claim_transfers, ..
        }
        | O::InvokeNoNormal {
            claim_transfers, ..
        } => vec![OwnershipEvent::ClaimTransfer(
            claim_transfers
                .iter()
                .map(|transfer| transfer.claim)
                .collect(),
        )],
        O::BoundaryCall {
            completion_receipts,
            ..
        } => vec![OwnershipEvent::ClaimCompletion(completion_receipts.clone())],
        abstract_operations::return_or_return_unit_pattern!(cleanup_actions, ..) => {
            vec![OwnershipEvent::Cleanup(cleanup_actions.clone())]
        }
        O::ReturnStructural {
            returned_claims,
            cleanup_actions,
            ..
        } => {
            vec![
                OwnershipEvent::StructuralReturn(
                    returned_claims
                        .iter()
                        .map(|binding| binding.claim)
                        .collect(),
                ),
                OwnershipEvent::Cleanup(cleanup_actions.clone()),
            ]
        }
        O::ExternalCompletion {
            frontier_lower_bound,
            ..
        } => {
            vec![OwnershipEvent::ExternalCompletionFrontier(
                frontier_lower_bound.clone(),
            )]
        }
        O::Crash {
            frontier_lower_bound,
            ..
        } => {
            vec![OwnershipEvent::CrashFrontier(frontier_lower_bound.clone())]
        }
        O::EstablishActivationClaim { .. }
        | O::ReleaseActivationClaim { .. }
        | O::SplitActivationClaim { .. }
        | O::MergeActivationClaim { .. }
        | O::EstablishNativeCodeLayout { .. }
        | O::ActivateTaskRegister { .. }
        | O::ReloadSegmentDescriptors { .. }
        | O::ReadSegmentSelector { .. }
        | O::ReadTaskRegister { .. }
        | O::GlobalDescriptorTable { .. }
        | O::StorageAddress { .. }
        | O::EstablishScalarArray { .. }
        | O::EstablishStructuralArray { .. }
        | O::EstablishPrimitiveLocal { .. }
        | O::PrimitiveLocalStore { .. }
        | O::PrimitiveScalarRead { .. }
        | O::StructuralCaseMembership { .. }
        | O::ByteSequenceSubslice { .. }
        | O::ByteSequenceRead { .. }
        | O::ByteSequencePredicate { .. }
        | O::ByteSequenceEqual { .. }
        | O::ByteSequenceWrite { .. }
        | O::StructuralByteSequenceFieldByteStore { .. }
        | O::ByteSequenceStore { .. }
        | O::StructuralByteSequenceFieldStore { .. }
        | O::StructuralByteViewFieldStore { .. }
        | O::StructuralByteSequenceFieldRead { .. }
        | O::StructuralByteSequenceFieldLength { .. }
        | O::ByteSequenceLength { .. }
        | O::EstablishElementView { .. }
        | O::ElementViewLength { .. }
        | O::ElementViewRead { .. }
        | O::ElementViewWrite { .. }
        | O::ElementViewSubslice { .. }
        | O::WriteOnlyPrimitiveStore { .. }
        | O::StructuralScalarFieldStore { .. }
        | O::MoveStructuralField { .. }
        | O::MoveStructuralValue { .. }
        | O::TransformContent { .. }
        | O::StructuralLeafCopy { .. }
        | O::StoreStructuralField { .. }
        | O::AtomicEvent { .. }
        | O::EstablishCase { .. }
        | O::EstablishByteSequenceConcatenation { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishZeroedStructural { .. }
        | O::EstablishStructuralLocal { .. }
        | O::EstablishRecord { .. }
        | O::SelectImplementation { .. }
        | O::SelectMachine { .. }
        | O::PortWrite { .. }
        | O::RegisterAccess { .. }
        | O::Call { .. }
        | O::IntegerConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::FloatResultPolicy { .. }
        | O::NearestIeeeFloatFusedMultiplyAdd { .. }
        | O::IeeeFloatCompare { .. }
        | O::BooleanConstant { .. }
        | O::StructuralScalarFieldRead { .. }
        | O::BooleanNot { .. }
        | O::BooleanEqual { .. }
        | O::IntegerEqual { .. }
        | O::IntegerLessThan { .. }
        | O::IntegerLessOrEqual { .. }
        | O::IntegerBitwiseNot { .. }
        | O::IntegerWiden { .. }
        | O::IntegerExactCast { .. }
        | O::ScalarRepresentationView { .. }
        | O::IntegerBitwiseAnd { .. }
        | O::IntegerBitwiseOr { .. }
        | O::IntegerBitwiseXor { .. }
        | O::IntegerShift { .. }
        | O::IntegerArithmetic { .. }
        | O::TrappingInteger { .. }
        | O::Jump { .. }
        | O::Conditional { .. }
        | O::StructuralCase { .. }
        | O::EstablishAccessReference { .. }
        | O::EstablishReference { .. }
        | O::ReleaseReference { .. } => Vec::new(),
    }
}
