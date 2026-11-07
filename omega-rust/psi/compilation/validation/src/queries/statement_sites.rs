//! Which statement of which state contains an expression.
//!
//! Two checks asked this per call by walking every statement's expression tree
//! in the machine, so their cost grew with calls times machine size. Core's
//! Build program has about 300 lowered calls in one state, and each call paid
//! for a walk of all of them. This index walks each statement once per program.
//!
//! The answers are exactly those of the two walks it replaces:
//! - `owner` records every expression `write_frames::caller_aliases`'s
//!   `expression_any` visits from a statement's value roots (the same
//!   `executable_subtree_any` edges, descending everywhere);
//! - `containment` records every expression `expression_owners::expression_contains`
//!   reaches from those roots, including its literal-root rule.
//!
//! Sites are appended in program order (machine, state, statement), once per
//! statement, so a lookup sees statements in the order the old loops did.
//! The index is GUARDED: it records the typed program it was built from, and
//! [`StatementSites::for_program`] returns it only when the asking input still
//! reads that exact program. A projected or otherwise different program gets
//! `None`, and its caller keeps the original walk.

use crate::CheckingInput;
use typed_trees::expression::ExpressionHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatementSite {
    pub machine: u32,
    pub state: u32,
    pub statement: u32,
}

#[derive(Debug)]
pub struct StatementSites {
    program: *const typed_trees::TypedTrees,
    owner: Vec<Vec<(ExpressionHandle, StatementSite)>>,
    containment: Vec<Vec<(ExpressionHandle, StatementSite)>>,
}

// The pointer is an identity guard only; it is never dereferenced.
unsafe impl Send for StatementSites {}
unsafe impl Sync for StatementSites {}

impl StatementSites {
    pub fn build(program: &dyn CheckingInput) -> Self {
        let mut sites = Self {
            program: program.typed_program(),
            owner: Vec::new(),
            containment: Vec::new(),
        };
        let edges = typed_trees::SubtreeEdges {
            pattern_dependencies: true,
            ..typed_trees::SubtreeEdges::VALUES
        };
        let mut owner_seen = Vec::new();
        for (machine_index, machine) in program.machines().iter().enumerate() {
            for (state_index, state) in program.machine_states(machine).iter().enumerate() {
                let statements = program.statement_table.statements(state.statement_nodes);
                for (statement_index, statement) in statements.iter().enumerate() {
                    let site = StatementSite {
                        machine: machine_index as u32,
                        state: state_index as u32,
                        statement: statement_index as u32,
                    };
                    owner_seen.clear();
                    let mut contained = Vec::new();
                    for root in crate::machine_calls::calls::statement_value_expression_roots(
                        program, statement,
                    ) {
                        crate::executable_subtree_any(program, root, edges, |expression, _| {
                            owner_seen.push(expression);
                            typed_trees::SubtreeVisit::Descend
                        });
                        if !root.is_valid() {
                            continue;
                        }
                        if matches!(
                            crate::frozen_program::expression_table(program).expression(root),
                            typed_trees::literal_expression_pattern!()
                        ) {
                            contained.push(root);
                        } else {
                            contained.extend(crate::expression_owners::reachable_expressions(
                                program, root,
                            ));
                        }
                    }
                    owner_seen.sort_by_key(|expression| {
                        (expression.arena_index(), expression.generation())
                    });
                    owner_seen.dedup();
                    for expression in &owner_seen {
                        push(&mut sites.owner, *expression, site);
                    }
                    contained.sort_by_key(|expression| {
                        (expression.arena_index(), expression.generation())
                    });
                    contained.dedup();
                    for expression in contained {
                        push(&mut sites.containment, expression, site);
                    }
                }
            }
        }
        sites
    }

    /// This index when `program` reads the exact program it was built from.
    pub fn for_program<'sites>(
        sites: Option<&'sites StatementSites>,
        program: &dyn CheckingInput,
    ) -> Option<&'sites StatementSites> {
        sites.filter(|sites| std::ptr::eq(sites.program, program.typed_program()))
    }

    /// Statements whose `expression_any` walk visits `expression`.
    pub fn owner_sites(
        &self,
        expression: ExpressionHandle,
    ) -> impl Iterator<Item = StatementSite> + '_ {
        sites_of(&self.owner, expression)
    }

    /// Statements whose `expression_contains` walk reaches `expression`.
    pub fn containing_sites(
        &self,
        expression: ExpressionHandle,
    ) -> impl Iterator<Item = StatementSite> + '_ {
        sites_of(&self.containment, expression)
    }
}

fn push(
    rows: &mut Vec<Vec<(ExpressionHandle, StatementSite)>>,
    expression: ExpressionHandle,
    site: StatementSite,
) {
    let slot = expression.arena_index() as usize;
    if rows.len() <= slot {
        rows.resize_with(slot + 1, Vec::new);
    }
    rows[slot].push((expression, site));
}

fn sites_of(
    rows: &[Vec<(ExpressionHandle, StatementSite)>],
    expression: ExpressionHandle,
) -> impl Iterator<Item = StatementSite> + '_ {
    rows.get(expression.arena_index() as usize)
        .filter(|_| expression.is_valid())
        .into_iter()
        .flatten()
        .filter(move |(retained, _)| *retained == expression)
        .map(|(_, site)| *site)
}
