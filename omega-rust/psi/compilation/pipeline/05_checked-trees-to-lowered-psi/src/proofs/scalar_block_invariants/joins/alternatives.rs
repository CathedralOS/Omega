//! Separate return alternatives for equation-based candidate discovery.
//! This is search scratch, not a fact derivation. A conjunction accumulates
//! facts in their original order; a disjunction forks the context. Implication
//! premises and conclusions remain opaque. Every scratch expansion and copied
//! reference draws from the caller's existing discovery budget.

use semantic_vocabulary::Proposition;

struct Context<'input> {
    pending: Vec<&'input Proposition>,
    facts: Vec<&'input Proposition>,
}

fn charge(remaining: &mut usize, work: usize) -> bool {
    match remaining.checked_sub(work) {
        Some(next) => {
            *remaining = next;
            true
        }
        None => {
            *remaining = 0;
            false
        }
    }
}

pub(super) fn contexts<'input>(
    axioms: &'input [Proposition],
    remaining: &mut usize,
) -> Vec<Vec<&'input Proposition>> {
    if !charge(remaining, axioms.len()) {
        return Vec::new();
    }
    let mut pending = vec![Context {
        pending: axioms.iter().rev().collect(),
        facts: Vec::new(),
    }];
    let mut completed = Vec::new();
    while let Some(mut context) = pending.pop() {
        loop {
            let Some(fact) = context.pending.pop() else {
                completed.push(context.facts);
                break;
            };
            if !charge(remaining, 1) {
                return completed;
            }
            match fact {
                Proposition::Conjunction(members) => {
                    if !charge(remaining, members.len()) {
                        return completed;
                    }
                    context.pending.extend(members.iter().rev());
                }
                Proposition::Disjunction(members) => {
                    for member in members.iter().rev() {
                        let work = context
                            .pending
                            .len()
                            .saturating_add(context.facts.len())
                            .saturating_add(1);
                        if !charge(remaining, work) {
                            return completed;
                        }
                        let mut branch = Context {
                            pending: context.pending.clone(),
                            facts: context.facts.clone(),
                        };
                        branch.pending.push(member);
                        pending.push(branch);
                    }
                    break;
                }
                _ => context.facts.push(fact),
            }
        }
    }
    completed
}

#[cfg(test)]
mod tests;
