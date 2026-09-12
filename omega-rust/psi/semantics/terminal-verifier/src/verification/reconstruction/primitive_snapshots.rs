//! Exact reaching-store identities for copied primitive SSA values.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{
    BlockId, EdgeId, MachineId, PlaceId, Proposition, ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::{Block, Operation, OperationKind, StructuralAccess, TerminalMachine};
use terminal_semantics::{StructuralEffectAction, structural_effect_semantic_row};

use crate::ModuleError;

const MAXIMUM_QUERY_WORK: usize = 1_000_000;

/// Invocation-owned query preparation, shared by every read in the machine.
/// Queries stop at exact definitions; they never evaluate source expressions or
/// use the requested contract to choose an equality.
pub(super) struct PrimitiveSnapshots<'a> {
    machine: &'a TerminalMachine,
    blocks: &'a BTreeMap<BlockId, &'a Block>,
    predecessors: BTreeMap<BlockId, Vec<(EdgeId, BlockId)>>,
    cuts: BTreeMap<EdgeId, BlockId>,
    fresh_locals: BTreeSet<PlaceId>,
    parameters: BTreeSet<PlaceId>,
    remaining_work: usize,
}

impl<'a> PrimitiveSnapshots<'a> {
    pub(super) fn new(
        machine: &'a TerminalMachine,
        blocks: &'a BTreeMap<BlockId, &'a Block>,
    ) -> Self {
        let has_reads = machine.blocks.iter().any(|block| {
            block.operations.iter().any(|operation| {
                matches!(operation.kind, OperationKind::PrimitiveScalarRead { .. })
                    && operation.result.scalar_ref().is_some_and(|result| {
                        !matches!(result.scalar_type, ScalarType::IeeeFloat(_))
                    })
            })
        });
        let mut predecessors = BTreeMap::<_, Vec<_>>::new();
        if has_reads {
            for (source, successors) in crate::control_graph::successors(machine) {
                for (edge, target) in successors {
                    predecessors.entry(target).or_default().push((edge, source));
                }
            }
        }
        Self {
            machine,
            blocks,
            predecessors,
            cuts: if has_reads {
                crate::control_graph::feedback_edges(machine)
            } else {
                BTreeMap::new()
            },
            fresh_locals: if has_reads {
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter_map(|operation| {
                        matches!(
                            operation.kind,
                            OperationKind::EstablishPrimitiveLocal { .. }
                        )
                        .then(|| operation.result.structural().map(|result| result.place))
                        .flatten()
                    })
                    .collect()
            } else {
                BTreeSet::new()
            },
            parameters: if has_reads {
                machine
                    .structural_parameters
                    .iter()
                    .map(|parameter| parameter.place)
                    .collect()
            } else {
                BTreeSet::new()
            },
            remaining_work: MAXIMUM_QUERY_WORK,
        }
    }

    pub(super) fn append_read_equation(
        &mut self,
        block: BlockId,
        position: usize,
        operation: &Operation,
        value_types: &BTreeMap<ValueId, ScalarType>,
        axioms: &mut Vec<Proposition>,
    ) -> Result<(), ModuleError> {
        let OperationKind::PrimitiveScalarRead { source } = operation.kind else {
            return Ok(());
        };
        let Some(result) = operation.result.scalar_ref() else {
            return Ok(());
        };
        if matches!(result.scalar_type, ScalarType::IeeeFloat(_)) {
            return Ok(());
        }
        let Some(value) = self.reaching_value(block, position, source)? else {
            return Ok(());
        };
        if value_types.get(&value) == Some(&result.scalar_type) {
            // Both sides denote immutable scalar values. Subsequent writes
            // expire storage observations, not this captured equality.
            axioms.push(Proposition::Equal(
                ScalarTerm::value(result.id, result.scalar_type),
                ScalarTerm::value(value, result.scalar_type),
            ));
        }
        Ok(())
    }

    fn reaching_value(
        &mut self,
        block: BlockId,
        position: usize,
        source: PlaceId,
    ) -> Result<Option<ValueId>, ModuleError> {
        let mut pending = vec![(block, position)];
        let mut visited = BTreeSet::new();
        let mut common = None;
        while let Some((current, before)) = pending.pop() {
            charge_work(&mut self.remaining_work, self.machine.id)?;
            if !visited.insert(current) {
                continue;
            }
            let selected = self.blocks[&current];
            let mut definition = None;
            for operation in selected.operations[..before].iter().rev() {
                charge_work(&mut self.remaining_work, self.machine.id)?;
                match self.operation_effect(operation, source)? {
                    ReachingEffect::Store(value) => {
                        definition = Some(value);
                        break;
                    }
                    ReachingEffect::Unknown => return Ok(None),
                    ReachingEffect::Preserve => {}
                }
            }
            if let Some(value) = definition {
                if common.is_some_and(|previous| previous != value) {
                    return Ok(None);
                }
                common = Some(value);
                continue;
            }
            if current == self.machine.entry {
                return Ok(None);
            }
            let Some(arrivals) = self
                .predecessors
                .get(&current)
                .filter(|arrivals| !arrivals.is_empty())
            else {
                return Ok(None);
            };
            for &(edge, predecessor) in arrivals {
                charge_work(&mut self.remaining_work, self.machine.id)?;
                // A cut is an unknown arrival, never an omitted predecessor.
                // The remaining query graph is acyclic, so shared diamond
                // prefixes may be visited once without mistaking them for loops.
                if self.cuts.contains_key(&edge) {
                    return Ok(None);
                }
                let predecessor_block = self.blocks[&predecessor];
                // Primitive reads validate only original primitive parameters
                // and unrestricted local allocations. Successor cleanup is
                // affine-only/no-code, so it cannot consume these roots. A
                // forwarded structural parameter is not a readable primitive
                // root; call uses of its aliases remain unknown effects below.
                pending.push((predecessor, predecessor_block.operations.len()));
            }
        }
        Ok(common)
    }

    fn operation_effect(
        &mut self,
        operation: &Operation,
        source: PlaceId,
    ) -> Result<ReachingEffect, ModuleError> {
        let stored = match operation.kind {
            OperationKind::EstablishPrimitiveLocal { value } => operation
                .result
                .structural()
                .map(|result| (result.place, value)),
            OperationKind::WriteOnlyPrimitiveStore { destination, value } => {
                Some((destination, value))
            }
            _ => None,
        };
        if let Some((destination, value)) = stored {
            return Ok(if destination == source {
                ReachingEffect::Store(value)
            } else if self.distinct_local_roots(source, destination) {
                ReachingEffect::Preserve
            } else {
                ReachingEffect::Unknown
            });
        }
        // Read effect metadata, not cloned field/array payloads or newly built
        // scalar equations. Ordinary operation reconstruction owns those facts.
        if let Some(row) = structural_effect_semantic_row(&operation.kind)
            .map_err(ModuleError::OperationSemanticSchema)?
        {
            return Ok(match row.schema().action() {
                StructuralEffectAction::EstablishPrimitiveLocal
                | StructuralEffectAction::StorePrimitive
                | StructuralEffectAction::StoreScalarField
                | StructuralEffectAction::StoreByteSequenceField
                | StructuralEffectAction::StoreByteSequenceFieldByte
                | StructuralEffectAction::WriteByteSequence => ReachingEffect::Unknown,
                StructuralEffectAction::ReadPrimitive
                | StructuralEffectAction::ObserveCaseMembership
                | StructuralEffectAction::ReadByteSequenceFieldLength
                | StructuralEffectAction::EstablishByteSequencePlace
                | StructuralEffectAction::ReadByteSequenceLength
                | StructuralEffectAction::ReadByteSequence
                | StructuralEffectAction::EstablishByteSequenceSubslice
                | StructuralEffectAction::ReadBooleanField
                | StructuralEffectAction::ReadIntegerField
                | StructuralEffectAction::EmitPortWrite
                | StructuralEffectAction::EstablishAffinePlace
                | StructuralEffectAction::EstablishRecord
                | StructuralEffectAction::EstablishScalarCase
                | StructuralEffectAction::EstablishScalarArray => ReachingEffect::Preserve,
            });
        }
        let arguments = match &operation.kind {
            OperationKind::CallUnit {
                structural_arguments,
                ..
            }
            | OperationKind::CallStructuralScalar {
                structural_arguments,
                ..
            }
            | OperationKind::CallStructural {
                structural_arguments,
                ..
            }
            | OperationKind::CallStructuralWithScalarArguments {
                structural_arguments,
                ..
            }
            | OperationKind::BoundaryCall {
                structural_arguments,
                ..
            } => Some(structural_arguments),
            OperationKind::Call { .. }
            | OperationKind::IeeeFloatConstant { .. }
            | OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
            | OperationKind::IeeeFloatCompare { .. } => return Ok(ReachingEffect::Preserve),
            _ => None,
        };
        if let Some(arguments) = arguments {
            for argument in arguments {
                charge_work(&mut self.remaining_work, self.machine.id)?;
                if argument.access != StructuralAccess::SharedBorrow
                    && !self.distinct_local_roots(source, argument.place)
                {
                    return Ok(ReachingEffect::Unknown);
                }
            }
            return Ok(ReachingEffect::Preserve);
        }
        if terminal_semantics::operation_semantic_row(&operation.kind)
            .map_err(ModuleError::OperationSemanticSchema)?
            .goal_free_scalar_leaf()
            .is_some()
            || terminal_semantics::proof_bearing_scalar_semantic_row(&operation.kind)
                .map_err(ModuleError::OperationSemanticSchema)?
                .is_some()
        {
            return Ok(ReachingEffect::Preserve);
        }
        // Dynamic descriptors/calls have catalog-owned effects. No absence of
        // an explicit structural argument list is a non-interference proof.
        Ok(ReachingEffect::Unknown)
    }

    fn distinct_local_roots(&self, left: PlaceId, right: PlaceId) -> bool {
        // Only original parameters and fresh allocations have this separation.
        // A different forwarded/reborrowed place identity may still alias a local.
        left != right
            && ((self.fresh_locals.contains(&left)
                && (self.fresh_locals.contains(&right) || self.parameters.contains(&right)))
                || (self.fresh_locals.contains(&right) && self.parameters.contains(&left)))
    }
}

enum ReachingEffect {
    Store(ValueId),
    Preserve,
    Unknown,
}

fn charge_work(remaining: &mut usize, machine: MachineId) -> Result<(), ModuleError> {
    *remaining = remaining.checked_sub(1).ok_or(
        ModuleError::PrimitiveSnapshotReconstructionLimitExceeded(machine),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_snapshot_work_exhaustion_rejects_without_wrapping_or_refilling() {
        let machine = MachineId::new(1).expect("machine");
        let mut remaining = 1;
        charge_work(&mut remaining, machine).expect("last work unit");
        for _ in 0..2 {
            assert_eq!(
                charge_work(&mut remaining, machine),
                Err(ModuleError::PrimitiveSnapshotReconstructionLimitExceeded(
                    machine
                ))
            );
            assert_eq!(remaining, 0);
        }
    }
}
