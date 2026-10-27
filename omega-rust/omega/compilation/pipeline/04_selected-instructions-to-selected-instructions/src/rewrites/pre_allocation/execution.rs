//! Ordered incremental pre-allocation execution and independent boundary replay.
//!
//! Discover enabled candidate sites once, in catalog/function/block/body order.
//! Each attempt reads the current plan; a commit remaps positions and invalidates
//! eligibility in its function, not every unrelated function. New instruction
//! kinds enter the same ordered frontier immediately. Earlier families keep
//! priority: draining load forwarding first can erase a copy's last use before
//! copy removal runs and leave a different fixed point.
//!
//! All current families preserve function/block rosters and change family
//! membership only at the committed site. Their eligibility depends on their
//! own function and unchanged target facts. A future cross-function admission or
//! structural edit must extend this invalidation domain before joining the
//! catalog. Budget/overflow failures are never cached as legality declines.
//! Iterations count discovery sweeps; evaluation and commit axes charge actual
//! work. Independent replay remains available at untrusted/test boundaries.
//!
//! One catalog row owns one family's admission. `PreAllocationRuleCatalogEntry`
//! payloads carry a `PreAllocationRuleAdmission` — the source-bound candidate
//! surface the frontier scans and the site attempt the driver runs — so a
//! family's enable/order declaration, candidate surface and attempt are one
//! declaration rather than three tables keyed on the same `Optimization`. Only
//! the shared protocol (audit charge, decline versus abort, committed-step
//! restaging) is authored here.
use crate::{StoredLoadForwardingError, forward_selected_stored_load};
use std::collections::BTreeSet;

use crate::AddressFoldError;
use crate::ConstantBooleanError;
use crate::ConstantBranchError;
use crate::CopiedCallOperandError;
use crate::CopyRemovalError;
use crate::DeadCompareError;
use crate::EquivalentCompareError;
use crate::OptimizedPreAllocationCustodyError;
use crate::PreAllocationPolicy;
use crate::RedundantCompareError;
use crate::RedundantExtensionError;
use crate::SelectedProgramRef;
use crate::StagedOptimizedAllocationLegality;
use crate::StagedOptimizedAllocationLegalityCustodyReceipt;
use crate::StagedOptimizedPreAllocationAttempt;
use crate::StagedOptimizedPreAllocationAttemptReceipt;
use crate::StagedOptimizedPreAllocationIterationReceipt;
use crate::StagedOptimizedPreAllocationStep;
use crate::StagedPreAllocationOptimizationCustodyReceipt;
use crate::StagedPreAllocationOptimizationRun;
use crate::ValidatedPreAllocationTransformation;
use crate::ValidatedSelectedAnalysis;
use crate::analyses::validated_machine_effect_catalog;
use crate::analyze_allocation_legality;
use crate::analyze_live_ranges;
use crate::analyze_liveness;
use crate::resolve_pre_allocation_rules;
use crate::rewrites::condition_state::adjacency;
use crate::rewrites::{
    SelectedFunction, address_fold_measured_steps, constant_boolean_measured_steps,
    constant_boolean_surface_sizes, constant_branch_measured_steps, constant_branch_surface_sizes,
    copied_call_operand_measured_steps, copy_removal_measured_steps, dead_compare_measured_steps,
    equivalent_compare_measured_steps, fold_selected_address, fold_selected_constant_boolean,
    fold_selected_constant_branch, fold_selected_copied_call_operand,
    redundant_compare_measured_steps, redundant_extension_measured_steps, remove_dead_compare,
    remove_equivalent_compare, remove_redundant_compare, remove_selected_copy,
    remove_selected_redundant_extension,
};
use optimization_core::{
    OptimizationSelections, OptimizationWorkBudget, OptimizationWorkUsage,
    PreAllocationOptimizationCompletionIdentity,
};
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::terminator_instruction;
use selected_instructions::{
    SelectedInstruction, SelectedInstructionKind, SelectedInstructionPlan,
};

use super::PRE_ALLOCATION_RULE_CATALOG;

/// One fresh site's admission. Most families evaluate one proposal; a copied
/// call may try several current producers before committing or declining.
/// `evaluated` counts the proposals this site evaluated, this commit
/// included, and `declined` the refusals among them.
pub(crate) enum CandidateAttempt {
    Clean {
        declined: usize,
        evaluated: usize,
        validation_steps: u64,
    },
    Applied {
        transformation: ValidatedPreAllocationTransformation,
        declined: usize,
        evaluated: usize,
        validation_steps: u64,
    },
}

impl CandidateAttempt {
    /// A site whose proposals all declined: the refusals among `evaluated`
    /// proposals and the audit steps charged before each.
    pub(crate) fn clean(declined: usize, evaluated: usize, validation_steps: u64) -> Self {
        Self::Clean {
            declined,
            evaluated,
            validation_steps,
        }
    }

    /// A committed proposal: the family's transformation and the audit steps
    /// charged for admission and for the commit's independent validation.
    pub(crate) fn applied(
        transformation: ValidatedPreAllocationTransformation,
        declined: usize,
        evaluated: usize,
        validation_steps: u64,
    ) -> Self {
        Self::Applied {
            transformation,
            declined,
            evaluated,
            validation_steps,
        }
    }
}

/// Sites describe construction scratch, not durable instruction identities.
/// Deletion renumbers those identities; body positions are remapped at commit.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CandidatePosition {
    Body(usize),
    Terminator,
}

/// One candidate instruction's position in the current plan. Construction
/// scratch; the row's attempt reads the family's rewrites through it.
#[derive(Clone, Copy)]
pub(crate) struct CandidateSite {
    function_index: usize,
    block_index: usize,
    position: CandidatePosition,
}

impl CandidateSite {
    fn instruction(
        self,
        plan: &SelectedInstructionPlan,
    ) -> Result<&SelectedInstruction, OptimizedPreAllocationCustodyError> {
        let block = plan
            .functions
            .get(self.function_index)
            .and_then(|function| function.blocks.get(self.block_index))
            .ok_or(OptimizedPreAllocationCustodyError::CandidateFrontierMismatch)?;
        match self.position {
            CandidatePosition::Body(position) => block
                .instructions
                .get(position)
                .ok_or(OptimizedPreAllocationCustodyError::CandidateFrontierMismatch),
            CandidatePosition::Terminator => Ok(terminator_instruction(&block.terminator)),
        }
    }
}

/// The source-bound candidate surface a family considers, scanned in
/// plan/function/block/instruction order before admission decides. Its
/// predicate reads only the instruction kind: admission, not this surface,
/// decides whether the candidate's relationship holds.
#[derive(Debug, Clone, Copy)]
pub(crate) enum PreAllocationCandidateSurface {
    /// A body-position predicate: any body instruction it admits is a site.
    Body(fn(SelectedInstructionKind) -> bool),
    /// A terminator predicate: the block's terminator is a site when admitted.
    Terminator(fn(SelectedInstructionKind) -> bool),
}

impl PreAllocationCandidateSurface {
    pub(crate) const fn body(predicate: fn(SelectedInstructionKind) -> bool) -> Self {
        Self::Body(predicate)
    }

    pub(crate) const fn terminator(predicate: fn(SelectedInstructionKind) -> bool) -> Self {
        Self::Terminator(predicate)
    }

    /// Whether the site at `position` holds an instruction kind this surface
    /// covers.
    fn admits(self, position: CandidatePosition, instruction: &SelectedInstruction) -> bool {
        match (self, position) {
            (Self::Body(predicate), CandidatePosition::Body(_)) => predicate(instruction.kind),
            (Self::Terminator(predicate), CandidatePosition::Terminator) => {
                predicate(instruction.kind)
            }
            _ => false,
        }
    }
}

/// One catalog row's admission mechanism: the candidate surface the frontier
/// scans and the site attempt the fixed-point driver runs. Both live in the
/// row beside the family's exact policy, so a family joins the slice by
/// declaring its row rather than by editing a second dispatch site.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PreAllocationRuleAdmission {
    surface: PreAllocationCandidateSurface,
    attempt: PreAllocationSiteAttempt,
}

/// One family's whole site admission: evaluate the family's audit, run its
/// rewrite at the site, and classify the outcome — an admitted proposal, a
/// decline, or a budget/identity/replay failure that aborts the run under the
/// family's own error.
pub(crate) type PreAllocationSiteAttempt =
    fn(
        SelectedProgramRef<'_>,
        CandidateSite,
        &PreAllocationAdmissionContext<'_>,
    ) -> Result<CandidateAttempt, OptimizedPreAllocationCustodyError>;

impl PreAllocationRuleAdmission {
    pub(crate) const fn new(
        surface: PreAllocationCandidateSurface,
        attempt: PreAllocationSiteAttempt,
    ) -> Self {
        Self { surface, attempt }
    }

    /// Whether the row's candidate surface covers `instruction` at `position`.
    pub(crate) fn admits(
        &self,
        position: CandidatePosition,
        instruction: &SelectedInstruction,
    ) -> bool {
        self.surface.admits(position, instruction)
    }

    pub(crate) fn attempt(&self) -> PreAllocationSiteAttempt {
        self.attempt
    }
}

/// The per-run facts every site attempt shares: the admitted target
/// environment and the per-pass budget. The machine-effect catalog the
/// descriptor families resolve their declarations in is a pure function of the
/// target and its constraint catalog, so each family builds it — through its
/// own already-cached [`validated_machine_effect_catalog`] lookup — where it
/// binds its declarations rather than through a second cache here.
pub(crate) struct PreAllocationAdmissionContext<'environment> {
    pub(crate) environment: &'environment ValidatedTargetRegisterEnvironment,
    pub(crate) budget: OptimizationWorkBudget,
}

impl<'environment> PreAllocationAdmissionContext<'environment> {
    pub(crate) const fn new(
        environment: &'environment ValidatedTargetRegisterEnvironment,
        budget: OptimizationWorkBudget,
    ) -> Self {
        Self {
            environment,
            budget,
        }
    }
}

enum CandidateDisposition {
    Ready,
    Declined {
        revision: usize,
        candidates: usize,
        declined: usize,
    },
    Retired,
}

struct PendingCandidate {
    family_index: usize,
    site: CandidateSite,
    disposition: CandidateDisposition,
}

impl PendingCandidate {
    fn order(&self) -> (usize, usize, usize, CandidatePosition) {
        (
            self.family_index,
            self.site.function_index,
            self.site.block_index,
            self.site.position,
        )
    }
}

/// One ordered discovery frontier owns every enabled family's candidate sites.
/// A commit invalidates failed eligibility only in the changed function and
/// publishes any new family membership at its instruction. Earlier families
/// retain priority: forwarding a later load must not erase a copy's last reader
/// before copy removal can act. No accepted proposal or dense id is reused.
struct CandidateFrontier {
    pending: Vec<PendingCandidate>,
    function_revisions: Vec<usize>,
    policy: PreAllocationPolicy,
}

impl CandidateFrontier {
    fn new(policy: PreAllocationPolicy, plan: &SelectedInstructionPlan) -> Self {
        let mut frontier = Self {
            pending: Vec::new(),
            function_revisions: vec![1; plan.functions.len()],
            policy,
        };
        for (family_index, row) in PRE_ALLOCATION_RULE_CATALOG.iter().enumerate() {
            if !policy.contains(row.payload().policy()) {
                continue;
            }
            let admission = row.payload().admission();
            for (function_index, function) in plan.functions.iter().enumerate() {
                for (block_index, block) in function.blocks.iter().enumerate() {
                    for (position, instruction) in block.instructions.iter().enumerate() {
                        let site = CandidateSite {
                            function_index,
                            block_index,
                            position: CandidatePosition::Body(position),
                        };
                        if admission.admits(site.position, instruction) {
                            frontier.pending.push(PendingCandidate {
                                family_index,
                                site,
                                disposition: CandidateDisposition::Ready,
                            });
                        }
                    }
                    let site = CandidateSite {
                        function_index,
                        block_index,
                        position: CandidatePosition::Terminator,
                    };
                    if admission.admits(site.position, terminator_instruction(&block.terminator)) {
                        frontier.pending.push(PendingCandidate {
                            family_index,
                            site,
                            disposition: CandidateDisposition::Ready,
                        });
                    }
                }
            }
        }
        frontier
    }

    fn next(&self) -> Option<usize> {
        self.pending
            .iter()
            .position(|candidate| match candidate.disposition {
                CandidateDisposition::Ready => true,
                CandidateDisposition::Declined { revision, .. } => {
                    revision != self.function_revisions[candidate.site.function_index]
                }
                CandidateDisposition::Retired => false,
            })
    }

    fn committed(
        &mut self,
        site: CandidateSite,
        previous_body_count: usize,
        plan: &SelectedInstructionPlan,
    ) -> Result<(), OptimizedPreAllocationCustodyError> {
        let function = plan
            .functions
            .get(site.function_index)
            .ok_or(OptimizedPreAllocationCustodyError::CandidateFrontierMismatch)?;
        let block = function
            .blocks
            .get(site.block_index)
            .ok_or(OptimizedPreAllocationCustodyError::CandidateFrontierMismatch)?;
        let removed = previous_body_count
            .checked_sub(block.instructions.len())
            .filter(|removed| *removed <= 1)
            .ok_or(OptimizedPreAllocationCustodyError::CandidateFrontierMismatch)?;
        let revision = &mut self.function_revisions[site.function_index];
        *revision = revision
            .checked_add(1)
            .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
        for candidate in &mut self.pending {
            if candidate.site.function_index != site.function_index {
                continue;
            }
            if removed != 0 && candidate.site.block_index == site.block_index {
                let CandidatePosition::Body(removed_position) = site.position else {
                    return Err(OptimizedPreAllocationCustodyError::CandidateFrontierMismatch);
                };
                if let CandidatePosition::Body(position) = &mut candidate.site.position {
                    if *position == removed_position {
                        candidate.disposition = CandidateDisposition::Retired;
                        continue;
                    }
                    if *position > removed_position {
                        *position -= removed;
                    }
                }
            }
            let admitted = PRE_ALLOCATION_RULE_CATALOG[candidate.family_index]
                .payload()
                .admission()
                .admits(candidate.site.position, candidate.site.instruction(plan)?);
            if !admitted {
                candidate.disposition = CandidateDisposition::Retired;
            }
        }
        self.pending
            .retain(|candidate| !matches!(candidate.disposition, CandidateDisposition::Retired));
        // Only in-place replacements introduce a new kind. Removal preserves
        // every surviving site's kind while changing its dense ordinal.
        if removed == 0 {
            let instruction = site.instruction(plan)?;
            for (family_index, row) in PRE_ALLOCATION_RULE_CATALOG.iter().enumerate() {
                if !self.policy.contains(row.payload().policy())
                    || !row.payload().admission().admits(site.position, instruction)
                {
                    continue;
                }
                let candidate = PendingCandidate {
                    family_index,
                    site,
                    disposition: CandidateDisposition::Ready,
                };
                if let Err(position) = self
                    .pending
                    .binary_search_by_key(&candidate.order(), PendingCandidate::order)
                {
                    self.pending.insert(position, candidate);
                }
            }
        }
        Ok(())
    }
}

macro_rules! retryable_candidate_error {
    ($error:ident, $binding:ident) => {
        $binding @ ($error::WorkBudgetExceeded
            | $error::IdentityOverflow
            | $error::ReplayMismatch)
    };
}

/// Whether the instruction kind is one of the same-block copy removals'
/// source surface: `CopyI64`. Admission decides removability; this is only
/// the source-bound candidate surface, scanned in plan order.
pub(crate) fn is_copy_i64(kind: SelectedInstructionKind) -> bool {
    kind == SelectedInstructionKind::CopyI64
}

/// Whether the instruction kind is one of the retained-store forwarding
/// family's referent loads. Admission decides the byte-exact pair; this is
/// only the source-bound candidate surface, scanned in plan order.
pub(crate) fn is_load_candidate(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::Load64 { .. }
            | selected_instructions::load_width_triple_pattern!(..)
            | SelectedInstructionKind::Load8Indexed
    )
}

/// Whether the instruction kind is one of the carrier extensions the
/// redundant-extension family considers: `ZeroExtend*`/`SignExtend*`.
/// Admission decides redundancy; this is only the source-bound candidate
/// surface, scanned in plan order.
pub(crate) fn is_extension_candidate(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::ZeroExtendU8
            | SelectedInstructionKind::ZeroExtendU16
            | SelectedInstructionKind::ZeroExtendU32
            | SelectedInstructionKind::SignExtendI8
            | SelectedInstructionKind::SignExtendI16
            | SelectedInstructionKind::SignExtendI32
    )
}

/// Whether the instruction kind is a displacement-carrying consumer the
/// address-fold family considers: the referent `Load*` forms, the
/// byte-addressable referent `Store` widths, and `AddressOffset` itself.
/// Admission decides foldability; this is only the source-bound candidate
/// surface, scanned in plan order.
pub(crate) fn is_fold_consumer(kind: SelectedInstructionKind) -> bool {
    match kind {
        selected_instructions::load_width_triple_pattern!(..)
        | SelectedInstructionKind::Load64 { .. }
        | SelectedInstructionKind::AddressOffset { .. } => true,
        SelectedInstructionKind::Store { byte_size, .. } => {
            matches!(byte_size, 1 | 2 | 4 | 8)
        }
        _ => false,
    }
}

/// Whether the instruction kind is a flag-reading `MaterializeBoolean*`
/// the constant-boolean family considers. Admission decides foldability;
/// this is only the source-bound candidate surface, scanned in plan order.
pub(crate) fn is_boolean_reader(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::MaterializeBooleanEqual
            | SelectedInstructionKind::MaterializeBooleanU64LessThan
            | SelectedInstructionKind::MaterializeBooleanI64LessThan
            | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
            | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
    )
}

/// Whether the terminator-carried instruction kind is a flag-reading
/// `ConditionalBranch*` the constant-branch family considers. Admission
/// decides foldability — the terminator/kind pairing, the zero-operand
/// shape, and the flag partition; this is only the source-bound candidate
/// surface, scanned in function/block order.
pub(crate) fn is_branch_reader(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::ConditionalBranchNonZero
            | SelectedInstructionKind::ConditionalBranchU64LessThan
            | SelectedInstructionKind::ConditionalBranchI64LessThan
    )
}

/// Whether the instruction kind is one of the flag-publisher compares the
/// dead-, redundant-, and equivalent-compare families consider:
/// `CompareI64`, `CompareI64Immediate`, or `CompareI64Zero`. Admission
/// decides removability — the canonical operand shape, the constraint
/// surface, the unit-wise audits, and the roster exclusions; this is only
/// the source-bound candidate surface, scanned in function/block/
/// instruction order.
pub(crate) fn is_compare_publisher(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::CompareI64
            | SelectedInstructionKind::CompareI64Immediate { .. }
            | SelectedInstructionKind::CompareI64Zero
    )
}

/// The distinct condition-state units a compare-shaped instruction
/// publishes — the `implicit_defs`/`clobbers` union size the compare
/// families' measured-step contracts charge per published unit.
fn published_flag_units(instruction: &SelectedInstruction) -> usize {
    instruction
        .implicit_defs
        .iter()
        .chain(instruction.clobbers.iter())
        .collect::<BTreeSet<&register_model::RegisterUnitId>>()
        .len()
}

/// The definition-chain length one `Use` operand adds to the joint
/// measure: the number of in-block definitions reached by following each
/// defining instruction's own operand-0 `Use` backward from the
/// operand's register. An address fold rebinds the consumer to the
/// producer's earlier base and skips the producer's node in this chain —
/// and in every chain routed through the consumer's result — a
/// copied-call-operand reroute drops exactly the copy's node from every
/// rebound call operand's chain, while a copy removal erases the copy's
/// node from every rebound chain.
fn operand_chain_length(
    instructions: &[SelectedInstruction],
    consumer_index: usize,
    operand_index: usize,
) -> usize {
    let mut register = match instructions[consumer_index].operands.get(operand_index) {
        Some(operand) if operand.access == RegisterOperandAccess::Use => operand.virtual_register,
        _ => return 0,
    };
    let mut position = consumer_index;
    let mut links = 0usize;
    while let Some(definition) = instructions[..position].iter().rposition(|instruction| {
        instruction.operands.iter().any(|operand| {
            operand.access != RegisterOperandAccess::Use && operand.virtual_register == register
        })
    }) {
        links += 1;
        match instructions[definition].operands.first() {
            Some(next) if next.access == RegisterOperandAccess::Use => {
                register = next.virtual_register;
                position = definition;
            }
            _ => break,
        }
    }
    links
}

/// Whether the instruction kind is one of the direct internal calls the
/// copied-call-operand family considers: `CallUnit`, `CallScalar`, or
/// `CallAggregate`, whose leading `Use` operands carry the argument
/// roster. Admission decides the pair; this is only the source-bound
/// candidate surface and the joint measure's call term.
pub(crate) fn is_call_consumer(kind: SelectedInstructionKind) -> bool {
    matches!(kind, selected_instructions::selected_call_pattern!(..))
}

/// The joint measure a committed step must strictly drop: the plan's
/// virtual registers and retained memory access rows, plus its remaining extension candidates, plus the
/// summed operand-0 definition-chain lengths of the fold-shaped consumers,
/// plus the summed per-`Use`-operand definition-chain lengths of the
/// direct internal calls, plus every flag unit the remaining
/// `MaterializeBoolean*` readers still observe, plus every implicit use
/// the remaining `ConditionalBranch*` terminators carry, plus one per
/// remaining flag-publisher compare. A copy removal drops the copy and
/// its destination's roster row and can only shorten chains; an
/// extension removal retires one extension while keeping every register
/// and chain; an address fold keeps both but skips a producer node in
/// its consumer's chain; a copied-call-operand reroute keeps the copy,
/// its register, and the call published but skips the copy's node in
/// every rebound operand's chain; a constant-boolean fold keeps
/// registers, chains, and instruction count while dropping the flag uses
/// the folded reader carried; a constant-branch fold keeps registers,
/// chains, and instruction count while the `Jump` row that replaces the
/// branch carries none of its flag uses; a compare removal drops one
/// compare-publisher term while a compare carries no register definitions
/// or flag uses the other terms charge.
fn measure(plan: &SelectedInstructionPlan, virtual_registers: usize) -> usize {
    let mut total = virtual_registers
        + plan
            .functions
            .iter()
            .map(|function| function.memory_accesses.len())
            .sum::<usize>();
    for function in &plan.functions {
        for block in &function.blocks {
            for (index, instruction) in block.instructions.iter().enumerate() {
                if is_extension_candidate(instruction.kind) {
                    total += 1;
                }
                if is_fold_consumer(instruction.kind) {
                    total += operand_chain_length(&block.instructions, index, 0);
                }
                if is_call_consumer(instruction.kind) {
                    for operand_index in 0..instruction.operands.len() {
                        total += operand_chain_length(&block.instructions, index, operand_index);
                    }
                }
                if is_boolean_reader(instruction.kind) {
                    total += instruction.implicit_uses.len();
                }
                if is_compare_publisher(instruction.kind) {
                    total += 1;
                }
            }
            let terminator = terminator_instruction(&block.terminator);
            if is_branch_reader(terminator.kind) {
                total += terminator.implicit_uses.len();
            }
        }
    }
    total
}

/// Admit the current copy. Ordinary legality refusals decline the site; budget,
/// identity and validator failures abort the run. Published measured-step costs
/// charge the admission audit and, on commit, its independent validation.
fn candidate_context<'a>(
    current: &'a SelectedProgramRef<'a>,
    site: CandidateSite,
) -> Result<
    (usize, &'a SelectedFunction, &'a SelectedInstruction),
    OptimizedPreAllocationCustodyError,
> {
    let function_index = site.function_index;
    let function = &current.selected_plan().functions[function_index];
    let instruction = site.instruction(current.selected_plan())?;
    Ok((function_index, function, instruction))
}

pub(crate) fn copy_removal_candidate(
    current: SelectedProgramRef<'_>,
    site: CandidateSite,
    context: &PreAllocationAdmissionContext<'_>,
) -> Result<CandidateAttempt, OptimizedPreAllocationCustodyError> {
    let (function_index, function, instruction) = candidate_context(&current, site)?;
    let audit_cost = copy_removal_measured_steps(current.selected_plan(), function)
        .map_err(OptimizedPreAllocationCustodyError::CopyRemoval)?;
    let charged = charge(0, audit_cost)?;
    match remove_selected_copy(
        &current,
        function_index,
        instruction.id,
        context.environment,
        context.budget,
    ) {
        Ok(removal) => Ok(CandidateAttempt::applied(
            ValidatedPreAllocationTransformation::CopyRemoval(removal),
            0,
            1,
            charge(charged, audit_cost)?,
        )),
        Err(retryable_candidate_error!(CopyRemovalError, error)) => {
            Err(OptimizedPreAllocationCustodyError::CopyRemoval(error))
        }
        Err(_) => Ok(CandidateAttempt::clean(1, 1, charged)),
    }
}

/// Admit the current extension under its own measured-step contract.
pub(crate) fn extension_removal_candidate(
    current: SelectedProgramRef<'_>,
    site: CandidateSite,
    context: &PreAllocationAdmissionContext<'_>,
) -> Result<CandidateAttempt, OptimizedPreAllocationCustodyError> {
    let (function_index, function, instruction) = candidate_context(&current, site)?;
    let audit_cost = redundant_extension_measured_steps(current.selected_plan(), function)
        .map_err(OptimizedPreAllocationCustodyError::RedundantExtension)?;
    let charged = charge(0, audit_cost)?;
    match remove_selected_redundant_extension(
        &current,
        function_index,
        instruction.id,
        context.environment,
        context.budget,
    ) {
        Ok(removal) => Ok(CandidateAttempt::applied(
            ValidatedPreAllocationTransformation::RedundantExtension(removal),
            0,
            1,
            charge(charged, audit_cost)?,
        )),
        Err(retryable_candidate_error!(RedundantExtensionError, error)) => Err(
            OptimizedPreAllocationCustodyError::RedundantExtension(error),
        ),
        Err(_) => Ok(CandidateAttempt::clean(1, 1, charged)),
    }
}

/// Admit the current displacement-carrying consumer.
pub(crate) fn address_fold_candidate(
    current: SelectedProgramRef<'_>,
    site: CandidateSite,
    context: &PreAllocationAdmissionContext<'_>,
) -> Result<CandidateAttempt, OptimizedPreAllocationCustodyError> {
    let (function_index, function, instruction) = candidate_context(&current, site)?;
    let audit_cost = address_fold_measured_steps(current.selected_plan(), function)
        .map_err(OptimizedPreAllocationCustodyError::AddressFold)?;
    let charged = charge(0, audit_cost)?;
    match fold_selected_address(
        &current,
        function_index,
        instruction.id,
        context.environment,
        context.budget,
    ) {
        Ok(removal) => Ok(CandidateAttempt::applied(
            ValidatedPreAllocationTransformation::AddressFold(removal),
            0,
            1,
            charge(charged, audit_cost)?,
        )),
        Err(retryable_candidate_error!(AddressFoldError, error)) => {
            Err(OptimizedPreAllocationCustodyError::AddressFold(error))
        }
        Err(_) => Ok(CandidateAttempt::clean(1, 1, charged)),
    }
}

/// Admit the current flag reader. The target flag universe bounds the reached
/// compare's definitions in the pre-admission work charge.
pub(crate) fn constant_boolean_candidate(
    current: SelectedProgramRef<'_>,
    site: CandidateSite,
    context: &PreAllocationAdmissionContext<'_>,
) -> Result<CandidateAttempt, OptimizedPreAllocationCustodyError> {
    let (function_index, function, instruction) = candidate_context(&current, site)?;
    let (flag_universe, materialize_surface) = constant_boolean_surface_sizes(context.environment)
        .map_err(OptimizedPreAllocationCustodyError::ConstantBoolean)?;
    // The condition-materialization pair descriptors admit the fold through
    // the environment's own validated catalog — a pure function of the target
    // and the constraint catalog, so the custody replay re-running this pass
    // binds the same declarations.
    let effect_catalog = validated_machine_effect_catalog(
        context.environment.target(),
        context.environment.constraints(),
    )
    .map_err(OptimizedPreAllocationCustodyError::MachineEffects)?;
    let (successors, _) = adjacency(function);
    let audit_cost = constant_boolean_measured_steps(
        current.selected_plan(),
        function,
        &successors,
        instruction.implicit_uses.len(),
        flag_universe,
        materialize_surface,
        flag_universe,
    )
    .map_err(OptimizedPreAllocationCustodyError::ConstantBoolean)?;
    let charged = charge(0, audit_cost)?;
    match fold_selected_constant_boolean(
        &current,
        function_index,
        instruction.id,
        context.environment,
        &effect_catalog,
        context.budget,
    ) {
        Ok(fold) => Ok(CandidateAttempt::applied(
            ValidatedPreAllocationTransformation::ConstantBoolean(fold),
            0,
            1,
            charge(charged, audit_cost)?,
        )),
        Err(retryable_candidate_error!(ConstantBooleanError, error)) => {
            Err(OptimizedPreAllocationCustodyError::ConstantBoolean(error))
        }
        Err(_) => Ok(CandidateAttempt::clean(1, 1, charged)),
    }
}

/// Admit the current branch terminator with the target flag-universe bound.
pub(crate) fn constant_branch_candidate(
    current: SelectedProgramRef<'_>,
    site: CandidateSite,
    context: &PreAllocationAdmissionContext<'_>,
) -> Result<CandidateAttempt, OptimizedPreAllocationCustodyError> {
    let (function_index, function, instruction) = candidate_context(&current, site)?;
    let (flag_universe, jump_surface) = constant_branch_surface_sizes(context.environment)
        .map_err(OptimizedPreAllocationCustodyError::ConstantBranch)?;
    // The decided-branch pair descriptors admit the control-flow relationship
    // through the environment's own validated catalog — a pure function of
    // the target and the constraint catalog, so the custody replay re-running
    // this pass binds the same declarations.
    let effect_catalog = validated_machine_effect_catalog(
        context.environment.target(),
        context.environment.constraints(),
    )
    .map_err(OptimizedPreAllocationCustodyError::MachineEffects)?;
    let (successors, _) = adjacency(function);
    let audit_cost = constant_branch_measured_steps(
        current.selected_plan(),
        function,
        &successors,
        instruction.implicit_uses.len(),
        flag_universe,
        jump_surface,
        flag_universe,
    )
    .map_err(OptimizedPreAllocationCustodyError::ConstantBranch)?;
    let charged = charge(0, audit_cost)?;
    match fold_selected_constant_branch(
        &current,
        function_index,
        instruction.id,
        context.environment,
        &effect_catalog,
        context.budget,
    ) {
        Ok(fold) => Ok(CandidateAttempt::applied(
            ValidatedPreAllocationTransformation::ConstantBranch(fold),
            0,
            1,
            charge(charged, audit_cost)?,
        )),
        Err(retryable_candidate_error!(ConstantBranchError, error)) => {
            Err(OptimizedPreAllocationCustodyError::ConstantBranch(error))
        }
        Err(_) => Ok(CandidateAttempt::clean(1, 1, charged)),
    }
}

/// Admit the current dead-compare proposal, charging its published flag units.
pub(crate) fn dead_compare_candidate(
    current: SelectedProgramRef<'_>,
    site: CandidateSite,
    context: &PreAllocationAdmissionContext<'_>,
) -> Result<CandidateAttempt, OptimizedPreAllocationCustodyError> {
    let (function_index, function, instruction) = candidate_context(&current, site)?;
    let audit_cost = dead_compare_measured_steps(
        current.selected_plan(),
        function,
        published_flag_units(instruction),
    )
    .map_err(OptimizedPreAllocationCustodyError::DeadCompare)?;
    let charged = charge(0, audit_cost)?;
    match remove_dead_compare(
        &current,
        function_index,
        instruction.id,
        context.environment,
        context.budget,
    ) {
        Ok(removal) => Ok(CandidateAttempt::applied(
            ValidatedPreAllocationTransformation::DeadCompare(removal),
            0,
            1,
            charge(charged, audit_cost)?,
        )),
        Err(retryable_candidate_error!(DeadCompareError, error)) => {
            Err(OptimizedPreAllocationCustodyError::DeadCompare(error))
        }
        Err(_) => Ok(CandidateAttempt::clean(1, 1, charged)),
    }
}

/// Admit the current redundant-compare proposal.
pub(crate) fn redundant_compare_candidate(
    current: SelectedProgramRef<'_>,
    site: CandidateSite,
    context: &PreAllocationAdmissionContext<'_>,
) -> Result<CandidateAttempt, OptimizedPreAllocationCustodyError> {
    let (function_index, function, instruction) = candidate_context(&current, site)?;
    let (successors, _) = adjacency(function);
    let audit_cost = redundant_compare_measured_steps(
        current.selected_plan(),
        function,
        &successors,
        published_flag_units(instruction),
    )
    .map_err(OptimizedPreAllocationCustodyError::RedundantCompare)?;
    let charged = charge(0, audit_cost)?;
    match remove_redundant_compare(
        &current,
        function_index,
        instruction.id,
        context.environment,
        context.budget,
    ) {
        Ok(removal) => Ok(CandidateAttempt::applied(
            ValidatedPreAllocationTransformation::RedundantCompare(removal),
            0,
            1,
            charge(charged, audit_cost)?,
        )),
        Err(retryable_candidate_error!(RedundantCompareError, error)) => {
            Err(OptimizedPreAllocationCustodyError::RedundantCompare(error))
        }
        Err(_) => Ok(CandidateAttempt::clean(1, 1, charged)),
    }
}

/// Admit the current equivalent-compare proposal.
pub(crate) fn equivalent_compare_candidate(
    current: SelectedProgramRef<'_>,
    site: CandidateSite,
    context: &PreAllocationAdmissionContext<'_>,
) -> Result<CandidateAttempt, OptimizedPreAllocationCustodyError> {
    let (function_index, function, instruction) = candidate_context(&current, site)?;
    let (successors, _) = adjacency(function);
    let audit_cost = equivalent_compare_measured_steps(
        current.selected_plan(),
        function,
        &successors,
        published_flag_units(instruction),
    )
    .map_err(OptimizedPreAllocationCustodyError::EquivalentCompare)?;
    let charged = charge(0, audit_cost)?;
    match remove_equivalent_compare(
        &current,
        function_index,
        instruction.id,
        context.environment,
        context.budget,
    ) {
        Ok(removal) => Ok(CandidateAttempt::applied(
            ValidatedPreAllocationTransformation::EquivalentCompare(removal),
            0,
            1,
            charge(charged, audit_cost)?,
        )),
        Err(retryable_candidate_error!(EquivalentCompareError, error)) => {
            Err(OptimizedPreAllocationCustodyError::EquivalentCompare(error))
        }
        Err(_) => Ok(CandidateAttempt::clean(1, 1, charged)),
    }
}

/// One measured-step charge against the per-pass budget: every
/// accumulation shares the same overflow refusal.
fn charge(left: u64, right: u64) -> Result<u64, OptimizedPreAllocationCustodyError> {
    left.checked_add(right)
        .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)
}

/// The measured work of one discovery pass. `rule_evaluations` counts the
/// candidates the pass reached; `validation_steps` carries the summed
/// measured-step cost each admit — and, for a commit, the independent
/// validate — charged against the per-pass budget.
fn pass_usage(
    evaluated: usize,
    validation_steps: u64,
    commits: usize,
) -> Result<OptimizationWorkUsage, OptimizedPreAllocationCustodyError> {
    let count = |value: usize| {
        u64::try_from(value).map_err(|_| OptimizedPreAllocationCustodyError::WorkOverflow)
    };
    let counted = count(evaluated)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: counted,
        candidates: counted,
        validation_steps,
        commits: count(commits)?,
        iterations: 0,
    })
}

pub(super) fn add_usage(
    left: OptimizationWorkUsage,
    right: OptimizationWorkUsage,
) -> Result<OptimizationWorkUsage, OptimizedPreAllocationCustodyError> {
    crate::work_accounting::combine(left, right)
        .map_err(|_| OptimizedPreAllocationCustodyError::WorkOverflow)
}

pub(super) fn ensure_pre_allocation_budget(
    usage: OptimizationWorkUsage,
    budget: OptimizationWorkBudget,
) -> Result<(), OptimizedPreAllocationCustodyError> {
    if usage.within(budget) {
        Ok(())
    } else {
        Err(
            OptimizedPreAllocationCustodyError::PreAllocationBudgetExceeded {
                required: usage,
                budget,
            },
        )
    }
}

/// Rebuild the analyses the committed step must carry: a transformation may
/// not leave an illegal program behind, so the transformed plan re-stages
/// liveness, ranges, and legality before the step is admitted.
fn complete_transformation(
    transformation: ValidatedPreAllocationTransformation,
    declined: usize,
    evaluated: usize,
    source: &StagedOptimizedAllocationLegality,
) -> Result<StagedOptimizedPreAllocationStep, OptimizedPreAllocationCustodyError> {
    let environment = source.register_environment();
    let liveness =
        analyze_liveness(&transformation).map_err(OptimizedPreAllocationCustodyError::Liveness)?;
    let ranges = analyze_live_ranges(&transformation, &liveness)
        .map_err(OptimizedPreAllocationCustodyError::LiveRanges)?;
    let legality = analyze_allocation_legality(
        &ranges,
        source.allocator_availability(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedPreAllocationCustodyError::AllocationLegality)?;
    let transition_count = legality.receipt().entry_transition_count();
    if transition_count != 0 {
        return Err(OptimizedPreAllocationCustodyError::RemainingTransitions {
            count: transition_count,
        });
    }
    Ok(StagedOptimizedPreAllocationStep {
        transformation,
        liveness,
        ranges,
        legality,
        declined,
        evaluated,
    })
}

/// The position of the last in-block definition of `register` ahead of
use super::super::operand_probes::last_definition_before;

/// Admit the current load. Memory-audit work remains charged against the
/// current plan; no accepted proposal survives a commit.
pub(crate) fn stored_load_forwarding_candidate(
    current: SelectedProgramRef<'_>,
    site: CandidateSite,
    context: &PreAllocationAdmissionContext<'_>,
) -> Result<CandidateAttempt, OptimizedPreAllocationCustodyError> {
    let function_index = site.function_index;
    let instruction = site.instruction(current.selected_plan())?;
    let surface = current
        .selected_plan()
        .functions
        .iter()
        .try_fold(1u64, |total, function| {
            let members = function.blocks.iter().try_fold(0u64, |total, block| {
                total
                    .checked_add(block.instructions.len() as u64)?
                    .checked_add(1)
            })?;
            total
                .checked_add(members)?
                .checked_add(function.memory_accesses.len() as u64)?
                .checked_add(function.virtual_registers.len() as u64)
        })
        .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
    let audit_cost = surface
        .checked_mul(surface)
        .and_then(|value| value.checked_mul(surface))
        .and_then(|value| value.checked_mul(2))
        .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
    let charged = charge(0, audit_cost)?;
    if charged > context.budget.validation_steps() {
        return Err(OptimizedPreAllocationCustodyError::StoredLoadForwarding(
            StoredLoadForwardingError::WorkBudgetExceeded,
        ));
    }
    match forward_selected_stored_load(
        &current,
        function_index,
        instruction.id,
        context.environment,
        context.budget,
    ) {
        Ok(forwarding) => Ok(CandidateAttempt::applied(
            ValidatedPreAllocationTransformation::StoredLoadForwarding(forwarding),
            0,
            1,
            charged,
        )),
        Err(retryable_candidate_error!(StoredLoadForwardingError, error)) => Err(
            OptimizedPreAllocationCustodyError::StoredLoadForwarding(error),
        ),
        Err(_) => Ok(CandidateAttempt::clean(1, 1, charged)),
    }
}

/// Try current producer/call pairs in operand order, once per distinct
/// producer. Renumbered dense identities are obtained from this attempt's
/// plan only.
pub(crate) fn copied_call_operand_candidate(
    current: SelectedProgramRef<'_>,
    site: CandidateSite,
    context: &PreAllocationAdmissionContext<'_>,
) -> Result<CandidateAttempt, OptimizedPreAllocationCustodyError> {
    let function_index = site.function_index;
    let function = &current.selected_plan().functions[function_index];
    let block = &function.blocks[site.block_index];
    let instruction = site.instruction(current.selected_plan())?;
    let CandidatePosition::Body(position) = site.position else {
        return Err(OptimizedPreAllocationCustodyError::CandidateFrontierMismatch);
    };
    // The copied-call-operand pair descriptors admit the retained-call
    // relationship through the environment's own validated catalog — a pure
    // function of the target and the constraint catalog, so the custody
    // replay re-running this pass binds the same declarations.
    let effect_catalog = validated_machine_effect_catalog(
        context.environment.target(),
        context.environment.constraints(),
    )
    .map_err(OptimizedPreAllocationCustodyError::MachineEffects)?;
    let audit_cost = copied_call_operand_measured_steps(current.selected_plan(), function, block)
        .map_err(OptimizedPreAllocationCustodyError::CopiedCallOperand)?;
    let mut evaluated = 0usize;
    let mut declined = 0usize;
    let mut validation_steps = 0u64;
    let mut tried = BTreeSet::new();
    for operand in instruction
        .operands
        .iter()
        .filter(|operand| operand.access == RegisterOperandAccess::Use)
    {
        let Some(producer_index) =
            last_definition_before(&block.instructions, position, operand.virtual_register)
        else {
            continue;
        };
        let producer = &block.instructions[producer_index];
        if producer.kind != SelectedInstructionKind::CopyI64 || !tried.insert(producer.id) {
            continue;
        }
        evaluated += 1;
        validation_steps = charge(validation_steps, audit_cost)?;
        match fold_selected_copied_call_operand(
            &current,
            function_index,
            producer.id,
            instruction.id,
            context.environment,
            &effect_catalog,
            context.budget,
        ) {
            Ok(reroute) => {
                validation_steps = charge(validation_steps, audit_cost)?;
                return Ok(CandidateAttempt::applied(
                    ValidatedPreAllocationTransformation::CopiedCallOperand(reroute),
                    declined,
                    evaluated,
                    validation_steps,
                ));
            }
            Err(retryable_candidate_error!(CopiedCallOperandError, error)) => {
                return Err(OptimizedPreAllocationCustodyError::CopiedCallOperand(error));
            }
            Err(_) => {
                declined += 1;
            }
        }
    }
    Ok(CandidateAttempt::clean(
        declined,
        evaluated,
        validation_steps,
    ))
}

/// Reach the canonical joint fixed point from one discovery frontier. Executor
/// and independent boundary replay each own their frontier and admission work.
fn run_passes(
    source: &StagedOptimizedAllocationLegality,
    policy: PreAllocationPolicy,
    iteration_bound: usize,
) -> Result<
    (
        Vec<StagedOptimizedPreAllocationStep>,
        StagedOptimizedPreAllocationAttempt,
        OptimizationWorkUsage,
    ),
    OptimizedPreAllocationCustodyError,
> {
    let budget = source.budget_per_pass();
    let context = PreAllocationAdmissionContext::new(source.register_environment(), budget);
    // Iterations meter discovery sweeps. Candidate evaluation and commit work
    // have their own axes; changing a site does not rediscover the whole plan.
    let mut usage = OptimizationWorkUsage {
        iterations: 1,
        ..OptimizationWorkUsage::default()
    };
    ensure_pre_allocation_budget(usage, budget)?;
    let mut steps = Vec::new();
    let mut previous_measure = iteration_bound;
    let mut current = SelectedProgramRef::new(source.selected());
    let mut frontier = CandidateFrontier::new(policy, current.selected_plan());
    while let Some(position) = frontier.next() {
        let pending = &frontier.pending[position];
        let site = pending.site;
        let previous_body_count = current.selected_plan().functions[site.function_index].blocks
            [site.block_index]
            .instructions
            .len();
        let admission = PRE_ALLOCATION_RULE_CATALOG[pending.family_index]
            .payload()
            .admission();
        let attempt = (admission.attempt())(current, site, &context)?;
        match attempt {
            CandidateAttempt::Clean {
                declined,
                evaluated,
                validation_steps,
            } => {
                usage = add_usage(usage, pass_usage(evaluated, validation_steps, 0)?)?;
                ensure_pre_allocation_budget(usage, budget)?;
                frontier.pending[position].disposition = CandidateDisposition::Declined {
                    revision: frontier.function_revisions[site.function_index],
                    candidates: evaluated,
                    declined,
                };
            }
            CandidateAttempt::Applied {
                transformation,
                declined,
                evaluated,
                validation_steps,
            } => {
                if steps.len() >= iteration_bound {
                    return Err(
                        OptimizedPreAllocationCustodyError::PreAllocationIterationBoundExceeded {
                            bound: iteration_bound,
                        },
                    );
                }
                usage = add_usage(usage, pass_usage(evaluated, validation_steps, 1)?)?;
                ensure_pre_allocation_budget(usage, budget)?;
                let step = complete_transformation(transformation, declined, evaluated, source)?;
                let current_measure = measure(
                    step.transformation.transformed(),
                    step.legality.receipt().virtual_register_count(),
                );
                if current_measure >= previous_measure {
                    return Err(
                        OptimizedPreAllocationCustodyError::PreAllocationMeasureMismatch {
                            previous: previous_measure,
                            current: current_measure,
                        },
                    );
                }
                previous_measure = current_measure;
                frontier.committed(site, previous_body_count, step.transformation.transformed())?;
                steps.push(step);
                current = SelectedProgramRef::new(
                    &steps
                        .last()
                        .ok_or(OptimizedPreAllocationCustodyError::CandidateFrontierMismatch)?
                        .transformation,
                );
            }
        }
    }
    let mut candidates = 0usize;
    let mut declined = 0usize;
    for pending in &frontier.pending {
        let CandidateDisposition::Declined {
            revision,
            candidates: remaining,
            declined: refusals,
        } = pending.disposition
        else {
            return Err(OptimizedPreAllocationCustodyError::CandidateFrontierMismatch);
        };
        if revision != frontier.function_revisions[pending.site.function_index] {
            return Err(OptimizedPreAllocationCustodyError::CandidateFrontierMismatch);
        }
        candidates = candidates
            .checked_add(remaining)
            .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
        declined = declined
            .checked_add(refusals)
            .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
    }
    let source_selected = current.selected_identity();
    Ok((
        steps,
        StagedOptimizedPreAllocationAttempt {
            source_selected,
            candidates,
            declined,
        },
        usage,
    ))
}

pub(super) fn execute_pre_allocation_optimizations(
    source: StagedOptimizedAllocationLegality,
    selections: OptimizationSelections,
    pre_allocation_selections: OptimizationSelections,
    policy: PreAllocationPolicy,
) -> Result<StagedPreAllocationOptimizationRun, OptimizedPreAllocationCustodyError> {
    let upstream = source.custody();
    let budget = source.budget_per_pass();
    let iteration_bound = measure(
        source.selected().selected_plan(),
        source.legality().receipt().virtual_register_count(),
    );
    let (steps, attempt, usage) = run_passes(&source, policy, iteration_bound)?;
    let custody = pre_allocation_custody_receipt(
        upstream,
        &selections,
        &pre_allocation_selections,
        policy,
        &source,
        &steps,
        &attempt,
        budget,
        usage,
        iteration_bound,
    );
    Ok(StagedPreAllocationOptimizationRun {
        source,
        selections,
        pre_allocation_selections,
        policy,
        steps,
        attempt,
        custody,
    })
}

/// Independently replay the retained run: re-resolve the exact selection,
/// re-run discovery and admission over the source plan, re-stage every
/// step's analyses, and require the replayed chain and rebuilt custody
/// receipt to equal the retained ones field for field.
pub(crate) fn validate_pre_allocation_optimization_custody(
    run: &StagedPreAllocationOptimizationRun,
) -> Result<StagedPreAllocationOptimizationCustodyReceipt, OptimizedPreAllocationCustodyError> {
    let upstream = run.source.custody();
    let expected_budget = run.source.budget_per_pass();
    let pre_allocation = run
        .selections
        .project_phase(optimization_core::OptimizationExecutionPhase::PreAllocation);
    let (projected, policy) = resolve_pre_allocation_rules(&pre_allocation)?;
    if run.selections != *run.source.selections()
        || run.custody.selections != run.source.selections().identity()
        || run.custody.budget != expected_budget
    {
        return Err(OptimizedPreAllocationCustodyError::SelectionProjectionMismatch);
    }
    if projected != run.pre_allocation_selections
        || policy != run.policy
        || run.custody.policy != policy
        || run.custody.pre_allocation_selections != run.pre_allocation_selections.identity()
    {
        return Err(OptimizedPreAllocationCustodyError::SelectionProjectionMismatch);
    }
    let iteration_bound = measure(
        run.source.selected().selected_plan(),
        run.source.legality().receipt().virtual_register_count(),
    );
    let (replayed, terminal, usage) = run_passes(&run.source, policy, iteration_bound)?;
    if replayed != run.steps {
        return Err(OptimizedPreAllocationCustodyError::StepMismatch { step: 0 });
    }
    if terminal != run.attempt {
        return Err(OptimizedPreAllocationCustodyError::TerminalAttemptMismatch);
    }
    let receipt = pre_allocation_custody_receipt(
        upstream,
        &run.selections,
        &run.pre_allocation_selections,
        policy,
        &run.source,
        &replayed,
        &terminal,
        expected_budget,
        usage,
        iteration_bound,
    );
    if receipt != run.custody {
        return Err(OptimizedPreAllocationCustodyError::ReceiptMismatch);
    }
    Ok(receipt)
}

#[allow(clippy::too_many_arguments)]
fn pre_allocation_custody_receipt(
    source_receipt: StagedOptimizedAllocationLegalityCustodyReceipt,
    selections: &OptimizationSelections,
    pre_allocation_selections: &OptimizationSelections,
    policy: PreAllocationPolicy,
    source: &StagedOptimizedAllocationLegality,
    steps: &[StagedOptimizedPreAllocationStep],
    attempt: &StagedOptimizedPreAllocationAttempt,
    budget: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    iteration_bound: usize,
) -> StagedPreAllocationOptimizationCustodyReceipt {
    let (final_selected, final_liveness, final_ranges, final_legality, final_vregs) =
        match steps.last() {
            Some(step) => (
                step.transformation.transformed_selected(),
                step.liveness.receipt().identity(),
                step.ranges.receipt().identity(),
                step.legality.receipt().identity(),
                step.legality.receipt().virtual_register_count(),
            ),
            None => (
                source.selected().receipt().identity(),
                source.liveness().receipt().identity(),
                source.ranges().receipt().identity(),
                source.legality().receipt().identity(),
                source.legality().receipt().virtual_register_count(),
            ),
        };
    let mut receipt = StagedPreAllocationOptimizationCustodyReceipt {
        identity: PreAllocationOptimizationCompletionIdentity::from_canonical_bytes(b"pending"),
        source: source_receipt,
        selections: selections.identity(),
        pre_allocation_selections: pre_allocation_selections.identity(),
        policy,
        budget,
        usage,
        iteration_bound,
        removal_count: steps.len(),
        initial_virtual_register_count: source.legality().receipt().virtual_register_count(),
        iterations: steps.iter().map(iteration_receipt).collect(),
        attempt: attempt_receipt(attempt),
        final_selected,
        final_liveness,
        final_ranges,
        final_legality,
        final_virtual_register_count: final_vregs,
    };
    receipt.identity = pre_allocation_completion_identity(&receipt);
    receipt
}

fn iteration_receipt(
    step: &StagedOptimizedPreAllocationStep,
) -> StagedOptimizedPreAllocationIterationReceipt {
    StagedOptimizedPreAllocationIterationReceipt {
        source_selected: step.transformation.source_selected(),
        transformation: step.transformation.identity(),
        function_index: step.transformation.function_index(),
        instruction: step.transformation.instruction(),
        transformed_selected: step.transformation.transformed_selected(),
        fresh_liveness: step.liveness.receipt().identity(),
        fresh_ranges: step.ranges.receipt().identity(),
        fresh_legality: step.legality.receipt().identity(),
        declined: step.declined,
        evaluated: step.evaluated,
    }
}

fn attempt_receipt(
    attempt: &StagedOptimizedPreAllocationAttempt,
) -> StagedOptimizedPreAllocationAttemptReceipt {
    StagedOptimizedPreAllocationAttemptReceipt {
        source_selected: attempt.source_selected,
        candidates: attempt.candidates,
        declined: attempt.declined,
    }
}

/// Canonical identity of the completed pre-allocation run: the upstream
/// custody chain, both selection identities, the admitted policy, measured
/// work, every iteration receipt — including the exact transformation family
/// — the terminal clean sweep, and the final analysis identities are all
/// part of the durable record.
fn pre_allocation_completion_identity(
    receipt: &StagedPreAllocationOptimizationCustodyReceipt,
) -> PreAllocationOptimizationCompletionIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.pre-allocation-optimization-completion.v6\0");
    let source = receipt.source;
    for identity in [
        source.optimization().bytes(),
        source.manifest().bytes(),
        source.register_environment().bytes(),
        source.allocator_availability().bytes(),
        source.selected().bytes(),
        source.liveness().bytes(),
        source.ranges().bytes(),
        source.legality().bytes(),
        receipt.selections.bytes(),
        receipt.pre_allocation_selections.bytes(),
    ] {
        canonical.extend_from_slice(&identity);
    }
    crate::codec_bytes::push_encoded(
        &mut canonical,
        receipt.policy.canonical_bits().to_le_bytes(),
    );
    canonical.extend_from_slice(&receipt.budget.encode());
    canonical.extend_from_slice(&receipt.usage.encode());
    for count in [
        receipt.iteration_bound,
        receipt.removal_count,
        receipt.initial_virtual_register_count,
        receipt.iterations.len(),
    ] {
        encode_count(&mut canonical, count);
    }
    for iteration in &receipt.iterations {
        canonical.extend_from_slice(&iteration.source_selected().bytes());
        match iteration.transformation() {
            crate::PreAllocationTransformationIdentity::CopyRemoval(identity) => {
                canonical.push(1);
                canonical.extend_from_slice(&identity.bytes());
            }
            crate::PreAllocationTransformationIdentity::RedundantExtension(identity) => {
                canonical.push(2);
                canonical.extend_from_slice(&identity.bytes());
            }
            crate::PreAllocationTransformationIdentity::AddressFold(identity) => {
                canonical.push(3);
                canonical.extend_from_slice(&identity.bytes());
            }
            crate::PreAllocationTransformationIdentity::ConstantBoolean(identity) => {
                canonical.push(4);
                canonical.extend_from_slice(&identity.bytes());
            }
            crate::PreAllocationTransformationIdentity::ConstantBranch(identity) => {
                canonical.push(5);
                canonical.extend_from_slice(&identity.bytes());
            }
            crate::PreAllocationTransformationIdentity::DeadCompare(identity) => {
                canonical.push(6);
                canonical.extend_from_slice(&identity.bytes());
            }
            crate::PreAllocationTransformationIdentity::RedundantCompare(identity) => {
                canonical.push(7);
                canonical.extend_from_slice(&identity.bytes());
            }
            crate::PreAllocationTransformationIdentity::EquivalentCompare(identity) => {
                canonical.push(8);
                canonical.extend_from_slice(&identity.bytes());
            }
            crate::PreAllocationTransformationIdentity::CopiedCallOperand(identity) => {
                canonical.push(9);
                canonical.extend_from_slice(&identity.bytes());
            }
            crate::PreAllocationTransformationIdentity::StoredLoadForwarding(identity) => {
                canonical.push(10);
                canonical.extend_from_slice(&identity.bytes());
            }
        }
        encode_count(&mut canonical, iteration.function_index());
        crate::codec_bytes::push_encoded(&mut canonical, iteration.instruction().0.to_le_bytes());
        canonical.extend_from_slice(&iteration.transformed_selected().bytes());
        canonical.extend_from_slice(&iteration.fresh_liveness().bytes());
        canonical.extend_from_slice(&iteration.fresh_ranges().bytes());
        canonical.extend_from_slice(&iteration.fresh_legality().bytes());
        encode_count(&mut canonical, iteration.declined());
        encode_count(&mut canonical, iteration.evaluated());
    }
    canonical.extend_from_slice(&receipt.attempt.source_selected().bytes());
    encode_count(&mut canonical, receipt.attempt.candidates());
    encode_count(&mut canonical, receipt.attempt.declined());
    canonical.extend_from_slice(&receipt.final_selected.bytes());
    canonical.extend_from_slice(&receipt.final_liveness.bytes());
    canonical.extend_from_slice(&receipt.final_ranges.bytes());
    canonical.extend_from_slice(&receipt.final_legality.bytes());
    encode_count(&mut canonical, receipt.final_virtual_register_count);
    PreAllocationOptimizationCompletionIdentity::from_canonical_bytes(&canonical)
}

fn encode_count(canonical: &mut Vec<u8>, count: usize) {
    canonical.extend_from_slice(
        &u64::try_from(count)
            .expect("pre-allocation completion count fits u64")
            .to_le_bytes(),
    );
}
