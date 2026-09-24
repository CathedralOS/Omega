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

mod dependencies;
mod rewrite;

use dependencies::{MachineIndex, collect_machine_edges, interface_retention_set};
use rewrite::{apply_pruning, validate_pruned_product};

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
    use crate::product_pruning::MachineIndex;
    use crate::product_pruning::collect_machine_edges;
    use crate::product_pruning::rewrite::validate_pruned_product;
    use crate::product_pruning::selection_identity;
    use crate::prune_checked_tree_product;
    use crate::tests::front_end::{checked_program, checked_program_result};

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
    fn unreachable_checked_body_machine_is_pruned() {
        let program = checked(
            r#"
            machine helper() -> u64 { 40u64 }
            machine unused() -> u64 { helper() }
            pub machine main() -> u64 { helper() }
            "#,
        );
        let before = program.typed.machines().len();
        let root = machine_named(&program.typed, "main").symbol;
        let helper = machine_named(&program.typed, "helper").symbol;
        let unused = machine_named(&program.typed, "unused").symbol;

        let plan =
            CheckedTreeProductPruning::new(CheckedTreeProductRoots::new([root]).expect("roots"));
        let outcome = prune_checked_tree_product(program, &plan).expect("prune");
        let selection = &outcome.selection;

        assert!(before >= 3);
        assert_eq!(
            outcome.checked.typed.machines().len(),
            selection.retained_machines().len()
        );
        assert!(selection.retained_machines().contains(&root));
        assert!(selection.retained_machines().contains(&helper));
        assert!(selection.pruned_machines().contains(&unused));
        assert_eq!(
            outcome.checked.facts.flow.terminal_machines.machines.len(),
            selection.retained_machines().len()
        );
        assert!(
            outcome
                .checked
                .facts
                .flow
                .terminal_machines
                .machines
                .iter()
                .all(|row| selection.retained_machines().contains(&row.machine))
        );
    }

    #[test]
    fn pruning_is_deterministic_for_the_same_plan() {
        let source = r#"
            machine helper() -> u64 { 40u64 }
            machine unused() -> u64 { 2u64 }
            pub machine main() -> u64 { helper() }
        "#;
        let first = checked(source);
        let second = checked(source);
        let root = machine_named(&first.typed, "main").symbol;
        let plan =
            CheckedTreeProductPruning::new(CheckedTreeProductRoots::new([root]).expect("roots"));

        let first = prune_checked_tree_product(first, &plan).expect("first prune");
        let second = prune_checked_tree_product(second, &plan).expect("second prune");
        assert_eq!(first.selection, second.selection);
        assert_eq!(first.selection.identity(), second.selection.identity());
    }

    #[test]
    fn transitive_machine_dependencies_are_retained() {
        let program = checked(
            r#"
            machine leaf() -> u64 { 3u64 }
            machine middle() -> u64 { leaf() }
            machine detached() -> u64 { 9u64 }
            pub machine main() -> u64 { middle() }
            "#,
        );
        let main = machine_named(&program.typed, "main").symbol;
        let middle = machine_named(&program.typed, "middle").symbol;
        let leaf = machine_named(&program.typed, "leaf").symbol;
        let detached = machine_named(&program.typed, "detached").symbol;

        let plan =
            CheckedTreeProductPruning::new(CheckedTreeProductRoots::new([main]).expect("roots"));
        let outcome = prune_checked_tree_product(program, &plan).expect("prune");

        assert!(outcome.selection.retained_machines().contains(&main));
        assert!(outcome.selection.retained_machines().contains(&middle));
        assert!(outcome.selection.retained_machines().contains(&leaf));
        assert!(outcome.selection.pruned_machines().contains(&detached));
        // The rosters partition the source machine table.
        assert_eq!(
            outcome.selection.retained_machines().len() + outcome.selection.pruned_machines().len(),
            4
        );
    }

    #[test]
    fn duplicate_nested_dependencies_preserve_exact_product_selection() {
        let program = checked(
            r#"
            machine leaf() -> u64 { 3u64 }
            machine identity(value: u64) -> u64 { value }
            machine detached() -> u64 { 9u64 }
            pub machine main() -> u64 { identity(identity(leaf())) }
            "#,
        );
        let root = machine_named(&program.typed, "main").symbol;
        let leaf = machine_named(&program.typed, "leaf").symbol;
        let identity = machine_named(&program.typed, "identity").symbol;
        let detached = machine_named(&program.typed, "detached").symbol;
        let index = MachineIndex::build(&program.typed);
        let edges = collect_machine_edges(&program.typed, &program.facts, &index);
        // Both checked call facts and nested typed expressions contribute
        // these targets; the final adjacency stores each identity once.
        assert_eq!(
            edges.get(&root).map(Vec::as_slice),
            Some(&[leaf, identity][..])
        );

        let plan =
            CheckedTreeProductPruning::new(CheckedTreeProductRoots::new([root]).expect("roots"));
        let outcome = prune_checked_tree_product(program, &plan).expect("prune");
        assert_eq!(
            outcome.selection.retained_machines(),
            &[leaf, identity, root]
        );
        assert_eq!(outcome.selection.pruned_machines(), &[detached]);
        assert_eq!(
            outcome.selection.identity(),
            selection_identity(&[root], &[leaf, identity, root], &[detached])
        );
        let rebuilt = MachineIndex::build(&outcome.checked.typed);
        let rebuilt_edges =
            collect_machine_edges(&outcome.checked.typed, &outcome.checked.facts, &rebuilt);
        assert_eq!(rebuilt_edges.get(&root), edges.get(&root));
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

    #[test]
    fn invalid_authored_declarations_fail_checking_before_pruning() {
        // `detached` is unreachable from any plausible root, yet its
        // unproven-arithmetic diagnostic must still surface from authored
        // checking. Pruning is unreachable here: the phase input is a
        // `CheckedTrees`, which only exists after all diagnostics clear.
        let Err(diagnostics) = checked_program_result(
            r#"
            machine helper() -> u64 { 40u64 }
            machine detached() -> u64 { helper() + 2u64 }
            pub machine main() -> u64 { helper() }
            "#,
        ) else {
            panic!("an invalid authored declaration must fail checking");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| format!("{diagnostic:?}").contains("may overflow"))
        );
    }

    #[test]
    fn boundary_machines_are_interface_surface_not_pruning_candidates() {
        let mut program = checked(
            r#"
            pub machine entry() -> u64 { 9u64 }
            "#,
        );
        // Synthesize a boundary-declaration machine without source support:
        // pruning must keep it regardless of reachability.
        let boundary_symbol = {
            let boundary = Machine {
                symbol: program
                    .typed
                    .machines()
                    .iter()
                    .map(|machine| machine.symbol.arena_index())
                    .max()
                    .map(|index| SymbolHandle::from_arena_index(index + 1))
                    .expect("a machine symbol must exist"),
                supply_mode: language_semantics::MachineSupplyMode::Boundary,
                body_is_present: false,
                ..Machine::default()
            };
            let symbol = boundary.symbol;
            program.typed.push_machine(boundary);
            symbol
        };
        let root = machine_named(&program.typed, "entry").symbol;
        let plan =
            CheckedTreeProductPruning::new(CheckedTreeProductRoots::new([root]).expect("roots"));
        let outcome = prune_checked_tree_product(program, &plan).expect("prune");
        assert!(
            outcome
                .selection
                .retained_machines()
                .contains(&boundary_symbol)
        );
    }

    #[test]
    fn the_pruned_product_is_a_fixed_point_for_the_same_plan() {
        let program = checked(
            r#"
            machine helper() -> u64 { 40u64 }
            machine unused() -> u64 { 2u64 }
            pub machine main() -> u64 { helper() }
            "#,
        );
        let root = machine_named(&program.typed, "main").symbol;
        let plan =
            CheckedTreeProductPruning::new(CheckedTreeProductRoots::new([root]).expect("roots"));
        let first = prune_checked_tree_product(program, &plan).expect("first prune");
        assert!(!first.selection.pruned_machines().is_empty());
        let retained = first.selection.retained_machines().to_vec();

        // The published outcome is itself a legal `CheckedTrees`: replaying
        // the same plan against it is a legal second input that finds no
        // remaining candidate, so the product is a fixed point rather than
        // merely reconstructible.
        let second = prune_checked_tree_product(first.checked, &plan).expect("fixed-point prune");
        assert_eq!(second.selection.roots().machines(), &[root]);
        assert_eq!(second.selection.retained_machines(), retained.as_slice());
        assert!(second.selection.pruned_machines().is_empty());
        assert_eq!(
            second.selection.identity(),
            selection_identity(&[root], &retained, &[])
        );
        assert_eq!(
            second
                .checked
                .typed
                .machines()
                .iter()
                .map(|machine| machine.symbol)
                .collect::<Vec<_>>(),
            retained
        );
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

    #[test]
    fn root_order_is_canonicalized_in_the_selection() {
        let program = checked(
            r#"
            machine other() -> u64 { 1u64 }
            pub machine main() -> u64 { other() }
            pub machine second() -> u64 { other() }
            "#,
        );
        let main = machine_named(&program.typed, "main").symbol;
        let second = machine_named(&program.typed, "second").symbol;

        let forward = prune_checked_tree_product(
            program.clone(),
            &CheckedTreeProductPruning::new(
                CheckedTreeProductRoots::new([main, second]).expect("roots"),
            ),
        )
        .expect("forward prune");
        let reverse = prune_checked_tree_product(
            program,
            &CheckedTreeProductPruning::new(
                CheckedTreeProductRoots::new([second, main]).expect("roots"),
            ),
        )
        .expect("reverse prune");
        // Root input order is canonicalized at admission: both permutations
        // publish the same roots, rosters, and selection identity.
        assert_eq!(forward.selection, reverse.selection);
    }

    #[test]
    fn selection_identity_rejects_swapped_and_truncated_rosters() {
        let program = checked(
            r#"
            machine helper() -> u64 { 40u64 }
            machine unused() -> u64 { 2u64 }
            pub machine main() -> u64 { helper() }
            "#,
        );
        let root = machine_named(&program.typed, "main").symbol;
        let plan =
            CheckedTreeProductPruning::new(CheckedTreeProductRoots::new([root]).expect("roots"));
        let outcome = prune_checked_tree_product(program, &plan).expect("prune");
        let retained = outcome.selection.retained_machines();
        let pruned = outcome.selection.pruned_machines();
        assert!(!pruned.is_empty());

        // The commitment binds the exact rosters in role order: swapping the
        // roles or truncating either side is a different selection.
        assert_eq!(
            outcome.selection.identity(),
            selection_identity(&[root], retained, pruned)
        );
        assert_ne!(
            outcome.selection.identity(),
            selection_identity(&[root], pruned, retained)
        );
        assert_ne!(
            outcome.selection.identity(),
            selection_identity(&[root], &retained[..retained.len() - 1], pruned)
        );
        assert_ne!(
            outcome.selection.identity(),
            selection_identity(&[root], retained, &pruned[..pruned.len() - 1])
        );
    }
}
