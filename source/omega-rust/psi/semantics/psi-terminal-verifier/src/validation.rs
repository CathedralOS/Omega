use std::collections::{BTreeMap, BTreeSet};

use psi_core::{
    BlockId, BoundaryMachineId, CanonicalStructuralPathSegment, ClaimId, ContentAlgebra,
    ContentConservation, ContentDomainId, ContentProjectionExpression, ContentProjectionIdentity,
    ContentProjectionScalar, ContentStructuralPlace, ContentTerm, ContractId, EdgeId,
    EvidenceTermId, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId, OperationId,
    PlaceId, Proposition, PropositionContext, PropositionId, RecursiveComponentId, ScalarTerm,
    ScalarType, ServiceId, StructuralDomainId, StructuralFieldId, StructuralPlaceKind,
    StructuralTypeId, ValueId, content_conservation_report_fingerprint,
};
use psi_terminal::{
    BoundaryMachineDeclaration, ClaimTransfer, CompletionReceipt, ContentPartitionComposition,
    CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, EntryClaim, EvidenceContractLaneKind,
    OperationKind, OperationResult, PropositionBinderArgumentKind, PropositionBinderKind,
    PropositionEvidence, StructuralAccess, StructuralArgument, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator,
    program_local_root_introduction_compatibility_report_identity,
};

use crate::verification::{
    substitute_proposition_structural_places, substitute_proposition_values,
};

mod affine_cleanup;
mod call_graph;
mod conformance_applications;
mod content;
mod contracts;
mod control_flow;
mod crash;
mod dynamic_dispatch;
mod error;
mod evidence;
mod float_meaning;
mod foundation;
mod frontier;
mod machine;
mod operations;
mod proof_recursion;
mod propositions;
mod quotient_correspondence;
mod ranked_scc;
mod root_service_reach;
mod structural_operations;
mod structural_qualification_rosters;
mod structural_result_contracts;
mod structural_scalar_fields;

use call_graph::validate_call_graph;
use conformance_applications::validate_closed_conformance_applications;
pub use error::{ContractClauseKind, ModuleError};
use evidence::{validate_evidence_contract_lanes, validate_proposition_vocabulary};
pub(crate) use foundation::structural_leaf_type;
pub use foundation::{ServiceCeilingOwner, StructuralSignatureOwner};
use foundation::{
    is_bounded_partial_affine_path, is_nonempty_field_path, partial_affine_residuals,
    resolve_structural_path, validate_structural_foundation,
};
pub use frontier::{
    VerifiedLiveClaim, VerifiedMachineStructuralFrontiers, VerifiedOwnedStructuralPlace,
    VerifiedPartialStructuralCustody, VerifiedStructuralOwnershipFrontier,
    VerifiedTerminalStructuralFrontiers,
};
use proof_recursion::validate_proof_recursive_components;
pub(crate) use structural_operations::{
    exact_payloadless_case_return_exits, structural_argument_canonical_prefix,
};

#[derive(Debug, Clone, Copy)]
pub struct ValidatedTerminalModule<'module> {
    module: &'module TerminalModule,
}

impl<'module> ValidatedTerminalModule<'module> {
    pub const fn module(self) -> &'module TerminalModule {
        self.module
    }

    pub fn machine(self, id: MachineId) -> Option<&'module TerminalMachine> {
        self.module.machines.iter().find(|machine| machine.id == id)
    }

    pub fn value_context(
        self,
        machine: &TerminalMachine,
    ) -> Result<PropositionContext, ModuleError> {
        PropositionContext::from_value_types_and_places(
            machine_value_types(machine),
            machine
                .structural_places
                .iter()
                .map(|place| (place.id, place.kind))
                .chain(
                    affine_cleanup::nominal_cleanup_contract_receiver(self.module, machine.id).map(
                        |receiver| {
                            (
                                receiver,
                                StructuralPlaceKind::Parameter {
                                    position: 0,
                                    is_self: true,
                                },
                            )
                        },
                    ),
                ),
        )
        .map_err(ModuleError::MalformedProposition)
    }
}

pub fn validate_module(
    module: &TerminalModule,
) -> Result<ValidatedTerminalModule<'_>, ModuleError> {
    validate_module_with_policy(module, ValidationPolicy::Execution)?;
    Ok(ValidatedTerminalModule { module })
}

/// Validate the exact Terminal-Psi subset admitted by the reference
/// interpreter.
///
/// This is deliberately a different carrier from [`ValidatedTerminalModule`]:
/// ranked countdowns are not thereby authorized for fixed-fuel derivation,
/// Omega lowering, or native installation.
#[derive(Debug, Clone, Copy)]
pub struct ValidatedInterpretableTerminalModule<'module> {
    validated: ValidatedTerminalModule<'module>,
}

impl<'module> ValidatedInterpretableTerminalModule<'module> {
    pub const fn module(self) -> &'module TerminalModule {
        self.validated.module()
    }

    pub fn value_context(
        self,
        machine: &TerminalMachine,
    ) -> Result<PropositionContext, ModuleError> {
        self.validated.value_context(machine)
    }

    pub(crate) const fn validated(self) -> ValidatedTerminalModule<'module> {
        self.validated
    }
}

pub fn validate_module_for_interpretation(
    module: &TerminalModule,
) -> Result<ValidatedInterpretableTerminalModule<'_>, ModuleError> {
    validate_module_with_policy(module, ValidationPolicy::Interpretation)?;
    Ok(ValidatedInterpretableTerminalModule {
        validated: ValidatedTerminalModule { module },
    })
}

/// Validate the exact Terminal-Psi subset admitted for target-neutral
/// optimizer analysis.
///
/// This carrier is deliberately distinct from ordinary execution,
/// interpretation, fixed-fuel, and native-ranked validation. It currently
/// adds only the already-validated exact unsigned-countdown representation to
/// the ordinary acyclic subset.
#[derive(Debug, Clone, Copy)]
pub struct ValidatedOptimizableTerminalModule<'module> {
    validated: ValidatedTerminalModule<'module>,
}

impl<'module> ValidatedOptimizableTerminalModule<'module> {
    pub const fn module(self) -> &'module TerminalModule {
        self.validated.module()
    }

    pub(crate) const fn validated(self) -> ValidatedTerminalModule<'module> {
        self.validated
    }
}

pub fn validate_module_for_optimization(
    module: &TerminalModule,
) -> Result<ValidatedOptimizableTerminalModule<'_>, ModuleError> {
    validate_module_with_policy(module, ValidationPolicy::Optimization)?;
    Ok(ValidatedOptimizableTerminalModule {
        validated: ValidatedTerminalModule { module },
    })
}

pub(crate) fn validate_module_for_native_ranked_countdown(
    module: &TerminalModule,
) -> Result<ValidatedTerminalModule<'_>, ModuleError> {
    validate_module_with_policy(module, ValidationPolicy::NativeRankedCountdown)?;
    Ok(ValidatedTerminalModule { module })
}

/// Validate a Terminal-Psi module and expose the exact deterministic ownership
/// frontier snapshots computed by the verifier's own custody walk.
pub fn reconstruct_structural_ownership_frontiers(
    module: &TerminalModule,
) -> Result<VerifiedTerminalStructuralFrontiers, ModuleError> {
    validate_module(module)?;
    reconstruct_validated_structural_ownership_frontiers(module)
}

pub(crate) fn reconstruct_validated_structural_ownership_frontiers(
    module: &TerminalModule,
) -> Result<VerifiedTerminalStructuralFrontiers, ModuleError> {
    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    let snapshots = module
        .machines
        .iter()
        .map(|machine| {
            let blocks = machine
                .blocks
                .iter()
                .map(|block| (block.id, block))
                .collect::<BTreeMap<_, _>>();
            let ranked_backedges = machine
                .ranked_scc
                .iter()
                .flat_map(|component| component.covered_cyclic_edges.iter().map(|row| row.edge))
                .collect::<BTreeSet<_>>();
            frontier::validate_structural_frontier(
                module,
                machine,
                &machines,
                &blocks,
                &ranked_backedges,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(VerifiedTerminalStructuralFrontiers {
        machines: snapshots,
    })
}

fn validate_placed_view_inputs(
    module: &TerminalModule,
    machines: &BTreeMap<psi_core::MachineId, &TerminalMachine>,
) -> Result<(), ModuleError> {
    let inputs = &module.placed_view_inputs;
    let mut coordinates = BTreeSet::new();
    for input in inputs {
        if !coordinates.insert((
            input.machine,
            input.source_state_identity.as_str(),
            input.position,
        )) {
            return Err(ModuleError::DuplicatePlacedViewInput {
                machine: input.machine,
                source_state_identity: input.source_state_identity.clone(),
                position: input.position,
            });
        }
        let invalid = !machines.contains_key(&input.machine)
            || matches!(input.access, psi_terminal::StructuralAccess::Owned)
            || !is_canonical_hermetic_identity(&input.source_machine_identity)
            || !is_canonical_hermetic_identity(&input.source_state_identity)
            || !is_canonical_hermetic_identity(&input.source_parameter_identity)
            || !is_canonical_hermetic_identity(&input.policy_identity)
            || !is_canonical_hermetic_identity(&input.policy_plan_machine_identity)
            || !is_canonical_hermetic_identity(&input.schema_identity)
            || input.view_identity
                != psi_terminal::canonical_placed_view_identity(
                    &input.policy_identity,
                    &input.schema_identity,
                )
            || input.placement_report_fingerprint == 0
            || input.placement_commitment == [0; 32];
        if invalid {
            return Err(ModuleError::InvalidPlacedViewInput {
                machine: input.machine,
                position: input.position,
            });
        }
    }
    if !inputs.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(ModuleError::NonCanonicalPlacedViewInputOrder);
    }
    Ok(())
}

fn valid_borrow_boundary(source: &psi_terminal::TerminalBorrowBoundarySource) -> bool {
    match source {
        psi_terminal::TerminalBorrowBoundarySource::Statement { .. } => true,
        psi_terminal::TerminalBorrowBoundarySource::Call {
            target_identity, ..
        } => is_canonical_borrow_identity(target_identity),
    }
}

fn valid_owner_path(path: &[psi_terminal::TerminalBorrowOwnerSegment]) -> bool {
    path.iter().all(|segment| match segment {
        psi_terminal::TerminalBorrowOwnerSegment::Field(identity)
        | psi_terminal::TerminalBorrowOwnerSegment::Case(identity) => {
            is_canonical_borrow_identity(identity)
        }
        psi_terminal::TerminalBorrowOwnerSegment::FixedIndex(_)
        | psi_terminal::TerminalBorrowOwnerSegment::DynamicIndex => true,
    })
}

fn valid_place_segment(segment: &psi_terminal::TerminalBorrowPlaceSegment) -> bool {
    match segment {
        psi_terminal::TerminalBorrowPlaceSegment::Field(identity)
        | psi_terminal::TerminalBorrowPlaceSegment::Case(identity) => {
            is_canonical_borrow_identity(identity)
        }
        psi_terminal::TerminalBorrowPlaceSegment::FixedIndex(_) => true,
        psi_terminal::TerminalBorrowPlaceSegment::FixedRange { start, end } => start <= end,
    }
}

fn validate_reborrow_root_handoffs(
    module: &TerminalModule,
    machines: &BTreeMap<psi_core::MachineId, &TerminalMachine>,
) -> Result<(), ModuleError> {
    let rows = &module.reborrow_root_handoffs;
    let mut coordinates = BTreeSet::new();
    for row in rows {
        let Some(leaf) = row.lineage.last() else {
            return Err(ModuleError::InvalidReborrowRootHandoff {
                machine: row.machine,
            });
        };
        if !coordinates.insert((
            row.machine,
            row.source_state_identity.as_str(),
            leaf.child_owner_identity.as_str(),
            leaf.child_activation.clone(),
        )) {
            return Err(ModuleError::DuplicateReborrowRootHandoff);
        }
        let mut invalid = !machines.contains_key(&row.machine)
            || !is_canonical_borrow_identity(&row.source_machine_identity)
            || !is_canonical_borrow_identity(&row.source_state_identity)
            || !is_canonical_borrow_identity(&row.direct_root_owner_identity)
            || !is_canonical_borrow_identity(&row.direct_root_place.root_identity)
            || !is_canonical_borrow_identity(&row.direct_root_lifetime_identity)
            || !valid_owner_path(&row.direct_root_owner_path)
            || !row
                .direct_root_place
                .segments
                .iter()
                .all(valid_place_segment)
            || row.direct_root_access != psi_terminal::StructuralAccess::MutableBorrow
            || row.direct_root_lifetime_identity != row.direct_root_place.root_identity
            || !valid_borrow_boundary(&row.direct_root_activation)
            || !valid_borrow_boundary(&row.direct_root_weakening);
        let mut parent_place = &row.direct_root_place;
        let mut parent_access = row.direct_root_access;
        for step in &row.lineage {
            let parent_segments = &parent_place.segments;
            let child_segments = &step.child_place.segments;
            let permitted_exclusive = matches!(
                (parent_access, step.child_access),
                (
                    psi_terminal::StructuralAccess::MutableBorrow,
                    psi_terminal::StructuralAccess::MutableBorrow
                        | psi_terminal::StructuralAccess::WriteOnlyBorrow
                ) | (
                    psi_terminal::StructuralAccess::WriteOnlyBorrow,
                    psi_terminal::StructuralAccess::WriteOnlyBorrow
                )
            );
            invalid |= !is_canonical_borrow_identity(&step.child_owner_identity)
                || !is_canonical_borrow_identity(&step.child_place.root_identity)
                || !valid_owner_path(&step.child_owner_path)
                || !child_segments.iter().all(valid_place_segment)
                || !step.projection_remainder.iter().all(valid_place_segment)
                || !permitted_exclusive
                || parent_place.root_identity != step.child_place.root_identity
                || !child_segments.starts_with(parent_segments)
                || child_segments[parent_segments.len()..] != step.projection_remainder
                || step.formation_boundary != step.child_activation
                || !valid_borrow_boundary(&step.child_activation)
                || !valid_borrow_boundary(&step.formation_boundary)
                || !valid_borrow_boundary(&step.child_weakening);
            parent_place = &step.child_place;
            parent_access = step.child_access;
        }
        if invalid {
            return Err(ModuleError::InvalidReborrowRootHandoff {
                machine: row.machine,
            });
        }
    }
    if !rows.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(ModuleError::NonCanonicalReborrowRootHandoffOrder);
    }
    Ok(())
}

fn validate_reborrow_restored_call_uses(
    module: &TerminalModule,
    machines: &BTreeMap<psi_core::MachineId, &TerminalMachine>,
) -> Result<(), ModuleError> {
    let rows = &module.reborrow_restored_call_uses;
    let mut coordinates = BTreeSet::new();
    let mut lifecycles = BTreeSet::new();
    let mut call_boundaries = BTreeSet::new();
    for row in rows {
        if !coordinates.insert((row.machine, row.operation)) {
            return Err(ModuleError::DuplicateReborrowRestoredCallUse);
        }
        if !lifecycles.insert((
            row.machine,
            &row.direct_root_activation,
            &row.child_activation,
            &row.child_weakening,
        )) {
            return Err(ModuleError::DuplicateReborrowRestoredCallLifecycle);
        }
        if let psi_terminal::TerminalBorrowBoundarySource::Call {
            statement_index,
            call_ordinal,
            ..
        } = &row.call_boundary
            && !call_boundaries.insert((row.machine, *statement_index, *call_ordinal))
        {
            return Err(ModuleError::DuplicateReborrowRestoredCallLifecycle);
        }
        let machine = machines.get(&row.machine).copied();
        let operations = machine
            .into_iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| operation.id == row.operation)
            .collect::<Vec<_>>();
        let exact_mutating_call = matches!(
            (machine, operations.as_slice()),
            (Some(caller), [operation])
                if exact_restored_mutating_call(
                    operation,
                    caller,
                    machines,
                    Some(row.call_target_machine),
                )
        );
        let unique_compatible_mutating_call = machine.is_some_and(|caller| {
            caller
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| exact_restored_mutating_call(operation, caller, machines, None))
                .count()
                == 1
        });
        let child_segments = &row.child_place.segments;
        let root_segments = &row.direct_root_place.segments;
        let exact_one_hop_lifecycle = matches!(
            (
                &row.direct_root_activation,
                &row.child_activation,
                &row.child_weakening,
                &row.direct_root_weakening,
            ),
            (
                psi_terminal::TerminalBorrowBoundarySource::Statement {
                    statement_index: parent_start,
                },
                psi_terminal::TerminalBorrowBoundarySource::Statement {
                    statement_index: child_start,
                },
                psi_terminal::TerminalBorrowBoundarySource::Statement {
                    statement_index: child_end,
                },
                psi_terminal::TerminalBorrowBoundarySource::Statement {
                    statement_index: parent_end,
                },
            ) if parent_start < child_start && child_start <= child_end && child_end < parent_end
        );
        let exact_call_boundary = matches!(
            (&row.call_boundary, &row.child_weakening),
            (
                psi_terminal::TerminalBorrowBoundarySource::Call {
                    statement_index: call_statement,
                    call_ordinal: 0,
                    target_identity,
                },
                psi_terminal::TerminalBorrowBoundarySource::Statement {
                    statement_index: child_end,
                },
            ) if call_statement == child_end && is_canonical_borrow_identity(target_identity)
        );
        let exact_restoration_class = match row.restoration_class {
            psi_terminal::TerminalReborrowRestorationClass::ExclusiveReactivation => {
                matches!(
                    row.child_access,
                    psi_terminal::StructuralAccess::MutableBorrow
                        | psi_terminal::StructuralAccess::WriteOnlyBorrow
                ) && row.shared_cohort.is_empty()
            }
            psi_terminal::TerminalReborrowRestorationClass::SharedFreezeRestoration => {
                if !matches!(row.shared_cohort.len(), 1 | 2 | 3) {
                    return Err(ModuleError::InvalidReborrowRestoredCallUse {
                        machine: row.machine,
                        operation: row.operation,
                    });
                }
                let cohort_is_unique = row
                    .shared_cohort
                    .iter()
                    .enumerate()
                    .all(|(index, member)| !row.shared_cohort[..index].contains(member));
                let cohort_owner_order =
                    row.shared_cohort.iter().enumerate().all(|(index, member)| {
                        !row.shared_cohort[..index].iter().any(|prior| {
                            (prior.child_owner_identity.as_str(), &prior.child_owner_path)
                                == (
                                    member.child_owner_identity.as_str(),
                                    &member.child_owner_path,
                                )
                        }) && (index == 0
                            || matches!(
                                (
                                    &row.shared_cohort[index - 1].child_activation,
                                    &member.child_activation,
                                ),
                                (
                                    psi_terminal::TerminalBorrowBoundarySource::Statement {
                                        statement_index: previous_start,
                                    },
                                    psi_terminal::TerminalBorrowBoundarySource::Statement {
                                        statement_index: member_start,
                                    },
                                ) if previous_start < member_start
                            ))
                    });
                let primary_count = row
                    .shared_cohort
                    .iter()
                    .filter(|member| {
                        member.child_owner_identity == row.child_owner_identity
                            && member.child_owner_path == row.child_owner_path
                            && member.child_place == row.child_place
                            && member.child_access == row.child_access
                            && member.child_activation == row.child_activation
                            && member.child_weakening == row.child_weakening
                    })
                    .count();
                let cohort_is_exact = row.shared_cohort.iter().all(|member| {
                    let member_segments = &member.child_place.segments;
                    let member_lifecycle = matches!(
                        (
                            &row.direct_root_activation,
                            &member.child_activation,
                            &member.child_weakening,
                            &row.direct_root_weakening,
                        ),
                        (
                            psi_terminal::TerminalBorrowBoundarySource::Statement {
                                statement_index: parent_start,
                            },
                            psi_terminal::TerminalBorrowBoundarySource::Statement {
                                statement_index: child_start,
                            },
                            psi_terminal::TerminalBorrowBoundarySource::Statement {
                                statement_index: child_end,
                            },
                            psi_terminal::TerminalBorrowBoundarySource::Statement {
                                statement_index: parent_end,
                            },
                        ) if parent_start < child_start
                            && child_start <= child_end
                            && child_end < parent_end
                    );
                    is_canonical_borrow_identity(&member.child_owner_identity)
                        && is_canonical_borrow_identity(&member.child_place.root_identity)
                        && valid_owner_path(&member.child_owner_path)
                        && member_segments.iter().all(valid_place_segment)
                        && member.child_access == psi_terminal::StructuralAccess::SharedBorrow
                        && member.child_place.root_identity == row.direct_root_place.root_identity
                        && member_segments.starts_with(root_segments)
                        && member.child_weakening == row.child_weakening
                        && valid_borrow_boundary(&member.child_activation)
                        && valid_borrow_boundary(&member.child_weakening)
                        && member_lifecycle
                });
                row.child_access == psi_terminal::StructuralAccess::SharedBorrow
                    && cohort_is_unique
                    && cohort_owner_order
                    && primary_count == 1
                    && cohort_is_exact
                    && (row.shared_cohort.len() == 1
                        || machine.is_some_and(|caller| {
                            exact_shared_cohort_observation(caller, machines, row.operation)
                        }))
            }
        };
        let invalid = !exact_mutating_call
            || (row.restoration_class
                == psi_terminal::TerminalReborrowRestorationClass::SharedFreezeRestoration
                && !unique_compatible_mutating_call)
            || !exact_call_boundary
            || !exact_restoration_class
            || !is_canonical_borrow_identity(&row.source_machine_identity)
            || !is_canonical_borrow_identity(&row.source_state_identity)
            || !is_canonical_borrow_identity(&row.direct_root_owner_identity)
            || !is_canonical_borrow_identity(&row.direct_root_place.root_identity)
            || !valid_owner_path(&row.direct_root_owner_path)
            || !row.direct_root_owner_path.is_empty()
            || !root_segments.iter().all(valid_place_segment)
            || !valid_borrow_boundary(&row.direct_root_activation)
            || !valid_borrow_boundary(&row.direct_root_weakening)
            || !exact_one_hop_lifecycle
            || !is_canonical_borrow_identity(&row.direct_root_lifetime_identity)
            || row.direct_root_lifetime_identity != row.direct_root_place.root_identity
            || !is_canonical_borrow_identity(&row.child_owner_identity)
            || !is_canonical_borrow_identity(&row.child_place.root_identity)
            || !valid_owner_path(&row.child_owner_path)
            || !child_segments.iter().all(valid_place_segment)
            || !row.projection_remainder.iter().all(valid_place_segment)
            || row.child_place.root_identity != row.direct_root_place.root_identity
            || !child_segments.starts_with(root_segments)
            || child_segments[root_segments.len()..] != row.projection_remainder
            || row.formation_boundary != row.child_activation
            || !valid_borrow_boundary(&row.child_activation)
            || !valid_borrow_boundary(&row.formation_boundary)
            || !valid_borrow_boundary(&row.child_weakening);
        if invalid {
            return Err(ModuleError::InvalidReborrowRestoredCallUse {
                machine: row.machine,
                operation: row.operation,
            });
        }
    }
    if !rows.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(ModuleError::NonCanonicalReborrowRestoredCallUseOrder);
    }
    Ok(())
}

fn exact_shared_cohort_observation(
    caller: &TerminalMachine,
    machines: &BTreeMap<psi_core::MachineId, &TerminalMachine>,
    mutation: psi_core::OperationId,
) -> bool {
    let locations = caller
        .blocks
        .iter()
        .filter_map(|block| {
            block
                .operations
                .iter()
                .position(|operation| operation.id == mutation)
                .map(|index| (block, index))
        })
        .collect::<Vec<_>>();
    let [(block, mutation_index)] = locations.as_slice() else {
        return false;
    };
    let Some(observation_index) = mutation_index.checked_sub(1) else {
        return false;
    };
    let mutation_operation = &block.operations[*mutation_index];
    let OperationKind::CallUnit {
        structural_arguments: mutation_arguments,
        ..
    } = &mutation_operation.kind
    else {
        return false;
    };
    let [mutation_argument] = mutation_arguments.as_slice() else {
        return false;
    };
    let observation = &block.operations[observation_index];
    let OperationKind::CallUnit {
        callee,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = &observation.kind
    else {
        return false;
    };
    if !matches!(structural_arguments.len(), 2 | 3) {
        return false;
    }
    let Some(observer) = machines.get(callee) else {
        return false;
    };
    let Some(caller_parameter) = caller
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == mutation_argument.place)
    else {
        return false;
    };
    if observer.structural_parameters.len() != structural_arguments.len() {
        return false;
    }
    let [observer_block] = observer.blocks.as_slice() else {
        return false;
    };
    observation.result == OperationResult::Unit
        && structural_arguments
            .iter()
            .zip(&observer.structural_parameters)
            .enumerate()
            .all(|(position, (argument, parameter))| {
                argument.place == mutation_argument.place
                    && argument.path.is_empty()
                    && argument.access == StructuralAccess::SharedBorrow
                    && u32::try_from(position).ok() == Some(parameter.position)
                    && !parameter.is_self
                    && parameter.access == StructuralAccess::SharedBorrow
                    && parameter.structural_type == caller_parameter.structural_type
            })
        && claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && crash_continuations.is_empty()
        && observer.parameters.is_empty()
        && observer.result == TerminalMachineResult::Unit
        && observer_block.operations.is_empty()
        && matches!(
            &observer_block.terminator,
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } if trivial_affine_discards.is_empty()
        )
}

fn exact_restored_mutating_call(
    operation: &psi_terminal::Operation,
    caller: &TerminalMachine,
    machines: &BTreeMap<psi_core::MachineId, &TerminalMachine>,
    expected_callee: Option<psi_core::MachineId>,
) -> bool {
    if operation.result != OperationResult::Unit {
        return false;
    }
    let OperationKind::CallUnit {
        callee,
        structural_arguments,
        claim_transfers,
        ..
    } = &operation.kind
    else {
        return false;
    };
    if expected_callee.is_some_and(|expected| expected != *callee) {
        return false;
    }
    let [argument] = structural_arguments.as_slice() else {
        return false;
    };
    let Some(callee) = machines.get(callee) else {
        return false;
    };
    argument.access == StructuralAccess::MutableBorrow
        && argument.path.is_empty()
        && claim_transfers.is_empty()
        && callee.parameters.is_empty()
        && callee.result == TerminalMachineResult::Unit
        && matches!(
            callee.structural_parameters.as_slice(),
            [parameter]
                if parameter.position == 0
                    && !parameter.is_self
                    && parameter.access == StructuralAccess::MutableBorrow
        )
        && caller.structural_parameters.iter().any(|parameter| {
            parameter.place == argument.place && parameter.access == StructuralAccess::MutableBorrow
        })
}

fn is_canonical_borrow_identity(identity: &str) -> bool {
    if let Some(digest) = identity.strip_prefix("terminal-borrow:") {
        return digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    }
    is_canonical_hermetic_identity(identity)
}

fn is_canonical_hermetic_identity(identity: &str) -> bool {
    if let Some(path) = identity.strip_prefix("toolchain::") {
        return !path.is_empty();
    }
    let Some(package) = identity.strip_prefix("package:") else {
        return false;
    };
    let Some((digest, path)) = package.split_once("::") else {
        return false;
    };
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && !path.is_empty()
}

/// Validate the representation-wide invariants needed by canonical codecs.
///
/// This remains a distinct entry point so canonical representation checks do
/// not themselves confer an execution-grade `ValidatedTerminalModule`.
pub fn validate_module_representation(module: &TerminalModule) -> Result<(), ModuleError> {
    validate_module_with_policy(module, ValidationPolicy::Representation)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationPolicy {
    Execution,
    Interpretation,
    Optimization,
    NativeRankedCountdown,
    Representation,
}

fn validate_module_with_policy(
    module: &TerminalModule,
    policy: ValidationPolicy,
) -> Result<(), ModuleError> {
    if module.machines.is_empty() {
        return Err(ModuleError::EmptyModule);
    }
    validate_proposition_vocabulary(module)?;
    validate_structural_foundation(module)?;
    validate_closed_conformance_applications(module)?;
    float_meaning::validate_float_meaning_projections(module)?;
    quotient_correspondence::validate_quotient_correspondences(module, policy)?;

    let mut registry = IdRegistry::default();
    for projection in module
        .structural_domains
        .iter()
        .filter_map(|domain| domain.content_projection.as_ref())
    {
        if registry
            .owner_content_projections
            .insert(
                projection.identity.domain,
                (projection.identity, projection.algebra.clone()),
            )
            .is_some()
        {
            return Err(ModuleError::ContentProjectionOwnerMismatch(
                projection.identity,
            ));
        }
    }
    content::validate_boundary_content_guarantees(module, &mut registry)?;
    for machine in &module.machines {
        insert_unique(
            &mut registry.machines,
            machine.id,
            ModuleError::DuplicateMachine,
        )?;
        insert_unique(
            &mut registry.contracts,
            machine.contract.id,
            ModuleError::DuplicateContract,
        )?;
    }
    validate_proof_recursive_components(module, &mut registry)?;
    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    validate_placed_view_inputs(module, &machines)?;
    validate_reborrow_root_handoffs(module, &machines)?;
    validate_reborrow_restored_call_uses(module, &machines)?;
    dynamic_dispatch::validate_dynamic_dispatches(module, &machines)?;
    validate_evidence_contract_lanes(module, &machines)?;
    for machine in &module.machines {
        machine::validate_machine(module, machine, &machines, &mut registry, policy)?;
    }
    validate_call_graph(module)?;
    if !registry.machines.contains(&module.entry) {
        return Err(ModuleError::UnknownEntryMachine(module.entry));
    }
    root_service_reach::validate_root_service_reach_exact(module)?;

    match policy {
        ValidationPolicy::Interpretation | ValidationPolicy::Optimization => {
            validate_interpretable_ranked_countdown_module(module)?;
        }
        ValidationPolicy::NativeRankedCountdown => {
            validate_native_ranked_countdown_module(module)?;
        }
        ValidationPolicy::Execution | ValidationPolicy::Representation => {}
    }

    Ok(())
}

fn validate_native_ranked_countdown_module(module: &TerminalModule) -> Result<(), ModuleError> {
    validate_interpretable_ranked_countdown_module(module)?;
    let Some(machine) = module
        .machines
        .iter()
        .find(|machine| machine.ranked_scc.is_some())
    else {
        return Err(ModuleError::NonExecutableRankedScc(module.entry));
    };
    let component = machine.ranked_scc.as_ref().expect("ranked machine");
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32)
        .expect("the fixed unsigned 32-bit carrier is valid");
    let [structural_parameter] = machine.structural_parameters.as_slice() else {
        return Err(ModuleError::NonExecutableRankedScc(machine.id));
    };
    let [structural_place] = machine.structural_places.as_slice() else {
        return Err(ModuleError::NonExecutableRankedScc(machine.id));
    };
    let affine_owned = !structural_parameter.is_self
        && structural_parameter.multiplicity == StructuralMultiplicity::Affine
        && structural_parameter.access == StructuralAccess::Owned;
    let persistent_receiver = structural_parameter.is_self
        && structural_parameter.access == StructuralAccess::MutableBorrow;
    if component.rank_type != u32_type
        || machine.parameters[0].scalar_type != ScalarType::Integer(u32_type)
        || structural_parameter.position != 0
        || (!affine_owned && !persistent_receiver)
        || !structural_parameter.qualifications.is_empty()
        || structural_place.id != structural_parameter.place
        || structural_place.kind
            != (StructuralPlaceKind::Parameter {
                position: 0,
                is_self: structural_parameter.is_self,
            })
        || !machine.entry_claims.is_empty()
        || !machine.content_entry_claims.is_empty()
    {
        return Err(ModuleError::NonExecutableRankedScc(machine.id));
    }
    Ok(())
}

fn validate_interpretable_ranked_countdown_module(
    module: &TerminalModule,
) -> Result<(), ModuleError> {
    let ranked = module
        .machines
        .iter()
        .filter(|machine| machine.ranked_scc.is_some())
        .collect::<Vec<_>>();
    let Some(machine) = ranked.first().copied() else {
        return Ok(());
    };
    let reject = || ModuleError::NonExecutableRankedScc(machine.id);
    if ranked.len() != 1
        || module.machines.len() != 1
        || !module.boundary_machines.is_empty()
        || !module.provider_candidates.is_empty()
        || machine.result != TerminalMachineResult::Unit
        || machine.parameters.len() != 1
        || machine.blocks.len() != 4
    {
        return Err(reject());
    }

    let component = machine.ranked_scc.as_ref().expect("ranked machine");
    let header = machine
        .blocks
        .iter()
        .find(|block| block.id == component.header)
        .ok_or_else(reject)?;
    let row = component.covered_cyclic_edges.first().ok_or_else(reject)?;
    let decrement = machine
        .blocks
        .iter()
        .find(|block| block.id == row.source)
        .ok_or_else(reject)?;
    let Terminator::Conditional {
        when_false: exit, ..
    } = &header.terminator
    else {
        return Err(reject());
    };
    let done = machine
        .blocks
        .iter()
        .find(|block| block.id == exit.target)
        .ok_or_else(reject)?;
    let entry = machine
        .blocks
        .iter()
        .find(|block| block.id == machine.entry)
        .ok_or_else(reject)?;

    if !entry.operations.is_empty()
        || header.parameters.len() != 1
        || header.operations.len() != 2
        || !decrement.parameters.is_empty()
        || decrement.operations.len() != 2
        || !done.parameters.is_empty()
        || !done.operations.is_empty()
        || !matches!(done.terminator, Terminator::ReturnUnit { .. })
    {
        return Err(reject());
    }
    Ok(())
}

#[derive(Default)]
struct IdRegistry {
    machines: BTreeSet<MachineId>,
    blocks: BTreeSet<BlockId>,
    contracts: BTreeSet<ContractId>,
    operations: BTreeSet<OperationId>,
    edges: BTreeSet<EdgeId>,
    obligations: BTreeSet<ObligationId>,
    recursive_components: BTreeSet<RecursiveComponentId>,
    values: BTreeSet<ValueId>,
    places: BTreeSet<PlaceId>,
    owner_content_projections:
        BTreeMap<ContentDomainId, (ContentProjectionIdentity, ContentAlgebra)>,
    content_projection_algebras: BTreeMap<ContentProjectionIdentity, ContentAlgebra>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum StructuralRootKey {
    Parameter(u32),
    Result,
    OperationResult(OperationId),
    ByteSequenceLiteral(u32),
    ProviderAttachment(StructuralTypeId, StructuralFieldId, BoundaryMachineId),
    TrivialAffineLocal(u32),
}

fn validate_boolean_structural_field(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: OperationId,
    source: PlaceId,
    field: StructuralFieldId,
) -> Result<(), ModuleError> {
    structural_scalar_fields::validate_boolean_structural_field(
        module, machine, operation, source, field,
    )
}

pub(crate) fn machine_value_types(
    machine: &TerminalMachine,
) -> impl Iterator<Item = (ValueId, ScalarType)> + '_ {
    machine
        .parameters
        .iter()
        .chain(machine.result.scalar_ref())
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| block.parameters.iter()),
        )
        .chain(machine.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(|operation| operation.result.scalar_ref())
        }))
        .map(|declaration| (declaration.id, declaration.scalar_type))
}

fn insert_value(
    values: &mut BTreeMap<ValueId, ScalarType>,
    module_values: &mut BTreeSet<ValueId>,
    id: ValueId,
    scalar_type: ScalarType,
) -> Result<(), ModuleError> {
    if values.insert(id, scalar_type).is_some() || !module_values.insert(id) {
        return Err(ModuleError::DuplicateValue(id));
    }
    Ok(())
}

fn insert_unique<T: Ord + Copy>(
    set: &mut BTreeSet<T>,
    value: T,
    error: impl FnOnce(T) -> ModuleError,
) -> Result<(), ModuleError> {
    if !set.insert(value) {
        return Err(error(value));
    }
    Ok(())
}
