/*
    Structural ownership, affine claim frontiers, argument binding, field-store
    custody, and borrowed window tracking in the Terminal Psi engine.

    We enforce single-move affine custody, split and merge claim frontiers across
    projections, and verify loan provenance at call and block boundaries.
*/

pub(crate) mod complete_values {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::engine::execution::TerminalExecution;
    use semantic_vocabulary::{PlaceId, StructuralTypeId};
    use terminal_psi::{StructuralMultiplicity, StructuralPathSegment};

    impl TerminalExecution<'_> {
        pub(crate) fn validate_complete_owned_value_claims(
            &self,
            place: PlaceId,
            structural_type: StructuralTypeId,
            region: &[StructuralPathSegment],
        ) -> Result<(), TerminalInterpretError> {
            let root = self.structural_value(place)?;
            if root.structural_type != structural_type {
                return Err(malformed());
            }
            let types = self.structural_types();
            terminal_semantics::owned_structure::validate_complete_claim_frontier(
                |identity| types.get(&identity),
                structural_type,
                region,
                self.live_claims.values().filter_map(|claim| {
                    (claim.place == Some(place)
                        && claim.multiplicity == Some(StructuralMultiplicity::Linear))
                    .then_some(claim.path.as_slice())
                }),
                &|path, case| {
                    crate::boundary::structural_inputs::case_membership::stored_case_at_projection(
                        &types,
                        root,
                        self.structural_cases.as_map(),
                        path,
                    ) == Some(case)
                },
            )
            .map_err(|reason| TerminalInterpretError::VerifiedOperationContractInvalid { reason })
        }
    }
}
pub(crate) mod field_stores {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::StructuralRuntimePlace;
    use crate::engine::execution::TerminalExecution;
    use semantic_vocabulary::{PlaceId, StructuralPlaceKind, StructuralTypeId};
    use std::collections::BTreeSet;
    use terminal_psi::{
        StructuralAccess, StructuralAffineDiscard, StructuralFieldType, StructuralMultiplicity,
        StructuralPathSegment, StructuralTypeShape,
    };

    impl TerminalExecution<'_> {
        pub(crate) fn restore_store_frontier(
            &self,
            frontier: &mut BTreeSet<StructuralAffineDiscard>,
            destination: PlaceId,
            path: &[StructuralPathSegment],
            hole: &StructuralRuntimePlace,
            structural_type: StructuralTypeId,
        ) -> Result<(), TerminalInterpretError> {
            if path.contains(&StructuralPathSegment::Referent) {
                return Ok(());
            }
            let machine = self.program.machine(self.current_machine)?;
            let parameter = machine.structural_parameter(destination);
            let owned_affine = if let Some(parameter) = parameter {
                parameter.access == StructuralAccess::Owned
                    && parameter.multiplicity == StructuralMultiplicity::Affine
            } else {
                let declaration = machine
                    .structural_place(destination)
                    .ok_or_else(malformed)?;
                let StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } = declaration.kind
                else {
                    return Err(malformed());
                };
                let result = machine
                    .operation(producer)
                    .and_then(|operation| operation.result.structural())
                    .ok_or_else(malformed)?;
                if result.place != destination || result.structural_type != structural_type {
                    return Err(malformed());
                }
                result.multiplicity == StructuralMultiplicity::Affine
            };
            if !owned_affine {
                return Ok(());
            }
            let root = self.structural_value(destination)?;
            let Some(owned) = self.owned_window_projection(destination, root, path, hole)? else {
                return Ok(());
            };
            let selected_path = owned.path;
            if frontier
                .iter()
                .any(|entry| entry.place == destination && selected_path.starts_with(&entry.path))
            {
                return Ok(());
            }
            if frontier
                .iter()
                .any(|entry| entry.place == destination && entry.path.starts_with(&selected_path))
            {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            frontier.insert(StructuralAffineDiscard {
                place: destination,
                path: selected_path.clone(),
                structural_type,
            });
            for depth in (0..selected_path.len()).rev() {
                let parent_path = &selected_path[..depth];
                let parent_type = crate::engine::custody::resolve_structural_path_type(
                    &self.structural_types(),
                    root.structural_type,
                    parent_path,
                )?;
                let declaration = self.structural_declaration(parent_type)?;
                let children = match &declaration.shape {
                    StructuralTypeShape::Record { fields, .. } => fields
                        .iter()
                        .filter(|field| !field.relevance.is_erased())
                        .filter_map(|field| match field.field_type {
                            StructuralFieldType::Structural(structural_type) => {
                                let mut path = parent_path.to_vec();
                                path.push(StructuralPathSegment::Field(field.identity));
                                Some(StructuralAffineDiscard {
                                    place: destination,
                                    path,
                                    structural_type,
                                })
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>(),
                    StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    } if usize::try_from(*length).is_ok_and(|length| length <= frontier.len()) => {
                        (0..*length)
                            .map(|index| {
                                let mut path = parent_path.to_vec();
                                path.push(StructuralPathSegment::FixedIndex(index));
                                StructuralAffineDiscard {
                                    place: destination,
                                    path,
                                    structural_type: *element,
                                }
                            })
                            .collect::<Vec<_>>()
                    }
                    _ => break,
                };
                if children.is_empty() || !children.iter().all(|child| frontier.contains(child)) {
                    break;
                }
                for child in children {
                    frontier.remove(&child);
                }
                frontier.insert(StructuralAffineDiscard {
                    place: destination,
                    path: parent_path.to_vec(),
                    structural_type: parent_type,
                });
            }
            Ok(())
        }
    }
}
pub(crate) mod windows {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::StructuralRuntimePlace;
    use crate::engine::custody::{project_structural_value, verified_structural_place};
    use crate::engine::execution::{LiveClaim, TerminalExecution};
    use semantic_vocabulary::{ClaimId, PlaceId};
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::{
        StructuralAccess, StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity,
        StructuralOperationResult, StructuralPathSegment,
    };
    use terminal_semantics::structural_paths_may_overlap;

    impl TerminalExecution<'_> {
        pub(crate) fn loan_completions_for_operation(
            &self,
            operation: semantic_vocabulary::OperationId,
        ) -> Result<Vec<terminal_psi::LoanCompletion>, TerminalInterpretError> {
            let machine = self.program.verified_machine(self.current_machine)?;
            Ok(machine
                .loan_completions
                .iter()
                .filter(|completion| completion.operation == operation)
                .cloned()
                .collect())
        }
        pub(crate) fn complete_loan_custody(
            &mut self,
            completions: &[terminal_psi::LoanCompletion],
        ) -> Result<(), TerminalInterpretError> {
            let mut claims = self.live_claims.clone();
            let mut frontier = self.live_affine_frontier.clone();
            for completion in completions {
                let owner = verified_structural_place(&self.structural_values, completion.place)?;
                let returned =
                    project_structural_value(&self.structural_types(), owner, &completion.path)?;
                if completion
                    .claims
                    .iter()
                    .any(|binding| !binding.path.starts_with(&completion.path))
                {
                    return Err(TerminalInterpretError::ClaimTransferMismatch);
                }
                claims.retain(|_, claim| {
                    claim.place != Some(completion.place)
                        || !claim.path.starts_with(&completion.path)
                });
                let active_claims = self.active_claims_in_value(owner, &completion.claims)?;
                for binding in &active_claims {
                    if claims
                        .insert(
                            binding.claim,
                            LiveClaim::at(
                                completion.place,
                                binding.path.clone(),
                                StructuralMultiplicity::Linear,
                            ),
                        )
                        .is_some()
                    {
                        return Err(TerminalInterpretError::ClaimTransferMismatch);
                    }
                }
                let case_membership = |current: &StructuralAffineDiscard| {
                    crate::boundary::structural_inputs::case_membership::stored_case_at_projection(
                        &self.structural_types(),
                        self.structural_values.get(&current.place)?,
                        self.structural_cases.as_map(),
                        &current.path,
                    )
                };
                if let Some(containing) = frontier
                    .iter()
                    .find(|entry| {
                        entry.place == completion.place
                            && entry.path != completion.path
                            && completion.path.starts_with(&entry.path)
                    })
                    .cloned()
                {
                    frontier.remove(&containing);
                    crate::engine::custody::split_affine_frontier_at_projection(
                        &self.structural_types(),
                        &mut frontier,
                        containing,
                        &completion.path,
                        &case_membership,
                    )?;
                }
                frontier.retain(|entry| {
                    entry.place != completion.place || !entry.path.starts_with(&completion.path)
                });
                if completion.multiplicity == StructuralMultiplicity::Affine {
                    frontier.insert(StructuralAffineDiscard {
                        place: completion.place,
                        path: completion.path.clone(),
                        structural_type: returned.structural_type,
                    });
                    crate::engine::custody::exclude_linear_regions(
                        &self.structural_types(),
                        &mut frontier,
                        active_claims
                            .iter()
                            .map(|claim| (completion.place, claim.path.as_slice())),
                        &case_membership,
                    )?;
                }
            }
            self.live_claims = claims;
            self.live_affine_frontier = frontier;
            Ok(())
        }

        pub(crate) fn owned_window_projection(
            &self,
            root: PlaceId,
            value: &crate::boundary::values::TerminalStructuralValue,
            path: &[StructuralPathSegment],
            hole: &StructuralRuntimePlace,
        ) -> Result<Option<StructuralArgument>, TerminalInterpretError> {
            let machine = self.program.machine(self.current_machine)?;
            if path.contains(&StructuralPathSegment::Referent)
                || self.element_view_values.contains_key(&root)
                || machine
                    .structural_parameter(root)
                    .is_some_and(|parameter| parameter.access == StructuralAccess::MutableBorrow)
            {
                return Ok(None);
            }
            if hole.opaque_identity != value.opaque_identity {
                return Err(malformed());
            }
            let path = hole
                .path
                .strip_prefix(value.path.as_slice())
                .ok_or_else(malformed)?
                .to_vec();
            Ok(Some(StructuralArgument {
                place: root,
                path,
                access: StructuralAccess::Owned,
            }))
        }

        pub(crate) fn stage_window_move_custody(
            &self,
            source: PlaceId,
            path: &[StructuralPathSegment],
            hole: &StructuralRuntimePlace,
            result: &StructuralOperationResult,
            frontier: &mut BTreeSet<StructuralAffineDiscard>,
        ) -> Result<BTreeMap<ClaimId, LiveClaim>, TerminalInterpretError> {
            let selected = crate::boundary::values::TerminalStructuralValue {
                opaque_identity: hole.opaque_identity,
                structural_type: result.structural_type,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                path: hole.path.clone(),
            };
            let mut active_result = result.clone();
            active_result.claims = self.active_claims_in_value(&selected, &result.claims)?;
            let result = &active_result;
            let case_membership = |current: &StructuralAffineDiscard| {
                crate::boundary::structural_inputs::case_membership::stored_case_at_projection(
                    &self.structural_types(),
                    &selected,
                    self.structural_cases.as_map(),
                    &current.path,
                )
            };
            let storage = self.structural_value(source)?;
            if let Some(argument) = self.owned_window_projection(source, storage, path, hole)? {
                let selected = self
                    .live_claims
                    .iter()
                    .filter(|(_, claim)| {
                        claim.place == Some(source) && claim.path.starts_with(&argument.path)
                    })
                    .collect::<Vec<_>>();
                if selected.len() != result.claims.len()
                    || result.claims.iter().any(|binding| {
                        !selected.iter().any(|(identity, claim)| {
                            **identity == binding.claim
                                && claim.path.strip_prefix(argument.path.as_slice())
                                    == Some(binding.path.as_slice())
                        })
                    })
                {
                    return Err(TerminalInterpretError::ClaimTransferMismatch);
                }
                if result.multiplicity == StructuralMultiplicity::Unrestricted
                    && selected.is_empty()
                    && !self.live_affine_frontier.iter().any(|entry| {
                        entry.place == source
                            && structural_paths_may_overlap(&entry.path, &argument.path)
                    })
                {
                    return Ok(self.live_claims.clone());
                }
                self.stage_constructor_operand(&argument, frontier, &mut Vec::new())?;
                return self.stage_constructor_claims(
                    crate::storage::record::ResultClaimOrigin::Transfer,
                    result,
                    &[(&argument, Vec::new())],
                    frontier,
                    &case_membership,
                );
            }
            let mut claims = self.live_claims.clone();
            for binding in &result.claims {
                super::bind_result_claim(&mut claims, binding, result.place, result.multiplicity)?;
            }
            self.stage_result_affine_frontier(result, frontier, &case_membership)?;
            Ok(claims)
        }

        pub(crate) fn stage_window_store_claims(
            &self,
            destination: PlaceId,
            storage: &crate::boundary::values::TerminalStructuralValue,
            path: &[StructuralPathSegment],
            hole: &StructuralRuntimePlace,
            value: &StructuralArgument,
            frontier: &mut BTreeSet<StructuralAffineDiscard>,
        ) -> Result<BTreeMap<ClaimId, LiveClaim>, TerminalInterpretError> {
            let destination = self.owned_window_projection(destination, storage, path, hole)?;
            self.require_owned_value_claims(value)?;
            let mut claims = self.live_claims.clone();
            for (identity, claim) in &self.live_claims {
                if claim.place != Some(value.place) {
                    continue;
                }
                claims.remove(identity);
                let Some(destination) = &destination else {
                    continue;
                };
                let mut restored = claim.clone();
                restored.place = Some(destination.place);
                restored.path = destination
                    .path
                    .iter()
                    .chain(&claim.path)
                    .cloned()
                    .collect();
                if claims.values().any(|held| {
                    held.place == restored.place
                        && structural_paths_may_overlap(&held.path, &restored.path)
                }) {
                    return Err(TerminalInterpretError::ClaimTransferMismatch);
                }
                if let Some(containing) = frontier
                    .iter()
                    .find(|entry| {
                        entry.place == destination.place && restored.path.starts_with(&entry.path)
                    })
                    .cloned()
                {
                    frontier.remove(&containing);
                    super::split_affine_frontier_at_projection(
                        &self.structural_types(),
                        frontier,
                        containing,
                        &restored.path,
                        &|current| {
                            let (root, path) = if let Some(relative) =
                                current.path.strip_prefix(destination.path.as_slice())
                            {
                                (self.structural_values.get(&value.place)?, relative)
                            } else {
                                (storage, current.path.as_slice())
                            };
                            crate::boundary::structural_inputs::case_membership::stored_case_at_projection(
                            &self.structural_types(),
                            root,
                            self.structural_cases.as_map(),
                            path,
                        )
                        },
                    )?;
                }
                claims.insert(*identity, restored);
            }
            Ok(claims)
        }
    }
}

use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::values::StructuralRuntimePlace;
use crate::boundary::values::{
    TerminalScalarValue, TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use crate::engine::execution::LiveClaim;
use crate::engine::scalar_operations::terminal_scalar_belongs_to_type;
use crate::storage::reference;
use crate::storage::runtime_rows::RuntimeRows;
use evaluation_accounting::EvaluationInvocationAccount;
use semantic_vocabulary::{
    ClaimId, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::TerminalMachine;
use terminal_psi::{
    BoundaryMachineDeclaration, ClaimTransfer, CompletionReceipt, EntryClaim, NominalAffineCleanup,
    StructuralAccess, StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralResultClaimBinding, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction, find_structural_case,
};

pub(crate) fn bind_result_claim(
    live_claims: &mut BTreeMap<ClaimId, crate::engine::execution::LiveClaim>,
    binding: &StructuralResultClaimBinding,
    place: PlaceId,
    multiplicity: StructuralMultiplicity,
) -> Result<(), TerminalInterpretError> {
    if live_claims
        .insert(
            binding.claim,
            crate::engine::execution::LiveClaim::at(place, binding.path.clone(), multiplicity),
        )
        .is_some()
    {
        return Err(malformed());
    }
    Ok(())
}

pub(crate) fn place_is_claimed(claims: &BTreeMap<ClaimId, LiveClaim>, place: PlaceId) -> bool {
    claims.values().any(|claim| claim.place == Some(place))
}

pub(crate) fn require_owned_value_claims<'claims>(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    live_claims: impl Iterator<Item = &'claims LiveClaim> + Clone,
    root_type: StructuralTypeId,
    argument: &StructuralArgument,
    selected_case: &dyn Fn(&[StructuralPathSegment], semantic_vocabulary::StructuralCaseId) -> bool,
) -> Result<(), TerminalInterpretError> {
    if argument.access != StructuralAccess::Owned {
        return Ok(());
    }
    let selected = resolve_structural_path_type(structural_types, root_type, &argument.path)?;
    let refusal = |reason| TerminalInterpretError::OwnedValueCustodyMismatch {
        source: argument.place,
        structural_type: selected,
        path_length: argument.path.len(),
        path_prefix: argument.path.iter().take(4).cloned().collect(),
        reason,
    };
    let claims = live_claims.filter(|claim| {
        claim.place == Some(argument.place)
            && (claim.path.starts_with(&argument.path) || argument.path.starts_with(&claim.path))
    });
    if claims
        .clone()
        .any(|claim| claim.multiplicity != Some(StructuralMultiplicity::Linear))
    {
        return Err(refusal(
            "owned value claim has no exact linear multiplicity",
        ));
    }
    terminal_semantics::owned_structure::validate_owned_claim_frontier(
        &|identity| structural_types.get(&identity),
        selected,
        &argument.path,
        claims.map(|claim| claim.path.clone()),
        selected_case,
    )
    .map_err(refusal)
}

impl crate::engine::execution::TerminalExecution<'_> {
    pub(crate) fn require_owned_value_claims(
        &self,
        argument: &StructuralArgument,
    ) -> Result<(), TerminalInterpretError> {
        if argument.access != StructuralAccess::Owned {
            return Ok(());
        }
        if let Some(home) = self.linear_homes.get(&argument.place) {
            return Err(TerminalInterpretError::OwnedValueCustodyMismatch {
                source: argument.place,
                structural_type: home.structural_type,
                path_length: argument.path.len(),
                path_prefix: argument.path.iter().take(4).cloned().collect(),
                reason: "owned operand is an unestablished storage home",
            });
        }
        let machine = self.program.machine(self.current_machine)?;
        let root_type = machine
            .structural_place_type(argument.place)
            .ok_or_else(malformed)?;
        if machine
            .structural_parameter(argument.place)
            .is_some_and(|parameter| parameter.access != StructuralAccess::Owned)
            || !terminal_semantics::projection_stays_in_inline_storage(
                self.structural_types().values(),
                root_type,
                &argument.path,
            )
        {
            return Ok(());
        }
        require_owned_value_claims(
            &self.structural_types(),
            self.live_claims.values(),
            root_type,
            argument,
            &|path, case| {
                self.structural_values.get(&argument.place).is_some_and(|value| {
                    crate::boundary::structural_inputs::case_membership::stored_case_at_projection(
                        &self.structural_types(),
                        value,
                        self.structural_cases.as_map(),
                        path,
                    ) == Some(case)
                })
            },
        )
    }
}

pub(crate) fn whole_path_discard(
    place: PlaceId,
    structural_type: StructuralTypeId,
) -> StructuralAffineDiscard {
    StructuralAffineDiscard {
        place,
        path: Vec::new(),
        structural_type,
    }
}

pub(crate) fn has_live_loan(
    references: &BTreeMap<StructuralRuntimePlace, crate::storage::reference::ReferenceReferent>,
    value: &TerminalStructuralValue,
) -> bool {
    references.values().any(|referent| {
        referent.view.opaque_identity == value.opaque_identity
            && terminal_semantics::structural_paths_may_overlap(&referent.view.path, &value.path)
    })
}

pub(crate) fn commit_cleanup_action(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    structural_cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::boundary::structural_inputs::case_membership::StructuralCaseContents,
    >,
    machines: &crate::embedding::program::ProgramDeclarations<'_, TerminalMachine>,
    current_machine: &TerminalMachine,
    structural_values: &mut RuntimeRows<PlaceId, TerminalStructuralValue>,
    linear_homes: &mut RuntimeRows<PlaceId, TerminalStructuralValue>,
    scalar_arrays: &mut crate::storage::scalar_array::ScalarArrayBindings,
    reference_referents: &mut RuntimeRows<
        StructuralRuntimePlace,
        crate::storage::reference::ReferenceReferent,
    >,
    element_views: &mut RuntimeRows<
        StructuralRuntimePlace,
        crate::storage::element_views::ElementView,
    >,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    live_claims: &mut BTreeMap<ClaimId, LiveClaim>,
    retired_homes: &mut Vec<(
        PlaceId,
        TerminalStructuralValue,
        Option<crate::storage::reference::ReferenceReferent>,
    )>,
    action: &TerminalAffineCleanupAction,
) -> Result<Option<(NominalAffineCleanup, TerminalStructuralValue)>, TerminalInterpretError> {
    match action {
        TerminalAffineCleanupAction::DiscardRoot(place) => {
            if place_is_claimed(live_claims, *place) {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            if reference::discard_structural_value(
                structural_values,
                reference_referents,
                element_views,
                retired_homes,
                *place,
                current_machine,
            )
            .is_none()
                || !remove_affine_root(frontier, *place)
            {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            live_claims.retain(|_, claim| claim.place != Some(*place));
        }
        TerminalAffineCleanupAction::DiscardResidual(discard) => {
            let root = crate::engine::custody::verified_structural_place(
                structural_values,
                discard.place,
            )?;
            consume_affine_discard(
                structural_types,
                structural_values,
                structural_cases,
                frontier,
                live_claims,
                discard,
            )?;
            let path = project_structural_value(structural_types, root, &discard.path)?.path;
            reference_referents.retain(|carrier, _| {
                carrier.opaque_identity != root.opaque_identity || !carrier.path.starts_with(&path)
            });
            element_views.retain(|carrier, _| {
                carrier.opaque_identity != root.opaque_identity || !carrier.path.starts_with(&path)
            });
        }
        TerminalAffineCleanupAction::RetireHome(place) => {
            if linear_homes.contains_key(place) && structural_values.contains_key(place) {
                return Err(malformed());
            }
            retire_cleanup_home(
                if linear_homes.contains_key(place) {
                    linear_homes
                } else {
                    structural_values
                },
                scalar_arrays,
                frontier,
                live_claims,
                *place,
            )?;
        }
        TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
            validate_nominal_cleanup_action(
                structural_types,
                structural_cases,
                machines,
                structural_values,
                frontier,
                live_claims,
                cleanup,
            )?;
            let root = crate::engine::custody::verified_structural_place(
                structural_values,
                cleanup.place,
            )?;
            let value = project_structural_value(structural_types, root, &cleanup.path)?;
            if has_live_loan(reference_referents.as_map(), &value) {
                return Err(TerminalInterpretError::NominalCleanupHasLiveLoans {
                    cleanup: cleanup.clone(),
                    backing_identity: value.opaque_identity,
                    backing_path: value.path.clone(),
                    loan_carriers: reference_referents
                        .iter()
                        .filter(|(_, referent)| {
                            referent.view.opaque_identity == value.opaque_identity
                                && terminal_semantics::structural_paths_may_overlap(
                                    &referent.view.path,
                                    &value.path,
                                )
                        })
                        .map(|(carrier, _)| (carrier.opaque_identity, carrier.path.clone()))
                        .collect(),
                    loan_carrier_homes: structural_values
                        .iter()
                        .filter(|(_, home)| {
                            reference_referents.iter().any(|(carrier, referent)| {
                                carrier.opaque_identity == home.opaque_identity
                                    && carrier.path.starts_with(&home.path)
                                    && referent.view.opaque_identity == value.opaque_identity
                                    && terminal_semantics::structural_paths_may_overlap(
                                        &referent.view.path,
                                        &value.path,
                                    )
                            })
                        })
                        .map(|(place, home)| (*place, home.clone()))
                        .collect(),
                });
            }
            begin_nominal_cleanup(
                structural_types,
                structural_values,
                structural_cases,
                frontier,
                live_claims,
                cleanup,
            )?;
            return Ok(Some((cleanup.clone(), value)));
        }
    }
    Ok(None)
}

pub(crate) fn begin_nominal_cleanup(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    values: &RuntimeRows<PlaceId, TerminalStructuralValue>,
    structural_cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::boundary::structural_inputs::case_membership::StructuralCaseContents,
    >,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    claims: &mut BTreeMap<ClaimId, LiveClaim>,
    cleanup: &NominalAffineCleanup,
) -> Result<(), TerminalInterpretError> {
    let retains_descendants = cleanup.retains_descendants;
    if cleanup.consumed_claims.is_empty() && !retains_descendants {
        consume_affine_projection(
            structural_types,
            values,
            structural_cases,
            frontier,
            &StructuralArgument {
                place: cleanup.place,
                path: cleanup.path.clone(),
                access: StructuralAccess::Owned,
            },
        )?;
        frontier.insert(StructuralAffineDiscard {
            place: cleanup.place,
            path: cleanup.path.clone(),
            structural_type: cleanup.structural_type,
        });
    }
    for claim in &cleanup.consumed_claims {
        claims.remove(claim);
    }
    if retains_descendants && !cleanup.consumed_claims.is_empty() {
        if !frontier.insert(StructuralAffineDiscard {
            place: cleanup.place,
            path: cleanup.path.clone(),
            structural_type: cleanup.structural_type,
        }) {
            return Err(TerminalInterpretError::AffineFrontierMismatch);
        }
        exclude_linear_regions(
            structural_types,
            frontier,
            claims
                .values()
                .filter(|claim| {
                    claim.place == Some(cleanup.place) && claim.path.starts_with(&cleanup.path)
                })
                .map(|claim| (cleanup.place, claim.path.as_slice())),
            &|current| {
                crate::boundary::structural_inputs::case_membership::stored_case_at_projection(
                    structural_types,
                    values.get(&current.place)?,
                    structural_cases,
                    &current.path,
                )
            },
        )?;
    }
    Ok(())
}

pub(crate) fn validate_nominal_cleanup_action(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    structural_cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::boundary::structural_inputs::case_membership::StructuralCaseContents,
    >,
    machines: &crate::embedding::program::ProgramDeclarations<'_, TerminalMachine>,
    values: &RuntimeRows<PlaceId, TerminalStructuralValue>,
    frontier: &BTreeSet<StructuralAffineDiscard>,
    claims: &BTreeMap<ClaimId, LiveClaim>,
    cleanup: &NominalAffineCleanup,
) -> Result<(), TerminalInterpretError> {
    let invalid = || TerminalInterpretError::AffineFrontierMismatch;
    let root = values.get(&cleanup.place).ok_or_else(invalid)?;
    let value = project_structural_value(structural_types, root, &cleanup.path)?;
    terminal_semantics::nominal_cleanup::target(
        structural_types.values(),
        machines.values(),
        cleanup,
    )
    .map_err(|_| invalid())?;
    let retains_descendants = cleanup.retains_descendants;
    let source_claims = claims
        .iter()
        .filter(|(identity, claim)| {
            claim.place == Some(cleanup.place)
                && if retains_descendants {
                    cleanup.consumed_claims.contains(identity)
                } else {
                    terminal_semantics::structural_paths_may_overlap(&claim.path, &cleanup.path)
                }
        })
        .collect::<Vec<_>>();
    if value.structural_type != cleanup.structural_type
        || cleanup.consumed_claims
            != source_claims
                .iter()
                .map(|(identity, _)| **identity)
                .collect::<Vec<_>>()
        || source_claims.iter().any(|(_, claim)| {
            !claim.path.starts_with(&cleanup.path)
                || claim.multiplicity != Some(StructuralMultiplicity::Linear)
        })
    {
        return Err(invalid());
    }
    if !retains_descendants {
        if source_claims.is_empty() {
            let mut remaining = frontier.clone();
            consume_affine_projection(
                structural_types,
                values,
                structural_cases,
                &mut remaining,
                &StructuralArgument {
                    place: cleanup.place,
                    path: cleanup.path.clone(),
                    access: StructuralAccess::Owned,
                },
            )
            .map_err(|_| invalid())?;
        } else if frontier.iter().any(|entry| {
            entry.place == cleanup.place
                && terminal_semantics::structural_paths_may_overlap(&entry.path, &cleanup.path)
        }) {
            return Err(invalid());
        }
    }
    Ok(())
}

pub(crate) fn complete_nominal_cleanup(
    values: &mut RuntimeRows<PlaceId, TerminalStructuralValue>,
    scalar_arrays: &mut impl crate::storage::scalar_array::PackedHomeRetirement,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    claims: &BTreeMap<ClaimId, LiveClaim>,
    cleanup: &NominalAffineCleanup,
) -> Result<(), TerminalInterpretError> {
    if cleanup.retains_descendants {
        return Ok(());
    }
    if if cleanup.consumed_claims.is_empty() {
        !frontier.remove(&StructuralAffineDiscard {
            place: cleanup.place,
            path: cleanup.path.clone(),
            structural_type: cleanup.structural_type,
        })
    } else {
        frontier.iter().any(|entry| {
            entry.place == cleanup.place
                && terminal_semantics::structural_paths_may_overlap(&entry.path, &cleanup.path)
        }) || cleanup
            .consumed_claims
            .iter()
            .any(|claim| claims.contains_key(claim))
    } {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    }
    if cleanup.path.is_empty() {
        retire_cleanup_home(values, scalar_arrays, frontier, claims, cleanup.place)?;
    }
    Ok(())
}

pub(crate) fn retire_cleanup_home(
    values: &mut RuntimeRows<PlaceId, TerminalStructuralValue>,
    scalar_arrays: &mut impl crate::storage::scalar_array::PackedHomeRetirement,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    claims: &BTreeMap<ClaimId, LiveClaim>,
    place: PlaceId,
) -> Result<(), TerminalInterpretError> {
    if place_is_claimed(claims, place) {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    }
    match (
        values.contains_key(&place),
        scalar_arrays.contains_packed_home(&place),
    ) {
        (true, false) => {
            values.remove(&place);
        }
        (false, true) => scalar_arrays.retire_packed_home(&place),
        (false, false) => {
            return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                place,
            ));
        }
        (true, true) => return Err(malformed()),
    }
    frontier.retain(|entry| entry.place != place);
    Ok(())
}

pub(crate) fn bind_arguments(
    parameters: &[terminal_psi::ValueDeclaration],
    arguments: &[TerminalScalarValue],
    account: Option<&EvaluationInvocationAccount>,
) -> Result<RuntimeRows<ValueId, TerminalScalarValue>, TerminalInterpretError> {
    if arguments.len() != parameters.len() {
        return Err(TerminalInterpretError::ArgumentCount {
            expected: parameters.len(),
            actual: arguments.len(),
        });
    }
    let mut values = RuntimeRows::new(account);
    for (parameter, argument) in parameters.iter().zip(arguments) {
        if parameter.scalar_type != argument.scalar_type() {
            return Err(TerminalInterpretError::ArgumentType {
                value: parameter.id,
                expected: parameter.scalar_type,
                actual: argument.scalar_type(),
            });
        }
        if let TerminalScalarValue::Integer { scalar_type, value } = argument
            && !scalar_type.admits(*value)
        {
            return Err(TerminalInterpretError::ArgumentIntegerOutsideType {
                value: parameter.id,
            });
        }
        values.insert(parameter.id, *argument)?;
    }
    Ok(values)
}

pub(crate) fn bind_boundary_arguments(
    parameters: &[ScalarType],
    arguments: &[TerminalScalarValue],
) -> Result<(), TerminalInterpretError> {
    if arguments.len() != parameters.len()
        || parameters
            .iter()
            .zip(arguments)
            .any(|(parameter, argument)| *parameter != argument.scalar_type())
    {
        return Err(malformed());
    }
    Ok(())
}

pub(crate) fn bind_active_structural_arguments(
    parameters: &[StructuralParameterDeclaration],
    arguments: &[TerminalStructuralValue],
    types: &crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::boundary::structural_inputs::case_membership::StructuralCaseContents,
    >,
    account: Option<&EvaluationInvocationAccount>,
) -> Result<RuntimeRows<PlaceId, TerminalStructuralValue>, TerminalInterpretError> {
    let active = active_structural_parameters(parameters, arguments, &|argument_index, rows| {
        crate::boundary::structural_inputs::case_membership::active_projected_qualifications(
            types,
            &arguments[argument_index],
            cases,
            rows,
        )
    })?;
    bind_structural_arguments(&active, arguments, account)
}

#[derive(Default)]
pub(crate) struct StructuralArgumentProvenance<'a> {
    pub(crate) parameter: Option<&'a StructuralParameterDeclaration>,
    pub(crate) reference_projection: Option<StructuralRuntimePlace>,
}

pub(crate) struct StructuralArgumentSources<'a, 'program> {
    pub(crate) arguments: &'a [StructuralArgument],
    pub(crate) provenance: &'a [StructuralArgumentProvenance<'program>],
}

impl crate::engine::execution::TerminalExecution<'_> {
    pub(crate) fn structural_argument_provenance(
        &self,
        arguments: &[StructuralArgument],
    ) -> Result<Vec<StructuralArgumentProvenance<'_>>, TerminalInterpretError> {
        arguments
            .iter()
            .map(|argument| {
                let reference_projection = if argument.access != StructuralAccess::Owned
                    && argument.path.contains(&StructuralPathSegment::Referent)
                {
                    self.resolve_reference_storage_projection(
                        self.structural_value(argument.place)?,
                        &argument.path,
                        argument.access,
                    )?
                    .map(|projection| StructuralRuntimePlace::from(&projection.value))
                } else {
                    None
                };
                Ok(StructuralArgumentProvenance {
                    parameter: self.owned_block_source(argument.place),
                    reference_projection,
                })
            })
            .collect()
    }
}

pub(crate) fn bind_active_retained_arguments(
    parameters: &[StructuralParameterDeclaration],
    sources: &[terminal_psi::StructuralArgument],
    arguments: &[TerminalStructuralValue],
    provenance: &[StructuralArgumentProvenance<'_>],
    active_qualifications: &impl Fn(
        usize,
        &[terminal_psi::StructuralPathQualification],
    ) -> Result<
        Vec<terminal_psi::StructuralPathQualification>,
        TerminalInterpretError,
    >,
    types: &crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    account: Option<&EvaluationInvocationAccount>,
) -> Result<RuntimeRows<PlaceId, TerminalStructuralValue>, TerminalInterpretError> {
    if sources.len() != parameters.len() || provenance.len() != parameters.len() {
        return Err(malformed());
    }
    let active = active_structural_parameters(parameters, arguments, active_qualifications)?;
    bind_existing_structural_places(
        &active,
        arguments,
        sources,
        provenance,
        Some(types),
        account,
    )
}

fn active_structural_parameters(
    parameters: &[StructuralParameterDeclaration],
    arguments: &[TerminalStructuralValue],
    active_qualifications: &impl Fn(
        usize,
        &[terminal_psi::StructuralPathQualification],
    ) -> Result<
        Vec<terminal_psi::StructuralPathQualification>,
        TerminalInterpretError,
    >,
) -> Result<Vec<StructuralParameterDeclaration>, TerminalInterpretError> {
    if parameters.len() != arguments.len() {
        return Err(TerminalInterpretError::StructuralArgumentCount {
            expected: parameters.len(),
            actual: arguments.len(),
        });
    }
    let mut active_parameters = parameters.to_vec();
    for (argument_index, parameter) in active_parameters.iter_mut().enumerate() {
        parameter.projected_qualifications =
            active_qualifications(argument_index, &parameter.projected_qualifications)?;
    }
    Ok(active_parameters)
}

pub(crate) fn bind_structural_arguments(
    parameters: &[StructuralParameterDeclaration],
    arguments: &[TerminalStructuralValue],
    account: Option<&EvaluationInvocationAccount>,
) -> Result<RuntimeRows<PlaceId, TerminalStructuralValue>, TerminalInterpretError> {
    bind_existing_structural_places(parameters, arguments, &[], &[], None, account)
}

fn retains_source_lender(
    parameters: &[StructuralParameterDeclaration],
    arguments: &[TerminalStructuralValue],
    sources: &[terminal_psi::StructuralArgument],
    provenance: &[StructuralArgumentProvenance<'_>],
    types: Option<&crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>>,
    first: usize,
    second: usize,
) -> bool {
    for (lender, child) in [(first, second), (second, first)] {
        if parameters[child].access == StructuralAccess::Owned {
            continue;
        }
        let (Some(lender_source), Some(child_source)) = (sources.get(lender), sources.get(child))
        else {
            continue;
        };
        let lender_parameter = provenance.get(lender).and_then(|source| source.parameter);
        let retained_mutable = parameters[lender].access == StructuralAccess::MutableBorrow;
        let lender_access = match parameters[lender].access {
            StructuralAccess::MutableBorrow
                if parameters[lender].multiplicity == StructuralMultiplicity::Unrestricted
                    && lender_source.access == StructuralAccess::MutableBorrow =>
            {
                let Some(types) = types else { continue };
                let Some(source_parameter) = lender_parameter.filter(|parameter| {
                    parameter.access == StructuralAccess::MutableBorrow
                        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                }) else {
                    continue;
                };
                let Some(suffix) = arguments[child]
                    .path
                    .strip_prefix(arguments[lender].path.as_slice())
                else {
                    continue;
                };
                if suffix.is_empty()
                    || !suffix.iter().any(|segment| {
                        !matches!(segment, StructuralPathSegment::RepresentationView { .. })
                    })
                    || !terminal_semantics::projection_stays_in_inline_storage(
                        types.values(),
                        arguments[lender].structural_type,
                        suffix,
                    )
                    || !terminal_semantics::projection_stays_in_inline_storage(
                        types.values(),
                        source_parameter.structural_type,
                        &lender_source.path,
                    )
                {
                    continue;
                }
                StructuralAccess::MutableBorrow
            }
            StructuralAccess::Owned
                if parameters[lender].multiplicity == StructuralMultiplicity::Affine
                    && lender_source.access == StructuralAccess::Owned =>
            {
                StructuralAccess::Owned
            }
            StructuralAccess::SharedBorrow
                if lender_source.access == StructuralAccess::SharedBorrow =>
            {
                match lender_parameter {
                    Some(parameter)
                        if parameter.access == StructuralAccess::MutableBorrow
                            && parameter.multiplicity == StructuralMultiplicity::Unrestricted =>
                    {
                        StructuralAccess::MutableBorrow
                    }
                    Some(parameter)
                        if parameter.access == StructuralAccess::Owned
                            && parameter.multiplicity == StructuralMultiplicity::Affine =>
                    {
                        StructuralAccess::Owned
                    }
                    _ => continue,
                }
            }
            _ => continue,
        };
        if (!retained_mutable && !lender_source.path.is_empty())
            || !arguments[child].path.starts_with(&arguments[lender].path)
            || child_source.access != parameters[child].access
            || !crate::storage::reference::access_covers(lender_access, parameters[child].access)
        {
            continue;
        }
        let resolved_reference = provenance
            .get(child)
            .and_then(|source| source.reference_projection.as_ref())
            .is_some_and(|source| *source == StructuralRuntimePlace::from(&arguments[child]));
        let forwarded_parameter = lender_parameter.is_some_and(|source| {
            source.access == lender_access
                && (source.access != StructuralAccess::Owned
                    || source.multiplicity == StructuralMultiplicity::Affine)
        }) && provenance
            .get(child)
            .and_then(|source| source.parameter)
            .is_some_and(|source| {
                source.access != StructuralAccess::Owned
                    && source.multiplicity == StructuralMultiplicity::Unrestricted
                    && crate::storage::reference::access_covers(source.access, child_source.access)
            });
        if lender_source.place == child_source.place || resolved_reference || forwarded_parameter {
            return true;
        }
    }
    false
}

fn bind_existing_structural_places(
    parameters: &[StructuralParameterDeclaration],
    arguments: &[TerminalStructuralValue],
    sources: &[terminal_psi::StructuralArgument],
    provenance: &[StructuralArgumentProvenance<'_>],
    types: Option<&crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>>,
    account: Option<&EvaluationInvocationAccount>,
) -> Result<RuntimeRows<PlaceId, TerminalStructuralValue>, TerminalInterpretError> {
    if arguments.len() != parameters.len() {
        return Err(TerminalInterpretError::StructuralArgumentCount {
            expected: parameters.len(),
            actual: arguments.len(),
        });
    }
    let mut values = RuntimeRows::new(account);
    for (argument_index, (parameter, argument)) in parameters.iter().zip(arguments).enumerate() {
        if argument
            .qualifications
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || argument
                .projected_qualifications
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(TerminalInterpretError::StructuralQualificationsNonCanonical);
        }
        if parameter.structural_type != argument.structural_type {
            return Err(TerminalInterpretError::StructuralArgumentType {
                place: parameter.place,
                expected: parameter.structural_type,
                actual: argument.structural_type,
            });
        }
        if parameter
            .qualifications
            .iter()
            .any(|domain| !argument.qualifications.contains(domain))
            || parameter
                .projected_qualifications
                .iter()
                .any(|row| !argument.projected_qualifications.contains(row))
        {
            return Err(TerminalInterpretError::StructuralQualificationMissing(
                parameter.place,
            ));
        }
        for (previous_index, (previous_parameter, previous_argument)) in parameters
            [..argument_index]
            .iter()
            .zip(&arguments[..argument_index])
            .enumerate()
        {
            if previous_argument.opaque_identity != argument.opaque_identity {
                continue;
            }
            let exclusive = matches!(
                previous_parameter.access,
                terminal_psi::exclusive_borrow_pair_pattern!()
            ) || matches!(
                parameter.access,
                terminal_psi::exclusive_borrow_pair_pattern!()
            );
            let overlapping = terminal_semantics::structural_paths_may_overlap(
                &previous_argument.path,
                &argument.path,
            );
            let custody = previous_parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted;
            if overlapping
                && (custody || exclusive)
                && !retains_source_lender(
                    parameters,
                    arguments,
                    sources,
                    provenance,
                    types,
                    previous_index,
                    argument_index,
                )
            {
                return Err(TerminalInterpretError::StructuralArgumentAliasing(
                    argument.opaque_identity,
                ));
            }
        }
        if values.insert(parameter.place, argument.clone())?.is_some() {
            return Err(malformed());
        }
    }
    Ok(values)
}

pub(crate) fn bind_structural_primitive_values(
    machine: &TerminalMachine,
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    structural_values: &RuntimeRows<PlaceId, TerminalStructuralValue>,
    arguments: &[TerminalStructuralPrimitiveValue],
    mut storage: crate::storage::scalar_storage::ScalarStorageRows<StructuralRuntimePlace>,
    zero_initialized_arguments: &[u32],
    account: Option<&evaluation_accounting::EvaluationInvocationAccount>,
) -> Result<
    (
        crate::storage::scalar_storage::ScalarStorageRows<StructuralRuntimePlace>,
        RuntimeRows<u32, StructuralRuntimePlace>,
    ),
    TerminalInterpretError,
> {
    let primitive_parameters = machine
        .structural_parameters
        .iter()
        .enumerate()
        .filter_map(|(argument_index, parameter)| {
            matches!(
                structural_types
                    .get(&parameter.structural_type)
                    .map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::PrimitiveScalar(_))
            )
            .then_some((argument_index as u32, parameter))
        })
        .collect::<BTreeMap<_, _>>();
    let expected = primitive_parameters
        .keys()
        .filter(|position| !zero_initialized_arguments.contains(position))
        .count();
    if expected != arguments.len() {
        return Err(TerminalInterpretError::StructuralPrimitiveValueCount {
            expected,
            actual: arguments.len(),
        });
    }

    let mut entry_places = RuntimeRows::new(account);
    for (position, parameter) in &primitive_parameters {
        if zero_initialized_arguments.contains(position) {
            let root = structural_values.require(&parameter.place)?;
            let place = StructuralRuntimePlace::from(root);
            if !storage.contains_key(&place) {
                return Err(malformed());
            }
            entry_places.insert(*position, place)?;
        }
    }
    for argument in arguments {
        let parameter = primitive_parameters
            .get(&argument.argument_index)
            .copied()
            .ok_or(TerminalInterpretError::StructuralPrimitiveValueInvalid {
                argument_index: argument.argument_index,
            })?;
        let Some(StructuralTypeShape::PrimitiveScalar(expected)) = structural_types
            .get(&parameter.structural_type)
            .map(|declaration| &declaration.shape)
        else {
            return Err(TerminalInterpretError::StructuralPrimitiveValueInvalid {
                argument_index: argument.argument_index,
            });
        };
        if argument.value.scalar_type() != *expected
            || !terminal_scalar_belongs_to_type(argument.value)
        {
            return Err(TerminalInterpretError::StructuralPrimitiveValueType {
                argument_index: argument.argument_index,
                expected: *expected,
                actual: argument.value.scalar_type(),
            });
        }
        let view =
            crate::engine::custody::verified_structural_place(structural_values, parameter.place)?;
        let place = StructuralRuntimePlace::from(view);
        if entry_places
            .insert(argument.argument_index, place.clone())?
            .is_some()
            || storage.insert(place, argument.value)?.is_some()
        {
            return Err(TerminalInterpretError::StructuralPrimitiveValueInvalid {
                argument_index: argument.argument_index,
            });
        }
    }
    Ok((storage, entry_places))
}

pub(crate) fn bind_affine_frontier(
    parameters: &[StructuralParameterDeclaration],
    values: &RuntimeRows<PlaceId, TerminalStructuralValue>,
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
) -> Result<BTreeSet<StructuralAffineDiscard>, TerminalInterpretError> {
    bind_affine_frontier_types(parameters, structural_types, |place| {
        values.get(&place).map(|value| value.structural_type)
    })
}

pub(crate) fn bind_affine_frontier_types(
    parameters: &[StructuralParameterDeclaration],
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    source_type: impl Fn(PlaceId) -> Option<StructuralTypeId>,
) -> Result<BTreeSet<StructuralAffineDiscard>, TerminalInterpretError> {
    let mut frontier = BTreeSet::new();
    for parameter in parameters.iter().filter(|parameter| {
        parameter.multiplicity == StructuralMultiplicity::Affine
            && (parameter.access == StructuralAccess::Owned
                || terminal_semantics::view_descriptor_parameter(
                    parameter,
                    structural_types.values(),
                ))
    }) {
        let structural_type = source_type(parameter.place).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(parameter.place),
        )?;
        if structural_type != parameter.structural_type
            || !frontier.insert(StructuralAffineDiscard {
                place: parameter.place,
                path: Vec::new(),
                structural_type: parameter.structural_type,
            })
        {
            return Err(malformed());
        }
    }
    Ok(frontier)
}

pub(crate) fn remove_affine_root(
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    place: PlaceId,
) -> bool {
    let Some(root) = frontier
        .iter()
        .find(|entry| entry.place == place && entry.path.is_empty())
        .cloned()
    else {
        return false;
    };
    frontier.remove(&root)
}

pub(crate) fn whole_value_frontier<'a>(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    place: PlaceId,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    claim_paths: impl IntoIterator<Item = &'a [StructuralPathSegment]>,
    case_membership: &impl Fn(&StructuralAffineDiscard) -> Option<semantic_vocabulary::StructuralCaseId>,
) -> Result<BTreeSet<StructuralAffineDiscard>, TerminalInterpretError> {
    let mut frontier = BTreeSet::new();
    if multiplicity == StructuralMultiplicity::Affine {
        frontier.insert(StructuralAffineDiscard {
            place,
            path: Vec::new(),
            structural_type,
        });
        exclude_linear_regions(
            structural_types,
            &mut frontier,
            claim_paths.into_iter().map(|path| (place, path)),
            case_membership,
        )?;
    }
    Ok(frontier)
}

pub(crate) fn exclude_linear_regions<'a>(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    claims: impl IntoIterator<Item = (PlaceId, &'a [StructuralPathSegment])>,
    case_membership: &impl Fn(&StructuralAffineDiscard) -> Option<semantic_vocabulary::StructuralCaseId>,
) -> Result<(), TerminalInterpretError> {
    let mut claims = claims.into_iter();
    let Some(first_claim) = claims.next() else {
        return Ok(());
    };
    let affine_roots = frontier
        .iter()
        .map(|entry| entry.place)
        .collect::<BTreeSet<_>>();
    for (place, path) in std::iter::once(first_claim).chain(claims) {
        if !affine_roots.contains(&place) {
            continue;
        }
        let containing = frontier
            .iter()
            .find(|entry| entry.place == place && path.starts_with(&entry.path))
            .cloned()
            .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
        frontier.remove(&containing);
        split_affine_frontier_at_projection(
            structural_types,
            frontier,
            containing,
            path,
            case_membership,
        )?;
    }
    Ok(())
}

pub(crate) fn projected_ownership_claims(
    claims: &BTreeMap<ClaimId, LiveClaim>,
    argument: &StructuralArgument,
) -> Result<Vec<(ClaimId, LiveClaim)>, TerminalInterpretError> {
    let mut selected = Vec::new();
    for (identity, claim) in claims {
        if claim.place != Some(argument.place) {
            continue;
        }
        if let Some(suffix) = claim.path.strip_prefix(argument.path.as_slice()) {
            let mut moved = claim.clone();
            moved.path = suffix.to_vec();
            selected.push((*identity, moved));
        } else if argument.path.starts_with(&claim.path) {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
    }
    Ok(selected)
}

pub(crate) fn consume_projected_ownership(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    values: &RuntimeRows<PlaceId, TerminalStructuralValue>,
    structural_cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::boundary::structural_inputs::case_membership::StructuralCaseContents,
    >,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    claims: &mut BTreeMap<ClaimId, LiveClaim>,
    argument: &StructuralArgument,
) -> Result<(), TerminalInterpretError> {
    if argument.access != StructuralAccess::Owned || argument.path.is_empty() {
        return Err(malformed());
    }
    let selected_claims = projected_ownership_claims(claims, argument)?;
    let affine = consume_projected_affine_complement(
        structural_types,
        values,
        structural_cases,
        frontier,
        argument,
    )?;
    if !affine && selected_claims.is_empty() {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    }
    for (identity, _) in selected_claims {
        claims.remove(&identity);
    }
    Ok(())
}

pub(crate) fn consume_projected_affine_complement(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    values: &RuntimeRows<PlaceId, TerminalStructuralValue>,
    structural_cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::boundary::structural_inputs::case_membership::StructuralCaseContents,
    >,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    argument: &StructuralArgument,
) -> Result<bool, TerminalInterpretError> {
    if argument.access != StructuralAccess::Owned || argument.path.is_empty() {
        return Err(malformed());
    }
    crate::boundary::structural_inputs::case_membership::validate_case_projection(
        structural_types,
        values,
        structural_cases,
        argument.place,
        &argument.path,
    )?;
    let containing = frontier
        .iter()
        .any(|entry| entry.place == argument.place && argument.path.starts_with(&entry.path));
    let contained = frontier
        .iter()
        .any(|entry| entry.place == argument.place && entry.path.starts_with(&argument.path));
    if containing {
        consume_affine_projection(
            structural_types,
            values,
            structural_cases,
            frontier,
            argument,
        )?;
    } else if contained {
        frontier.retain(|entry| {
            entry.place != argument.place || !entry.path.starts_with(&argument.path)
        });
    }
    Ok(containing || contained)
}

pub(crate) fn consume_affine_discard(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    values: &RuntimeRows<PlaceId, TerminalStructuralValue>,
    structural_cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::boundary::structural_inputs::case_membership::StructuralCaseContents,
    >,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    claims: &BTreeMap<ClaimId, LiveClaim>,
    discard: &StructuralAffineDiscard,
) -> Result<(), TerminalInterpretError> {
    let root = verified_structural_place(values, discard.place)?;
    if resolve_structural_path_type(structural_types, root.structural_type, &discard.path)?
        != discard.structural_type
        || claims.values().any(|claim| {
            claim.place == Some(discard.place)
                && terminal_semantics::structural_paths_may_overlap(&claim.path, &discard.path)
        })
    {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    }
    consume_affine_projection(
        structural_types,
        values,
        structural_cases,
        frontier,
        &StructuralArgument {
            place: discard.place,
            path: discard.path.clone(),
            access: StructuralAccess::Owned,
        },
    )
}

pub(crate) fn consume_affine_projection(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    values: &RuntimeRows<PlaceId, TerminalStructuralValue>,
    structural_cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::boundary::structural_inputs::case_membership::StructuralCaseContents,
    >,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    argument: &StructuralArgument,
) -> Result<(), TerminalInterpretError> {
    let root = crate::engine::custody::verified_structural_place(values, argument.place)?;
    crate::boundary::structural_inputs::case_membership::validate_case_projection(
        structural_types,
        values,
        structural_cases,
        argument.place,
        &argument.path,
    )?;
    let Some(containing) = frontier
        .iter()
        .find(|entry| {
            entry.place == argument.place && argument.path.starts_with(entry.path.as_slice())
        })
        .cloned()
    else {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    };
    if containing.path.is_empty() && containing.structural_type != root.structural_type {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    }
    frontier.remove(&containing);
    let case_membership = |current: &StructuralAffineDiscard| {
        crate::boundary::structural_inputs::case_membership::stored_case_at_projection(
            structural_types,
            root,
            structural_cases,
            &current.path,
        )
    };
    split_affine_frontier_at_projection(
        structural_types,
        frontier,
        containing,
        &argument.path,
        &case_membership,
    )
}

fn cleanup_field_type(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    field: &terminal_psi::StructuralFieldDeclaration,
) -> Result<Option<StructuralTypeId>, TerminalInterpretError> {
    if let Some(structural) = field.field_type.as_structural() {
        return Ok(Some(structural));
    }
    match field.field_type.byte_sequence_carrier() {
        Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }) | None => Ok(None),
        Some(_) => terminal_semantics::inline_carrier_declaration(structural_types.values(), field)
            .map(|declaration| Some(declaration.id))
            .ok_or_else(
                || TerminalInterpretError::AffineProjectionNotRepresentable {
                    context: format!(
                        "inline view field has no descriptor declaration: {:?}",
                        field.field_type
                    ),
                },
            ),
    }
}

pub(crate) fn split_affine_frontier_at_projection(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    current: StructuralAffineDiscard,
    projected_path: &[StructuralPathSegment],
    case_membership: &impl Fn(&StructuralAffineDiscard) -> Option<semantic_vocabulary::StructuralCaseId>,
) -> Result<(), TerminalInterpretError> {
    if current.path == projected_path {
        return Ok(());
    }
    let Some(next_segment) = projected_path.get(current.path.len()) else {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    };
    let declaration = structural_types.declaration(&current.structural_type)?;
    let selected = match &declaration.shape {
        StructuralTypeShape::Record { fields, .. } => {
            let mut selected = None;
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                let Some(field_type) = cleanup_field_type(structural_types, field)? else {
                    continue;
                };
                let segment = StructuralPathSegment::Field(field.identity);
                let mut path = current.path.clone();
                path.push(segment.clone());
                let child = StructuralAffineDiscard {
                    place: current.place,
                    path,
                    structural_type: field_type,
                };
                if &segment == next_segment {
                    selected = Some(child);
                } else if !frontier.insert(child) {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
            }
            selected
        }
        StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        } if matches!(next_segment, StructuralPathSegment::FixedIndex(index) if index < length) => {
            let StructuralPathSegment::FixedIndex(selected_index) = next_segment else {
                unreachable!()
            };
            let mut selected = None;
            for index in 0..*length {
                let mut path = current.path.clone();
                path.push(StructuralPathSegment::FixedIndex(index));
                let child = StructuralAffineDiscard {
                    place: current.place,
                    path,
                    structural_type: *element,
                };
                if index == *selected_index {
                    selected = Some(child);
                } else if !frontier.insert(child) {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
            }
            selected
        }
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
            let active_case = case_membership(&current).ok_or_else(malformed)?;
            let selected_case = find_structural_case(cases, active_case).ok_or_else(malformed)?;
            let mut selected = None;
            for (case, fields) in declaration
                .shape
                .direct_fields()
                .map(|fields| (None, fields))
                .into_iter()
                .chain(std::iter::once((
                    Some(selected_case.id),
                    selected_case.fields.as_slice(),
                )))
            {
                for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                    let Some(field_type) = cleanup_field_type(structural_types, field)? else {
                        continue;
                    };
                    let segment = StructuralPathSegment::Field(field.identity);
                    let mut path = current.path.clone();
                    if let Some(case) = case {
                        path.push(StructuralPathSegment::Case(case));
                    }
                    path.push(segment.clone());
                    let child = StructuralAffineDiscard {
                        place: current.place,
                        path,
                        structural_type: field_type,
                    };
                    if projected_path.starts_with(&child.path) {
                        selected = Some(child);
                    } else if !frontier.insert(child) {
                        return Err(TerminalInterpretError::AffineFrontierMismatch);
                    }
                }
            }
            selected
        }
        StructuralTypeShape::Opaque
        | StructuralTypeShape::PrimitiveScalar(_)
        | StructuralTypeShape::AtomicCell(_)
        | StructuralTypeShape::Reference { .. }
        | StructuralTypeShape::ByteSequence(_)
        | StructuralTypeShape::ElementView { .. }
        | StructuralTypeShape::FixedArray { .. } => None,
    }
    .ok_or_else(
        || TerminalInterpretError::AffineProjectionNotRepresentable {
            context: format!(
                "cannot split affine type {:?} at {:?} toward {:?}",
                current.structural_type, current.path, projected_path
            ),
        },
    )?;
    split_affine_frontier_at_projection(
        structural_types,
        frontier,
        selected,
        projected_path,
        case_membership,
    )
}

pub(crate) fn literal_fixed_array(
    types: &crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
) -> Result<(StructuralTypeId, u64), TerminalInterpretError> {
    let StructuralTypeShape::FixedArray {
        element,
        length: terminal_psi::ArrayExtent::Literal(length),
    } = types.declaration(&structural_type)?.shape
    else {
        return Err(malformed());
    };
    Ok((element, length))
}

pub(crate) fn fixed_index_step(
    types: &crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    segment: &StructuralPathSegment,
) -> Result<(StructuralTypeId, u64, u64), TerminalInterpretError> {
    let StructuralPathSegment::FixedIndex(index) = segment else {
        return Err(malformed());
    };
    let (element, length) = literal_fixed_array(types, structural_type)?;
    if *index >= length {
        return Err(malformed());
    }
    Ok((element, *index, length))
}

pub(crate) fn resolve_structural_path_type(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Result<StructuralTypeId, TerminalInterpretError> {
    resolve_structural_path_type_with_leaf(
        structural_types,
        root,
        path,
        None,
        terminal_psi::StructuralFieldType::canonical_leaf_type,
    )
}

pub(crate) fn resolve_structural_path_shape<'types>(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        'types,
        StructuralTypeDeclaration,
    >,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Result<std::borrow::Cow<'types, StructuralTypeShape>, TerminalInterpretError> {
    if let Some(carrier) = inline_structural_field(structural_types, root, path)
        .and_then(|field| field.field_type.byte_sequence_carrier())
    {
        return Ok(std::borrow::Cow::Owned(StructuralTypeShape::ByteSequence(
            carrier,
        )));
    }
    terminal_semantics::runtime_structural_path_tip(structural_types.values(), root, path)
        .map(|declaration| std::borrow::Cow::Borrowed(&declaration.shape))
        .ok_or_else(
            || TerminalInterpretError::AffineProjectionNotRepresentable {
                context: format!("no structural shape for type {:?} at {:?}", root, path),
            },
        )
}

pub(crate) fn resolve_structural_path_type_with_leaf(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    expected: Option<StructuralTypeId>,
    leaf_type: impl Fn(&terminal_psi::StructuralFieldType) -> Option<terminal_psi::StructuralFieldType>
    + Copy,
) -> Result<StructuralTypeId, TerminalInterpretError> {
    if let Some(field) = inline_structural_field(structural_types, root, path)
        && !matches!(
            field.field_type,
            terminal_psi::StructuralFieldType::Structural(_)
        )
    {
        return terminal_semantics::inline_field_presentation(
            structural_types.values(),
            field,
            expected,
            leaf_type(&field.field_type),
        )
        .ok_or_else(
            || TerminalInterpretError::AffineProjectionNotRepresentable {
                context: format!(
                    "inline field {:?} has no presentation {:?}",
                    field.field_type, expected
                ),
            },
        );
    }
    terminal_semantics::runtime_structural_path_tip(structural_types.values(), root, path)
        .map(|declaration| declaration.id)
        .ok_or_else(
            || TerminalInterpretError::AffineProjectionNotRepresentable {
                context: format!(
                    "no structural endpoint for type {:?} at {:?}, expected {:?}",
                    root, path, expected
                ),
            },
        )
}

pub(crate) fn inline_structural_field<'a>(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        'a,
        StructuralTypeDeclaration,
    >,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<&'a terminal_psi::StructuralFieldDeclaration> {
    terminal_semantics::runtime_structural_field(structural_types.values(), root, path)
}

pub(crate) fn bind_entry_claims(
    entry_claims: &[EntryClaim],
    content_entry_claims: &[terminal_psi::ContentEntryClaim],
    parameters: &[StructuralParameterDeclaration],
    values: &RuntimeRows<PlaceId, TerminalStructuralValue>,
) -> Result<BTreeMap<ClaimId, LiveClaim>, TerminalInterpretError> {
    let mut claims = BTreeMap::new();
    for entry_claim in entry_claims {
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.place == entry_claim.input)
            .ok_or(malformed())?;
        if parameter.multiplicity == StructuralMultiplicity::Unrestricted
            || !values.contains_key(&parameter.place)
            || claims
                .insert(
                    entry_claim.claim,
                    LiveClaim::at(
                        parameter.place,
                        entry_claim.path.clone(),
                        parameter.multiplicity,
                    ),
                )
                .is_some()
        {
            return Err(malformed());
        }
    }
    for entry_claim in content_entry_claims {
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.place == entry_claim.input.root);
        claims.entry(entry_claim.claim).or_insert(LiveClaim {
            place: parameter.map(|_| entry_claim.input.root),
            path: Vec::new(),
            multiplicity: parameter.map(|parameter| parameter.multiplicity),
        });
    }
    Ok(claims)
}

pub(crate) fn resolve_structural_arguments(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    values: &RuntimeRows<PlaceId, TerminalStructuralValue>,
    arguments: &[StructuralArgument],
) -> Result<Vec<TerminalStructuralValue>, TerminalInterpretError> {
    arguments
        .iter()
        .map(|argument| {
            let value = crate::engine::custody::verified_structural_place(values, argument.place)?;
            project_structural_value(structural_types, value, &argument.path)
        })
        .collect()
}

pub(crate) fn verified_structural_place<'v, V: 'v>(
    values: &'v impl std::borrow::Borrow<BTreeMap<PlaceId, V>>,
    place: PlaceId,
) -> Result<&'v V, TerminalInterpretError> {
    values
        .borrow()
        .get(&place)
        .ok_or(TerminalInterpretError::VerifiedStructuralPlaceMissing(
            place,
        ))
}

pub(crate) fn resolve_single_structural_argument(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    values: &RuntimeRows<PlaceId, TerminalStructuralValue>,
    argument: &StructuralArgument,
) -> Result<TerminalStructuralValue, TerminalInterpretError> {
    resolve_structural_arguments(structural_types, values, std::slice::from_ref(argument))?
        .pop()
        .ok_or_else(malformed)
}

pub(crate) fn project_structural_value(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    root: &TerminalStructuralValue,
    path: &[StructuralPathSegment],
) -> Result<TerminalStructuralValue, TerminalInterpretError> {
    project_structural_value_with_leaf(
        structural_types,
        root,
        path,
        None,
        terminal_psi::StructuralFieldType::canonical_leaf_type,
    )
}

pub(crate) fn project_shared_structural_observation(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    root: &TerminalStructuralValue,
    path: &[StructuralPathSegment],
) -> Result<TerminalStructuralValue, TerminalInterpretError> {
    project_structural_value_with_leaf(
        structural_types,
        root,
        path,
        None,
        terminal_psi::StructuralFieldType::shared_observation_type,
    )
}

pub(crate) fn project_structural_value_with_leaf(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    root: &TerminalStructuralValue,
    path: &[StructuralPathSegment],
    expected: Option<StructuralTypeId>,
    leaf_type: impl Fn(&terminal_psi::StructuralFieldType) -> Option<terminal_psi::StructuralFieldType>
    + Copy,
) -> Result<TerminalStructuralValue, TerminalInterpretError> {
    let mut value = root.clone();
    value.structural_type = resolve_structural_path_type_with_leaf(
        structural_types,
        root.structural_type,
        path,
        expected,
        leaf_type,
    )?;
    (value.qualifications, value.projected_qualifications) =
        terminal_psi::StructuralPathQualification::select(
            &root.qualifications,
            &root.projected_qualifications,
            path,
        );
    value
        .path
        .extend_from_slice(crate::boundary::values::structural_storage_path(path));
    Ok(value)
}

pub(crate) fn direct_scalar_field_type(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
) -> Option<ScalarType> {
    let declaration = structural_types.get(&structural_type)?;
    let fields = declaration.shape.direct_fields()?;
    fields.iter().find_map(|candidate| {
        (candidate.id == field && !candidate.relevance.is_erased())
            .then_some(&candidate.field_type)
            .and_then(terminal_psi::StructuralFieldType::scalar_type)
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn transfer_claims(
    structural_types: &crate::embedding::program::ProgramDeclarations<
        '_,
        StructuralTypeDeclaration,
    >,
    structural_cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::boundary::structural_inputs::case_membership::StructuralCaseContents,
    >,
    caller_claims: &BTreeMap<ClaimId, LiveClaim>,
    caller_arguments: &[StructuralArgument],
    transfers: &[ClaimTransfer],
    callee_parameters: &[StructuralParameterDeclaration],
    callee_entry_claims: &[EntryClaim],
    callee_content_entry_claims: &[terminal_psi::ContentEntryClaim],
    callee_values: &RuntimeRows<PlaceId, TerminalStructuralValue>,
) -> Result<(BTreeMap<ClaimId, LiveClaim>, BTreeMap<ClaimId, LiveClaim>), TerminalInterpretError> {
    if caller_arguments.len() != callee_parameters.len()
        || transfers
            .iter()
            .map(|transfer| transfer.claim)
            .collect::<BTreeSet<_>>()
            .len()
            != transfers.len()
    {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }
    let mut expected_by_argument = BTreeMap::<u32, Vec<&EntryClaim>>::new();
    for entry_claim in callee_entry_claims {
        let (index, parameter) = callee_parameters
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.place == entry_claim.input)
            .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
        if parameter.multiplicity == StructuralMultiplicity::Unrestricted
            || !callee_values.contains_key(&parameter.place)
        {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        if !crate::boundary::structural_inputs::case_membership::case_projection_is_active(
            structural_types,
            callee_values,
            structural_cases,
            entry_claim.input,
            &entry_claim.path,
        )? {
            continue;
        }
        expected_by_argument
            .entry(index as u32)
            .or_default()
            .push(entry_claim);
    }
    let mut actual_by_argument = BTreeMap::<u32, Vec<&ClaimTransfer>>::new();
    for transfer in transfers {
        let parameter = callee_parameters
            .get(transfer.argument_index as usize)
            .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
        if !callee_entry_claims
            .iter()
            .any(|entry| entry.input == parameter.place && entry.path == transfer.path)
        {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        let active =
            crate::boundary::structural_inputs::case_membership::case_projection_is_active(
                structural_types,
                callee_values,
                structural_cases,
                parameter.place,
                &transfer.path,
            )?;
        if !active {
            if caller_claims.contains_key(&transfer.claim) {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            continue;
        }
        if !caller_claims.contains_key(&transfer.claim) {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        actual_by_argument
            .entry(transfer.argument_index)
            .or_default()
            .push(transfer);
    }
    if expected_by_argument.keys().collect::<Vec<_>>()
        != actual_by_argument.keys().collect::<Vec<_>>()
    {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }

    let mut remaining = caller_claims.clone();
    let mut callee_claims = BTreeMap::new();
    for (argument_index, expected) in expected_by_argument {
        let actual = &actual_by_argument[&argument_index];
        if actual.len() != expected.len() {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        let caller_place = caller_arguments[argument_index as usize].place;
        let argument_path = &caller_arguments[argument_index as usize].path;
        let mut unmatched = expected;
        for transfer in actual {
            let caller_claim = remaining
                .remove(&transfer.claim)
                .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
            if caller_claim.place != Some(caller_place) {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            let suffix = caller_claim
                .path
                .strip_prefix(argument_path.as_slice())
                .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
            if suffix != transfer.path {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            let position = unmatched
                .iter()
                .position(|entry_claim| entry_claim.path == suffix)
                .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
            let entry_claim = unmatched.swap_remove(position);
            let parameter = callee_parameters
                .get(argument_index as usize)
                .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
            if callee_claims
                .insert(
                    entry_claim.claim,
                    LiveClaim::at(
                        parameter.place,
                        entry_claim.path.clone(),
                        parameter.multiplicity,
                    ),
                )
                .is_some()
            {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
        }
    }
    for argument in caller_arguments
        .iter()
        .filter(|argument| argument.access == StructuralAccess::Owned)
    {
        if remaining.values().any(|claim| {
            claim.place == Some(argument.place)
                && (claim.path.starts_with(&argument.path)
                    || argument.path.starts_with(&claim.path))
        }) {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
    }
    for entry_claim in callee_content_entry_claims {
        callee_claims.entry(entry_claim.claim).or_insert(LiveClaim {
            place: None,
            path: Vec::new(),
            multiplicity: None,
        });
    }
    Ok((remaining, callee_claims))
}

pub(crate) fn rebind_structural_result_claims(
    caller_claims: &BTreeMap<ClaimId, LiveClaim>,
    callee_claims: &BTreeMap<ClaimId, LiveClaim>,
    source: PlaceId,
    result: &StructuralOperationResult,
    returned_claims: &[StructuralResultClaimBinding],
) -> Result<BTreeMap<ClaimId, LiveClaim>, TerminalInterpretError> {
    let mut unmatched = returned_claims
        .iter()
        .map(|binding| binding.claim)
        .collect::<BTreeSet<_>>();
    if unmatched.len() != returned_claims.len() || returned_claims.len() != result.claims.len() {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }
    let mut rebound = caller_claims.clone();
    for binding in &result.claims {
        let expected_multiplicity =
            LiveClaim::path_multiplicity(&binding.path, result.multiplicity);
        let mut held = returned_claims.iter().filter(|returned| {
            returned.path == binding.path
                && callee_claims
                    .get(&returned.claim)
                    .is_some_and(|live| live.place == Some(source) && live.path == returned.path)
        });
        let Some(callee_claim) = held.next() else {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        };
        if held.next().is_some()
            || !unmatched.remove(&callee_claim.claim)
            || callee_claims
                .get(&callee_claim.claim)
                .is_none_or(|live| live.multiplicity != Some(expected_multiplicity))
            || rebound
                .insert(
                    binding.claim,
                    LiveClaim::at(result.place, binding.path.clone(), result.multiplicity),
                )
                .is_some()
        {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
    }
    if !unmatched.is_empty() {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }
    Ok(rebound)
}

pub(crate) fn validate_boundary_requirements<'value>(
    boundary: &BoundaryMachineDeclaration,
    argument: impl Fn(usize) -> Option<&'value TerminalStructuralValue>,
) -> Result<(), TerminalInterpretError> {
    for requirement in &boundary.requires {
        let argument = argument(requirement.argument_index as usize).ok_or(malformed())?;
        let supplied = if requirement.path.is_empty() {
            argument.qualifications.contains(&requirement.domain)
        } else {
            argument
                .projected_qualifications
                .iter()
                .any(|row| row.path == requirement.path && row.domain == requirement.domain)
        };
        if !supplied {
            return Err(TerminalInterpretError::BoundaryQualificationMissing {
                boundary: boundary.id,
                argument_index: requirement.argument_index,
                domain: requirement.domain,
            });
        }
    }
    Ok(())
}

pub(crate) fn complete_claims(
    caller_claims: &BTreeMap<ClaimId, LiveClaim>,
    caller_arguments: &[StructuralArgument],
    receipts: &[CompletionReceipt],
    _boundary_parameters: &[StructuralParameterDeclaration],
) -> Result<BTreeMap<ClaimId, LiveClaim>, TerminalInterpretError> {
    let expected = caller_arguments
        .iter()
        .enumerate()
        .flat_map(|(index, argument)| {
            caller_claims.iter().filter_map(move |(claim, live)| {
                if live.place != Some(argument.place) {
                    return None;
                }
                live.path
                    .strip_prefix(argument.path.as_slice())
                    .map(|path| (index as u32, *claim, path))
            })
        })
        .collect::<BTreeSet<_>>();
    let mut remaining = caller_claims.clone();
    let mut actual = BTreeSet::new();
    for receipt in receipts {
        let occurrence = (
            receipt.argument_index,
            receipt.claim,
            receipt.path.as_slice(),
        );
        if !actual.insert(occurrence) || !expected.contains(&occurrence) {
            return Err(TerminalInterpretError::CompletionReceiptMismatch);
        }
        let argument = caller_arguments
            .get(receipt.argument_index as usize)
            .ok_or(TerminalInterpretError::CompletionReceiptMismatch)?;
        let claim = remaining
            .remove(&receipt.claim)
            .ok_or(TerminalInterpretError::CompletionReceiptMismatch)?;
        if claim.place != Some(argument.place)
            || claim.path.strip_prefix(argument.path.as_slice()) != Some(receipt.path.as_slice())
        {
            return Err(TerminalInterpretError::CompletionReceiptMismatch);
        }
    }
    if actual != expected {
        return Err(TerminalInterpretError::CompletionReceiptMismatch);
    }
    Ok(remaining)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{bind_structural_arguments, transfer_claims};
    use crate::boundary::errors::TerminalInterpretError;
    use crate::boundary::values::TerminalStructuralValue;
    use crate::engine::execution::LiveClaim;
    use crate::storage::runtime_rows::RuntimeRows;
    use semantic_vocabulary::{ClaimId, PlaceId, StructuralTypeId};
    use std::collections::BTreeMap;
    use terminal_psi::{
        ClaimTransfer, EntryClaim, StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralPathSegment,
    };

    fn place(raw: u64) -> PlaceId {
        PlaceId::new(raw).unwrap()
    }

    fn claim(raw: u64) -> ClaimId {
        ClaimId::new(raw).unwrap()
    }

    fn structural_type(raw: u64) -> StructuralTypeId {
        StructuralTypeId::new(raw).unwrap()
    }

    fn field(name: &str) -> StructuralPathSegment {
        let index = match name {
            "left" => 1,
            "right" => 2,
            "middle" => 3,
            _ => 0,
        };
        StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(index))
    }

    fn parameter(
        raw_place: u64,
        position: u32,
        raw_type: u64,
        multiplicity: StructuralMultiplicity,
    ) -> StructuralParameterDeclaration {
        StructuralParameterDeclaration {
            place: place(raw_place),
            position,
            structural_type: structural_type(raw_type),
            multiplicity,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
        }
    }

    fn value(
        opaque_identity: u64,
        raw_type: u64,
        path: Vec<StructuralPathSegment>,
    ) -> TerminalStructuralValue {
        TerminalStructuralValue {
            opaque_identity,
            structural_type: structural_type(raw_type),
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            path,
        }
    }

    fn linear(raw_place: u64, path: Vec<StructuralPathSegment>) -> LiveClaim {
        LiveClaim {
            place: Some(place(raw_place)),
            path,
            multiplicity: Some(StructuralMultiplicity::Linear),
        }
    }

    fn pair_transfer(
        caller_claims: &BTreeMap<ClaimId, LiveClaim>,
    ) -> Result<(BTreeMap<ClaimId, LiveClaim>, BTreeMap<ClaimId, LiveClaim>), TerminalInterpretError>
    {
        let callee = parameter(2, 0, 1, StructuralMultiplicity::Affine);
        let callee_values =
            RuntimeRows::from_rows(None, [(callee.place, value(9, 1, Vec::new()))]).unwrap();
        transfer_claims(
            &crate::embedding::program::ProgramDeclarations::new(&[]),
            &BTreeMap::new(),
            caller_claims,
            &[StructuralArgument {
                place: place(5),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }],
            &[
                ClaimTransfer {
                    claim: claim(2),
                    argument_index: 0,
                    path: vec![field("right")],
                },
                ClaimTransfer {
                    claim: claim(3),
                    argument_index: 0,
                    path: vec![field("left")],
                },
            ],
            std::slice::from_ref(&callee),
            &[
                EntryClaim {
                    claim: claim(1),
                    input: callee.place,
                    path: vec![field("left")],
                },
                EntryClaim {
                    claim: claim(2),
                    input: callee.place,
                    path: vec![field("right")],
                },
            ],
            &[],
            &callee_values,
        )
    }

    #[test]
    fn claim_transfers_join_entry_claims_by_path_not_position() {
        let caller = BTreeMap::from([
            (claim(2), linear(5, vec![field("right")])),
            (claim(3), linear(5, vec![field("left")])),
        ]);
        let (remaining, callee) = pair_transfer(&caller).unwrap();
        assert!(remaining.is_empty());
        assert_eq!(callee[&claim(1)], linear(2, vec![field("left")]));
        assert_eq!(callee[&claim(2)], linear(2, vec![field("right")]));
    }

    #[test]
    fn claim_transfer_without_its_entry_path_refuses() {
        let caller = BTreeMap::from([
            (claim(2), linear(5, vec![field("right")])),
            (claim(3), linear(5, vec![field("middle")])),
        ]);
        assert_eq!(
            pair_transfer(&caller),
            Err(TerminalInterpretError::ClaimTransferMismatch)
        );
    }

    #[test]
    fn claim_transfers_cannot_match_one_entry_claim_twice() {
        let caller = BTreeMap::from([
            (claim(2), linear(5, vec![field("right")])),
            (claim(3), linear(5, vec![field("right")])),
        ]);
        assert_eq!(
            pair_transfer(&caller),
            Err(TerminalInterpretError::ClaimTransferMismatch)
        );
    }

    #[test]
    fn disjoint_projections_of_one_root_bind_as_separate_arguments() {
        let parameters = [
            parameter(1, 0, 3, StructuralMultiplicity::Affine),
            parameter(2, 1, 3, StructuralMultiplicity::Affine),
        ];
        let bound = bind_structural_arguments(
            &parameters,
            &[
                value(7, 3, vec![field("right")]),
                value(7, 3, vec![field("left")]),
            ],
            None,
        )
        .unwrap();
        assert_eq!(bound.len(), 2);
    }

    #[test]
    fn overlapping_custody_bearing_arguments_still_alias() {
        let parameters = [
            parameter(1, 0, 3, StructuralMultiplicity::Affine),
            parameter(2, 1, 3, StructuralMultiplicity::Affine),
        ];
        assert_eq!(
            bind_structural_arguments(
                &parameters,
                &[
                    value(7, 3, vec![field("left")]),
                    value(7, 3, vec![field("left")]),
                ],
                None,
            )
            .unwrap_err(),
            TerminalInterpretError::StructuralArgumentAliasing(7)
        );
    }

    fn mutable_lender_pair(
        root_shape: terminal_psi::StructuralTypeShape,
        child_path: Vec<StructuralPathSegment>,
        admitted_parent_access: StructuralAccess,
    ) -> Result<RuntimeRows<PlaceId, TerminalStructuralValue>, TerminalInterpretError> {
        let declarations = [
            terminal_psi::StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                id: structural_type(1),
                identity: terminal_psi::TerminalIdentityIndex(1),
                owns_linear_claim: false,
                copyable: true,
                no_code_affine_cleanup: true,
                descriptor_cleanup: false,
                shape: root_shape,
                scalar_restriction: None,
            },
            terminal_psi::StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                id: structural_type(2),
                identity: terminal_psi::TerminalIdentityIndex(2),
                owns_linear_claim: false,
                copyable: true,
                no_code_affine_cleanup: true,
                descriptor_cleanup: false,
                shape: terminal_psi::StructuralTypeShape::PrimitiveScalar(
                    semantic_vocabulary::ScalarType::Boolean,
                ),
                scalar_restriction: None,
            },
        ];
        let mut parameters = [
            parameter(1, 0, 1, StructuralMultiplicity::Unrestricted),
            parameter(2, 1, 2, StructuralMultiplicity::Unrestricted),
        ];
        for parameter in &mut parameters {
            parameter.access = StructuralAccess::MutableBorrow;
        }
        let values = [value(7, 1, Vec::new()), value(7, 2, child_path)];
        let sources = [50, 51].map(|raw| StructuralArgument {
            place: place(raw),
            path: Vec::new(),
            access: StructuralAccess::MutableBorrow,
        });
        let mut source_parameters = parameters.clone();
        source_parameters[0].place = place(50);
        source_parameters[0].access = admitted_parent_access;
        source_parameters[1].place = place(51);
        let provenance =
            source_parameters
                .each_ref()
                .map(|parameter| super::StructuralArgumentProvenance {
                    parameter: Some(parameter),
                    reference_projection: None,
                });
        assert_eq!(
            bind_structural_arguments(&parameters, &values, None).unwrap_err(),
            TerminalInterpretError::StructuralArgumentAliasing(7),
        );
        let types = crate::embedding::program::ProgramDeclarations::new(&declarations);
        super::bind_active_retained_arguments(
            &parameters,
            &sources,
            &values,
            &provenance,
            &|argument_index, rows| {
                crate::boundary::structural_inputs::case_membership::active_projected_qualifications(
                    &types,
                    &values[argument_index],
                    &BTreeMap::new(),
                    rows,
                )
            },
            &types,
            None,
        )
    }

    fn boolean_array() -> terminal_psi::StructuralTypeShape {
        terminal_psi::StructuralTypeShape::FixedArray {
            element: structural_type(2),
            length: terminal_psi::ArrayExtent::Literal(2),
        }
    }

    #[test]
    fn internal_call_retains_admitted_mutable_parent_and_inline_child() {
        let values = mutable_lender_pair(
            boolean_array(),
            vec![StructuralPathSegment::FixedIndex(0)],
            StructuralAccess::MutableBorrow,
        )
        .unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(
            values.get(&place(2)).unwrap().path,
            vec![StructuralPathSegment::FixedIndex(0)]
        );
    }

    #[test]
    fn retained_mutable_parent_does_not_admit_duplicate_extent_or_shared_source() {
        for (path, access) in [
            (Vec::new(), StructuralAccess::MutableBorrow),
            (
                vec![StructuralPathSegment::FixedIndex(0)],
                StructuralAccess::SharedBorrow,
            ),
        ] {
            assert_eq!(
                mutable_lender_pair(boolean_array(), path, access).unwrap_err(),
                TerminalInterpretError::StructuralArgumentAliasing(7),
            );
        }
    }

    #[test]
    fn stored_pointer_cannot_supply_inline_child_lineage() {
        assert_eq!(
            mutable_lender_pair(
                terminal_psi::StructuralTypeShape::Reference {
                    referent: structural_type(2),
                    access: StructuralAccess::MutableBorrow,
                },
                vec![StructuralPathSegment::Referent],
                StructuralAccess::MutableBorrow,
            )
            .unwrap_err(),
            TerminalInterpretError::StructuralArgumentAliasing(7),
        );
    }
}
