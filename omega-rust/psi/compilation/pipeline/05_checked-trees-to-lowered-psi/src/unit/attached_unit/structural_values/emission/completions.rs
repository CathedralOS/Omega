//! Result completion: the value-join that commits one selected place to
//! its continuation, and whole-loan completion. The join binds owner slots
//! positionally against the same roster both sides already read.

use super::super::super::super::{StructuralAccess, StructuralArgument};
use super::super::super::{LoweringError, PlaceId, lookup_type_id, unsupported};
use super::{Emission, ProjectedMove, ValueContinuation};
use crate::expression_preparation::bindings::LoanAccess;
use crate::expression_preparation::bindings::structural_paths::lower_static_argument_path;

impl Emission<'_, '_, '_> {
    /// Completes `place` against `continuation` when one is armed and hands
    /// the place back — the tail every already-built structural value shares.
    pub(super) fn completed(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        place: PlaceId,
        continuation: Option<&ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        if let Some(continuation) = continuation {
            self.complete_value(
                identities,
                StructuralArgument {
                    place,
                    path: Vec::new(),
                    access: self.result_access,
                },
                None,
                continuation,
            )?;
        }
        Ok(place)
    }

    pub(super) fn complete_value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        selected: StructuralArgument,
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
            && !self
                .sources
                .iter()
                .any(|source| source.place == selected.place))
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
                .position(|owner| owner.value.place == selected.place)
        });
        let mut unselected = self
            .sources
            .iter()
            .map(|source| source.place)
            .filter(|place| *place != selected.place && Some(*place) != displaced_place);
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
                if projected.root != selected.place
                    || projected.path.is_empty()
                    || !projected.root_path.is_empty()
                {
                    return unsupported(
                        "projected selection root disagrees with its moved argument",
                    );
                }
                let moved = [(projected.path.as_slice(), projected.leaf_type_identity)];
                super::residual_complement::complement(
                    &self
                        .checked
                        .facts
                        .flow
                        .terminal_unit_effects
                        .structural_types,
                    &projected.root_source,
                    projected.root_type_identity,
                    &moved,
                )?
                .iter()
                .map(|residual| {
                    Ok(terminal_psi::StructuralAffineDiscard {
                        place: selected.place,
                        path: lower_static_argument_path(
                            identities,
                            &residual.path,
                            lookup_type_id(self.type_ids, &projected.root_type_identity)?,
                            self.structural_types,
                        )?,
                        structural_type: lookup_type_id(self.type_ids, &residual.type_identity)?,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?
            }
            None => Vec::new(),
        };
        let mut trivial_affine_discards = Vec::new();
        structural_arguments.push(selected);
        if structural_arguments.len() != continuation.structural_parameters.len() {
            return unsupported("structural value has unequal residual ownership at its join");
        }
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
}
