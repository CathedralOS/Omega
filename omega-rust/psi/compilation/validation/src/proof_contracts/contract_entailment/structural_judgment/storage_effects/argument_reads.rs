//! Arguments retain their saved values separately from live borrowed contents.
//! Reference subjects and completed owned values name exact existing cells at
//! the post-operand cut. Entry snapshots and initializer replay cannot replace
//! a cell retired by later argument effects.

use super::super::StructuralTerm;
use super::{CapturedStorageCoordinate, CapturedStorageValue, StorageAnchor, StructuralJudge};
use symbols::SymbolHandle;
use typed_trees::{expression::ExpressionHandle, state::State};

impl StructuralJudge<'_> {
    /// Moving an owned carrier also moves its reference descriptors. Preserve
    /// already observed current scalars under those descriptors, using the
    /// exact completed value rather than treating its pointer words as bytes.
    pub(super) fn owned_argument_storage_values(
        &self,
        entry: &State,
        caller: &State,
        bindings: &[(SymbolHandle, Option<StructuralTerm>)],
    ) -> Vec<CapturedStorageValue> {
        if !self.requires_value_preservation()
            || (self.storage_graph().is_some()
                && !self
                    .storage_version
                    .is_some_and(|version| version.state == caller.symbol))
        {
            return Vec::new();
        }
        let mut values = Vec::new();
        for (symbol, actual) in bindings {
            let Some(parameter) = self.program.state_parameter_by_symbol(entry, *symbol) else {
                continue;
            };
            if crate::reference_shell(self.program, parameter.type_reference).is_some() {
                continue;
            }
            let Some(actual) = actual else {
                continue;
            };
            let Ok((captured, prefix)) = self.completed_result_read_source(actual, 0) else {
                continue;
            };
            let StructuralTerm::CapturedValue {
                expression,
                result_type,
                ..
            } = captured
            else {
                continue;
            };
            if !self
                .program
                .expression_table
                .expression_is_valid(expression)
            {
                continue;
            }
            let Some(source_type) =
                crate::project_type_reference_from_segments(self.program, result_type, &prefix)
            else {
                continue;
            };
            if !crate::declared_carriers_match(self.program, source_type, parameter.type_reference)
            {
                continue;
            }
            let Some(source) = self.capture_storage_place(crate::CanonicalPlace {
                root: facts::PlaceRoot::Expression(expression),
                segments: prefix,
            }) else {
                continue;
            };
            for stored in self.stored_values.iter() {
                if stored.backing.is_none()
                    || stored.current.is_none()
                    || stored.coordinate.root != source.root
                    || !self.storage_value_transfer_is_current(stored, self.storage_version)
                {
                    continue;
                }
                let Some(suffix) = stored
                    .coordinate
                    .segments
                    .strip_prefix(source.segments.as_slice())
                else {
                    continue;
                };
                let coordinate = CapturedStorageCoordinate {
                    root: facts::PlaceRoot::Symbol(parameter.symbol),
                    segments: suffix.to_vec(),
                };
                let Some(path) = Self::coordinate_place(&coordinate) else {
                    continue;
                };
                let Some(geometry) = crate::structural_scalar_coordinate_in_type(
                    self.program,
                    path.root,
                    parameter.type_reference,
                    &path.segments,
                ) else {
                    continue;
                };
                // This common read owner verifies the original capture,
                // backing and current cut, including its public scalar law.
                let Some(current) = self.captured_scalar_read(actual, &path.segments) else {
                    continue;
                };
                let current = self.resolve(current);
                if !super::super::super::structural_terms::scalar_operand_is_observed(
                    self.program,
                    geometry.result_type_reference,
                    &current,
                ) {
                    continue;
                }
                values.push(CapturedStorageValue {
                    coordinate,
                    entry: Some(current.clone()),
                    current: Some(current),
                    selected_case: None,
                    completed_read: None,
                    backing: None,
                    current_anchors: vec![StorageAnchor::Incoming],
                    anchor_state: entry.symbol,
                });
            }
        }
        values
    }

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
        if subject.has_aggregate_view(self.program) {
            return Err("a represented aggregate needs its captured logical field observation");
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
