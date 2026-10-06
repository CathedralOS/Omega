//! Repeated observations of one unchanged storage projection.
//!
//! Selector identity comes from current reconstructed equality facts, never a
//! producer opcode or source spelling. Every incoming path must reach the same
//! scalar read without a mutation, call, custody change or feedback cut. The
//! conclusion relates immutable SSA values and creates no canonical field fact.

use super::{
    BTreeMap, BTreeSet, BlockId, ModuleError, OperationKind, PlaceId, PrimitiveSnapshots,
    Proposition, ScalarTerm, ScalarType, StructuralEffectAction, ValueId, charge_work,
    structural_effect_semantic_row,
};
use terminal_psi::{StructuralPathSegment, Terminator};

impl PrimitiveSnapshots<'_> {
    pub(super) fn reaching_observation(
        &mut self,
        block: BlockId,
        position: usize,
        source: PlaceId,
        path: &[StructuralPathSegment],
        scalar_type: ScalarType,
        value_types: &BTreeMap<ValueId, ScalarType>,
        axioms: &[Proposition],
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
                if let OperationKind::PrimitiveScalarRead {
                    source: prior_source,
                    path: prior_path,
                } = &operation.kind
                    && *prior_source == source
                    && let Some(result) = operation.result.scalar_ref()
                    && result.scalar_type == scalar_type
                    && self.same_observed_path(path, prior_path, value_types, axioms)?
                {
                    definition = Some(result.id);
                    break;
                }
                if !preserves_observations(&operation.kind)? {
                    return Ok(None);
                }
            }
            if let Some(value) = definition {
                if common.is_some_and(|prior| prior != value) {
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
                .filter(|rows| !rows.is_empty())
            else {
                return Ok(None);
            };
            for &(edge, predecessor) in arrivals {
                charge_work(&mut self.remaining_work, self.machine.id)?;
                let predecessor_block = self.blocks[&predecessor];
                if self.cuts.contains_key(&edge)
                    || !edge_preserves_custody(&predecessor_block.terminator, edge)
                {
                    return Ok(None);
                }
                pending.push((predecessor, predecessor_block.operations.len()));
            }
        }
        Ok(common)
    }

    fn same_observed_path(
        &mut self,
        left: &[StructuralPathSegment],
        right: &[StructuralPathSegment],
        value_types: &BTreeMap<ValueId, ScalarType>,
        axioms: &[Proposition],
    ) -> Result<bool, ModuleError> {
        if left.len() != right.len() {
            return Ok(false);
        }
        for (left, right) in left.iter().zip(right) {
            charge_work(&mut self.remaining_work, self.machine.id)?;
            match (left.runtime_index(), right.runtime_index()) {
                (Some((left, _)), Some((right, _))) => {
                    // Obligations authenticate each use's bound, not its
                    // selected address. Equal integer values select equally.
                    if !self.equal_selectors(left, right, value_types, axioms)? {
                        return Ok(false);
                    }
                }
                (None, None) if left == right => {}
                _ => return Ok(false),
            }
        }
        Ok(true)
    }

    fn equal_selectors(
        &mut self,
        left: ValueId,
        right: ValueId,
        value_types: &BTreeMap<ValueId, ScalarType>,
        axioms: &[Proposition],
    ) -> Result<bool, ModuleError> {
        let Some(scalar_type @ ScalarType::Integer(_)) = value_types.get(&left) else {
            return Ok(false);
        };
        if value_types.get(&right) != Some(scalar_type) {
            return Ok(false);
        }
        let expected = ScalarTerm::value(right, *scalar_type);
        let mut pending = vec![ScalarTerm::value(left, *scalar_type)];
        let mut visited = Vec::new();
        while let Some(term) = pending.pop() {
            charge_work(&mut self.remaining_work, self.machine.id)?;
            if term == expected {
                return Ok(true);
            }
            if visited.contains(&term) {
                continue;
            }
            for axiom in axioms {
                charge_work(&mut self.remaining_work, self.machine.id)?;
                let Proposition::Equal(left, right) = axiom else {
                    continue;
                };
                if left == &term && !visited.contains(right) {
                    pending.push(right.clone());
                }
                if right == &term && !visited.contains(left) {
                    pending.push(left.clone());
                }
            }
            visited.push(term);
        }
        Ok(false)
    }
}

fn preserves_observations(kind: &OperationKind) -> Result<bool, ModuleError> {
    if let Some(row) =
        structural_effect_semantic_row(kind).map_err(ModuleError::OperationSemanticSchema)?
    {
        return Ok(matches!(
            row.schema().action(),
            StructuralEffectAction::ReadPrimitive
                | StructuralEffectAction::ObserveCaseMembership
                | StructuralEffectAction::ReadByteSequenceFieldLength
                | StructuralEffectAction::ReadByteSequenceFieldByte
                | StructuralEffectAction::ReadByteSequenceLength
                | StructuralEffectAction::ReadByteSequence
                | StructuralEffectAction::EvaluateByteSequencePredicate
                | StructuralEffectAction::EvaluateByteSequenceEqual
                | StructuralEffectAction::ReadElementViewLength
                | StructuralEffectAction::ReadElementView
                | StructuralEffectAction::ReadBooleanField
                | StructuralEffectAction::ReadIntegerField
        ));
    }
    Ok(terminal_semantics::operation_semantic_row(kind)
        .map_err(ModuleError::OperationSemanticSchema)?
        .goal_free_scalar_leaf()
        .is_some()
        || terminal_semantics::proof_bearing_scalar_semantic_row(kind)
            .map_err(ModuleError::OperationSemanticSchema)?
            .is_some())
}

fn edge_preserves_custody(terminator: &Terminator, selected: semantic_vocabulary::EdgeId) -> bool {
    match terminator {
        Terminator::Jump {
            edge,
            cleanup_actions,
            ..
        } => *edge == selected && cleanup_actions.is_empty(),
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false]
            .iter()
            .any(|edge| edge.edge == selected && edge.cleanup_actions.is_empty()),
        // Case dispatch can consume the selected structural carrier. Other
        // terminators have no continuation to this observation.
        _ => false,
    }
}
