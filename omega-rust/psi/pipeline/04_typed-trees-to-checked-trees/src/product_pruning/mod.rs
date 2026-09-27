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
//!   recursive-component edges, suspension-crossing targets, carry-topology
//!   contained machine targets, specialization machine arguments, and
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
//! (`flow.control.states`, `service_reaches.machines`,
//! `carry.machine_topologies`) that no `Handle`/`HandleSpan` references into.
//! Arena-resident rows keyed by non-machine identities (expressions, state
//! fact handles, conformances, evidence terms, span-referenced fact rows)
//! remain as retained dead evidence: they are valid checked rows that are
//! unreachable through the pruned declaration surface, and compacting them
//! would remap handles owned by representations outside this crate.
//!
//! After rewriting, the phase independently re-extracts the edge relation on
//! the pruned product and fails closed if any retained machine still
//! references a pruned one.

use std::collections::HashSet;

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use sha2::{Digest, Sha256};
use symbols::SymbolHandle;
use typed_trees::{AuthoredDeclarationSelectionOccurrenceId, TypedTrees};

mod dependencies;
mod rewrite;
mod sibling_evidence;

use dependencies::{MachineIndex, collect_machine_edges, interface_retention_set};
use rewrite::{apply_pruning, apply_typed_pruning, validate_pruned_product};

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

    if let Err(failures) = validate_pruned_product(&checked, &retained, &pruned, plan.roots()) {
        return Err(failures
            .into_iter()
            .map(|failure| {
                Diagnostic::error(format!(
                    "checked-tree product pruning internal validation failed: {failure}"
                ))
            })
            .collect());
    }

    let retained_machines: Vec<SymbolHandle> = checked
        .typed
        .machines()
        .iter()
        .map(|machine| machine.symbol)
        .collect();
    let pruned_machines: Vec<SymbolHandle> = {
        let retained_lookup: HashSet<SymbolHandle> = retained_machines.iter().copied().collect();
        // Declaration order is the canonical roster order; the source order
        // is recovered from the pre-pruning declaration sequence embedded in
        // the edge/retention maps (arena order equals declaration order).
        let mut pruned: Vec<SymbolHandle> = pruned.into_iter().collect();
        pruned.sort_unstable_by_key(|symbol| symbol.arena_index());
        debug_assert!(
            pruned
                .iter()
                .all(|symbol| !retained_lookup.contains(symbol))
        );
        pruned
    };

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
pub fn prune_target_siblings(mut checked: CheckedTrees) -> Result<CheckedTrees, Vec<Diagnostic>> {
    let pruned: HashSet<SymbolHandle> = checked
        .typed
        .machines()
        .iter()
        .filter(|machine| machine.target.is_some())
        .map(|machine| machine.symbol)
        .collect();
    if pruned.is_empty() {
        return Ok(checked);
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
    let retained_in_order: Vec<SymbolHandle> = checked
        .typed
        .machines()
        .iter()
        .map(|machine| machine.symbol)
        .filter(|symbol| !pruned.contains(symbol))
        .collect();
    let retained: HashSet<SymbolHandle> = retained_in_order.iter().copied().collect();
    apply_pruning(&mut checked, &retained, &pruned, &index);
    retain_live_family_members(&mut checked.typed);
    let roots =
        CheckedTreeProductRoots::new(retained_in_order.iter().copied()).map_err(|error| {
            vec![Diagnostic::error(format!(
                "target sibling pruning roots: {error:?}"
            ))]
        })?;
    validate_pruned_product(&checked, &retained, &pruned, &roots).map_err(|failures| {
        failures
            .into_iter()
            .map(|failure| {
                Diagnostic::error(format!(
                    "target sibling pruning internal validation failed: {failure}"
                ))
            })
            .collect::<Vec<_>>()
    })?;
    Ok(checked)
}

/// Drop every target sibling from typed trees that will only be checked
/// provisionally (the preliminary pass before build evaluation): their bodies
/// are checked once, in the settled pass. Transitional like
/// `prune_target_siblings`.
pub fn prune_target_siblings_typed(
    typed: &mut TypedTrees,
) -> HashSet<AuthoredDeclarationSelectionOccurrenceId> {
    let pruned: HashSet<SymbolHandle> = typed
        .machines()
        .iter()
        .filter(|machine| machine.target.is_some())
        .map(|machine| machine.symbol)
        .collect();
    if pruned.is_empty() {
        return HashSet::new();
    }
    // The selections recorded inside the pruned bodies stay late-bound in a
    // provisional check; the caller skips them instead of reporting them.
    let dead_occurrences = target_sibling_selection_occurrences(typed);
    let retained: HashSet<SymbolHandle> = typed
        .machines()
        .iter()
        .map(|machine| machine.symbol)
        .filter(|symbol| !pruned.contains(symbol))
        .collect();
    apply_typed_pruning(typed, &retained, &pruned);
    retain_live_family_members(typed);
    dead_occurrences
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
    // A sibling still carries its record; a sibling this program already
    // pruned keeps only its machine symbol.
    let owner_is_sibling = |symbol: SymbolHandle| match typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
    {
        Some(machine) => machine.target.is_some(),
        None => typed.symbols.get(symbol).kind == symbols::SymbolKind::Machine,
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

#[cfg(test)]
mod tests {
    use super::{CheckedTrees, SymbolHandle};
    use crate::CheckedTreeProductPruning;
    use crate::CheckedTreeProductRoots;
    use crate::CheckedTreeProductRootsError;

    use crate::product_pruning::rewrite::validate_pruned_product;

    use crate::prune_checked_tree_product;
    use crate::tests::front_end::checked_program;

    use std::collections::HashSet;

    use typed_trees::TypedTrees;
    use typed_trees::machine::Machine;

    fn checked(source: &str) -> CheckedTrees {
        checked_program(source)
    }

    fn machine_named<'a>(program: &'a TypedTrees, name: &str) -> &'a Machine {
        program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name} must exist"))
    }

    #[test]
    fn duplicate_roots_are_rejected() {
        let root = SymbolHandle::from_arena_index(4);
        let result = CheckedTreeProductRoots::new([root, root]);
        assert_eq!(
            result,
            Err(CheckedTreeProductRootsError::DuplicateRoot(root))
        );
    }

    #[test]
    fn unknown_root_is_rejected_before_pruning() {
        let program = checked(
            r#"
            pub machine main() -> u64 { 7u64 }
            "#,
        );
        let plan = CheckedTreeProductPruning::new(
            CheckedTreeProductRoots::new([SymbolHandle::from_arena_index(999_999)]).expect("roots"),
        );
        let Err(diagnostics) = prune_checked_tree_product(program, &plan) else {
            panic!("an undeclared root must be rejected");
        };
        assert!(diagnostics.iter().any(|diagnostic| {
            format!("{diagnostic:?}").contains("does not name a declared machine")
        }));
    }

    #[test]
    fn selection_identity_binds_the_exact_root_set() {
        let program = checked(
            r#"
            machine other() -> u64 { 1u64 }
            pub machine main() -> u64 { other() }
            pub machine second() -> u64 { other() }
            "#,
        );
        let main = machine_named(&program.typed, "main").symbol;
        let second = machine_named(&program.typed, "second").symbol;

        let main_plan =
            CheckedTreeProductPruning::new(CheckedTreeProductRoots::new([main]).expect("roots"));
        let pair_plan = CheckedTreeProductPruning::new(
            CheckedTreeProductRoots::new([main, second]).expect("roots"),
        );

        let one_root = prune_checked_tree_product(program.clone(), &main_plan).expect("prune");
        let two_roots = prune_checked_tree_product(program, &pair_plan).expect("prune");
        assert_ne!(
            one_root.selection.identity(),
            two_roots.selection.identity()
        );
        assert_eq!(one_root.selection.roots().machines(), &[main]);
        assert_eq!(two_roots.selection.roots().machines(), &[main, second]);
        assert!(two_roots.selection.pruned_machines().is_empty());
    }

    /// A static boundary requirement's plan is keyed by its trait signature,
    /// which is not a machine. Pruning keeps it: only plans of a pruned
    /// machine leave. (Keeping only retained machines dropped the plan behind
    /// every `Console::write_line` call once target-sibling pruning ran on
    /// each compile.)
    #[test]
    fn static_boundary_requirement_plans_survive_pruning() {
        let program = checked(
            r#"
            boundary trait Console {
                machine exit_process(return_code: i32)
                reaches Console;
            }
            data Root {}
            machine Root::enter()
            reaches Console
            {
                Console::exit_process(37);
            }
            machine unused() -> u64 { 2u64 }
            "#,
        );
        let requirement_plans = |program: &CheckedTrees| {
            program
                .facts
                .flow
                .terminal_unit_effects
                .boundary_machines
                .iter()
                .filter(|plan| {
                    program
                        .typed
                        .machines()
                        .iter()
                        .all(|machine| machine.symbol != plan.machine)
                })
                .map(|plan| plan.machine)
                .collect::<Vec<_>>()
        };
        let before = requirement_plans(&program);
        assert_eq!(before.len(), 1, "one static requirement plan");
        let root = machine_named(&program.typed, "Root::enter").symbol;
        let plan =
            CheckedTreeProductPruning::new(CheckedTreeProductRoots::new([root]).expect("roots"));
        let outcome = prune_checked_tree_product(program, &plan).expect("prune");
        assert!(!outcome.selection.pruned_machines().is_empty());
        assert_eq!(requirement_plans(&outcome.checked), before);
    }

    #[test]
    fn typed_pruning_preserves_trait_calling_plans_and_removes_deleted_machine_rows() {
        use typed_trees::typed_trees::{
            BoundaryCallingPlanCommitment, BoundaryCallingPlanIdentity,
        };

        let mut program = checked(
            r#"
            boundary trait Console { machine exit_process(return_code: i32); }
            machine retained() -> u64 { 1u64 }
            machine removed() -> u64 { 2u64 }
            "#,
        );
        let boundary = program
            .typed
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "Console")
            .expect("boundary declaration");
        let requirement = program.typed.trait_machine_signatures(boundary)[0].symbol;
        let retained = machine_named(&program.typed, "retained").symbol;
        let removed = machine_named(&program.typed, "removed").symbol;
        let identity = |requirement_machine| BoundaryCallingPlanIdentity {
            boundary_trait: boundary.symbol,
            boundary_arguments: Vec::new(),
            requirement_machine,
            report_fingerprint: 7,
            commitment: BoundaryCallingPlanCommitment::from_digest([7; 32]),
        };
        let trait_plan = identity(requirement);
        let retained_plan = identity(retained);
        program.typed.boundary_calling_plans =
            vec![trait_plan.clone(), retained_plan.clone(), identity(removed)];

        super::rewrite::apply_typed_pruning(
            &mut program.typed,
            &HashSet::from([retained]),
            &HashSet::from([removed]),
        );

        assert_eq!(
            program.typed.boundary_calling_plans,
            vec![trait_plan, retained_plan],
            "machine pruning preserves exact trait-signature commitments",
        );
        assert_eq!(program.typed.machines().len(), 1);
        assert_eq!(program.typed.machines()[0].symbol, retained);
    }

    #[test]
    fn empty_roots_retain_only_the_interface_surface() {
        let mut program = checked(
            r#"
            machine helper() -> u64 { 40u64 }
            machine unused() -> u64 { 2u64 }
            pub machine main() -> u64 { helper() }
            "#,
        );
        let authored: Vec<SymbolHandle> = program
            .typed
            .machines()
            .iter()
            .map(|machine| machine.symbol)
            .collect();
        let boundary_symbol = {
            let boundary = Machine {
                symbol: SymbolHandle::from_arena_index(
                    authored
                        .iter()
                        .map(|symbol| symbol.arena_index())
                        .max()
                        .expect("a machine symbol must exist")
                        + 1,
                ),
                supply_mode: language_semantics::MachineSupplyMode::Boundary,
                body_is_present: false,
                ..Default::default()
            };
            let symbol = boundary.symbol;
            program.typed.push_machine(boundary);
            symbol
        };
        // The phase entrance rejects an empty root set before running the
        // transform; the transform itself still defines the canonical
        // boundary: with no roots, only interface surface survives.
        let plan = CheckedTreeProductPruning::new(CheckedTreeProductRoots::new([]).expect("roots"));
        let outcome = prune_checked_tree_product(program, &plan).expect("prune");
        assert_eq!(outcome.selection.retained_machines(), &[boundary_symbol]);
        assert_eq!(outcome.selection.pruned_machines(), authored.as_slice());
        assert_eq!(outcome.checked.typed.machines().len(), 1);
        assert_eq!(
            outcome.checked.typed.machines()[0].supply_mode,
            language_semantics::MachineSupplyMode::Boundary
        );
    }

    #[test]
    fn independent_product_validation_rejects_roster_corruption() {
        let program = checked(
            r#"
            machine helper() -> u64 { 40u64 }
            machine unused() -> u64 { 2u64 }
            pub machine main() -> u64 { helper() }
            "#,
        );
        let root = machine_named(&program.typed, "main").symbol;
        let helper = machine_named(&program.typed, "helper").symbol;
        let plan =
            CheckedTreeProductPruning::new(CheckedTreeProductRoots::new([root]).expect("roots"));
        let outcome = prune_checked_tree_product(program, &plan).expect("prune");
        let retained: HashSet<SymbolHandle> = outcome
            .selection
            .retained_machines()
            .iter()
            .copied()
            .collect();
        let pruned: HashSet<SymbolHandle> = outcome
            .selection
            .pruned_machines()
            .iter()
            .copied()
            .collect();

        // A retained roster missing a surviving machine fails the
        // machine-table count and names the unretained symbol.
        let mut shrunk = retained.clone();
        shrunk.remove(&helper);
        let failures = validate_pruned_product(
            &outcome.checked,
            &shrunk,
            &pruned,
            outcome.selection.roots(),
        )
        .expect_err("a shrunk retained roster must reject");
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("pruned machine table holds"))
        );
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("unretained machine symbol"))
        );

        // A foreign symbol inflating the retained roster fails the same count
        // check.
        let mut inflated = retained.clone();
        inflated.insert(SymbolHandle::from_arena_index(999_999));
        let failures = validate_pruned_product(
            &outcome.checked,
            &inflated,
            &pruned,
            outcome.selection.roots(),
        )
        .expect_err("an inflated retained roster must reject");
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("pruned machine table holds"))
        );

        // Marking a retained machine as pruned rejects the still-open edge
        // that names it.
        let mut poisoned = pruned.clone();
        poisoned.insert(helper);
        let failures = validate_pruned_product(
            &outcome.checked,
            &retained,
            &poisoned,
            outcome.selection.roots(),
        )
        .expect_err("a retained machine marked pruned must reject");
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("still references pruned machine"))
        );

        // A root absent from the pruned machine table is rejected.
        let absent_roots =
            CheckedTreeProductRoots::new([SymbolHandle::from_arena_index(424_242)]).expect("roots");
        let failures = validate_pruned_product(&outcome.checked, &retained, &pruned, &absent_roots)
            .expect_err("an absent product root must reject");
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("product root symbol"))
        );
    }

    /// The family roster tracks the pruned machine table: members whose
    /// machines left are dropped and a family with no surviving member keeps
    /// no row, while `canonical` keeps the contract-head symbol even when its
    /// own body was pruned.
    #[test]
    fn target_sibling_pruning_retains_only_live_family_members() {
        let mut program = checked(
            r#"
            demo_target machine helper() -> u64 { 2u64 }
            machine main() -> u64 { 1u64 }
            "#,
        );
        let sibling = program
            .typed
            .machines()
            .iter()
            .find(|machine| machine.target.is_some())
            .expect("the demo_target machine is a sibling")
            .symbol;
        let main = machine_named(&program.typed, "main").symbol;
        program.typed.target_machine_families = vec![
            typed_trees::machine::TargetMachineFamily {
                name: typed_trees::name::Identifier::generated("helper"),
                canonical: sibling,
                members: vec![
                    typed_trees::machine::TargetMachineFamilyMember {
                        target: typed_trees::name::Identifier::generated("demo_target"),
                        machine: sibling,
                    },
                    typed_trees::machine::TargetMachineFamilyMember {
                        target: typed_trees::name::Identifier::generated("linux_x86_64"),
                        machine: main,
                    },
                ],
            },
            typed_trees::machine::TargetMachineFamily {
                name: typed_trees::name::Identifier::generated("gone"),
                canonical: sibling,
                members: vec![typed_trees::machine::TargetMachineFamilyMember {
                    target: typed_trees::name::Identifier::generated("demo_target"),
                    machine: sibling,
                }],
            },
        ];

        let pruned = super::prune_target_siblings(program).expect("prune");

        let [family] = pruned.typed.target_machine_families.as_slice() else {
            panic!("the family with a live member survives; the all-pruned family drops")
        };
        assert_eq!(family.name.as_str(), "helper");
        assert_eq!(family.canonical, sibling);
        assert_eq!(
            family
                .members
                .iter()
                .map(|member| member.machine)
                .collect::<Vec<_>>(),
            vec![main]
        );
    }
}
