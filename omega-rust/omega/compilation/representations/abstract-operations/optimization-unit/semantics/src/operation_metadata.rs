//! Reconstructs operation bookkeeping independently of the optimization-unit constructor.
//! The producer cannot depend on this receiving crate. Explicit no-result,
//! no-input and no-ownership cases make new instruction kinds a compile error
//! until both producer and receiver classify them. These checks establish
//! metadata consistency, not an independent proof of the instruction semantics.
use abstract_operations::{AbstractOperation, AbstractStructuralCallResult, binding_arguments};
use optimization_unit::{OwnershipEvent, ValueDefinition, ValueDefinitionSite, ValueUse};
use semantic_vocabulary::{BlockId, ScalarType, ValueId};

/// Checks bookkeeping only; callers separately check provenance, edges and semantics.
pub(crate) fn metadata_matches(
    recorded: &optimization_unit::OptimizationNode,
    block: BlockId,
    node: u32,
) -> bool {
    recorded.definitions == expected_definitions(&recorded.operation, block, node)
        && recorded.uses == expected_uses(&recorded.operation, block, node)
        && recorded.ownership == expected_ownership(&recorded.operation)
}

fn scalar_definition(operation: &AbstractOperation) -> Option<(ValueId, ScalarType)> {
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
        | O::StorageAddress { result, .. }
        | O::PrimitiveScalarRead { result, .. }
        | O::StructuralCaseMembership { result, .. }
        | O::ByteSequenceRead { result, .. }
        | O::ByteSequencePredicate { result, .. }
        | O::ByteSequenceEqual { result, .. }
        | O::ByteSequenceLength { result, .. }
        | O::ElementViewLength { result, .. }
        | O::ElementViewRead { result, .. }
        | O::StructuralByteSequenceFieldLength { result, .. }
        | O::StructuralByteSequenceFieldRead { result, .. }
        | O::StructuralScalarFieldRead { result, .. } => Some((result.value, result.scalar_type)),
        O::BoundaryCall {
            result: abstract_operations::AbstractBoundaryResult::Scalar(result),
            ..
        } => Some((result.value, result.scalar_type)),
        O::BooleanConstant { result, .. }
        | O::BooleanNot { result, .. }
        | O::BooleanEqual { result, .. }
        | O::IeeeFloatCompare { result, .. }
        | O::IntegerEqual { result, .. }
        | O::IntegerLessThan { result, .. }
        | O::IntegerLessOrEqual { result, .. } => Some((*result, ScalarType::Boolean)),
        O::IntegerBitwiseNot {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseAnd {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseOr {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseXor {
            result,
            scalar_type,
            ..
        }
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
        O::IntegerWiden {
            result,
            target_type,
            ..
        }
        | O::IntegerExactCast {
            result,
            target_type,
            ..
        } => Some((*result, ScalarType::Integer(*target_type))),
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

fn scalar_uses(operation: &AbstractOperation) -> Vec<ValueId> {
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
        O::EstablishCase { fields, .. } | O::EstablishRecord { fields, .. } => fields
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
        O::ByteSequenceRead { index, length, .. }
        | O::StructuralByteSequenceFieldRead { index, length, .. } => {
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
        O::ByteSequenceStore { length, .. }
        | O::StructuralByteSequenceFieldStore { length, .. } => vec![*length],
        O::ByteSequenceWrite {
            index,
            value,
            length,
            ..
        }
        | O::StructuralByteSequenceFieldByteStore {
            index,
            value,
            length,
            ..
        }
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
        O::EstablishPrimitiveLocal { value, .. }
        | O::PrimitiveLocalStore { value, .. }
        | O::WriteOnlyPrimitiveStore { value, .. }
        | O::StructuralScalarFieldStore { value, .. } => {
            vec![value.value]
        }
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. }
        | O::ScalarRepresentationView { operand, .. } => vec![*operand],
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
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

pub(crate) fn expected_ownership(operation: &AbstractOperation) -> Vec<OwnershipEvent> {
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
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => {
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

pub(crate) fn expected_definitions(
    operation: &AbstractOperation,
    block: BlockId,
    node: u32,
) -> Vec<ValueDefinition> {
    scalar_definition(operation)
        .into_iter()
        .map(|(value, scalar_type)| ValueDefinition {
            value,
            scalar_type,
            site: ValueDefinitionSite::Node { block, node },
        })
        .collect()
}
pub(crate) fn expected_uses(
    operation: &AbstractOperation,
    block: BlockId,
    node: u32,
) -> Vec<ValueUse> {
    scalar_uses(operation)
        .into_iter()
        .map(|value| ValueUse { value, block, node })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use abstract_operations::AbstractOperation as O;
    use optimization_unit::{EffectLink, OptimizationNode, OwnershipEvent};
    use semantic_vocabulary::{ClaimId, EdgeId, ObligationId, OperationId, PlaceId};
    use terminal_psi::{CrashCause, StructuralAccess, StructuralArgument, StructuralPathSegment};

    fn node(operation: O) -> OptimizationNode {
        OptimizationNode {
            operation,
            provenance: Vec::new(),
            work: Vec::new(),
            effect: EffectLink {
                input: 0,
                output: 1,
            },
            definitions: Vec::new(),
            uses: Vec::new(),
            successors: Vec::new(),
            ownership: Vec::new(),
        }
    }
    fn site() -> (BlockId, u32) {
        (BlockId::new(8).unwrap(), 7)
    }
    fn value(raw: u64) -> ValueId {
        ValueId::new(raw).unwrap()
    }

    #[test]
    fn element_write_inputs_and_path_selector_are_not_optional() {
        let (block, position) = site();
        let operation = O::ElementViewWrite {
            psi_operation: OperationId::new(1).unwrap(),
            destination: StructuralArgument::whole(
                PlaceId::new(1).unwrap(),
                StructuralAccess::MutableBorrow,
            ),
            path: vec![StructuralPathSegment::RuntimeIndex {
                index: value(4),
                obligation: ObligationId::new(2).unwrap(),
            }],
            index: value(1),
            value: value(2),
            length: value(3),
            obligation: ObligationId::new(1).unwrap(),
        };
        let expected: Vec<_> = (1..=4)
            .map(|raw| ValueUse {
                value: value(raw),
                block,
                node: position,
            })
            .collect();
        assert_eq!(expected_uses(&operation, block, position), expected);
        assert_eq!(
            optimization_unit::operation_uses(&operation),
            (1..=4).map(value).collect::<Vec<_>>()
        );
        let mut recorded = node(operation);
        assert!(
            !metadata_matches(&recorded, block, position),
            "an omitted input table must refuse"
        );
        recorded.uses = expected;
        assert!(metadata_matches(&recorded, block, position));
        recorded.uses.remove(1);
        assert!(
            !metadata_matches(&recorded, block, position),
            "the stored value is a real SSA use"
        );
    }

    #[test]
    fn scalar_output_identity_type_and_site_must_match() {
        let (block, position) = site();
        let mut recorded = node(O::BooleanConstant {
            psi_operation: OperationId::new(1).unwrap(),
            result: value(5),
            value: true,
        });
        assert!(!metadata_matches(&recorded, block, position));
        recorded.definitions = vec![ValueDefinition {
            value: value(5),
            scalar_type: ScalarType::Boolean,
            site: ValueDefinitionSite::Node {
                block,
                node: position,
            },
        }];
        assert!(metadata_matches(&recorded, block, position));
        recorded.definitions[0].value = value(6);
        assert!(!metadata_matches(&recorded, block, position));
        recorded.definitions[0].value = value(5);
        recorded.definitions[0].scalar_type = ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 32)
                .unwrap(),
        );
        assert!(!metadata_matches(&recorded, block, position));
        recorded.definitions[0].scalar_type = ScalarType::Boolean;
        recorded.definitions[0].site = ValueDefinitionSite::Node {
            block,
            node: position + 1,
        };
        assert!(!metadata_matches(&recorded, block, position));
    }

    #[test]
    fn recorded_ownership_cannot_omit_or_replace_the_frontier() {
        let (block, position) = site();
        let claim = ClaimId::new(1).unwrap();
        let mut recorded = node(O::Crash {
            psi_edge: EdgeId::new(1).unwrap(),
            cause: CrashCause::Trap,
            site_guard: Vec::new(),
            frontier_lower_bound: vec![claim],
        });
        assert!(!metadata_matches(&recorded, block, position));
        recorded.ownership = vec![OwnershipEvent::CrashFrontier(vec![claim])];
        assert!(metadata_matches(&recorded, block, position));
        recorded.ownership = vec![OwnershipEvent::CrashFrontier(vec![
            ClaimId::new(2).unwrap(),
        ])];
        assert!(!metadata_matches(&recorded, block, position));
    }
}
