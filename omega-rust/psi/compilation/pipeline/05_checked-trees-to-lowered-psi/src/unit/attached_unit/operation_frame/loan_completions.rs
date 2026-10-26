//! Statement calls use the same normal-return custody join as computed calls.
use super::OperationFrame;
use super::StructuralArgument;
use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::lowering_error::LoweringError;

impl OperationFrame<'_, '_> {
    pub(super) fn publish_loan_completions(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        call: &checked_trees::CheckedUnitCallPlan,
    ) -> Result<(), LoweringError> {
        let coordinate = SourceCallCoordinate {
            state: self.state,
            statement_index: call.coordinate.statement_index as usize,
            call_ordinal: call.coordinate.call_ordinal as usize,
        };
        let (_, state) = crate::declarations::state_declaration(self.checked, self.state)?;
        let prepared = crate::emission::loan_completions::prepare(
            identities,
            self.checked,
            self.machine,
            coordinate,
            self.structural_types.declarations(),
            |root| {
                crate::expression_preparation::bindings::owned_structural_home(
                    root,
                    self.checked.state_parameters(state),
                    &self.evaluation.structural_parameters,
                    &self.evaluation.structural_locals,
                )
            },
        )?;
        for completion in &prepared {
            let mut qualifications = Vec::new();
            let mut projected_qualifications = Vec::new();
            for (semantic_domain, path, carrier) in &completion.qualifications {
                let domain = if let Some(carrier) = carrier {
                    crate::terminal_identities::lookup_domain_id_for_carrier(
                        self.callees.domain_ids,
                        *semantic_domain,
                        *carrier,
                    )
                    .or_else(|_| {
                        crate::terminal_identities::lookup_domain_id(
                            self.callees.domain_ids,
                            *semantic_domain,
                        )
                    })?
                } else {
                    crate::terminal_identities::lookup_domain_id(
                        self.callees.domain_ids,
                        *semantic_domain,
                    )?
                };
                if path.is_empty() {
                    qualifications.push(domain);
                } else {
                    projected_qualifications.push(terminal_psi::StructuralPathQualification {
                        path: path.clone(),
                        domain,
                    });
                }
            }
            qualifications.sort();
            qualifications.dedup();
            projected_qualifications.sort();
            projected_qualifications.dedup();
            if !qualifications.is_empty() || !projected_qualifications.is_empty() {
                for (_, parameter) in &mut self.evaluation.structural_parameters {
                    if parameter.place == completion.place {
                        parameter.qualifications.extend_from_slice(&qualifications);
                        parameter.qualifications.sort();
                        parameter.qualifications.dedup();
                        parameter
                            .projected_qualifications
                            .extend_from_slice(&projected_qualifications);
                        parameter.projected_qualifications.sort();
                        parameter.projected_qualifications.dedup();
                    }
                }
                for parameter in &mut self.evaluation.block_structural_parameters {
                    if parameter.place == completion.place {
                        parameter.qualifications.extend_from_slice(&qualifications);
                        parameter.qualifications.sort();
                        parameter.qualifications.dedup();
                        parameter
                            .projected_qualifications
                            .extend_from_slice(&projected_qualifications);
                        parameter.projected_qualifications.sort();
                        parameter.projected_qualifications.dedup();
                    }
                }
            }
        }
        crate::emission::loan_completions::emit(&prepared, coordinate, self.operations, self.calls)?;
        self.publish_call_ensures_guarantees(identities, coordinate, state)
    }

    fn publish_call_ensures_guarantees(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        coordinate: SourceCallCoordinate,
        state: &checked_trees::state::State,
    ) -> Result<(), LoweringError> {
        let point = facts::ProgramPoint::CallEnsures {
            machine_symbol: self.machine,
            state_symbol: self.state,
            statement_index: coordinate.statement_index,
            call_ordinal: coordinate.call_ordinal,
        };
        let types = self.structural_types.declarations().to_vec();
        for (_, fact) in self.checked.facts.semantic.facts.iter() {
            if fact.origin != facts::FactOrigin::CallEnsures || fact.point != point {
                continue;
            }
            let Some(place_handle) = fact.place.as_place() else {
                continue;
            };
            let place = self.checked.facts.semantic.places.get(place_handle);
            let Some((_, _, _domain_symbol, semantic_domain)) = fact.payload.domain_membership() else {
                continue;
            };
            let home = self
                .evaluation
                .structural_locals
                .iter()
                .find(|local| local.root == place.root)
                .map(|local| (local.argument.clone(), local.structural_type))
                .or_else(|| {
                    let symbol = place.root.as_symbol()?;
                    let position = self
                        .checked
                        .state_parameters(state)
                        .iter()
                        .position(|param| param.symbol == symbol)?;
                    let (_, parameter) = self
                        .evaluation
                        .structural_parameters
                        .iter()
                        .find(|(source, _)| *source as usize == position)?;
                    Some((
                        StructuralArgument {
                            place: parameter.place,
                            path: Vec::new(),
                            access: parameter.access,
                        },
                        parameter.structural_type,
                    ))
                });
            let Some((owner, structural_type)) = home else {
                continue;
            };
            let Some(declaration) = terminal_psi::find_structural_type(&types, structural_type) else {
                continue;
            };
            let (target_carrier, prefix_path) = match declaration.shape {
                terminal_psi::StructuralTypeShape::Reference { referent, .. } => {
                    (referent, vec![terminal_psi::StructuralPathSegment::Referent])
                }
                _ => (structural_type, Vec::new()),
            };
            let segments = self
                .checked
                .facts
                .semantic
                .place_segments
                .span_or_empty(place.segments);
            let mut full_path = prefix_path;
            if !segments.is_empty() {
                let Some(reference) = validation::canonical_place_type_reference(
                    &self.checked.typed,
                    self.state,
                    coordinate.statement_index,
                    &validation::CanonicalPlace {
                        root: place.root,
                        segments: segments.to_vec(),
                    },
                ) else {
                    continue;
                };
                let Ok((fact_path, _)) = validation::structural_owned_path(
                    &self.checked.typed,
                    reference,
                    segments,
                ) else {
                    continue;
                };
                let Ok(lowered_segments) = crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                    identities,
                    &fact_path,
                    target_carrier,
                    &types,
                ) else {
                    continue;
                };
                full_path.extend(lowered_segments);
            }
            let carrier = if full_path.is_empty() {
                Some(target_carrier)
            } else {
                terminal_semantics::qualification_path_carrier(
                    types.iter(),
                    structural_type,
                    &full_path,
                )
            };
            let domain = if let Some(carrier) = carrier {
                crate::terminal_identities::lookup_domain_id_for_carrier(
                    self.callees.domain_ids,
                    semantic_domain,
                    carrier,
                )
                .or_else(|_| {
                    crate::terminal_identities::lookup_domain_id(
                        self.callees.domain_ids,
                        semantic_domain,
                    )
                })?
            } else {
                crate::terminal_identities::lookup_domain_id(
                    self.callees.domain_ids,
                    semantic_domain,
                )?
            };
            let (qualifications, projected_qualifications) = if full_path.is_empty() {
                (vec![domain], Vec::new())
            } else {
                (
                    Vec::new(),
                    vec![terminal_psi::StructuralPathQualification {
                        path: full_path,
                        domain,
                    }],
                )
            };
            self.operations.add_structural_qualifications(
                owner.place,
                &qualifications,
                &projected_qualifications,
            );
            for (_, parameter) in &mut self.evaluation.structural_parameters {
                if parameter.place == owner.place {
                    parameter.qualifications.extend_from_slice(&qualifications);
                    parameter.qualifications.sort();
                    parameter.qualifications.dedup();
                    parameter
                        .projected_qualifications
                        .extend_from_slice(&projected_qualifications);
                    parameter.projected_qualifications.sort();
                    parameter.projected_qualifications.dedup();
                }
            }
            for parameter in &mut self.evaluation.block_structural_parameters {
                if parameter.place == owner.place {
                    parameter.qualifications.extend_from_slice(&qualifications);
                    parameter.qualifications.sort();
                    parameter.qualifications.dedup();
                    parameter
                        .projected_qualifications
                        .extend_from_slice(&projected_qualifications);
                    parameter.projected_qualifications.sort();
                    parameter.projected_qualifications.dedup();
                }
            }
        }
        Ok(())
    }
}
