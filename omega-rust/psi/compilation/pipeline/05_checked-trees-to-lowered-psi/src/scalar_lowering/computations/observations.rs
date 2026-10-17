//! Storage observations interleave address capture with selector computations.
//! A canonical field/index path is geometry, not an evaluation schedule. Each
//! borrowed continuation binds the actual collection before its selector; the
//! next collection is reached only after that selector completes. No contents
//! are copied and no reference carrier or loan is manufactured.
use super::{
    Argument, Computation, Expansion, LoweringError, QualifiedScalarType, Site, unsupported,
};
use crate::emission::operation_emission::expressions::{
    LoweredDirectExpression, LoweredElementPathSegment,
};
use crate::expression_preparation::bindings::structural_paths::{
    self, PreparedStructuralPathSegment,
};
use crate::scalar_lowering::branch_states::prepared::{
    LoweredScalarBranchState, LoweredScalarBranchTerminator,
};
use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarExpression, CheckedScalarReadProjection,
    CheckedStructuralPredicatePathSegment, CheckedUnitStructuralPathSegment,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration};

#[derive(Clone)]
pub(crate) struct Slot {
    computation: Computation,
    places: Vec<PlaceId>,
}

pub(crate) fn prepare(
    checked: &checked_trees::CheckedTrees,
    machines: &[symbols::SymbolHandle],
    next_place: &mut u64,
) -> Result<Vec<Slot>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut slots: Vec<Slot> = Vec::new();
    for root in plans.machine_roots(machines) {
        for computation in super::reachable_nodes(checked, &[root.root])? {
            if slots.iter().any(|slot| slot.computation == computation) {
                continue;
            }
            let CheckedScalarComputationKind::StorageObservation {
                operands,
                collections,
                ..
            } = plans.nodes.get(computation).kind
            else {
                continue;
            };
            let operands = plans
                .operands
                .span(operands)
                .ok_or(LoweringError::Unsupported(
                    "storage observation lost selector operands",
                ))?;
            let collections = plans.observation_collections.span(collections).ok_or(
                LoweringError::Unsupported("storage observation lost collection carriers"),
            )?;
            if operands.len() != collections.len() {
                return unsupported("storage observation selector and carrier rosters disagree");
            }
            let places = collections
                .iter()
                .map(|_| super::super::allocate_dense(next_place).map(super::super::place_id))
                .collect::<Result<Vec<_>, _>>()?;
            slots.push(Slot {
                computation,
                places,
            });
        }
    }
    Ok(slots)
}

fn checked_path(
    segment: &CheckedStructuralPredicatePathSegment,
) -> CheckedUnitStructuralPathSegment {
    match segment {
        CheckedStructuralPredicatePathSegment::Field(field) => {
            CheckedUnitStructuralPathSegment::Field(field.clone())
        }
        CheckedStructuralPredicatePathSegment::Case(case) => {
            CheckedUnitStructuralPathSegment::Case(case.clone())
        }
        CheckedStructuralPredicatePathSegment::FixedIndex(index) => {
            CheckedUnitStructuralPathSegment::FixedIndex(*index)
        }
        CheckedStructuralPredicatePathSegment::RuntimeIndex(index) => {
            CheckedUnitStructuralPathSegment::RuntimeIndex(index.clone())
        }
    }
}

struct Capture {
    source: PlaceId,
    path: Vec<LoweredElementPathSegment>,
    access: StructuralAccess,
    place: PlaceId,
    structural_type: StructuralTypeId,
}

impl Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn storage_observation(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        computation: Computation,
        root_type: typed_trees::types::TypeReferenceHandle,
        expression: &CheckedScalarExpression,
        operands: arena::HandleSpan<Computation>,
        collections: arena::HandleSpan<typed_trees::types::TypeReferenceHandle>,
        input_types: &[QualifiedScalarType],
        result_type: QualifiedScalarType,
        target: usize,
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        let plans = &self.checked.facts.values.scalar_computations;
        let operands = plans
            .operands
            .span(operands)
            .ok_or(LoweringError::Unsupported(
                "storage observation lost operands",
            ))?
            .to_vec();
        let collections = plans
            .observation_collections
            .span(collections)
            .ok_or(LoweringError::Unsupported(
                "storage observation lost carriers",
            ))?
            .to_vec();
        if operands.len() != collections.len() {
            return unsupported(
                "storage observation selector/carrier correspondence is incomplete",
            );
        }
        let mut prefixes = vec![input_types.to_vec()];
        for operand in &operands {
            let mut next = prefixes.last().expect("observation input prefix").clone();
            next.push(self.argument_type(
                identities,
                &Argument::Computation(*operand),
                site,
                input_types,
            )?);
            prefixes.push(next);
        }
        let complete_types = prefixes.last().expect("observation complete prefix");
        let lowered = site
            .bindings
            .clone()
            .with_computation_operands(
                input_types.len(),
                complete_types[input_types.len()..]
                    .iter()
                    .map(|value| value.scalar_type),
            )
            .expression(
                identities,
                &self.checked.facts.proven_scalar_indexes,
                expression,
            )?;
        if lowered.value_type(complete_types)? != result_type {
            return unsupported("storage observation changes its result carrier");
        }
        let (root, mut pending, selectors, suffix) = match expression {
            CheckedScalarExpression::StructuralParameterIndexedRead {
                root,
                path,
                index,
                element_path,
                ..
            } => {
                let mut selectors = vec![index.as_ref().clone()];
                let mut pending = path.iter().map(checked_path).collect::<Vec<_>>();
                let mut prefixes = vec![std::mem::take(&mut pending)];
                for step in element_path {
                    match step {
                        CheckedScalarReadProjection::Static(step) => {
                            pending.push(checked_path(step))
                        }
                        CheckedScalarReadProjection::RuntimeIndex(selector) => {
                            prefixes.push(std::mem::take(&mut pending));
                            selectors.push(selector.clone());
                        }
                    }
                }
                (*root, prefixes, selectors, pending)
            }
            CheckedScalarExpression::StructuralSequenceLength { root, path } => {
                let mut selectors = Vec::new();
                let mut prefixes = Vec::new();
                let mut pending = Vec::new();
                for step in path {
                    match step {
                        CheckedScalarReadProjection::Static(step) => {
                            pending.push(checked_path(step))
                        }
                        CheckedScalarReadProjection::RuntimeIndex(selector) => {
                            prefixes.push(std::mem::take(&mut pending));
                            selectors.push(selector.clone());
                        }
                    }
                }
                (*root, prefixes, selectors, pending)
            }
            _ => {
                // A declaration-known length can discard its pure collection
                // bits, but every selector computation still executes once.
                let finish = self.binding(
                    complete_types,
                    input_types.len(),
                    target,
                    crate::emission::operation_emission::LoweredScalarBinding::Expression(lowered),
                );
                let arguments = operands
                    .into_iter()
                    .map(Argument::Computation)
                    .collect::<Vec<_>>();
                return self.sequence(identities, &arguments, input_types, finish, site, active);
            }
        };
        if selectors.len() != operands.len() || pending.len() != operands.len() {
            return unsupported("storage observation path does not cover its selector roster");
        }
        if operands.is_empty() {
            return Ok(self.binding(
                input_types,
                input_types.len(),
                target,
                crate::emission::operation_emission::LoweredScalarBinding::Expression(lowered),
            ));
        }
        let slot = self
            .observations
            .iter()
            .find(|slot| slot.computation == computation)
            .ok_or(LoweringError::Unsupported(
                "storage observation has no reserved captures",
            ))?
            .clone();
        if slot.places.len() != operands.len() {
            return unsupported("storage observation reserved capture roster disagrees");
        }
        let (mut source, mut source_type) = site.bindings.storage_observation_home(root)?;
        let mut logical_root =
            validation::structural_type_identity(&self.checked.typed, root_type, &[], &[]);
        let mut physical_root =
            site.bindings
                .structural_place_type(source.place)
                .ok_or(LoweringError::Unsupported(
                    "storage observation lost its physical root carrier",
                ))?;
        let mut multiplicity = self
            .source_carriers
            .iter()
            .find(|(place, _)| *place == source.place)
            .map(|(_, multiplicity)| *multiplicity)
            .ok_or(LoweringError::Unsupported(
                "storage observation lost its emitted source multiplicity",
            ))?;
        let mut captures = Vec::new();
        for (ordinal, reference) in collections.iter().enumerate() {
            let identity =
                validation::structural_type_identity(&self.checked.typed, *reference, &[], &[]);
            if !identity.is_valid() {
                return unsupported("storage observation collection has no exact identity");
            }
            let declaration = crate::terminal_identities::structural_declaration_by_identity(
                self.structural_types,
                identities,
                self.checked,
                identity,
                "storage observation collection has no retained declaration",
            )?;
            let path = structural_paths::prepare_observation_path(
                identities,
                self.checked,
                &pending[ordinal],
                source_type,
                logical_root,
                identity,
                StructuralAccess::SharedBorrow,
                self.structural_types,
            )?;
            let mut selected = source
                .path
                .iter()
                .cloned()
                .map(LoweredElementPathSegment::Static)
                .collect::<Vec<_>>();
            for segment in path {
                selected.push(match segment {
                    PreparedStructuralPathSegment::Static(step) => LoweredElementPathSegment::Static(step),
                    PreparedStructuralPathSegment::ViewExtent { bytes } => LoweredElementPathSegment::ViewExtent { source: source.place, bytes },
                    PreparedStructuralPathSegment::RuntimeIndex(checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth }) => {
                        let selector = selectors.get(depth as usize).ok_or(LoweringError::Unsupported("storage observation lost an earlier selector"))?;
                        LoweredElementPathSegment::RuntimeIndex { selector: super::parameter(input_types.len() + depth as usize, prefixes[depth as usize + 1][input_types.len() + depth as usize]), extent: None, bound_decided: crate::expression_preparation::prepare_expression::selector_bound_is_decided(selector, &self.checked.facts.proven_scalar_indexes) }
                    }
                    _ => return unsupported("storage observation prefix has an unprepared selector"),
                });
            }
            // Permission queries need only declaration geometry. Runtime
            // selector values and bounds stay in `selected` and are emitted
            // from their actual completed operands; these scratch element
            // markers establish neither an index nor an address equality.
            let geometry = selected
                .iter()
                .filter_map(|segment| match segment {
                    LoweredElementPathSegment::Static(segment) => Some(segment.clone()),
                    LoweredElementPathSegment::RuntimeIndex { .. } => {
                        Some(terminal_psi::StructuralPathSegment::FixedIndex(0))
                    }
                    LoweredElementPathSegment::ViewExtent { .. } => None,
                })
                .collect::<Vec<_>>();
            let readable = |requested| {
                terminal_semantics::descriptor_source_projection_accessible(
                    self.structural_types.iter(),
                    physical_root,
                    multiplicity,
                    source.access,
                    &geometry,
                    requested,
                )
            };
            let access = if readable(StructuralAccess::MutableBorrow) {
                StructuralAccess::MutableBorrow
            } else if readable(StructuralAccess::SharedBorrow) {
                StructuralAccess::SharedBorrow
            } else {
                return unsupported(
                    "storage observation collection has no readable source authority",
                );
            };
            captures.push(Capture {
                source: source.place,
                path: selected,
                access,
                place: slot.places[ordinal],
                structural_type: declaration.id,
            });
            source = terminal_psi::StructuralArgument {
                place: slot.places[ordinal],
                path: Vec::new(),
                access,
            };
            source_type = declaration.id;
            logical_root = identity;
            physical_root = declaration.id;
            multiplicity = StructuralMultiplicity::Unrestricted;
            if ordinal + 1 < operands.len() {
                pending[ordinal + 1].insert(
                    0,
                    CheckedUnitStructuralPathSegment::RuntimeIndex(
                        checked_trees::CheckedRuntimeIndex::AssignmentIndex {
                            depth: ordinal as u32,
                        },
                    ),
                );
            }
        }
        let final_capture = captures.last().expect("nonempty observation captures");
        let last = selectors.len() - 1;
        let final_expression = self.completed_storage_observation(
            identities,
            site,
            expression,
            final_capture.place,
            final_capture.structural_type,
            &suffix,
            input_types.len() + last,
            complete_types[input_types.len() + last],
            &selectors[last],
            input_types.len(),
            complete_types,
            final_capture.access,
        )?;
        let mut continuation = self.binding(
            complete_types,
            input_types.len(),
            target,
            crate::emission::operation_emission::LoweredScalarBinding::Expression(final_expression),
        );
        for ordinal in (0..operands.len()).rev() {
            let saved = self.completed.len();
            self.completed.extend(
                operands[..ordinal]
                    .iter()
                    .enumerate()
                    .map(|(index, operand)| (*operand, input_types.len() + index)),
            );
            continuation = self.argument(
                identities,
                &Argument::Computation(operands[ordinal]),
                &prefixes[ordinal],
                continuation,
                site,
                active,
            )?;
            self.completed.truncate(saved);
            let capture = &captures[ordinal];
            self.states[continuation - self.base]
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: capture.place,
                    position: 0,
                    structural_type: capture.structural_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: capture.access,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                });
            continuation = self.push(LoweredScalarBranchState {
                structural_parameters: Vec::new(),
                parameter_types: prefixes[ordinal].clone(),
                erased_formal_types: Vec::new(),
                erased_proof_formals: Vec::new(),
                bindings: Vec::new(),
                structural_effects: Vec::new(),
                terminator: LoweredScalarBranchTerminator::Capture {
                    target: continuation,
                    arguments: super::parameters(&prefixes[ordinal]),
                    source: capture.source,
                    path: capture.path.clone(),
                    access: capture.access,
                },
            });
        }
        Ok(continuation)
    }
}

impl Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    fn completed_storage_observation(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        site: &Site<'_>,
        expression: &CheckedScalarExpression,
        place: PlaceId,
        structural_type: StructuralTypeId,
        suffix: &[CheckedUnitStructuralPathSegment],
        _selector_position: usize,
        _selector_type: QualifiedScalarType,
        selector: &checked_trees::CheckedScalarSelector,
        input_count: usize,
        complete_types: &[QualifiedScalarType],
        access: StructuralAccess,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let suffix = suffix
            .iter()
            .map(|step| match step {
                CheckedUnitStructuralPathSegment::Field(field) => {
                    Ok(CheckedScalarReadProjection::Static(
                        CheckedStructuralPredicatePathSegment::Field(field.clone()),
                    ))
                }
                CheckedUnitStructuralPathSegment::Case(case) => {
                    Ok(CheckedScalarReadProjection::Static(
                        CheckedStructuralPredicatePathSegment::Case(case.clone()),
                    ))
                }
                CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                    Ok(CheckedScalarReadProjection::Static(
                        CheckedStructuralPredicatePathSegment::FixedIndex(*index),
                    ))
                }
                _ => unsupported("completed observation has an unconsumed selector"),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (root, read) = match expression {
            CheckedScalarExpression::StructuralParameterIndexedRead {
                root,
                primitive_type,
                ..
            } => (
                *root,
                CheckedScalarExpression::StructuralParameterIndexedRead {
                    root: *root,
                    path: Vec::new(),
                    index: Box::new(selector.clone()),
                    element_path: suffix,
                    primitive_type: *primitive_type,
                },
            ),
            CheckedScalarExpression::StructuralSequenceLength { root, .. } => {
                let mut path = vec![CheckedScalarReadProjection::RuntimeIndex(selector.clone())];
                path.extend(suffix);
                (
                    *root,
                    CheckedScalarExpression::StructuralSequenceLength { root: *root, path },
                )
            }
            _ => return unsupported("completed storage observation lost its read operation"),
        };
        site.bindings
            .clone()
            .with_completed_storage_observation(
                root,
                terminal_psi::StructuralArgument {
                    place,
                    path: Vec::new(),
                    access,
                },
                structural_type,
            )
            .with_computation_operands(
                input_count,
                complete_types[input_count..]
                    .iter()
                    .map(|value| value.scalar_type),
            )
            .expression(identities, &self.checked.facts.proven_scalar_indexes, &read)
    }
}
