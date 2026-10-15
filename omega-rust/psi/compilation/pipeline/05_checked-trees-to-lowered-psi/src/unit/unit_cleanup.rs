//! Structural Unit cleanup lowering.
//!
//! Nominal cleanup contracts are prepared with ordinary callable signatures
//! (`contexts`); the shared completion owner lowers their source-owned actions
//! for entries and called members alike. `ordered` only roots a selected nominal
//! entry. A partial-affine body is an ordinary body: its residual complement
//! and home retirement are actions of its checked normal exit, lowered with
//! every other completion action, never appended to return edges afterwards.
//! Build cleanup requirements and their obligation identities here, but leave
//! certificates to final operation proof emission. Owned-field requirements are
//! validity-scoped observations, not permanent assumption slots; their available
//! premises are known only after the complete caller and cleanup edge exist.

use super::{
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedTrees, CheckedUnitEffectOperationPlan,
    LoweringError, MachineId, NominalAffineCleanup, PlaceId, Proposition, ScalarTerm,
    StructuralTypeId, StructuralTypeShape, TerminalMachine, lookup_machine_id, lookup_type_id,
    lower_unit_closure, obligation_id, unsupported,
};
use checked_trees::CheckedUnitNominalAffineCleanupPlan;
use symbols::SymbolHandle;
pub(crate) mod contexts;
mod dispositions;
pub(crate) use dispositions::outstanding_actions;
mod ordered;
mod reference_retirement;
use ordered::lower_ordered_nominal_affine_unit_cleanup_machine;
pub(crate) use reference_retirement::lower_reference_completion_regions;

/// Emission scratch for one checked region. Conditions remain control, not a
/// property of the cleanup callable or authority to discharge its contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CleanupCaseCondition {
    pub(crate) place: PlaceId,
    pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) case: semantic_vocabulary::StructuralCaseId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CleanupRegion {
    /// Source-authorized conditional death of this exact occurrence. Lowering
    /// specializes its enclosing continuation before publishing Terminal Psi.
    pub(crate) selected_region: Option<terminal_psi::StructuralArgument>,
    pub(crate) conditions: Vec<CleanupCaseCondition>,
    pub(crate) action: terminal_psi::TerminalAffineCleanupAction,
}

impl CleanupRegion {
    pub(crate) fn unconditional(action: terminal_psi::TerminalAffineCleanupAction) -> Self {
        Self {
            selected_region: None,
            conditions: Vec::new(),
            action,
        }
    }
}

pub(crate) fn lower_cleanup_conditions(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    control: &checked_trees::CheckedCleanupControlPlans,
    selected: arena::HandleSpan<checked_trees::CheckedCleanupCaseCondition>,
    types: &[terminal_psi::StructuralTypeDeclaration],
    mut storage: impl FnMut(
        &mut terminal_psi::TerminalIdentityTableBuilder,
        &[checked_trees::CheckedUnitStructuralPathSegment],
    ) -> Result<
        (PlaceId, Vec<terminal_psi::StructuralPathSegment>),
        LoweringError,
    >,
) -> Result<Vec<CleanupCaseCondition>, LoweringError> {
    if selected.is_empty() {
        return Ok(Vec::new());
    }
    let conditions = control
        .conditions
        .span(selected)
        .ok_or(LoweringError::Unsupported(
            "cleanup conditions lost their owning catalog",
        ))?;
    conditions
        .iter()
        .map(|condition| {
            let path = if condition.path.is_empty() {
                &[][..]
            } else {
                control
                    .paths
                    .span(condition.path)
                    .ok_or(LoweringError::Unsupported(
                        "cleanup condition lost its retained path",
                    ))?
            };
            let (place, path) = storage(identities, path)?;
            let structural_type = crate::terminal_identities::structural_declaration_by_identity(
                types,
                identities,
                checked,
                condition.type_identity,
                "cleanup condition has no structural type",
            )?
            .id;
            let (_, case) = crate::expression_preparation::bindings::structural_cases::resolve_at(
                identities,
                types,
                structural_type,
                &[],
                &condition.case_identity,
            )?;
            Ok(CleanupCaseCondition {
                place,
                path,
                structural_type,
                case,
            })
        })
        .collect()
}

/// Lower a nominal affine Unit cleanup machine. One route serves any number
/// of structural parameters: each root owns exactly one cleanup action, and
/// the actions run in reverse declaration order (`ordered`).
pub(crate) fn lower_nominal_affine_unit_cleanup_machine(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    nominal: &CheckedNominalAffineUnitCleanupMachinePlan,
    program_roots: crate::TerminalProgramRoots<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    lower_ordered_nominal_affine_unit_cleanup_machine(
        identities,
        checked,
        nominal,
        program_roots,
        exact_toolchain_sources,
    )
}

/// Selected cleanup operations have the same hook contract for Unit and scalar exits.
pub(crate) fn continuation_cleanups<'a>(
    operations: impl IntoIterator<Item = &'a CheckedUnitEffectOperationPlan>,
) -> impl Iterator<Item = &'a CheckedUnitNominalAffineCleanupPlan> {
    operations
        .into_iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::ContinuationCleanup {
                cleanup_actions, ..
            } => Some(cleanup_actions),
            _ => None,
        })
        .flatten()
        .filter_map(checked_trees::CheckedNormalCleanupAction::nominal)
}

/// Resolve the checked schedule once, retaining its selection conditions for
/// ordinary control emission. The storage resolver owns captured coordinates;
/// neither action paths nor tag observations may evaluate a selector again.
pub(crate) fn lower_checked_cleanup_regions<'action>(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    actions: impl IntoIterator<Item = &'action checked_trees::CheckedNormalCleanupAction>,
    selected_regions: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    control: &checked_trees::CheckedCleanupControlPlans,
    types: &[terminal_psi::StructuralTypeDeclaration],
    type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    mut storage: impl FnMut(
        &mut terminal_psi::TerminalIdentityTableBuilder,
        &checked_trees::CheckedUnitStructuralArgumentSourcePlan,
        &[checked_trees::CheckedUnitStructuralPathSegment],
    ) -> Result<
        (PlaceId, Vec<terminal_psi::StructuralPathSegment>),
        LoweringError,
    >,
    mut carrier_multiplicity: impl FnMut(
        PlaceId,
    )
        -> Result<terminal_psi::StructuralMultiplicity, LoweringError>,
    mut nominal: impl FnMut(
        &CheckedUnitNominalAffineCleanupPlan,
        PlaceId,
        StructuralTypeId,
        Vec<terminal_psi::StructuralPathSegment>,
    ) -> Result<NominalAffineCleanup, LoweringError>,
) -> Result<Vec<CleanupRegion>, LoweringError> {
    let mut regions = actions
        .into_iter()
        .map(|action| {
            let (place, path) = storage(identities, action.source(), action.path())?;
            let selected = selected_regions
                .iter()
                .filter(|region| {
                    region.source == *action.source() && action.path().starts_with(&region.path)
                })
                .max_by_key(|region| region.path.len());
            let selected_region = if let Some(region) = selected {
                let (place, path) = storage(identities, &region.source, &region.path)?;
                Some(terminal_psi::StructuralArgument {
                    place,
                    path,
                    access: terminal_psi::StructuralAccess::Owned,
                })
            } else {
                None
            };
            let conditions = lower_cleanup_conditions(
                identities,
                checked,
                control,
                action.case_conditions(),
                types,
                |builder, path| storage(builder, action.source(), path),
            )?;
            let lowered = match action {
                checked_trees::CheckedNormalCleanupAction::RetireHome { .. } => {
                    terminal_psi::TerminalAffineCleanupAction::RetireHome(place)
                }
                checked_trees::CheckedNormalCleanupAction::Discard(_)
                    if path.is_empty() && conditions.is_empty() =>
                {
                    // The source-selected death may end a copyable descriptor
                    // without owning an affine resource. Use its actual emitted
                    // occurrence, not the source action's generic discard name.
                    if carrier_multiplicity(place)?
                        == terminal_psi::StructuralMultiplicity::Unrestricted
                    {
                        terminal_psi::TerminalAffineCleanupAction::RetireHome(place)
                    } else {
                        terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place)
                    }
                }
                checked_trees::CheckedNormalCleanupAction::Discard(_) => {
                    terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
                        terminal_psi::StructuralAffineDiscard {
                            place,
                            path,
                            structural_type: lookup_type_id(type_ids, &action.type_identity())?,
                        },
                    )
                }
                checked_trees::CheckedNormalCleanupAction::InvokeNominal(cleanup) => {
                    terminal_psi::TerminalAffineCleanupAction::InvokeNominal(nominal(
                        cleanup,
                        place,
                        lookup_type_id(type_ids, &action.type_identity())?,
                        path,
                    )?)
                }
            };
            Ok(CleanupRegion {
                selected_region,
                conditions,
                action: lowered,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    retain_cleanup_descendants(&mut regions);
    Ok(regions)
}

/// Resolve parent disposition while the complete checked schedule is intact,
/// before case selection splits it across edges. The recursive producer orders
/// each parent's descendant partition immediately after its hook. A selected
/// empty alternative can dispose the same region rather than a deeper path.
fn retain_cleanup_descendants(regions: &mut [CleanupRegion]) {
    for position in 0..regions.len().saturating_sub(1) {
        let (prefix, suffix) = regions.split_at_mut(position + 1);
        let terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) =
            &mut prefix[position].action
        else {
            continue;
        };
        let next = &suffix[0].action;
        cleanup.retains_descendants = next.place() == cleanup.place
            && next.path().starts_with(&cleanup.path)
            && (next.path().len() > cleanup.path.len()
                || matches!(
                    next,
                    terminal_psi::TerminalAffineCleanupAction::DiscardResidual(_)
                ));
    }
}

pub(crate) fn computation_cleanups(
    checked: &CheckedTrees,
    machine: SymbolHandle,
) -> Result<Vec<&CheckedUnitNominalAffineCleanupPlan>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans.roots_for_machine(machine);
    let mut cleanups = Vec::new();
    for identity in
        crate::expression_preparation::computation_graph::reachable_nodes(checked, &roots)?
    {
        let retained = match plans.nodes.get(identity).kind {
            checked_trees::CheckedScalarComputationKind::Call {
                temporary_cleanups, ..
            } => temporary_cleanups,
            _ => continue,
        };
        for row in plans.temporary_cleanups.span_or_empty(retained) {
            cleanups.extend(
                plans
                    .temporary_cleanup_actions
                    .span_or_empty(row.actions)
                    .iter()
                    .filter_map(checked_trees::CheckedNormalCleanupAction::nominal),
            );
        }
    }
    Ok(cleanups)
}

/// Every published action participates in hook subject/receiver preparation.
pub(crate) fn selected_cleanups(
    checked: &CheckedTrees,
) -> impl Iterator<Item = &CheckedUnitNominalAffineCleanupPlan> {
    let plans = &checked.facts.flow.terminal_unit_effects;
    let composed_states = || {
        plans
            .composed_machines
            .iter()
            .flat_map(|machine| &machine.states)
    };
    checked
        .facts
        .values
        .scalar_computations
        .temporary_cleanup_actions
        .iter()
        .filter_map(|(_, action)| action.nominal())
        .chain(
            plans
                .machines
                .iter()
                .flat_map(|machine| continuation_cleanups(&machine.operations)),
        )
        .chain(composed_states().flat_map(|state| continuation_cleanups(&state.operations)))
        .chain(
            plans
                .machines
                .iter()
                .map(|machine| machine.normal_exit_cleanups)
                .chain(composed_states().map(|state| state.normal_exit_cleanups))
                .flat_map(move |rows| plans.cleanup_control.normal_actions(rows))
                .filter_map(checked_trees::CheckedNormalCleanupAction::nominal),
        )
}

/// Materialize one checked normal-cleanup operation in the shared closure namespace.
/// The caller owns return value custody; this operation only borrows its exact owner.
pub(crate) fn lower_nominal_cleanup_action(
    cleanup: &CheckedUnitNominalAffineCleanupPlan,
    place: PlaceId,
    structural_type: StructuralTypeId,
    path: Vec<terminal_psi::StructuralPathSegment>,
    machine_ids: &[(SymbolHandle, MachineId)],
    receivers: &[(SymbolHandle, PlaceId)],
    requirement_counts: &[(SymbolHandle, usize)],
    next_obligation: &mut u64,
) -> Result<NominalAffineCleanup, LoweringError> {
    materialize_nominal_cleanup_action(
        cleanup,
        place,
        structural_type,
        path,
        machine_ids,
        receivers.iter().find_map(|(source, receiver)| {
            (*source == cleanup.cleanup_machine).then_some(*receiver)
        }),
        requirement_count(requirement_counts, cleanup.cleanup_machine)?,
        &mut || crate::terminal_identities::allocate_dense(next_obligation).map(obligation_id),
    )
}

/// Resolve only the claim names retained by checked disposition selection.
/// Calls and constructions may have forwarded them before this edge; the
/// current invocation namespace remains their sole executable identity owner.
/// Every nominal cleanup action binds its consumed claims here, whether the
/// roster is the checked plan's or one a prepared edge retained.
pub(crate) fn bind_consumed_claims(
    action: &mut NominalAffineCleanup,
    consumed_claims: &[language_semantics::PermissionClaimIdentity],
    bindings: &[(
        language_semantics::PermissionClaimIdentity,
        semantic_vocabulary::ClaimId,
    )],
) -> Result<(), LoweringError> {
    action.consumed_claims = consumed_claims
        .iter()
        .map(|identity| crate::terminal_identities::lookup_claim_id(bindings, *identity))
        .collect::<Result<Vec<_>, _>>()?;
    action.consumed_claims.sort_unstable();
    if action
        .consumed_claims
        .windows(2)
        .any(|claims| claims[0] == claims[1])
    {
        return Err(LoweringError::Unsupported(
            "nominal cleanup repeats its consumed claim",
        ));
    }
    Ok(())
}

/// Bind a selected action to the closure's executable identities. Target and
/// subject selection belong to the checked producer; this only allocates each
/// action's independent obligation roster in the emitting body's namespace.
fn materialize_nominal_cleanup_action(
    cleanup: &CheckedUnitNominalAffineCleanupPlan,
    place: PlaceId,
    structural_type: StructuralTypeId,
    path: Vec<terminal_psi::StructuralPathSegment>,
    machine_ids: &[(SymbolHandle, MachineId)],
    cleanup_receiver: Option<PlaceId>,
    requirement_count: usize,
    allocate_requirement: &mut impl FnMut() -> Result<semantic_vocabulary::ObligationId, LoweringError>,
) -> Result<NominalAffineCleanup, LoweringError> {
    Ok(NominalAffineCleanup {
        retains_descendants: false,
        place,
        path,
        structural_type,
        cleanup_machine: lookup_machine_id(machine_ids, cleanup.cleanup_machine)?,
        consumed_claims: Vec::new(),
        cleanup_receiver,
        requirement_obligations: (0..requirement_count)
            .map(|_| allocate_requirement())
            .collect::<Result<Vec<_>, _>>()?,
    })
}

/// Cleanup and authored calls owe the same normalized callable roster, including
/// runtime prerequisites merged with authored clauses. Source clause counts and
/// a separate hook predicate parser cannot predict that published roster.
fn requirement_count(
    counts: &[(SymbolHandle, usize)],
    machine: SymbolHandle,
) -> Result<usize, LoweringError> {
    counts
        .iter()
        .find_map(|(source, count)| (*source == machine).then_some(*count))
        .ok_or(LoweringError::Unsupported(
            "nominal cleanup target prerequisite signature is absent",
        ))
}

/// Scalar operand expansion reserves storage before the shared call closure is
/// allocated. Preserve selected hook identity until emission instead of
/// inventing a provisional machine or obligation identity during expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PreparedNormalCleanupAction {
    Resolved(CleanupRegion),
    InvokeNominal {
        conditions: Vec<CleanupCaseCondition>,
        cleanup: CheckedUnitNominalAffineCleanupPlan,
        place: PlaceId,
        structural_type: StructuralTypeId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        consumed_claims: Vec<language_semantics::PermissionClaimIdentity>,
    },
}

impl PreparedNormalCleanupAction {
    pub(crate) fn discard_root(place: PlaceId) -> Self {
        Self::Resolved(CleanupRegion::unconditional(
            terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place),
        ))
    }
}

impl From<&CleanupRegion> for PreparedNormalCleanupAction {
    fn from(region: &CleanupRegion) -> Self {
        Self::Resolved(region.clone())
    }
}

pub(crate) fn emit_prepared_cleanup_regions(
    actions: &[PreparedNormalCleanupAction],
    calls: &mut crate::emission::operation_emission::calls::CallEmissionContext<'_>,
) -> Result<Vec<CleanupRegion>, LoweringError> {
    let mut regions = actions
        .iter()
        .map(|action| {
            Ok(match action {
                PreparedNormalCleanupAction::Resolved(action) => action.clone(),
                PreparedNormalCleanupAction::InvokeNominal {
                    conditions,
                    cleanup,
                    place,
                    structural_type,
                    path,
                    consumed_claims,
                } => {
                    let machine_ids = calls.machine_ids;
                    let receiver = calls
                        .cleanup_receivers
                        .iter()
                        .find_map(|(source, receiver)| {
                            (*source == cleanup.cleanup_machine).then_some(*receiver)
                        });
                    let mut action = materialize_nominal_cleanup_action(
                        cleanup,
                        *place,
                        *structural_type,
                        path.clone(),
                        machine_ids,
                        receiver,
                        requirement_count(calls.requirement_counts, cleanup.cleanup_machine)?,
                        // Scalar helpers own bounded identity ranges. Use
                        // their allocator rather than an unbounded counter.
                        &mut || calls.allocate_requirement(),
                    )?;
                    bind_consumed_claims(&mut action, consumed_claims, calls.claims.bindings())?;
                    CleanupRegion {
                        selected_region: None,
                        conditions: conditions.clone(),
                        action: terminal_psi::TerminalAffineCleanupAction::InvokeNominal(action),
                    }
                }
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    retain_cleanup_descendants(&mut regions);
    Ok(regions)
}

/// Retain each selected hook's semantic owner independently of attachment
/// storage. Its ordinary structural parameter carries the borrowed subject;
/// this binding neither schedules disposal nor changes captured receiver places.
pub(super) fn bind_nominal_cleanup_subjects(
    checked: &CheckedTrees,
    machines: &mut [TerminalMachine],
    type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    machine_ids: &[(SymbolHandle, MachineId)],
) -> Result<(), LoweringError> {
    for cleanup in selected_cleanups(checked) {
        let Ok(target_id) = lookup_machine_id(machine_ids, cleanup.cleanup_machine) else {
            continue;
        };
        let owner = lookup_type_id(type_ids, &cleanup.type_identity)?;
        let emitted = machines
            .iter_mut()
            .find(|machine| machine.id == target_id)
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup target was not emitted",
            ))?;
        // Terminal attachment names semantic ownership, not an implicit
        // argument. A static named subject still carries the same selected
        // T::drop owner while its ordinary parameter carries the storage.
        emitted.attachment = Some(owner);
        emitted.nominal_cleanup_subject = crate::machine_lowering::nominal_cleanup_subject(
            checked,
            cleanup.cleanup_machine,
            type_ids,
        )?;
    }
    Ok(())
}
