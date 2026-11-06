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
//! Batched visitors share one state, sit outside transition targets and
//! transition statements, and use the same judge. Those are the visitor
//! properties the walker's own choices read (dead-local rejoins and leaving the
//! state), so each choice matches the visitor's solo walk.

use super::call_requirements::{
    PreparedRequirement, RequirementWalk, contains_case_premise, prepare_selected_value,
};
use super::structural_case_arms::recognize_structural_case_arms_batched;
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
use typed_trees::statement::StatementNode;

/// One proved requirement occurrence: caller state, statement, call expression
/// (invalid for a statement call) and the exact required expression.
type ProvenSite = (SymbolHandle, usize, ExpressionHandle, ExpressionHandle);

/// Requirement sites a proof machine's shared walks proved.
#[derive(Debug, Default)]
pub(crate) struct SharedProofWalks {
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
    if !shares_walk(program, walk) {
        return false;
    }
    let Some(memos) = program.checking_memos() else {
        return false;
    };
    let site = proven_site(walk);
    let existing = memos
        .shared_proof_walks
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&machine.symbol)
        .cloned();
    let shared = match existing {
        Some(shared) => shared,
        None => {
            if BUILDING.with(|building| building.borrow().contains(&machine.symbol)) {
                return false;
            }
            BUILDING.with(|building| building.borrow_mut().push(machine.symbol));
            let built = Arc::new(build(program, machine));
            BUILDING.with(|building| building.borrow_mut().retain(|m| *m != machine.symbol));
            // Another worker may have built the same verdicts meanwhile; both
            // describe the same program, so the first one stored is kept.
            memos
                .shared_proof_walks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .entry(machine.symbol)
                .or_insert(built)
                .clone()
        }
    };
    let found = shared.proven.contains(&site);
    if found {
        phase_profile::count(
            "psi.05.check_checked_facts.check_flow_call_contracts.cap.shared_walk_hits",
            1,
        );
    }
    found
}

fn proven_site(walk: &RequirementWalk<'_>) -> ProvenSite {
    let requirement = &walk.requirement;
    (
        requirement.state,
        requirement.statement_index,
        requirement.call_expression,
        requirement.expression,
    )
}

/// A requirement can join a shared walk when its solo check would take the
/// proof caller's case-arm walk from machine entry and its site meets the
/// batching rule in the module documentation.
fn shares_walk(program: &dyn CheckingInput, walk: &RequirementWalk<'_>) -> bool {
    let requirement = &walk.requirement;
    if walk.runtime_values
        || walk.case_facts_only
        || requirement.is_inherited()
        || requirement.in_transition_target
    {
        return false;
    }
    program
        .state_by_symbol(requirement.state)
        .and_then(|state| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .get(requirement.statement_index)
        })
        .is_some_and(|statement| !matches!(statement, StatementNode::Transition(_)))
}

fn build(program: &dyn CheckingInput, machine: &Machine) -> SharedProofWalks {
    let mut shared = SharedProofWalks::default();
    let Some(machine) = program.machine_by_symbol(machine.symbol) else {
        return shared;
    };
    let classification = crate::proof_only_classification(program);
    if !classification.is_proof_machine(program, machine) {
        return shared;
    }
    let mut walks = Vec::new();
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
                    ) && shares_walk(program, &walk)
                    {
                        walks.push(*walk);
                    }
                }
            }
        }
    }
    // One walk per caller state and judge kind; a lone requirement gains
    // nothing from sharing and keeps its solo walk.
    walks.sort_by_key(|walk| {
        (
            walk.requirement.state.arena_index(),
            contains_case_premise(program, walk.requirement.expression),
        )
    });
    let mut start = 0;
    while start < walks.len() {
        let key = |walk: &RequirementWalk<'_>| {
            (
                walk.requirement.state,
                contains_case_premise(program, walk.requirement.expression),
            )
        };
        let group_key = key(&walks[start]);
        let end = start
            + walks[start..]
                .iter()
                .take_while(|walk| key(walk) == group_key)
                .count();
        if end - start >= 2 {
            walk_group(
                program,
                machine,
                &classification,
                &mut walks[start..end],
                &mut shared,
            );
        }
        start = end;
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
    let requires = group[0].requires.clone();
    let judge = if contains_case_premise(program, group[0].requirement.expression) {
        StructuralJudge::from_case_requires(program, machine, &requires)
    } else {
        StructuralJudge::from_requires(program, machine, &requires, true)
    };
    let mut diagnostics = Vec::new();
    let recognized = phase_profile::scope(
        "psi.05.check_checked_facts.check_flow_call_contracts.cap.shared_walk",
        || {
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
        },
    );
    if recognized.is_none() || !diagnostics.is_empty() {
        return;
    }
    for walk in group.iter() {
        if walk.requirement.has_positive_proof() {
            shared.proven.push(proven_site(walk));
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
