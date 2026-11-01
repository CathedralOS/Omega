//! Capture-avoiding de Bruijn operations over a scoped handle worklist.
//! The canonical term layout supplies both child slots and binder increments.
//! Each query owns its mapping: equal handles under different binder scopes
//! are distinct inputs, while repeated DAG paths in one scope share output.

use std::collections::HashMap;

use super::term::{Level, MAX_SUBTERMS, Sort, Term, TermArena, TermHandle, instantiate_level};

/// Increase every free variable at or above `cutoff` by `amount`. Returns
/// `term` unchanged when `amount` is zero or no variable qualifies.
pub fn shift(arena: &mut TermArena, term: TermHandle, cutoff: u32, amount: u32) -> TermHandle {
    if amount == 0 {
        return term;
    }
    remap(arena, term, cutoff, Remapping::Shift { amount })
        .expect("a term shift has no universe parameters to refuse")
}

/// Replace de Bruijn index 0 in `body` with `argument`, shifting `argument`
/// under each binder it passes and decrementing greater free indices.
/// Unchanged subterms retain their original handles.
pub fn substitute(arena: &mut TermArena, body: TermHandle, argument: TermHandle) -> TermHandle {
    remap(arena, body, 0, Remapping::Substitute { argument })
        .expect("a term substitution has no universe parameters to refuse")
}

/// Instantiate universe parameters throughout the term. Term binders do not
/// scope universe parameters. An out-of-range parameter returns its index:
/// typing owns scope checking and must still refuse malformed instantiations.
pub fn instantiate_levels(
    arena: &mut TermArena,
    term: TermHandle,
    arguments: &[Level],
) -> Result<TermHandle, u32> {
    remap(arena, term, 0, Remapping::Levels { arguments })
}

#[derive(Clone, Copy)]
enum Remapping<'a> {
    Shift { amount: u32 },
    Substitute { argument: TermHandle },
    Levels { arguments: &'a [Level] },
}

impl Remapping<'_> {
    fn child_scope(self, scope: u32, binders: u32) -> u32 {
        match self {
            Self::Levels { .. } => 0,
            Self::Shift { .. } | Self::Substitute { .. } => scope
                .checked_add(binders)
                .expect("term binder depth overflow"),
        }
    }

    /// Payload rules are separate from structural traversal. Shift and
    /// substitution borrow constants unchanged, including their level syntax.
    fn leaf(self, arena: &mut TermArena, term: TermHandle, scope: u32) -> Result<TermHandle, u32> {
        match (self, arena.node(term)) {
            (Self::Shift { amount }, Term::Variable(index)) if *index >= scope => {
                let index = index.checked_add(amount).expect("variable shift overflow");
                Ok(arena.insert(Term::Variable(index)))
            }
            (Self::Substitute { argument }, Term::Variable(index)) => match index.cmp(&scope) {
                std::cmp::Ordering::Equal => Ok(shift(arena, argument, 0, scope)),
                std::cmp::Ordering::Greater => {
                    let index = index - 1;
                    Ok(arena.insert(Term::Variable(index)))
                }
                std::cmp::Ordering::Less => Ok(term),
            },
            (Self::Levels { arguments }, Term::Sort(sort)) => {
                let level = instantiate_level(&sort.level(), arguments)?;
                let sort = match sort {
                    Sort::Type(_) => Sort::Type(level),
                    Sort::Strict(_) => Sort::Strict(level),
                };
                Ok(arena.insert(Term::Sort(sort)))
            }
            (
                Self::Levels { arguments },
                Term::Constant {
                    declaration,
                    levels,
                },
            ) => {
                let declaration = *declaration;
                let levels = levels
                    .iter()
                    .map(|level| instantiate_level(level, arguments))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(arena.insert(Term::Constant {
                    declaration,
                    levels,
                }))
            }
            _ => Ok(term),
        }
    }
}

/// A frame stores handles and scope, never a recursively owned term tree.
/// A completed child is placed on `values`; Finish consumes the slots in
/// declaration order and publishes the result for this exact scoped input.
#[derive(Clone, Copy)]
enum Work {
    Enter { term: TermHandle, scope: u32 },
    Finish { term: TermHandle, scope: u32 },
}

fn remap(
    arena: &mut TermArena,
    root: TermHandle,
    scope: u32,
    operation: Remapping<'_>,
) -> Result<TermHandle, u32> {
    let mut pending = vec![Work::Enter { term: root, scope }];
    let mut values = Vec::new();
    let mut mapped = HashMap::new();
    while let Some(work) = pending.pop() {
        match work {
            Work::Enter { term, scope } => {
                if let Some(&result) = mapped.get(&(term, scope)) {
                    values.push(result);
                    continue;
                }
                let slots = arena.node(term).subterms();
                if slots.is_empty() {
                    let result = operation.leaf(arena, term, scope)?;
                    mapped.insert((term, scope), result);
                    values.push(result);
                } else {
                    pending.push(Work::Finish { term, scope });
                    pending.extend(slots.iter().rev().map(|(term, binders)| Work::Enter {
                        term,
                        scope: operation.child_scope(scope, binders),
                    }));
                }
            }
            Work::Finish { term, scope } => {
                let node = arena.node(term);
                let slots = node.subterms();
                let mut children = [term; MAX_SUBTERMS];
                for index in (0..slots.len()).rev() {
                    children[index] = values.pop().expect("one result per child slot");
                }
                let changed = slots
                    .iter()
                    .zip(children)
                    .any(|((child, _), new)| child != new);
                let result = if changed {
                    let rebuilt = node.with_children(&children[..slots.len()]);
                    arena.insert(rebuilt)
                } else {
                    term
                };
                mapped.insert((term, scope), result);
                values.push(result);
            }
        }
    }
    let result = values.pop().expect("the root has one remapped result");
    debug_assert!(values.is_empty());
    Ok(result)
}
