//! Checked-trees optimization phase: exact opt-in whole-product root
//! selection and unreachable machine-declaration pruning.
//!
//! # Phase contract
//!
//! This module is the Psi-side execution seam for the checked-tree
//! optimization phase (`OptimizationExecutionPhase::CheckedTrees`). It runs
//! strictly AFTER authored checking: the input is a complete, diagnostic-
//! cleared [`CheckedTrees`], so pruning can never hide an invalid authored
//! declaration -- checking, diagnostics, and all fact construction have
//! already committed before this phase sees the program.
//!
//! The opt-in plan is [`CheckedTreeProductPruning`]: an exact, explicit set of
//! product root machines supplied by the coordinator. Nothing is enabled by
//! default, by an optimization level, or by inference; an absent plan is an
//! identity execution with no second pipeline route. Each execution returns a
//! [`CheckedTreeProductSelection`] evidence record carrying the exact roots,
//! the retained/pruned machine rosters in declaration order, and a
//! domain-separated SHA-256 identity, so every target/root-selected Psi
//! product is independently reproducible and joins to its source selection.
//!
//! # Retention bound
//!
//! Only `MachineSupplyMode::CheckedBody` machines are pruning candidates.
//! Requirement slots, top-level requirements, boundary declarations,
//! admission claims, and external realizations are product interface surface:
//! Omega-side provider selection, callback admission, and trust reporting
//! still consume them, so they are always retained.
//!
//! Retention is the transitive closure of the product roots over every
//! machine edge the checked representation enumerates:
//!
//! - checked call facts (`facts.flow.control`) and the typed statement/expression
//!   call nodes they join to, including `machine_arguments`,
//!   `static_requirement_dispatch`, quotient-theorem, and private-layout
//!   applications;
//! - authored `invokes` service targets and `reaches` rows;
//! - `satisfies` `via` expressions, conformance-bound selections, owned-data
//!   initializers, signature/contract proof facts, and const-expression type
//!   positions reachable from each machine;
//! - contract evidence calls, static conformance applications, proof
//!   recursive-component edges, suspension-crossing targets, specialization
//!   machine arguments, and
//!   `AutomaticCleanupMachine`/machine-valued semantic dependencies;
//! - conservative whole-surface retention for provider-selected identities a
//!   root cannot bound: closed-conformance realization machines, boundary
//!   calling plans, nominal machine-use selections, fact-call-projection
//!   targets, boundary adapter realizations, placed-view policy machines,
//!   open-index providers, fused service erasures, and every machine
//!   referenced by a retained non-machine declaration (data, trait,
//!   conformance, proposition, measure, const, operator, domain, wire).
//!
//! # What pruning changes
//!
//! Pruning rebuilds the machine declaration arena and every machine-keyed
//! `Vec` table in the typed sidecars and `CheckFacts`/`FlowFacts` terminal
//! plan families, plus the machine-keyed roster arenas
//! (`flow.control.states`, `service_reaches.machines`) that no `Handle`/`HandleSpan` references into.
//! Arena-resident rows keyed by non-machine identities (expressions, state
//! fact handles, conformances, evidence terms, span-referenced fact rows)
//! remain as retained dead evidence: they are valid checked rows that are
//! unreachable through the pruned declaration surface, and compacting them
//! would remap handles owned by representations outside this crate.
//!
//! The retained set is the closure of the edge relation, so no retained
//! machine references a pruned one by construction; the phase does not
//! re-extract the edges of its own output to confirm it.

use std::collections::HashSet;
use std::sync::Arc;

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use sha2::{Digest, Sha256};
use symbols::SymbolHandle;
use typed_trees::{AuthoredDeclarationSelectionOccurrenceId, TypedTrees};

pub(crate) mod dependencies;
pub(crate) mod rewrite;
mod sibling_evidence;

use dependencies::{MachineIndex, collect_machine_edges, interface_retention_set};
use rewrite::apply_pruning;

const PRODUCT_SELECTION_IDENTITY_DOMAIN: &[u8] = b"omega.psi.checked-tree-product-selection.v1\0";

/// Exact product roots for one checked-tree product selection. Roots are
/// machine symbols chosen by the coordinator (entry/product surface); the
/// phase validates each against the checked program and never invents or
/// infers an additional root.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedTreeProductRoots {
    machines: Vec<SymbolHandle>,
}

impl CheckedTreeProductRoots {
    /// Bind an exact root set. Duplicates are rejected rather than
    /// canonicalized so the selection identity never differs from the
    /// coordinator's input.
    pub fn new(
        machines: impl IntoIterator<Item = SymbolHandle>,
    ) -> Result<Self, CheckedTreeProductRootsError> {
        let mut machines: Vec<SymbolHandle> = machines.into_iter().collect();
        machines.sort_unstable_by_key(|symbol| symbol.arena_index());
        if let Some(duplicate) = machines
            .windows(2)
            .find(|pair| pair[0] == pair[1])
            .map(|pair| pair[0])
        {
            return Err(CheckedTreeProductRootsError::DuplicateRoot(duplicate));
        }
        Ok(Self { machines })
    }

    pub fn machines(&self) -> &[SymbolHandle] {
        &self.machines
    }
}

/// A product root selection that cannot be a canonical input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedTreeProductRootsError {
    DuplicateRoot(SymbolHandle),
}

/// The exact opt-in plan for the checked-tree product-pruning phase.
///
/// This is the phase-local projection a coordinator supplies when the unified
/// build selection names this optimization for the `CheckedTrees` phase. The
/// plan owns no defaults: every retained root is explicit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedTreeProductPruning {
    roots: CheckedTreeProductRoots,
}

impl CheckedTreeProductPruning {
    pub fn new(roots: CheckedTreeProductRoots) -> Self {
        Self { roots }
    }

    pub const fn roots(&self) -> &CheckedTreeProductRoots {
        &self.roots
    }
}

/// Domain-separated commitment to one executed product selection. The digest
/// binds the phase identity, the exact roots, and the retained/pruned machine
/// rosters in declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedTreeProductSelectionIdentity([u8; 32]);

impl CheckedTreeProductSelectionIdentity {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Identity-bearing evidence for one executed checked-tree product
/// selection. The rosters partition the source program's machine table: the
/// same plan against the same checked program always reproduces the same
/// record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTreeProductSelection {
    roots: CheckedTreeProductRoots,
    retained_machines: Vec<SymbolHandle>,
    pruned_machines: Vec<SymbolHandle>,
    identity: CheckedTreeProductSelectionIdentity,
}

impl CheckedTreeProductSelection {
    pub const fn roots(&self) -> &CheckedTreeProductRoots {
        &self.roots
    }

    /// Retained machine symbols in source declaration order.
    pub fn retained_machines(&self) -> &[SymbolHandle] {
        &self.retained_machines
    }

    /// Pruned machine symbols in source declaration order.
    pub fn pruned_machines(&self) -> &[SymbolHandle] {
        &self.pruned_machines
    }

    pub const fn identity(&self) -> CheckedTreeProductSelectionIdentity {
        self.identity
    }
}

/// The pruned checked product plus its identity-bearing selection evidence.
#[derive(Debug)]
pub struct CheckedTreeProductPruningOutcome {
    pub checked: CheckedTrees,
    pub selection: CheckedTreeProductSelection,
}

/// Execute the optional checked-product selection after authored checking.
///
/// An absent root selection is the identity operation, retaining the same
/// shared program and no selection evidence. A requested selection must name
/// at least one canonical root; it cannot silently prune the whole product.
/// The lower-level pruning transform also supports interface-only products,
/// but that is not an executable product selection.
pub fn select_checked_tree_product(
    checked: Arc<CheckedTrees>,
    product_root_machines: Option<Vec<SymbolHandle>>,
) -> Result<(Arc<CheckedTrees>, Option<CheckedTreeProductSelection>), Vec<Diagnostic>> {
    let Some(product_root_machines) = product_root_machines else {
        return Ok((checked, None));
    };
    let roots = CheckedTreeProductRoots::new(product_root_machines).map_err(|error| {
        vec![Diagnostic::error(format!(
            "checked-tree product root selection is not canonical: {error:?}"
        ))]
    })?;
    if roots.machines().is_empty() {
        return Err(vec![Diagnostic::error(
            "CheckedTreeProductPruning requires at least one bound product root; this compilation's build bound none",
        )]);
    }
    let program = Arc::try_unwrap(checked).unwrap_or_else(|shared| (*shared).clone());
    let outcome = prune_checked_tree_product(program, &CheckedTreeProductPruning::new(roots))?;
    Ok((Arc::new(outcome.checked), Some(outcome.selection)))
}

/// Run the checked-tree product-pruning phase.
///
/// The input must be a fully checked program: this phase runs only after all
/// authored source has been parsed, resolved, typed, and checked, and it
/// returns diagnostics rather than ever relaxing that contract. A root that
/// does not name a declared machine is rejected before any mutation.
pub fn prune_checked_tree_product(
    mut checked: CheckedTrees,
    plan: &CheckedTreeProductPruning,
) -> Result<CheckedTreeProductPruningOutcome, Vec<Diagnostic>> {
    let index = MachineIndex::build(&checked.typed);

    let mut diagnostics = Vec::new();
    for root in plan.roots().machines() {
        if !index.machine_symbols.contains(root) {
            diagnostics.push(Diagnostic::error(format!(
                "checked-tree product pruning root does not name a declared machine (symbol #{})",
                root.arena_index()
            )));
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let edges = collect_machine_edges(&checked.typed, &checked.facts, &index);
    let mut retained = interface_retention_set(&checked.typed, &checked.facts, &index);
    retained.extend(plan.roots().machines().iter().copied());

    // Transitive closure over the checked edge relation. The relation is
    // finite (bounded by the machine table) so iteration always converges.
    let mut frontier: Vec<SymbolHandle> = retained.iter().copied().collect();
    while let Some(machine) = frontier.pop() {
        let Some(targets) = edges.get(&machine) else {
            continue;
        };
        for target in targets {
            if retained.insert(*target) {
                frontier.push(*target);
            }
        }
    }

    let pruned: HashSet<SymbolHandle> = index
        .machine_symbols
        .iter()
        .copied()
        .filter(|symbol| !retained.contains(symbol))
        .collect();

    apply_pruning(&mut checked, &retained, &pruned, &index);
    retain_live_family_members(&mut checked.typed);

    let retained_machines: Vec<SymbolHandle> = checked
        .typed
        .machines()
        .iter()
        .map(|machine| machine.symbol)
        .collect();
    // Declaration order is the canonical roster order; the source order is
    // recovered from the pre-pruning declaration sequence embedded in the
    // edge/retention maps (arena order equals declaration order).
    let mut pruned_machines: Vec<SymbolHandle> = pruned.into_iter().collect();
    pruned_machines.sort_unstable_by_key(|symbol| symbol.arena_index());

    let identity = selection_identity(
        plan.roots().machines(),
        &retained_machines,
        &pruned_machines,
    );
    let selection = CheckedTreeProductSelection {
        roots: plan.roots().clone(),
        retained_machines,
        pruned_machines,
        identity,
    };
    Ok(CheckedTreeProductPruningOutcome { checked, selection })
}

/// Domain-separated SHA-256 identity of one executed product selection.
/// Drop every target sibling once checking has committed: their bodies were
/// checked for their own targets, and this realization supplies nothing from
/// them. Transitional: realization-time selection between siblings belongs
/// to Omega (board item PROVIDER-SELECTION-AFTER-TERMINAL), after which every
/// body travels through Terminal Psi instead of being pruned here.
pub fn prune_target_siblings(mut checked: CheckedTrees) -> CheckedTrees {
    let pruned: HashSet<SymbolHandle> = checked
        .typed
        .machines()
        .iter()
        .filter(|machine| machine.target.is_some())
        .map(|machine| machine.symbol)
        .collect();
    if pruned.is_empty() {
        return checked;
    }
    let index = MachineIndex::build(&checked.typed);
    let mut dead = sibling_evidence::DeadEvidence::default();
    for machine in checked
        .typed
        .machines()
        .iter()
        .filter(|machine| pruned.contains(&machine.symbol))
    {
        sibling_evidence::collect_machine_evidence(&checked.typed, machine, &mut dead);
    }
    sibling_evidence::drop_dead_operator_evidence(&mut checked.facts, &dead);
    let retained: HashSet<SymbolHandle> = checked
        .typed
        .machines()
        .iter()
        .map(|machine| machine.symbol)
        .filter(|symbol| !pruned.contains(symbol))
        .collect();
    apply_pruning(&mut checked, &retained, &pruned, &index);
    retain_live_family_members(&mut checked.typed);
    checked
}

/// Keep the target-machine family roster consistent with the pruned machine
/// table: `members` names only bodies this product still carries, and a
/// family whose members all left keeps no row. `canonical` is retained
/// regardless — it is the family's contract-head identity, a symbol that
/// survives machine pruning even when its own body did not.
fn retain_live_family_members(typed: &mut TypedTrees) {
    if typed.target_machine_families.is_empty() {
        return;
    }
    let live: HashSet<SymbolHandle> = typed
        .machines()
        .iter()
        .map(|machine| machine.symbol)
        .collect();
    for family in &mut typed.target_machine_families {
        family
            .members
            .retain(|member| live.contains(&member.machine));
    }
    typed
        .target_machine_families
        .retain(|family| !family.members.is_empty());
}

/// The authored declaration selections recorded inside every target sibling,
/// present or already pruned from `typed`: another target's realization
/// resolves them, so
/// this realization's checking and admission do not report the late-bound
/// ones. Ownership follows source position: a selection belongs to the last
/// top-level declaration that starts before it in its file, which is robust
/// to typing re-homing an occurrence onto an expression no statement reaches.
pub fn target_sibling_selection_occurrences(
    typed: &TypedTrees,
) -> HashSet<AuthoredDeclarationSelectionOccurrenceId> {
    // Projection retains backing declarations. Absence from executable roots
    // alone is not a target-sibling designation: ordinary private projections
    // remove machine roots too, and their unknown occurrences must still fail.
    let owner_is_sibling = |symbol: SymbolHandle| {
        let mut declarations = typed
            .tables
            .machines
            .iter()
            .filter(|(_, machine)| machine.symbol == symbol)
            .map(|(_, machine)| machine);
        let Some(first) = declarations.next() else {
            return false;
        };
        first.target.is_some() && declarations.all(|machine| machine.target == first.target)
    };
    let mut starts: Vec<(usize, usize, SymbolHandle)> = typed
        .symbols
        .child_handles(typed.symbols.root())
        .into_iter()
        .flatten()
        .filter_map(|symbol| {
            let span = typed.symbols.symbol_source_span(symbol)?;
            Some((span.source_id.0, span.span.start, symbol))
        })
        .collect();
    starts.sort_unstable_by_key(|(source, start, _)| (*source, *start));
    typed
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            let span = selection.source_span();
            let owner = starts
                .partition_point(|(source, start, _)| {
                    (*source, *start) <= (span.source_id.0, span.span.start)
                })
                .checked_sub(1)
                .and_then(|index| starts.get(index))
                .filter(|(source, _, _)| *source == span.source_id.0)
                .map(|(_, _, symbol)| *symbol);
            owner.is_some_and(owner_is_sibling)
        })
        .map(|selection| selection.occurrence_id())
        .collect()
}

fn selection_identity(
    roots: &[SymbolHandle],
    retained: &[SymbolHandle],
    pruned: &[SymbolHandle],
) -> CheckedTreeProductSelectionIdentity {
    let mut hasher = Sha256::new();
    hasher.update(PRODUCT_SELECTION_IDENTITY_DOMAIN);
    let mut encode = |symbols: &[SymbolHandle]| {
        hasher.update((symbols.len() as u64).to_le_bytes());
        for symbol in symbols {
            hasher.update(symbol.arena_index().to_le_bytes());
            hasher.update(symbol.generation().to_le_bytes());
        }
    };
    encode(roots);
    encode(retained);
    encode(pruned);
    CheckedTreeProductSelectionIdentity(hasher.finalize().into())
}
