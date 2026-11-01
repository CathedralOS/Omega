//! One exact store can preserve current scalar cells outside its destination.
//! Capture after operand effects, then join across the ordinary coarse frame.
//! Calls and unknown stores still retire associations through that frame alone.

use super::super::scalar_fields::{capture_path, same_projection, source_path};
use super::super::{CurrentScalarValues, ScalarField, ScalarProjection, integer_carrier};
use typed_trees::expression::BinaryOperator;
use typed_trees::statement::StatementNode;

impl CurrentScalarValues<'_> {
    pub(in super::super) fn disjoint_scalar_fields_for_store(
        &self,
        statement: &StatementNode,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Vec<ScalarField> {
        if !self.fields.iter().any(|field| field.value.is_some()) {
            return Vec::new();
        }
        self.capture_disjoint_scalar_fields(statement, frames)
            .unwrap_or_default()
    }

    fn capture_disjoint_scalar_fields(
        &self,
        statement: &StatementNode,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<Vec<ScalarField>> {
        let frames = frames?;
        // RHS effects have already retired observations. They must also leave
        // the captured destination's selectors and descriptor bindings current.
        frames
            .assignment_destination_remains_current(self.machine, statement)
            .then_some(())?;
        let origins = frames.assignment_storage_origins(self.machine, statement)?;
        let [destination] = origins.destinations.as_slice() else {
            return None;
        };
        if !destination.root.is_valid()
            || destination.invocation_output.is_some()
            || destination.is_image_storage
            || !destination.recasts.is_empty()
            || destination.representation.is_some()
        {
            return None;
        }
        let place = validation::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(destination.root),
            segments: destination.segments.clone(),
        };
        frames
            .captured_assignment_destination_remains_current(self.machine, statement, &place)
            .then_some(())?;
        let written = frames
            .assignment_write_frame(self.machine, statement)
            .into_complete_paths()?;
        let mut footprint = validation::frame_coordinate_path(
            self.program,
            destination.root,
            &destination.segments,
        )?;
        if destination.collection_coarse
            && destination.segments.iter().any(|segment| {
                matches!(
                    segment,
                    facts::PlaceSegment::Index { .. } | facts::PlaceSegment::FixedIndex { .. }
                )
            })
        {
            footprint.push_str("[*]");
        }
        if written != [footprint] {
            return None;
        }
        let mut reference = validation::declaration_type_reference(self.program, destination.root)?;
        if let Some(shell) = validation::reference_shell(self.program, reference) {
            // As in the normal storage owner, only an incoming exclusive
            // reference to reference-free storage has a stable root namespace.
            // A local alias or a nested descriptor needs its own backing join.
            let parameter = self
                .program
                .state_parameter_by_symbol(self.state, destination.root)?;
            if parameter.type_reference != reference
                || shell.access != language_semantics::ReferenceAccess::Mutable
                || !frames.value_is_reference_free(shell.referee)
            {
                return None;
            }
            reference = shell.referee;
        }
        validation::owned_projection_type_reference(
            self.program,
            reference,
            &destination.segments,
        )?;
        let destination_path = capture_path(&destination.segments, |selector| {
            self.capture(self.program, self.machine, self.state, selector)
        })?;
        Some(
            self.fields
                .iter()
                .filter(|field| field.owner == destination.root && field.value.is_some())
                .filter(|field| {
                    written.iter().any(|write| {
                        validation::frame_paths_overlap(write, &field.name)
                            || field.read_dependencies.iter().any(|dependency| {
                                validation::frame_paths_overlap(write, dependency)
                            })
                    })
                })
                .filter(|field| {
                    let path = source_path(&field.path);
                    // No reference may be crossed or loaded by this scalar
                    // association. Distinct lexical roots prove no separation.
                    let Some(selected) =
                        validation::owned_projection_type_reference(self.program, reference, &path)
                    else {
                        return false;
                    };
                    if validation::reference_shell(self.program, selected).is_some()
                        || integer_carrier(self.program, selected) != Some(field.carrier)
                        || !self.scalar_projections_disjoint(&field.path, &destination_path)
                    {
                        return false;
                    }
                    field.read_dependencies.iter().all(|dependency| {
                        !written
                            .iter()
                            .any(|write| validation::frame_paths_overlap(write, dependency))
                            || (0..=path.len()).any(|length| {
                                // Inline collection reads retain a coarse backing
                                // prefix as a dependency. The typed, frozen path
                                // above proves that backing stable for this store;
                                // unrelated overlapping dependencies still expire.
                                validation::frame_coordinate_path(
                                    self.program,
                                    field.owner,
                                    &path[..length],
                                )
                                .is_some_and(|prefix| prefix == *dependency)
                            })
                    })
                })
                .cloned()
                .collect(),
        )
    }

    fn scalar_projections_disjoint(
        &self,
        left: &[ScalarProjection],
        right: &[ScalarProjection],
    ) -> bool {
        for (left, right) in left.iter().zip(right) {
            if same_projection(
                self.occurrence(),
                std::slice::from_ref(left),
                std::slice::from_ref(right),
            ) {
                continue;
            }
            return match (left, right) {
                (ScalarProjection::Field(left), ScalarProjection::Field(right)) => left != right,
                (
                    ScalarProjection::Element { selector: left, .. },
                    ScalarProjection::Element {
                        selector: right, ..
                    },
                ) => {
                    validation::scoped_arithmetic_values_relation(
                        self.program,
                        self.machine,
                        &self.hypotheses,
                        BinaryOperator::NotEqual,
                        left,
                        right,
                    ) == validation::StrictArithmeticImplicationJudgment::Proven
                }
                _ => false,
            };
        }
        false
    }

    pub(in super::super) fn restore_disjoint_scalar_fields(&mut self, preserved: Vec<ScalarField>) {
        for saved in preserved {
            if let Some(field) = self.fields.iter_mut().find(|field| {
                field.owner == saved.owner
                    && field.path == saved.path
                    && field.carrier == saved.carrier
                    && field.value.is_none()
            }) {
                // Restore the current value and its original dependencies,
                // never an entry snapshot or a newly evaluated initializer.
                field.value = saved.value;
                field.read_dependencies = saved.read_dependencies;
            }
        }
    }
}
