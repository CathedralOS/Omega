//! One structural walk per proof-caller state, shared by its call sites.
//!
//! Proving a proof caller's call requirement walks the caller from machine
//! entry to that call, and every lemma citation passed on the way re-admits its
//! own premises against all guarantees established before it. A long proof such
//! as `rat_close_triangle_split` (`core/rat.omg`) therefore repeats nearly the
//! same walk once for each precondition of each lemma call, and each repeat is
//! longer than the last: its checks cost 34 ms at statement 26 and 387 ms at
//! statement 37 (`wiki/drafts/measurements/checker_citation_replay_2026_11_05`).
//!
//! Here every requirement of one state that would take that walk is prepared
//! exactly as its solo check prepares it (`call_requirements::prepare_selected_value`)
//! and handed to one walk as a visitor. `recognize_structural_case_arms_batched`
//! delivers to each visitor the events its own walk would produce.
//!
//! Only a positive verdict is kept: the shared walk returned, produced no
//! diagnostics, and the visitor saw its site with every path proven. The solo
//! walk of a batched visitor is a prefix of the shared walk, so it reaches the
//! same verdict at that site with a subset of the diagnostics. Any other site,
//! and any re-entrant query while a machine's walks are being built, takes its
//! solo walk unchanged.
//!
//! Batched visitors give equal `SharedChoiceReads`, the visitor properties the
//! walker's own choices read, so each choice matches the visitor's solo walk.
//! They also use the same judge, and each judges its observations from fresh
//! query caches (`StructuralJudge::with_fresh_query_state`), so no member reads a cache
//! entry another member made.
//!
//! A verdict is reused only for the exact stored machine and state it was
//! proved on (`ptr::eq`). A verdict built inside a recursion query
//! (`CheckingQuery`) reaches the outermost context only when its build read no
//! recursion cut (`CutGuard`); otherwise it stays in that query's memos.

use super::call_requirements::{
    PreparedRequirement, RequirementWalk, contains_case_premise, prepare_selected_value,
};
use super::structural_case_arms::{SharedChoiceReads, recognize_structural_case_arms_batched};
use super::structural_judgment::StructuralJudge;
use super::structural_prefix::StructuralPrefixVisitor;
use crate::CheckingInput;
use crate::frozen_program::expression_table;
use std::cell::RefCell;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;

/// The call a requirement belongs to within its statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SiteCall {
    Expression(ExpressionHandle),
    /// A statement call has no expression; its table row is its identity in
    /// the immutable statement table the verdict was proved on.
    Statement(usize),
}

/// One proved requirement occurrence: the exact stored caller state (by
/// address), statement, call and required expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProvenSite {
    state: SymbolHandle,
    stored_state: usize,
    statement_index: usize,
    call: SiteCall,
    required: ExpressionHandle,
}

/// Requirement sites a proof machine's shared walks proved, with the address
/// of the stored machine row they were proved on.
#[derive(Debug, Default)]
pub(crate) struct SharedProofWalks {
    stored_machine: usize,
    proven: Vec<ProvenSite>,
}

thread_local! {
    /// Machines whose shared walks this thread is building. A requirement
    /// query reached from inside that build takes its solo walk.
    static BUILDING: RefCell<Vec<SymbolHandle>> = const { RefCell::new(Vec::new()) };
}

/// Whether `walk`'s requirement was proved by its machine's shared walk.
/// `false` means only "not taken here": the caller walks it alone.
pub(super) fn proven(
    program: &dyn CheckingInput,
    machine: &Machine,
    walk: &RequirementWalk<'_>,
) -> bool {
    if shares_walk(program, walk).is_none() {
        return false;
    }
    if program.checking_memos().is_none() {
        return false;
    }
    // Verdicts describe the stored machine row; any other row with this
    // symbol (a specialization, a rewritten copy) walks alone.
    if !program
        .machine(machine.symbol)
        .is_some_and(|stored| std::ptr::eq(stored, machine))
    {
        return false;
    }
    let Some(site) = proven_site(program, machine, walk) else {
        return false;
    };
    let existing = crate::frozen_program::readable_memos(program).find_map(|memos| {
        memos
            .shared_proof_walks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&machine.symbol)
            .cloned()
    });
    let shared = match existing {
        Some(shared) => shared,
        None => {
            if BUILDING.with(|building| building.borrow().contains(&machine.symbol)) {
                return false;
            }
            BUILDING.with(|building| building.borrow_mut().push(machine.symbol));
            let guard = crate::frozen_program::CutGuard::start(program);
            let built = Arc::new(build(program, machine));
            BUILDING.with(|building| building.borrow_mut().retain(|m| *m != machine.symbol));
            // Another worker may have built the same verdicts meanwhile. Each
            // build that reaches the outermost memos read no recursion cut and
            // judged every member from fresh caches, so both are the same and
            // the first is kept.
            match guard.memos() {
                Some(memos) => memos
                    .shared_proof_walks
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .entry(machine.symbol)
                    .or_insert(built)
                    .clone(),
                None => built,
            }
        }
    };
    shared.stored_machine == address(machine) && shared.proven.contains(&site)
}

/// Members of one shared walk agree on all of these, so the walk's single judge
/// and every choice it makes are those of each member's solo walk.
type GroupKey = (SharedChoiceReads, bool, Vec<ExpressionHandle>);

fn address<T>(row: &T) -> usize {
    std::ptr::from_ref(row) as usize
}

/// The site key of `walk`'s requirement in the stored `machine`, or `None`
/// when its state or call has no stored row to key on.
fn proven_site(
    program: &dyn CheckingInput,
    machine: &Machine,
    walk: &RequirementWalk<'_>,
) -> Option<ProvenSite> {
    let requirement = &walk.requirement;
    let stored_state = program.state_in_machine(machine, requirement.state)?;
    let call = if requirement.call_expression.is_valid() {
        SiteCall::Expression(requirement.call_expression)
    } else {
        match requirement.call_site? {
            crate::CallSite::Statement(call) => SiteCall::Statement(address(call)),
            _ => return None,
        }
    };
    Some(ProvenSite {
        state: requirement.state,
        stored_state: address(stored_state),
        statement_index: requirement.statement_index,
        call,
        required: requirement.expression,
    })
}

/// A requirement can join a shared walk when its solo check would take the
/// proof caller's case-arm walk from machine entry and its site is local to
/// its state. The result is the requirement's group: the walker's shared
/// choice reads, the judge kind and the premises the judge is built from.
fn shares_walk(program: &dyn CheckingInput, walk: &RequirementWalk<'_>) -> Option<GroupKey> {
    if walk.runtime_values || walk.case_facts_only || walk.requirement.is_inherited() {
        return None;
    }
    let reads = SharedChoiceReads::of(program, &walk.requirement);
    reads.local_site().then(|| {
        (
            reads,
            contains_case_premise(program, walk.requirement.expression),
            walk.requires.clone(),
        )
    })
}

/// `machine` is the stored row `proven` checked.
fn build(program: &dyn CheckingInput, machine: &Machine) -> SharedProofWalks {
    let mut shared = SharedProofWalks {
        stored_machine: address(machine),
        proven: Vec::new(),
    };
    let classification = crate::proof_only_classification(program);
    if !classification.is_proof_machine(program, machine) {
        return shared;
    }
    // Requirements by group, groups in caller-state order, members in
    // statement order.
    let mut groups: Vec<(GroupKey, Vec<RequirementWalk<'_>>)> = Vec::new();
    for state in program.machine_states(machine) {
        let statement_count = program
            .statement_table
            .statements(state.statement_nodes)
            .len();
        for statement_index in 0..statement_count {
            for site in
                crate::collect_call_sites_in_statement(program, machine, state, statement_index)
            {
                if matches!(site, crate::CallSite::TransitionNamed { .. }) {
                    continue;
                }
                let Some(owner) = super::specification_calls::selected_call_site_contract(
                    program,
                    machine.symbol,
                    site,
                ) else {
                    continue;
                };
                for expression in required_expressions(program, &owner) {
                    if let PreparedRequirement::Walk(walk) = prepare_selected_value(
                        program,
                        machine,
                        state,
                        statement_index,
                        site,
                        expression,
                        false,
                        None,
                    ) && let Some(key) = shares_walk(program, &walk)
                    {
                        match groups.iter_mut().find(|(group, _)| *group == key) {
                            Some((_, members)) => members.push(*walk),
                            None => groups.push((key, vec![*walk])),
                        }
                    }
                }
            }
        }
    }
    groups.sort_by_key(|(_, members)| {
        (
            members[0].requirement.state.arena_index(),
            contains_case_premise(program, members[0].requirement.expression),
        )
    });
    // A lone requirement gains nothing from sharing and keeps its solo walk.
    for (_, members) in &mut groups {
        if members.len() >= 2 {
            walk_group(program, machine, &classification, members, &mut shared);
        }
    }
    shared
}

fn walk_group(
    program: &dyn CheckingInput,
    machine: &Machine,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    group: &mut [RequirementWalk<'_>],
    shared: &mut SharedProofWalks,
) {
    for walk in group.iter_mut() {
        walk.requirement.isolated_judges = true;
    }
    let requires = group[0].requires.clone();
    let judge = if contains_case_premise(program, group[0].requirement.expression) {
        StructuralJudge::from_case_requires(program, machine, &requires)
    } else {
        StructuralJudge::from_requires(program, machine, &requires, true)
    };
    let mut diagnostics = Vec::new();
    let recognized = {
        let mut visitors: Vec<&mut dyn StructuralPrefixVisitor> = group
            .iter_mut()
            .map(|walk| &mut walk.requirement as &mut dyn StructuralPrefixVisitor)
            .collect();
        recognize_structural_case_arms_batched(
            program,
            machine,
            &judge,
            classification,
            &mut diagnostics,
            &mut visitors,
        )
    };
    if recognized.is_none() || !diagnostics.is_empty() {
        return;
    }
    for walk in group.iter() {
        if walk.requirement.has_positive_proof()
            && let Some(site) = proven_site(program, machine, walk)
        {
            shared.proven.push(site);
        }
    }
}

/// Each Requires fact of the selected contract and every conjunct inside it:
/// the expressions `prepare_selected_value` admits for this owner.
fn required_expressions(
    program: &dyn CheckingInput,
    owner: &super::specification_calls::SelectedCallContract<'_>,
) -> Vec<ExpressionHandle> {
    let mut expressions = Vec::new();
    for contract in super::specification_calls::owner_contracts(program, owner)
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let typed_trees::domain::ProofFact::Expression(required) = fact else {
                continue;
            };
            let mut pending = vec![*required];
            while let Some(required) = pending.pop() {
                if !expressions.contains(&required) {
                    expressions.push(required);
                }
                if let Some(binary) = expression_table(program).binary_node(required)
                    && binary.operator == BinaryOperator::And
                {
                    pending.extend([binary.left, binary.right]);
                }
            }
        }
    }
    expressions
}
