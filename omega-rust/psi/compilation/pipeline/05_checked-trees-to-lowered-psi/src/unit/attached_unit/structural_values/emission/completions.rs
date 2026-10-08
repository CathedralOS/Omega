//! Result completion: the value-join that commits one selected place to
//! its continuation, whole-loan completion, and projected case-place
//! materialization. The join binds owner slots positionally against the
//! same roster both sides already read.

use super::super::super::super::{StructuralAccess, StructuralArgument};
use super::super::super::{
    LoweringError, OperationKind, PlaceId, StructuralTypeShape, lookup_type_id,
    lower_structural_path,
    operation_frame::{
        allocate_structural_result, operation_result_declaration, structural_type_declaration,
    },
    unsupported,
};
use super::{Emission, ProjectedMove, ValueContinuation};
use crate::expression_preparation::bindings::LoanAccess;

impl Emission<'_, '_, '_> {
    /// Completes `place` against `continuation` when one is armed and hands
    /// the place back — the tail every already-built structural value shares.
    pub(super) fn completed(
        &mut self,
        place: PlaceId,
        continuation: Option<&ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        if let Some(continuation) = continuation {
            self.complete_value(place, None, continuation)?;
        }
        Ok(place)
    }

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
            owned_continuation =
                self.joined_value_continuation(self.values.clone(), self.owners.clone())?;
            &owned_continuation
        };
        let source_count = self.values.len();
        for (position, case) in cases.iter().enumerate() {
            let mut fallback = None;
            if position + 1 < cases.len() {
                self.boolean_case_split(
                    OperationKind::StructuralCaseMembership {
                        source,
                        path: path.clone(),
                        case: case.id,
                    },
                    &mut fallback,
                )?;
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
            self.restore_fallback(fallback);
        }
        if is_root {
            self.record_root_continuation(continuation);
        }
        Ok(continuation.place)
    }
}
