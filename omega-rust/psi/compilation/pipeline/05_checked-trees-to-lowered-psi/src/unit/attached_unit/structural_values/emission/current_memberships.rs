//! A physical home keeps its birth declaration; each read transports current evidence.
//! Unchanged Contents receipts preserve producer guarantees, while changed subjects
//! require live contextual or completed Content evidence. Receiver types mint nothing.
use super::{Emission, LoweringError, StructuralArgument};
use terminal_psi::{StructuralPathQualification, StructuralPathSegment};

mod descriptor_bindings;

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
        let (mut whole, projected) =
            StructuralPathQualification::select(whole, projected, &source.path);
        if whole.is_empty() && projected.is_empty() {
            return Ok((whole, projected));
        }
        let projection = self.completed_projections.iter().rev().find(|projection| {
            projection.expression == expression && projection.argument == *source
        });
        let source_root = projection.map_or(source.place, |projection| projection.source_root);
        let projection_captures = projection
            .map(|_| {
                validation::completed_storage::consumer_captures(
                    &self.checked.facts.flow.control,
                    self.state,
                    self.statement as usize,
                    expression,
                )
                .ok_or(LoweringError::Unsupported(
                    "completed projection has no exact consumer capture",
                ))
            })
            .transpose()?;
        if projection_captures.as_ref().is_some_and(|captures| {
            !captures
                .iter()
                .all(|capture| descriptor_bindings::unchanged(self.checked, *capture))
        }) {
            // The physical endpoint remains captured, but facts about its old
            // source spelling no longer describe it. Withhold those memberships.
            return Ok((Vec::new(), Vec::new()));
        }
        let is_local = self
            .evaluation
            .structural_locals
            .iter()
            .any(|local| local.argument.place == source_root);
        let is_ingress = self
            .evaluation
            .structural_parameters
            .iter()
            .any(|(_, parameter)| parameter.place == source_root);
        if !is_local && !is_ingress {
            // Materialization records only actual producers or owned arrivals.
            // Their exact contract above remains authoritative even after the
            // producing operation moves into a sealed private block.
            if self
                .operations
                .structural_materialization_statement(source_root, self.state)?
                == Some(self.statement)
            {
                return Ok((whole, projected));
            }
            return Err(LoweringError::at_statement(
                self.checked,
                self.machine,
                self.state,
                self.statement,
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
        let prefix = if let Some(local) = self
            .evaluation
            .structural_locals
            .iter()
            .find(|local| local.argument.place == source_root && local.root == subject.root)
        {
            local.argument.path.as_slice()
        } else {
            let (_, state) = crate::declarations::state_declaration(self.checked, self.state)?;
            self.evaluation
                .structural_parameters
                .iter()
                .any(|(index, parameter)| {
                    parameter.place == source_root
                        && self
                            .checked
                            .state_parameters(state)
                            .get(*index as usize)
                            .is_some_and(|parameter| {
                                subject.root == facts::PlaceRoot::Symbol(parameter.symbol)
                            })
                })
                .then_some(&[][..])
                .ok_or(LoweringError::Unsupported(
                    "resident membership home has no exact source projection",
                ))?
        };
        if projection.is_none()
            && !source.path.strip_prefix(prefix).is_some_and(|suffix| {
                self.membership_path_matches(identities, suffix, &subject.segments)
            })
        {
            return Err(LoweringError::Unsupported(
                "resident membership physical and source paths differ",
            ));
        }
        let captures = match projection_captures {
            Some(captures) => captures,
            None => validation::completed_storage::consumer_captures(
                &self.checked.facts.flow.control,
                self.state,
                self.statement as usize,
                expression,
            )
            .ok_or(LoweringError::Unsupported(
                "resident membership read has no exact completed consumer",
            ))?,
        };
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
        let source_type = validation::canonical_place_type_reference(
            &self.checked.typed,
            self.state,
            self.statement as usize,
            &subject,
        )
        .ok_or(LoweringError::Unsupported(
            "membership source lost its exact declared carrier",
        ))?;
        let current = |domain, path: &[StructuralPathSegment], relative: &[facts::PlaceSegment]| {
            let Some(binding) = self
                .domain_ids
                .iter()
                .find(|binding| binding.domain == domain)
            else {
                return false;
            };
            let domain_symbols = self
                .checked
                .domain_definitions()
                .iter()
                .filter(|definition| definition.semantic_id == binding.semantic_domain)
                .map(|definition| definition.symbol)
                .collect::<Vec<_>>();
            let mut target = subject.clone();
            target.segments.extend_from_slice(relative);
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
                let contexts = self
                    .checked
                    .facts
                    .flow
                    .contexts
                    .semantic_constraint_contexts(operand.constraints)
                    .collect::<Vec<_>>();
                let evidence = validation::membership_evidence::MembershipEvidence::prepare(
                    &self.checked.typed,
                    &self.checked.facts.semantic,
                    &contexts,
                );
                // One checked semantic family can have several capacity
                // declarations. Each receipt retains its exact declaration;
                // a first-match reverse lookup would select another capacity.
                // Same-family consistency is checked before Terminal lowering.
                domain_symbols.iter().any(|domain_symbol| {
                    evidence.contains(&target, *domain_symbol, binding.semantic_domain)
                        || validation::completed_storage::completed_membership(
                            &self.checked.typed,
                            &self.checked.facts,
                            *capture,
                            relative,
                            *domain_symbol,
                            binding.semantic_domain,
                        )
                })
            })
        };
        whole.retain(|domain| current(*domain, &[], &[]));
        let mut retained = Vec::with_capacity(projected.len());
        for row in projected {
            let relative = self.membership_source_path(identities, source_type, &row.path)?;
            if current(row.domain, &row.path, &relative) {
                retained.push(row);
            }
        }
        Ok((whole, retained))
    }

    fn membership_source_path(
        &self,
        identities: &terminal_psi::TerminalIdentityTableBuilder,
        source_type: typed_trees::types::TypeReferenceHandle,
        path: &[StructuralPathSegment],
    ) -> Result<Vec<facts::PlaceSegment>, LoweringError> {
        use checked_trees::CheckedUnitStructuralPathSegment as Segment;
        let mut checked_path = Vec::with_capacity(path.len());
        for segment in path {
            checked_path.push(match segment {
                StructuralPathSegment::Field(identity) => {
                    Segment::Field(checked_trees::CheckedMemberIdentity::of_spelling(
                        identities
                            .text(*identity)
                            .ok_or(LoweringError::Unsupported(
                                "membership field identity is absent",
                            ))?,
                    ))
                }
                StructuralPathSegment::Case(selected) => {
                    let case = self
                        .structural_types
                        .iter()
                        .flat_map(|declaration| declaration.shape.sum_cases().into_iter().flatten())
                        .find(|case| case.id == *selected)
                        .ok_or(LoweringError::Unsupported(
                            "membership case identity is absent",
                        ))?;
                    Segment::Case(checked_trees::CheckedMemberIdentity::of_spelling(
                        identities
                            .text(case.identity)
                            .ok_or(LoweringError::Unsupported(
                                "membership case spelling is absent",
                            ))?,
                    ))
                }
                StructuralPathSegment::FixedIndex(index) => Segment::FixedIndex(*index),
                StructuralPathSegment::AllElements => Segment::AllElements,
                StructuralPathSegment::Referent => Segment::Referent,
                _ => {
                    return Err(LoweringError::Unsupported(
                        "membership has a nonstatic path coordinate",
                    ));
                }
            });
        }
        validation::canonical_qualification_path(&self.checked.typed, source_type, &checked_path)
            .map_err(LoweringError::Unsupported)
    }

    /// Source symbols already identify their exact owner. Compare their scoped
    /// path identities forward to physical geometry, never reverse a spelling
    /// through an unrelated global declaration.
    fn membership_path_matches(
        &self,
        identities: &terminal_psi::TerminalIdentityTableBuilder,
        physical: &[StructuralPathSegment],
        source: &[facts::PlaceSegment],
    ) -> bool {
        let physical = physical
            .iter()
            .filter(|segment| !matches!(segment, StructuralPathSegment::Referent))
            .collect::<Vec<_>>();
        physical.len() == source.len()
            && physical
                .into_iter()
                .zip(source)
                .all(|(physical, source)| match (physical, source) {
                    (
                        StructuralPathSegment::Field(identity),
                        facts::PlaceSegment::Field { symbol },
                    ) => validation::structural_member_identity(&self.checked.typed, *symbol)
                        .is_some_and(|member| {
                            identities.text(*identity) == Some(member.spelling().as_str())
                        }),
                    (
                        StructuralPathSegment::Case(selected),
                        facts::PlaceSegment::Case { variant },
                    ) => {
                        let identity = self
                            .structural_types
                            .iter()
                            .flat_map(|declaration| {
                                declaration.shape.sum_cases().into_iter().flatten()
                            })
                            .find(|case| case.id == *selected)
                            .and_then(|case| identities.text(case.identity));
                        validation::structural_member_identity(&self.checked.typed, *variant)
                            .is_some_and(|member| identity == Some(member.spelling().as_str()))
                    }
                    (
                        StructuralPathSegment::FixedIndex(index),
                        facts::PlaceSegment::FixedIndex { index: source },
                    ) => usize::try_from(*index).ok() == Some(*source),
                    (
                        StructuralPathSegment::AllElements,
                        facts::PlaceSegment::FixedRange {
                            start: 0,
                            end: usize::MAX,
                        },
                    ) => true,
                    _ => false,
                })
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
