//! Complete each actual place in its authored descriptor/selector order.
//! Address continuations retain existing access to the selected storage; only
//! an admitted copyable final value publishes a new owned occurrence.

use super::{LoweringError, OperationFrame, StructuralArgument};
use crate::emission::completed_place::Frontier;
use crate::emission::operation_emission::calls::PreparedStructuralArgument;

#[derive(Clone)]
pub(crate) struct CompletedStructuralArgument {
    pub source: checked_trees::CheckedUnitStructuralArgumentPlan,
    pub argument: StructuralArgument,
    pub private_homes: Vec<semantic_vocabulary::PlaceId>,
    pub progress: Option<ArgumentProgress>,
}

#[derive(Clone)]
pub(crate) struct ArgumentProgress {
    prepared: PreparedStructuralArgument,
    frontiers: Vec<Frontier>,
    next: usize,
    removed_path: usize,
    consumed_selectors: usize,
}

impl OperationFrame<'_, '_> {
    pub(in crate::unit::attached_unit) fn capture_argument(
        self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        operation: &checked_trees::CheckedUnitEffectOperationPlan,
        ordinal: usize,
        selectors: &[usize],
        previous: Option<CompletedStructuralArgument>,
        complete: bool,
    ) -> Result<CompletedStructuralArgument, LoweringError> {
        let call = operation.call().ok_or(LoweringError::Unsupported(
            "structural operand capture has no checked call",
        ))?;
        let source = call
            .structural_arguments
            .get(ordinal)
            .ok_or(LoweringError::Unsupported(
                "structural operand capture has no checked actual",
            ))?;
        let mut captured = if let Some(captured) = previous {
            if captured.source != *source || captured.progress.is_none() {
                return Err(LoweringError::Unsupported(
                    "structural actual repeated or changed its source",
                ));
            }
            captured
        } else {
            self.evaluation.prepare_borrowed_argument(
                identities,
                self.checked,
                self.state,
                source,
                self.values,
                self.structural_types.declarations(),
                self.calls.domain_declarations,
                self.next_place,
                self.operations,
                self.private_places,
            )?;
            let earlier = self
                .results
                .earlier(
                    std::slice::from_ref(source),
                    self.operations,
                    self.evaluation,
                )?
                .into_owned();
            let mut prepared = super::super::parameters::prepare_structural_argument(
                identities,
                self.checked,
                source,
                self.structural_types.declarations(),
                &self.parameters,
                &[],
                &earlier,
                &self.operations.structural_values.clone(),
                &[],
                &mut 0,
                self.primitive_locals,
                &self.evaluation.structural_locals,
                Some(&self.evaluation.scalar_bindings),
            )?;
            let place = prepared.source.known_place()?;
            prepared.root_type = prepared
                .root_type
                .or_else(|| {
                    self.parameters
                        .iter()
                        .chain(self.evaluation.structural_root_parameters())
                        .find(|row| row.place == place)
                        .map(|row| row.structural_type)
                })
                .or_else(|| {
                    self.operations
                        .iter()
                        .chain(
                            self.evaluation
                                .blocks
                                .iter()
                                .flat_map(|block| block.operations.iter()),
                        )
                        .find_map(|operation| {
                            operation
                                .result
                                .structural()
                                .filter(|result| result.place == place)
                                .map(|result| result.structural_type)
                        })
                })
                .or_else(|| {
                    self.private_places
                        .iter()
                        .find(|row| row.id == place)
                        .and_then(|row| {
                            row.kind.as_operation_result().map(|(_, identity)| identity)
                        })
                });
            let parameters =
                super::super::call_parameters::structural_parameters(self.checked, call)?;
            let parameter = parameters.get(ordinal).ok_or(LoweringError::Unsupported(
                "structural operand capture lost its selected formal",
            ))?;
            let copy = source.access == checked_trees::CheckedStructuralAccess::Owned
                && parameter.multiplicity == language_semantics::Multiplicity::Unrestricted;
            let frontiers = if let Some(root) = prepared.root_type {
                crate::emission::completed_place::plan(
                    self.structural_types.declarations(),
                    root,
                    &prepared.path,
                    copy,
                    prepared.access,
                )?
            } else if prepared.path.is_empty() && !copy {
                Vec::new()
            } else {
                return Err(LoweringError::Unsupported(
                    "completed actual lost its root carrier",
                ));
            };
            CompletedStructuralArgument {
                source: source.clone(),
                argument: StructuralArgument::whole(place, prepared.access),
                private_homes: Vec::new(),
                progress: Some(ArgumentProgress {
                    prepared,
                    frontiers,
                    next: 0,
                    removed_path: 0,
                    consumed_selectors: 0,
                }),
            }
        };
        let progress = captured
            .progress
            .as_mut()
            .ok_or(LoweringError::Unsupported(
                "completed actual has no active place evaluator",
            ))?;
        progress.prepared.selector_positions = selectors
            .get(progress.consumed_selectors..)
            .ok_or(LoweringError::Unsupported(
                "completed actual lost its selector prefix",
            ))?
            .to_vec();
        while let Some(frontier) = progress.frontiers.get(progress.next).cloned() {
            match frontier {
                Frontier::Selector { .. } => {
                    if complete {
                        return Err(LoweringError::Unsupported(
                            "actual completion skipped an authored selector",
                        ));
                    }
                    progress.next += 1;
                    return Ok(captured);
                }
                Frontier::Address {
                    prefix,
                    structural_type,
                } => {
                    let prefix = prefix.checked_sub(progress.removed_path).ok_or(
                        LoweringError::Unsupported(
                            "actual place frontier precedes its saved address",
                        ),
                    )?;
                    let mut prepared = progress.prepared.clone();
                    prepared.path.truncate(prefix);
                    let mut source =
                        prepared.emit(self.values, self.calls, self.next_value, self.operations)?;
                    source.access = match source.access {
                        terminal_psi::StructuralAccess::Owned => {
                            terminal_psi::StructuralAccess::SharedBorrow
                        }
                        access => access,
                    };
                    let (qualifications, projected) = selected_memberships(
                        &source,
                        &self.parameters,
                        self.evaluation,
                        self.operations,
                    )?;
                    let place = super::place_id(super::allocate_dense(self.next_place)?);
                    crate::emission::completed_address::capture_into(
                        source.place,
                        source.path,
                        structural_type,
                        source.access,
                        qualifications,
                        projected,
                        place,
                        self.evaluation,
                        self.values,
                        self.next_value,
                        self.next_block,
                        self.next_edge,
                        self.private_places,
                        self.operations,
                    )?;
                    progress.removed_path += prefix;
                    progress.consumed_selectors += prepared.selector_positions.len();
                    progress.prepared.source = place.into();
                    progress.prepared.path.drain(..prefix);
                    progress.prepared.root_type = Some(structural_type);
                    progress.prepared.selector_positions.clear();
                }
                Frontier::Value { structural_type } => {
                    if !complete {
                        return Err(LoweringError::Unsupported(
                            "copy completed before its authored selector",
                        ));
                    }
                    let source = progress.prepared.emit(
                        self.values,
                        self.calls,
                        self.next_value,
                        self.operations,
                    )?;
                    let place = super::place_id(super::allocate_dense(self.next_place)?);
                    let declaration = emit_snapshot(
                        &source,
                        place,
                        structural_type,
                        &self.parameters,
                        self.evaluation,
                        self.operations,
                    )?;
                    self.private_places.push(declaration);
                    self.operations.register_structural_materialization(
                        place,
                        self.state,
                        call.coordinate.statement_index,
                    )?;
                    captured.private_homes.push(place);
                    progress.prepared = PreparedStructuralArgument::from_static(
                        StructuralArgument::whole(place, terminal_psi::StructuralAccess::Owned),
                        Some(structural_type),
                    )?;
                }
            }
            progress.next += 1;
        }
        if !complete {
            return Err(LoweringError::Unsupported(
                "selector exceeds the actual place frontier",
            ));
        }
        captured.argument =
            progress
                .prepared
                .emit(self.values, self.calls, self.next_value, self.operations)?;
        captured.progress = None;
        Ok(captured)
    }
}

pub(crate) fn selected_memberships(
    source: &StructuralArgument,
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    evaluation: &super::Evaluation,
    operations: &crate::emission::operation_emission::buffer::OperationBuffer,
) -> Result<
    (
        Vec<semantic_vocabulary::StructuralDomainId>,
        Vec<terminal_psi::StructuralPathQualification>,
    ),
    LoweringError,
> {
    let memberships = super::super::structural_values::member_memberships::source_memberships_in(
        source.place,
        parameters
            .iter()
            .chain(evaluation.structural_root_parameters()),
        operations.iter().chain(
            evaluation
                .blocks
                .iter()
                .flat_map(|block| block.operations.iter()),
        ),
    )
    .ok_or(LoweringError::Unsupported(
        "completed operand lost its supplying memberships",
    ))?;
    Ok(terminal_psi::StructuralPathQualification::select(
        memberships.1,
        memberships.2,
        &source.path,
    ))
}

/// Both scalar and structural call schedules publish the same copy result.
/// Only completed source contracts supply memberships. The saved occurrence is
/// unrestricted because its representation was admitted as copyable; it does
/// not inherit a callee parameter's disposal metadata or mint a loan/claim.
pub(crate) fn emit_snapshot(
    source: &StructuralArgument,
    place: semantic_vocabulary::PlaceId,
    structural_type: semantic_vocabulary::StructuralTypeId,
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    evaluation: &super::Evaluation,
    operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
) -> Result<terminal_psi::StructuralPlaceDeclaration, LoweringError> {
    let (qualifications, projected_qualifications) =
        selected_memberships(source, parameters, evaluation, operations)?;
    let producer = operations.allocate();
    operations.push(terminal_psi::Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: producer,
        result: terminal_psi::OperationResult::Structural(
            terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                qualifications,
                projected_qualifications,
                claims: Vec::new(),
            },
        ),
        kind: terminal_psi::OperationKind::StructuralLeafCopy {
            source: source.place,
            path: source.path.clone(),
        },
    });
    Ok(terminal_psi::StructuralPlaceDeclaration {
        id: place,
        kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        },
    })
}
