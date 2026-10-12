//! Arm-local structural schedules join one selected result and its residual
//! source owners. Reservation is shared; only the selected schedule executes.
use super::*;
use crate::emission::owned_value_join::{Owner, selected_complement};
use crate::scalar_lowering::computations::dispatch::DispatchEntry;
use crate::unit::attached_unit::argument_evaluation::SelectionCleanup;

struct Arm {
    pattern: checked_trees::CheckedScalarDispatchPattern,
    equality_use: checked_trees::CheckedOperatorUseHandle,
    value: PreparedValue,
    selected: StructuralArgument,
    cleanup: Vec<crate::unit::unit_cleanup::PreparedNormalCleanupAction>,
}

pub(super) struct Prepared {
    subject: checked_trees::CheckedScalarComputationHandle,
    arms: Vec<Arm>,
    parameters: Vec<StructuralParameterDeclaration>,
    owners: Vec<Owner>,
    sources: Vec<PlaceId>,
    receipt: arena::Handle<checked_trees::FlowOwnedSelectionReceipt>,
    claims: Vec<PreparedClaim>,
    parameter_sources: Vec<(PlaceId, typed_trees::types::TypeReferenceHandle)>,
}

impl Preparation<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn dispatch(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        value: checked_trees::CheckedStructuralValueHandle,
        reference: checked_trees::types::TypeReferenceHandle,
        claim_path: &[terminal_psi::StructuralPathSegment],
        subject: checked_trees::CheckedScalarComputationHandle,
        arms: arena::HandleSpan<checked_trees::CheckedStructuralDispatchArm>,
    ) -> Result<PlaceId, LoweringError> {
        let plans = &self.checked.facts.values.structural_values;
        let node = plans.nodes.get(value);
        let arms = plans
            .dispatch_arms
            .span(arms)
            .ok_or(LoweringError::Unsupported(
                "structural dispatch arm span is stale",
            ))?
            .to_vec();
        let declaration = carrier_declaration(identities, self.checked, self.types, reference)?;
        let multiplicity = terminal_multiplicity(self.checked.type_multiplicity(reference));
        let place = self.reserved.place(value)?;
        let ownership = &self.checked.facts.flow.ownership;
        let selection = ownership.owned_selection_for_expression(
            node.state,
            node.statement_index,
            node.expression,
        );
        let receipt = selection.map(|(handle, _)| handle).unwrap_or_default();
        let (_, state) = crate::declarations::state_declaration(self.checked, self.state)?;
        let mut owners = Vec::new();
        let mut source_references = Vec::new();
        if let Some((_, selection)) = selection {
            for source in ownership
                .selection_sources
                .span_or_empty(selection.sources)
                .iter()
                .rev()
            {
                let (argument, structural_type) = self.bindings.owned_structural_home(
                    facts::PlaceRoot::Symbol(source.symbol),
                    self.checked.state_parameters(state),
                )?;
                if !argument.path.is_empty() || argument.access != StructuralAccess::Owned {
                    return unsupported("owned selection source requires its whole current home");
                }
                if self
                    .bindings
                    .structural_parameter_by_place(argument.place)
                    .is_some_and(|parameter| !parameter.is_unqualified_owned_affine())
                {
                    return unsupported(
                        "owned selection parameter source lacks whole plain-affine custody",
                    );
                }
                owners.push(Owner {
                    place: argument.place,
                    structural_type,
                });
                let reference = validation::canonical_place_type_reference(
                    &self.checked.typed,
                    self.state,
                    node.statement_index as usize,
                    &validation::CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(source.symbol),
                        segments: Vec::new(),
                    },
                )
                .ok_or(LoweringError::Unsupported(
                    "selection source lost its exact occurrence type",
                ))?;
                if self.checked.type_multiplicity(reference) != Multiplicity::Affine
                    || !crate::content_classes::owned_or_shared_view_contents(
                        self.checked,
                        reference,
                    )
                {
                    return unsupported("selection source requires whole plain-affine custody");
                }
                source_references.push(reference);
            }
        }
        let sources = owners.iter().map(|owner| owner.place).collect::<Vec<_>>();
        if !sources.is_empty() && multiplicity != StructuralMultiplicity::Affine {
            return unsupported("owned selection requires uniform affine root cleanup");
        }
        let residuals = self
            .reserved
            .joins
            .iter()
            .find(|(candidate, _)| *candidate == value)
            .map(|(_, places)| places)
            .ok_or(LoweringError::Unsupported(
                "structural dispatch has no reserved join",
            ))?;
        if residuals.len() != sources.len().saturating_sub(1) {
            return unsupported("selection residual reservation changed");
        }
        let mut parameters = owners
            .iter()
            .skip(1)
            .zip(residuals)
            .enumerate()
            .map(|(position, (owner, place))| {
                Ok(StructuralParameterDeclaration {
                    place: *place,
                    position: u32::try_from(position).map_err(|_| {
                        LoweringError::Unsupported("selection parameter count exceeds u32")
                    })?,
                    structural_type: owner.structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        parameters.push(StructuralParameterDeclaration {
            place,
            position: u32::try_from(parameters.len())
                .map_err(|_| LoweringError::Unsupported("selection parameter count exceeds u32"))?,
            structural_type: declaration.id,
            multiplicity,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
        let mut parameter_sources = parameters
            .iter()
            .zip(source_references.iter().skip(1))
            .map(|(parameter, reference)| (parameter.place, *reference))
            .collect::<Vec<_>>();
        parameter_sources.push((place, reference));
        let mut prepared_arms = Vec::new();
        for arm in arms {
            let mut branch = Preparation {
                checked: self.checked,
                qualifications: self.qualifications,
                state: self.state,
                calls: self.calls,
                consumed_calls: Vec::new(),
                bindings: self.bindings,
                types: self.types,
                reserved: self.reserved,
                prefix: self.prefix.clone(),
                steps: Vec::new(),
                consumed: Vec::new(),
                storage: Vec::new(),
                claims: self.claims.clone(),
            };
            let selected = branch.operand(identities, arm.value, reference, claim_path)?;
            let mut cleanup = Vec::new();
            if !selected.path.is_empty() {
                let child = plans.nodes.get(arm.value);
                let CheckedStructuralValueKind::Projection { source, path, .. } = &child.kind
                else {
                    return unsupported("selected projection lost its checked root");
                };
                let root = plans.nodes.get(*source);
                let source_plan = match &root.kind {
                    CheckedStructuralValueKind::Place(argument) => argument.source.clone(),
                    CheckedStructuralValueKind::Call { .. } => {
                        let call =
                            self.calls.iter().find(|call| call.value == *source).ok_or(
                                LoweringError::Unsupported("projected call has no producer"),
                            )?;
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: call
                                .operation()
                                .structural_result_binding()
                                .ok_or(LoweringError::Unsupported(
                                    "projected call lost its receiving binding",
                                ))?
                                .binding_ordinal,
                        }
                    }
                    _ => return unsupported("projected selection has no whole source custody"),
                };
                let root_identity = validation::structural_type_identity(
                    &self.checked.typed,
                    root.type_reference,
                    &[],
                    &[],
                );
                let moved = [(
                    path.as_slice(),
                    validation::structural_type_identity(&self.checked.typed, reference, &[], &[]),
                )];
                for residual in
                    crate::unit::attached_unit::structural_values::projected_owned_complement(
                        &self
                            .checked
                            .facts
                            .flow
                            .terminal_unit_effects
                            .structural_types,
                        &source_plan,
                        root_identity,
                        &moved,
                    )?
                {
                    let root_type = carrier_declaration(
                        identities,
                        self.checked,
                        self.types,
                        root.type_reference,
                    )?
                    .id;
                    let path = crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(identities, &residual.path, root_type, self.types)?;
                    let structural_type =
                        crate::terminal_identities::structural_declaration_by_identity(
                            self.types,
                            identities,
                            self.checked,
                            residual.type_identity,
                            "selection residual has no type",
                        )?
                        .id;
                    cleanup.push(
                        crate::unit::unit_cleanup::PreparedNormalCleanupAction::Resolved(
                            crate::unit::unit_cleanup::CleanupRegion::unconditional(
                                terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
                                    terminal_psi::StructuralAffineDiscard {
                                        place: selected.place,
                                        path,
                                        structural_type,
                                    },
                                ),
                            ),
                        ),
                    );
                }
            }
            if branch.consumed.iter().any(|place| !sources.contains(place)) {
                return unsupported("structural selection lost its conditional source receipt");
            }
            for call in &branch.consumed_calls {
                if !self.consumed_calls.contains(call) {
                    self.consumed_calls.push(*call);
                }
            }
            prepared_arms.push(Arm {
                pattern: arm.pattern,
                equality_use: arm.equality_use,
                selected,
                cleanup,
                value: PreparedValue {
                    place: place,
                    structural_type: declaration.id,
                    statement: node.statement_index,
                    steps: branch.steps,
                    consumed: branch.consumed,
                    storage: branch.storage,
                },
            });
        }
        for source in &sources {
            if self.consumed.contains(source) {
                return unsupported("selection repeats an already transferred owner");
            }
            self.consumed.push(*source);
        }
        self.steps.push(Step::Dispatch {
            prepared: Prepared {
                subject,
                arms: prepared_arms,
                parameters,
                owners,
                sources,
                receipt,
                claims: claims_below_path(&self.claims, claim_path),
                parameter_sources,
            },
            prefix: self.prefix.clone(),
        });
        Ok(place)
    }
}

impl Prepared {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn finish(
        self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        state: symbols::SymbolHandle,
        statement: u32,
        bindings: &storage::ScalarBindings,
        prefix: &[QualifiedScalarType],
        target: usize,
        computations: &mut computations::Expansion<'_>,
    ) -> Result<usize, LoweringError> {
        let result = self
            .parameters
            .last()
            .ok_or(LoweringError::Unsupported(
                "selection has no result parameter",
            ))?
            .place;
        for (place, reference) in &self.parameter_sources {
            computations.record_parameter_source(*place, *reference);
        }
        let joined = computations.push(LoweredScalarBranchState::jumping(
            target,
            self.parameters.clone(),
            prefix.to_vec(),
            prefix,
            vec![LoweredScalarEffect::StructuralCompletion {
                parameter: self
                    .parameters
                    .last()
                    .expect("result parameter checked above")
                    .clone(),
                claims: self.claims,
            }],
        ));
        if !self.sources.is_empty() {
            computations.record_selection(
                joined,
                SelectionCleanup {
                    selection: self.receipt,
                    selected: result,
                    sources: self.sources.iter().rev().copied().collect(),
                    remaining: self
                        .sources
                        .iter()
                        .skip(1)
                        .zip(self.parameters.iter())
                        .map(|(source, receiving)| (*source, receiving.place))
                        .collect(),
                    retired_reference_paths: Vec::new(),
                    pass_through: Vec::new(),
                    next_operation: 0,
                },
            );
        }
        let mut entries = Vec::new();
        for arm in self.arms {
            let complement = selected_complement(
                &self.owners,
                &self.sources,
                arm.selected.place,
                &arm.value.consumed,
            )?;
            let mut arguments = complement
                .places
                .into_iter()
                .map(whole_argument)
                .collect::<Vec<_>>();
            arguments.push(arm.selected);
            if arguments.len() != self.parameters.len() {
                return unsupported("selection has unequal residual ownership at its join");
            }
            let mut cleanup = arm.cleanup;
            if let Some(displaced) = complement.displaced {
                cleanup.push(
                    crate::unit::unit_cleanup::PreparedNormalCleanupAction::discard_root(displaced),
                );
            }
            let complete = computations.push(LoweredScalarBranchState {
                structural_parameters: Vec::new(),
                parameter_types: prefix.to_vec(),
                erased_formal_types: Vec::new(),
                erased_proof_formals: Vec::new(),
                bindings: Vec::new(),
                structural_effects: Vec::new(),
                terminator: LoweredScalarBranchTerminator::Jump {
                    target: joined,
                    arguments: computations::parameters(prefix),
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    structural_arguments: arguments,
                    cleanup_actions: cleanup,
                },
            });
            let entry =
                arm.value
                    .finish(identities, state, bindings, prefix, complete, computations)?;
            entries.push(DispatchEntry {
                pattern: arm.pattern,
                equality_use: arm.equality_use,
                target: entry,
            });
        }
        computations.ordered_dispatch(
            identities,
            self.subject,
            entries,
            prefix,
            state,
            statement,
            bindings,
        )
    }
}
