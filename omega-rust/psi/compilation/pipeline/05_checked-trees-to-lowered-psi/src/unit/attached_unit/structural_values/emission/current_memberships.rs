//! A physical home keeps its birth declaration; each read transports current evidence.
//! Unchanged Contents receipts preserve producer guarantees, while changed subjects
//! require live contextual or completed Content evidence. Receiver types mint nothing.
use super::{Emission, LoweringError, StructuralArgument};
use terminal_psi::{StructuralPathQualification, StructuralPathSegment};

type Memberships = (
    Vec<semantic_vocabulary::StructuralDomainId>,
    Vec<StructuralPathQualification>,
);

impl Emission<'_, '_, '_> {
    pub(in crate::unit::attached_unit::structural_values) fn refresh_memberships(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        source: StructuralArgument,
        expression: typed_trees::expression::ExpressionHandle,
        first_new_place: u64,
    ) -> Result<StructuralArgument, LoweringError> {
        if source.place.get() >= first_new_place {
            return Ok(source);
        }
        let current = self.occurrence_memberships(identities, &source, expression)?;
        let (whole, projected) =
            self.completed_memberships(source.place)
                .ok_or(LoweringError::Unsupported(
                    "completed source membership contract disappeared",
                ))?;
        let born = StructuralPathQualification::select(whole, projected, &source.path);
        if current == born {
            return Ok(source);
        }
        if self.multiplicity == super::StructuralMultiplicity::Unrestricted {
            let place = self.copied_leaf_place(
                identities,
                &source,
                arena::HandleSpan::empty(),
                expression,
            )?;
            Ok(StructuralArgument::whole(
                place,
                super::StructuralAccess::Owned,
            ))
        } else {
            let completed =
                self.capture_operand_claims(identities, source, expression, first_new_place)?;
            self.move_completed_operand(completed, current.0, current.1)
        }
    }

    pub(super) fn occurrence_memberships(
        &self,
        identities: &terminal_psi::TerminalIdentityTableBuilder,
        source: &StructuralArgument,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Result<Memberships, LoweringError> {
        let (whole, projected) =
            self.completed_memberships(source.place)
                .ok_or(LoweringError::Unsupported(
                    "completed source has no published memberships",
                ))?;
        let (mut whole, mut projected) =
            StructuralPathQualification::select(whole, projected, &source.path);
        if whole.is_empty() && projected.is_empty() {
            return Ok((whole, projected));
        }
        let is_local = self
            .evaluation
            .structural_locals
            .iter()
            .any(|local| local.argument.place == source.place);
        let is_ingress = self
            .evaluation
            .structural_parameters
            .iter()
            .any(|(_, parameter)| parameter.place == source.place);
        if !is_local && !is_ingress {
            let published_here = self
                .operations
                .structural_materialization_statement(source.place, self.state)?
                == Some(self.statement)
                && (self.operations.iter().any(|operation| {
                    operation
                        .result
                        .structural()
                        .is_some_and(|result| result.place == source.place)
                }) || self
                    .arrival_claims
                    .iter()
                    .any(|(place, _)| *place == source.place));
            if published_here {
                return Ok((whole, projected));
            }
            return Err(LoweringError::Unsupported(
                "membership source lacks its completed physical/source correspondence",
            ));
        }
        let subject = validation::canonical_place_from_expression_in_state(
            &self.checked.typed,
            self.state,
            self.statement as usize,
            expression,
        )
        .ok_or(LoweringError::Unsupported(
            "resident membership source has no canonical observation",
        ))?;
        let mut expected_path = if let Some(local) = self
            .evaluation
            .structural_locals
            .iter()
            .find(|local| local.argument.place == source.place && local.root == subject.root)
        {
            self.membership_source_path(identities, &local.argument.path)
        } else {
            let (_, state) = crate::declarations::state_declaration(self.checked, self.state)?;
            self.evaluation
                .structural_parameters
                .iter()
                .any(|(index, parameter)| {
                    parameter.place == source.place
                        && self
                            .checked
                            .state_parameters(state)
                            .get(*index as usize)
                            .is_some_and(|parameter| {
                                subject.root == facts::PlaceRoot::Symbol(parameter.symbol)
                            })
                })
                .then(Vec::new)
        }
        .ok_or(LoweringError::Unsupported(
            "resident membership home has no exact source projection",
        ))?;
        expected_path.extend_from_slice(&subject.segments);
        let physical_path = self
            .membership_source_path(identities, &source.path)
            .ok_or(LoweringError::Unsupported(
                "resident membership read lacks its typed physical path",
            ))?;
        if expected_path.len() != physical_path.len()
            || !expected_path
                .iter()
                .zip(&physical_path)
                .all(|(left, right)| {
                    validation::storage_path_relations::canonical_place_segments_equal(
                        &self.checked.typed,
                        *left,
                        *right,
                    )
                })
        {
            return Err(LoweringError::Unsupported(
                "resident membership physical and source paths differ",
            ));
        }
        let captures = validation::completed_storage::consumer_captures(
            &self.checked.facts.flow.control,
            self.state,
            self.statement as usize,
            expression,
        )
        .ok_or(LoweringError::Unsupported(
            "resident membership read has no exact completed consumer",
        ))?;
        let birth = self
            .operations
            .structural_materialization_statement(source.place, self.state)?
            .and_then(|birth| {
                self.checked
                    .facts
                    .flow
                    .control
                    .states
                    .iter()
                    .find_map(|(_, state)| {
                        (state.state_symbol == self.state)
                            .then(|| {
                                self.checked
                                    .facts
                                    .flow
                                    .control
                                    .statements
                                    .span_or_empty(state.statements)
                                    .iter()
                                    .find(|statement| statement.statement_index == birth as usize)
                                    .map(|statement| statement.storage_snapshot)
                            })
                            .flatten()
                    })
            });
        let crosses_reference = source
            .path
            .iter()
            .any(|segment| matches!(segment, StructuralPathSegment::Referent))
            || self
                .evaluation
                .structural_locals
                .iter()
                .filter(|local| local.argument.place == source.place && local.root == subject.root)
                .any(|local| {
                    local
                        .argument
                        .path
                        .iter()
                        .any(|segment| matches!(segment, StructuralPathSegment::Referent))
                });
        let current = |domain, path: &[StructuralPathSegment]| {
            let Some(relative) = self.membership_source_path(identities, path) else {
                return false;
            };
            let Some(binding) = self
                .domain_ids
                .iter()
                .find(|binding| binding.domain == domain)
            else {
                return false;
            };
            let domain_symbol = self
                .checked
                .domain_definitions()
                .iter()
                .find(|definition| definition.semantic_id == binding.semantic_domain)
                .map(|definition| definition.symbol);
            let mut target = subject.clone();
            target.segments.extend_from_slice(&relative);
            captures.iter().all(|capture| {
                let operand = self.checked.facts.flow.control.operands.get(*capture);
                // A missing/unknown storage observation never becomes an empty
                // proof. Only identical complete producer receipts conserve it.
                if !crosses_reference
                    && !path
                        .iter()
                        .any(|segment| matches!(segment, StructuralPathSegment::Referent))
                    && birth.is_some_and(|birth| {
                        same_contents(
                            &self.checked.typed,
                            &self.checked.facts.flow.control,
                            birth,
                            operand.storage_snapshot,
                            &target,
                        )
                    })
                {
                    return true;
                }
                let Some(domain_symbol) = domain_symbol else {
                    return false;
                };
                let contexts = self
                    .checked
                    .facts
                    .flow
                    .contexts
                    .semantic_constraint_contexts(operand.constraints)
                    .collect::<Vec<_>>();
                validation::membership_evidence::MembershipEvidence::prepare(
                    &self.checked.typed,
                    &self.checked.facts.semantic,
                    &contexts,
                )
                .contains(&target, domain_symbol, binding.semantic_domain)
                    || validation::completed_storage::completed_membership(
                        &self.checked.typed,
                        &self.checked.facts,
                        *capture,
                        &relative,
                        domain_symbol,
                        binding.semantic_domain,
                    )
            })
        };
        whole.retain(|domain| current(*domain, &[]));
        projected.retain(|row| current(row.domain, &row.path));
        Ok((whole, projected))
    }

    fn membership_source_path(
        &self,
        identities: &terminal_psi::TerminalIdentityTableBuilder,
        path: &[StructuralPathSegment],
    ) -> Option<Vec<facts::PlaceSegment>> {
        use checked_trees::data::DataMember;
        let mut result = Vec::new();
        for segment in path {
            result.push(match segment {
                StructuralPathSegment::Field(identity) => {
                    let identity = identities.text(*identity)?;
                    let symbol = self
                        .checked
                        .data_definitions()
                        .iter()
                        .find_map(|definition| {
                            self.checked
                                .data_members(definition)
                                .iter()
                                .find_map(|member| match member {
                                    DataMember::Field(field)
                                        if field.path_identity() == identity =>
                                    {
                                        Some(field.symbol)
                                    }
                                    DataMember::Variant(variant) => self
                                        .checked
                                        .data_payload_fields(variant)
                                        .iter()
                                        .find(|field| field.path_identity() == identity)
                                        .map(|field| field.symbol),
                                    _ => None,
                                })
                        })?;
                    facts::PlaceSegment::Field { symbol }
                }
                StructuralPathSegment::Case(case) => {
                    let identity = self
                        .structural_types
                        .iter()
                        .flat_map(|ty| ty.shape.sum_cases().into_iter().flatten())
                        .find(|candidate| candidate.id == *case)
                        .and_then(|case| identities.text(case.identity))?;
                    let variant =
                        self.checked
                            .data_definitions()
                            .iter()
                            .find_map(|definition| {
                                self.checked
                                    .data_members(definition)
                                    .iter()
                                    .find_map(|member| match member {
                                        DataMember::Variant(variant)
                                            if variant.path_identity() == identity =>
                                        {
                                            Some(variant.symbol)
                                        }
                                        _ => None,
                                    })
                            })?;
                    facts::PlaceSegment::Case { variant }
                }
                StructuralPathSegment::FixedIndex(index) => facts::PlaceSegment::FixedIndex {
                    index: usize::try_from(*index).ok()?,
                },
                StructuralPathSegment::AllElements => facts::PlaceSegment::FixedRange {
                    start: 0,
                    end: usize::MAX,
                },
                // Semantic membership coordinates denote the referent. The
                // caller forbids Contents preservation across this boundary;
                // the published Terminal row retains its Referent segment.
                StructuralPathSegment::Referent => continue,
                _ => return None,
            });
        }
        Some(result)
    }
}

fn same_contents(
    program: &dyn validation::CheckingInput,
    control: &checked_trees::FlowControlFacts,
    birth: arena::Handle<checked_trees::FlowStorageSnapshot>,
    current: arena::Handle<checked_trees::FlowStorageSnapshot>,
    subject: &validation::CanonicalPlace,
) -> bool {
    use validation::completed_storage::{StoredObservation, at_snapshot};
    let read =
        |cut| at_snapshot(program, control, cut, subject, StoredObservation::Contents)?.captures();
    let (Some(before), Some(after)) = (read(birth), read(current)) else {
        return false;
    };
    !before.is_empty()
        && before.len() == after.len()
        && before.iter().all(|left| {
            after.iter().any(|right| {
                left.capture == right.capture
                    && left.reference == right.reference
                    && left.projection == right.projection
            })
        })
}
