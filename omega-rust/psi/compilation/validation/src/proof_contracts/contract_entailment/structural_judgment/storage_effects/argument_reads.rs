//! A captured reference argument retains its address, while its pointee is
//! observed at the post-operand cut. Exact invocation subjects supply the full
//! source join. Only current cells may answer; entry snapshots and initializer
//! replay cannot replace a cell retired by later argument effects.

use super::super::StructuralTerm;
use super::{CapturedStorageCoordinate, StructuralJudge};
use typed_trees::{expression::ExpressionHandle, state::State};

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn argument_scalar_value(
        &self,
        state: &State,
        subject: &crate::machine_calls::calls::InvocationEntrySubject,
        path: &[facts::PlaceSegment],
        selector: impl FnMut(ExpressionHandle) -> Option<StructuralTerm>,
    ) -> Result<StructuralTerm, &'static str> {
        if !crate::reference_shell(self.program, subject.parameter_type)
            .is_some_and(|shell| shell.access.is_readable())
        {
            return Err("the arriving reference does not permit content observation");
        }
        let geometry = crate::structural_scalar_coordinate_in_type(
            self.program,
            facts::PlaceRoot::Symbol(subject.parameter),
            subject.parameter_type,
            path,
        )
        .ok_or("the arriving rank reference has no readable scalar coordinate")?;
        let suffix = self
            .capture_storage_place_with_selectors(
                crate::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(subject.parameter),
                    segments: path.to_vec(),
                },
                selector,
            )
            .ok_or("the arriving rank reference has an uncaptured selector")?;
        if subject.sources.is_empty() {
            return Err("the arriving rank reference has no exact source subjects");
        }
        let frames = self
            .frame_resolver()
            .ok_or("the arriving rank reference has no frame resolver")?;
        let mut selected = None;
        for source in &subject.sources {
            // The current-cell ledger is declaration-relative. Performed
            // outputs, opaque result referents, and image backing retain
            // other namespaces; none may be relabelled as a same-symbol
            // declaration observation.
            if source.invocation_output.is_some()
                || source.invocation_result.is_some()
                || source.is_image_storage
            {
                return Err(
                    "the arriving rank source has no declaration-relative current-cell namespace",
                );
            }
            let scalar = self
                .program
                .primitive_type_reference(subject.type_reference)
                .and_then(|_| source.scalar_storage_coordinate(self.program, state));
            let place = scalar
                .as_ref()
                .map(|scalar| scalar.place.clone())
                .unwrap_or_else(|| crate::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(source.root),
                    segments: source.segments.clone(),
                });
            let mut coordinate: CapturedStorageCoordinate = self
                .capture_storage_place(place)
                .ok_or("the arriving rank reference lost its captured address selectors")?;
            if !coordinate.has_stable_backing(self.program, state, frames)
                && self.storage_binding_invalidated(source.root)
            {
                return Err("an argument effect invalidated the arriving rank reference binding");
            }
            coordinate.segments.extend_from_slice(&suffix.segments);
            let value = self
                .current_storage_value(&coordinate)
                .ok_or("the arriving rank reference has no current cell observation")?;
            let value = match scalar {
                Some(scalar) if !scalar.preserves_scalar_value => {
                    super::super::super::structural_terms::normalize_scalar_representation(
                        scalar.source_primitive,
                        scalar.view_primitive,
                        value,
                    )
                }
                _ => value,
            };
            if !super::super::super::structural_terms::scalar_operand_is_observed(
                self.program,
                geometry.result_type_reference,
                &value,
            ) {
                return Err(
                    "the arriving rank reference supplies a descriptor without scalar contents",
                );
            }
            if selected.as_ref().is_some_and(|known| *known != value) {
                return Err(
                    "the arriving rank reference sources do not agree on the observed value",
                );
            }
            selected = Some(value);
        }
        selected.ok_or("the arriving rank reference has no complete current value")
    }
}
