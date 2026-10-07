//! Result completion: the value-join that commits one selected place to
//! its continuation, whole-loan completion, and projected case-place
//! materialization. The join binds owner slots positionally against the
//! same roster both sides already read.

use super::super::super::super::{StructuralAccess, StructuralArgument, block_id};
use super::super::super::{
    LoweringError, Operation, OperationKind, PlaceId, ScalarType, StructuralTypeShape, Terminator,
    ValueDeclaration, allocate_dense, argument_evaluation, lookup_type_id, lower_structural_path,
    operation_frame::{
        allocate_structural_result, block_parameter_declaration, operation_result_declaration,
        structural_parameter_declaration, structural_type_declaration,
    },
    place_id, unsupported, value_id,
};
use super::{Emission, ProjectedMove, ValueContinuation};
use crate::emission::operation_emission::buffer::scalar_result_declaration;
use crate::expression_preparation::bindings::LoanAccess;

impl Emission<'_, '_, '_> {
    pub(super) fn complete_value(
        &mut self,
        selected: PlaceId,
        projection: Option<&ProjectedMove>,
        continuation: &ValueContinuation,
    ) -> Result<(), LoweringError> {
        // The join binds owner slots positionally, so arguments are built
        // against the same roster: the displaced slot (the first candidate,
        // or the moved root on an ordinary move) is absent, every other
        // candidate slot receives the next still-unselected source in roster
        // order, and owners interleaved between candidates keep their own
        // places. A fresh result cannot occupy a source's residual slot, so
        // the unselected source the result slot displaced dies on this edge.
        // Plain-affine contents have no cleanup, making it a trivial discard,
        // and the join frontier is identical on every incoming path.
        let displaced = (!self.sources.is_empty()
            && !self.sources.iter().any(|source| source.place == selected))
        .then(|| self.first_candidate())
        .flatten();
        let displaced_place = displaced.map(|position| self.owners[position].value.place);
        // The first candidate's owner slot is the one the result parameter
        // displaced; a selected non-first candidate's slot is an ordinary
        // residual slot filled by the surviving candidates. An ordinary move
        // instead removes the moved root's own slot.
        let removed = self.first_candidate().or_else(|| {
            self.owners
                .iter()
                .position(|owner| owner.value.place == selected)
        });
        let mut unselected = self
            .sources
            .iter()
            .map(|source| source.place)
            .filter(|place| *place != selected && Some(*place) != displaced_place);
        let mut structural_arguments = Vec::new();
        for (position, owner) in self.owners.iter().enumerate() {
            if Some(position) == removed {
                continue;
            }
            let place = if self
                .sources
                .iter()
                .any(|source| source.place == owner.value.place)
            {
                let residual = unselected.next().ok_or(LoweringError::Unsupported(
                    "owned selection residual sources do not cover their slots",
                ))?;
                // A residual slot carries the surviving source at the slot's
                // own type. The roster's ordered complement can only rebind a
                // source of the same type: on an edge selecting a later
                // alternative the survivor sequence shifts, so a differently
                // typed root would land in its sibling's slot. Reject that
                // custody join here rather than emit an edge the terminal
                // verifier must refuse.
                let residual_type = self
                    .owners
                    .iter()
                    .find(|candidate| candidate.value.place == residual)
                    .map(|candidate| candidate.value.structural_type)
                    .ok_or(LoweringError::Unsupported(
                        "owned selection residual source has no frontier type",
                    ))?;
                if residual_type != owner.value.structural_type {
                    return unsupported(
                        "owned selection residual sources need uniform custody types",
                    );
                }
                residual
            } else {
                owner.value.place
            };
            structural_arguments.push(StructuralArgument {
                place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            });
        }
        if unselected.next().is_some() {
            return unsupported("owned selection has untransported residual candidates");
        }
        let selected_path = projection
            .map(|projected| lower_structural_path(&projected.path))
            .transpose()?
            .unwrap_or_default();
        structural_arguments.push(StructuralArgument {
            place: selected,
            path: selected_path,
            access: StructuralAccess::Owned,
        });
        if structural_arguments.len() != continuation.structural_parameters.len() {
            return unsupported("structural value has unequal residual ownership at its join");
        }
        // A projected move keeps the root's untouched structural complement
        // owned on this same edge. Reconstruction against the checked root
        // type yields the exact residual schedule in positional order; the
        // terminal verifier replays it independently.
        let residual_affine_discards = match projection {
            Some(projected) => {
                // The residual complement is derived against the checked
                // root type, so the selected place must be that whole root.
                // An aliased root holds no storage of its own and has no
                // residual schedule to derive here.
                if projected.root != selected
                    || projected.path.is_empty()
                    || !projected.root_path.is_empty()
                {
                    return unsupported(
                        "projected selection root disagrees with its moved argument",
                    );
                }
                let moved = [(
                    projected.path.as_slice(),
                    projected.leaf_type_identity.as_str(),
                )];
                crate::unit::unit_cleanup::checked_partial_affine_residuals(
                    &self
                        .checked
                        .facts
                        .flow
                        .terminal_unit_effects
                        .structural_types,
                    &projected.root_source,
                    &projected.root_type_identity,
                    &moved,
                )?
                .iter()
                .map(|residual| {
                    Ok(terminal_psi::StructuralAffineDiscard {
                        place: selected,
                        path: lower_structural_path(&residual.path)?,
                        structural_type: lookup_type_id(self.type_ids, &residual.type_identity)?,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?
            }
            None => Vec::new(),
        };
        let mut trivial_affine_discards = Vec::new();
        if let Some(place) = displaced_place {
            trivial_affine_discards.push(place);
        }
        let edge = self.edge(
            continuation.block,
            self.values.iter().map(|value| value.id).collect(),
            structural_arguments,
        )?;
        self.finish_jump(
            edge,
            trivial_affine_discards
                .into_iter()
                .map(terminal_psi::TerminalAffineCleanupAction::DiscardRoot)
                .chain(
                    residual_affine_discards
                        .into_iter()
                        .map(terminal_psi::TerminalAffineCleanupAction::DiscardResidual),
                )
                .collect(),
        );
        Ok(())
    }

    /// A borrowed branch joins one referent place at the result parameter
    /// under its loan. Unlike `complete_value` there is no owner roster, no
    /// displaced candidate and no residual schedule: the referent's owner
    /// stays live, and the single structural parameter must be the result
    /// slot of the loan's access this edge's argument fills. A shared slot is
    /// also `Unrestricted`; an exclusive slot keeps the multiplicity the
    /// binding declares.
    pub(super) fn complete_loan(
        &mut self,
        argument: StructuralArgument,
        continuation: &ValueContinuation,
        loan: LoanAccess,
    ) -> Result<(), LoweringError> {
        if continuation.structural_parameters.len() != 1
            || !continuation.pass_through.is_empty()
            || !continuation.residuals.is_empty()
        {
            return unsupported(match loan {
                LoanAccess::Shared => "borrowed selection joined residual ownership",
                LoanAccess::Mutable => "mutable borrowed selection joined residual ownership",
            });
        }
        let edge = self.edge(
            continuation.block,
            self.values.iter().map(|value| value.id).collect(),
            vec![argument],
        )?;
        self.finish_jump(edge, Vec::new());
        Ok(())
    }

    /// A copyable payloadless case leaf projected out of shared-borrowed
    /// storage: `self.bitness` observes which case the borrowed record holds
    /// through `StructuralCaseMembership`, then establishes that same
    /// constructor fresh under `EstablishScalarCase`. Each non-final case
    /// guards its own block; the closed sum makes the last case unconditional.
    /// Owners join untouched exactly as a scalar dispatch's residual frontier.
    pub(super) fn projected_case_place(
        &mut self,
        argument: &StructuralArgument,
        continuation: Option<&ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        let leaf_type = self.structural_type;
        let StructuralTypeShape::Sum { cases } = &structural_type_declaration(
            self.structural_types,
            leaf_type,
            "projected case copy lost its leaf type",
        )?
        .shape
        else {
            return unsupported("projected case copy requires a case-sum leaf");
        };
        if cases.is_empty() || cases.iter().any(|case| !case.fields.is_empty()) {
            return unsupported("projected case copy requires payloadless cases");
        }
        let cases = cases.clone();
        let source = argument.place;
        let path = argument.path.clone();
        let owned_continuation;
        let is_root = continuation.is_none();
        let continuation = if let Some(continuation) = continuation {
            continuation
        } else {
            let join = block_id(allocate_dense(self.next_block)?);
            let joined_values = self
                .values
                .iter()
                .map(|value| {
                    Ok(ValueDeclaration {
                        id: value_id(allocate_dense(self.next_value)?),
                        ..*value
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let mut structural_parameters = Vec::new();
            let mut remaining_owners = self.owners.clone();
            let mut pass_through = Vec::new();
            let mut residuals = Vec::new();
            for position in 0..remaining_owners.len() + 1 {
                let (structural_type, multiplicity) = remaining_owners
                    .get(position)
                    .map_or((self.structural_type, self.multiplicity), |owner| {
                        (owner.value.structural_type, owner.value.multiplicity)
                    });
                let is_result_slot = position == remaining_owners.len();
                let owner = remaining_owners.get_mut(position);
                let position = u32::try_from(position).map_err(|_| {
                    LoweringError::Unsupported("owned selection parameter count exceeds u32")
                })?;
                let place = place_id(allocate_dense(self.next_place)?);
                if let Some(owner) = owner {
                    if self
                        .sources
                        .iter()
                        .any(|source| source.place == owner.value.place)
                    {
                        owner.symbol = symbols::SymbolHandle::invalid();
                        residuals.push(place);
                    } else {
                        pass_through.push((owner.value.place, place));
                    }
                    owner.value.place = place;
                }
                self.temporary_places
                    .push(block_parameter_declaration(place, join, position));
                structural_parameters.push(structural_parameter_declaration(
                    place,
                    position,
                    structural_type,
                    multiplicity,
                    if is_result_slot {
                        self.result_access
                    } else {
                        StructuralAccess::Owned
                    },
                ));
            }
            let place = structural_parameters
                .last()
                .ok_or(LoweringError::Unsupported(
                    "structural continuation has no result",
                ))?
                .place;
            owned_continuation = ValueContinuation {
                block: join,
                parameters: joined_values,
                structural_parameters,
                place,
                remaining_owners,
                pass_through,
                residuals,
            };
            &owned_continuation
        };
        let source_count = self.values.len();
        for (position, case) in cases.iter().enumerate() {
            let mut fallback = None;
            if position + 1 < cases.len() {
                let condition = value_id(allocate_dense(self.next_value)?);
                let id = self.operations.allocate();
                self.operations.push(Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id,
                    result: scalar_result_declaration(condition, ScalarType::Boolean),
                    kind: OperationKind::StructuralCaseMembership {
                        source,
                        path: path.clone(),
                        case: case.id,
                    },
                });
                let selected = block_id(allocate_dense(self.next_block)?);
                let next = block_id(allocate_dense(self.next_block)?);
                let when_true = self.edge(selected, Vec::new(), Vec::new())?;
                let when_false = self.edge(next, Vec::new(), Vec::new())?;
                self.finish(Terminator::Conditional {
                    condition,
                    when_true,
                    when_false,
                });
                fallback = Some((next, self.values.clone()));
                self.start(selected);
            }
            self.values.truncate(source_count);
            let (place, operation) = allocate_structural_result(
                self.operations,
                self.next_place,
                leaf_type,
                self.multiplicity,
                Vec::new(),
                OperationKind::EstablishScalarCase {
                    result_case: case.id,
                    fields: Vec::new(),
                },
            )?;
            self.temporary_places
                .push(operation_result_declaration(place, operation, leaf_type));
            self.complete_value(place, None, continuation)?;
            if let Some((next, values)) = fallback {
                self.start(next);
                *self.values = values;
            }
        }
        if is_root {
            self.start(continuation.block);
            *self.values = continuation.parameters.clone();
            self.evaluation.parameters = continuation.parameters.clone();
            self.evaluation.block_structural_parameters =
                continuation.structural_parameters.clone();
            if !self.sources.is_empty() {
                self.evaluation
                    .selection_cleanups
                    .push(argument_evaluation::SelectionCleanup {
                        selected: continuation.place,
                        sources: self
                            .sources
                            .iter()
                            .rev()
                            .map(|source| source.place)
                            .collect(),
                        remaining: continuation.residuals.iter().rev().copied().collect(),
                        pass_through: continuation.pass_through.clone(),
                        next_operation: self.operations.next_identity,
                    });
                self.evaluation.structural_value_owners = continuation.remaining_owners.clone();
                self.evaluation.structural_locals.retain(
                    |crate::expression_preparation::bindings::StructuralLocalBinding {
                         argument,
                         ..
                     }| {
                        !self
                            .sources
                            .iter()
                            .any(|source| source.place == argument.place)
                    },
                );
                for crate::expression_preparation::bindings::StructuralLocalBinding {
                    argument,
                    ..
                } in &mut self.evaluation.structural_locals
                {
                    if let Some((_, target)) = continuation
                        .pass_through
                        .iter()
                        .find(|(source, _)| *source == argument.place)
                    {
                        argument.place = *target;
                    }
                }
            }
        }
        Ok(continuation.place)
    }
}
