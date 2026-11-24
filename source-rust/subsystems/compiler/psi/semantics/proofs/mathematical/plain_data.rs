//! A plain, unindexed `data` declaration as a W type.
//!
//! Each case contributes one label and one child position per field that
//! holds the data itself. The labels and the positions of one case are finite
//! types built from `Two`, `Box` and `Empty`, so the encoding adds no
//! assumption: constructors are `sup` terms and a definition by cases is an
//! `indW` term whose step selects a branch by eliminating the label.
//!
//! W types have no function extensionality. A branch is therefore stated for
//! an arbitrary child function `k`, and a field of the case is `k` applied to
//! that field's canonical position. Functions defined by `indW` compute on
//! `sup(label, k)` for every `k`, which is what a proof by cases needs.

use crate::substitution::shift;
use crate::term::{Level, Sort, Term, TermArena, TermHandle};

/// The number of fields holding the data itself, for each case in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlainData {
    arities: Vec<u32>,
}

fn apply(arena: &mut TermArena, function: TermHandle, argument: TermHandle) -> TermHandle {
    arena.insert(Term::Apply { function, argument })
}

fn lambda(arena: &mut TermArena, domain: TermHandle, body: TermHandle) -> TermHandle {
    arena.insert(Term::Lambda { domain, body })
}

fn pi(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
    arena.insert(Term::Pi { domain, codomain })
}

fn variable(arena: &mut TermArena, index: u32) -> TermHandle {
    arena.insert(Term::Variable(index))
}

fn type_zero(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::Sort(Sort::Type(Level::Constant(0))))
}

/// The strict proposition `Empty -> Empty`, proved by the identity function.
fn truth(arena: &mut TermArena) -> TermHandle {
    let domain = arena.insert(Term::Empty);
    let codomain = arena.insert(Term::Empty);
    pi(arena, domain, codomain)
}

fn unit_value(arena: &mut TermArena) -> TermHandle {
    let ty = truth(arena);
    let domain = arena.insert(Term::Empty);
    let body = variable(arena, 0);
    let value = lambda(arena, domain, body);
    arena.insert(Term::BoxIntro { ty, value })
}

/// A closed relevant type with exactly `size` canonical elements.
fn finite(arena: &mut TermArena, size: u32) -> TermHandle {
    match size {
        0 => {
            let ty = arena.insert(Term::Empty);
            arena.insert(Term::Box { ty })
        }
        1 => {
            let ty = truth(arena);
            arena.insert(Term::Box { ty })
        }
        2 => arena.insert(Term::Two),
        _ => {
            let domain = arena.insert(Term::Two);
            let codomain = finite_tail(arena, size);
            arena.insert(Term::Sigma { domain, codomain })
        }
    }
}

/// Under the tag of a type with more than two elements: the unit for the
/// first element, the remaining elements otherwise.
fn finite_tail(arena: &mut TermArena, size: u32) -> TermHandle {
    let two = arena.insert(Term::Two);
    let universe = type_zero(arena);
    let motive = lambda(arena, two, universe);
    let zero_branch = finite(arena, 1);
    let one_branch = finite(arena, size - 1);
    let scrutinee = variable(arena, 0);
    arena.insert(Term::CaseTwo {
        motive,
        zero_branch,
        one_branch,
        scrutinee,
    })
}

fn element(arena: &mut TermArena, size: u32, index: u32) -> TermHandle {
    debug_assert!(index < size);
    match size {
        1 => unit_value(arena),
        2 if index == 0 => arena.insert(Term::TwoZero),
        2 => arena.insert(Term::TwoOne),
        _ if index == 0 => {
            let first = arena.insert(Term::TwoZero);
            let second = unit_value(arena);
            arena.insert(Term::Pair { first, second })
        }
        _ => {
            let first = arena.insert(Term::TwoOne);
            let second = element(arena, size - 1, index - 1);
            arena.insert(Term::Pair { first, second })
        }
    }
}

/// Dependent case analysis on a finite type. `motive`, every branch and the
/// scrutinee share one context; branch `i` has type `motive(element i)`.
fn eliminate(
    arena: &mut TermArena,
    size: u32,
    motive: TermHandle,
    branches: &[TermHandle],
    scrutinee: TermHandle,
) -> TermHandle {
    debug_assert_eq!(branches.len(), size as usize);
    match size {
        0 => {
            let domain = arena.insert(Term::Empty);
            let payload = arena.insert(Term::Empty);
            let proof = variable(arena, 0);
            let boxed = arena.insert(Term::BoxIntro {
                ty: payload,
                value: proof,
            });
            let motive_under = shift(arena, motive, 0, 1);
            let ty = apply(arena, motive_under, boxed);
            let scrutinee_under = variable(arena, 0);
            let absurd = arena.insert(Term::EmptyElim {
                ty,
                scrutinee: scrutinee_under,
            });
            let body = lambda(arena, domain, absurd);
            arena.insert(Term::BoxElim {
                motive,
                body,
                scrutinee,
            })
        }
        1 => {
            let domain = truth(arena);
            let branch = shift(arena, branches[0], 0, 1);
            let body = lambda(arena, domain, branch);
            arena.insert(Term::BoxElim {
                motive,
                body,
                scrutinee,
            })
        }
        2 => arena.insert(Term::CaseTwo {
            motive,
            zero_branch: branches[0],
            one_branch: branches[1],
            scrutinee,
        }),
        _ => {
            // The second component's type depends on the tag, so each branch
            // is a function of that component and the result is applied to it.
            let two = arena.insert(Term::Two);
            let tail = finite_tail(arena, size);
            let tag = variable(arena, 1);
            let rest = variable(arena, 0);
            let pair = arena.insert(Term::Pair {
                first: tag,
                second: rest,
            });
            let motive_under = shift(arena, motive, 0, 2);
            let landing = apply(arena, motive_under, pair);
            let function = pi(arena, tail, landing);
            let tag_motive = lambda(arena, two, function);
            let mut tagged = [arena.insert(Term::Dummy); 2];
            for (slot, first) in [Term::TwoZero, Term::TwoOne].into_iter().enumerate() {
                let remaining = if slot == 0 { 1 } else { size - 1 };
                let domain = finite(arena, remaining);
                let first = arena.insert(first);
                let second = variable(arena, 0);
                let pair = arena.insert(Term::Pair { first, second });
                let motive_under = shift(arena, motive, 0, 2);
                let landing = apply(arena, motive_under, pair);
                let inner_domain = finite(arena, remaining);
                let inner_motive = lambda(arena, inner_domain, landing);
                let inner_branches: Vec<_> = if slot == 0 {
                    vec![shift(arena, branches[0], 0, 1)]
                } else {
                    branches[1..]
                        .iter()
                        .map(|&branch| shift(arena, branch, 0, 1))
                        .collect()
                };
                let bound = variable(arena, 0);
                let body = eliminate(arena, remaining, inner_motive, &inner_branches, bound);
                tagged[slot] = lambda(arena, domain, body);
            }
            let first = arena.insert(Term::Fst { pair: scrutinee });
            let second = arena.insert(Term::Snd { pair: scrutinee });
            let selected = arena.insert(Term::CaseTwo {
                motive: tag_motive,
                zero_branch: tagged[0],
                one_branch: tagged[1],
                scrutinee: first,
            });
            apply(arena, selected, second)
        }
    }
}

impl PlainData {
    /// A declaration needs at least one case to have a label type.
    pub fn new(arities: Vec<u32>) -> Option<Self> {
        (!arities.is_empty() && u32::try_from(arities.len()).is_ok()).then_some(Self { arities })
    }

    pub fn cases(&self) -> usize {
        self.arities.len()
    }

    pub fn arity(&self, case: usize) -> u32 {
        self.arities[case]
    }

    fn size(&self) -> u32 {
        self.arities.len() as u32
    }

    /// The closed type of case labels.
    pub fn carrier(&self, arena: &mut TermArena) -> TermHandle {
        finite(arena, self.size())
    }

    /// The closed family giving each label its child positions.
    pub fn children(&self, arena: &mut TermArena) -> TermHandle {
        let carrier = self.carrier(arena);
        let motive_domain = self.carrier(arena);
        let universe = type_zero(arena);
        let motive = lambda(arena, motive_domain, universe);
        let branches: Vec<_> = self
            .arities
            .iter()
            .map(|&arity| finite(arena, arity))
            .collect();
        let label = variable(arena, 0);
        let body = eliminate(arena, self.size(), motive, &branches, label);
        lambda(arena, carrier, body)
    }

    /// The closed W type of the declaration.
    pub fn ty(&self, arena: &mut TermArena) -> TermHandle {
        let carrier = self.carrier(arena);
        let children = self.children(arena);
        arena.insert(Term::W { carrier, children })
    }

    pub fn label(&self, arena: &mut TermArena, case: usize) -> TermHandle {
        element(arena, self.size(), case as u32)
    }

    /// The canonical child position of one field of a case.
    pub fn position(&self, arena: &mut TermArena, case: usize, field: u32) -> TermHandle {
        element(arena, self.arities[case], field)
    }

    /// The closed constructor of a case: a function of its fields in order.
    /// `ty` names the declaration in the binders, for example by its constant.
    pub fn constructor(&self, arena: &mut TermArena, case: usize, ty: TermHandle) -> TermHandle {
        let arity = self.arities[case];
        let carrier = self.carrier(arena);
        let children = self.children(arena);
        let label = self.label(arena, case);
        let positions = finite(arena, arity);
        // Under the field binders and the position binder.
        let fields: Vec<_> = (0..arity)
            .map(|field| variable(arena, arity - field))
            .collect();
        let selected = match arity {
            1 => fields[0],
            _ => {
                let domain = finite(arena, arity);
                let motive = lambda(arena, domain, ty);
                let position = variable(arena, 0);
                eliminate(arena, arity, motive, &fields, position)
            }
        };
        let function = lambda(arena, positions, selected);
        let mut term = arena.insert(Term::Sup {
            carrier,
            children,
            label,
            function,
        });
        for _ in 0..arity {
            term = lambda(arena, ty, term);
        }
        term
    }

    pub fn constructor_type(
        &self,
        arena: &mut TermArena,
        case: usize,
        ty: TermHandle,
    ) -> TermHandle {
        let mut term = ty;
        for _ in 0..self.arities[case] {
            term = pi(arena, ty, term);
        }
        term
    }

    /// The value of a case whose child function is the outer of the two
    /// branch binders, seen past `inner` further binders.
    pub fn node(&self, arena: &mut TermArena, case: usize, inner: u32) -> TermHandle {
        let carrier = self.carrier(arena);
        let children = self.children(arena);
        let label = self.label(arena, case);
        let function = variable(arena, inner + 1);
        arena.insert(Term::Sup {
            carrier,
            children,
            label,
            function,
        })
    }

    /// The types of the two binders a branch of `step` lives under: the child
    /// function of the case, then, under that binder, its induction
    /// hypothesis. `motive` is in the caller's context.
    pub fn case_binders(
        &self,
        arena: &mut TermArena,
        motive: TermHandle,
        case: usize,
    ) -> (TermHandle, TermHandle) {
        let ty = self.ty(arena);
        let children = self.children(arena);
        let label = self.label(arena, case);
        let positions = apply(arena, children, label);
        let function = pi(arena, positions, ty);
        let motive_under = shift(arena, motive, 0, 2);
        let function_bound = variable(arena, 1);
        let position = variable(arena, 0);
        let child = apply(arena, function_bound, position);
        let landing = apply(arena, motive_under, child);
        let children = self.children(arena);
        let label = self.label(arena, case);
        let positions = apply(arena, children, label);
        let hypothesis = pi(arena, positions, landing);
        (function, hypothesis)
    }

    /// What a branch of `step` must prove, under the binders for the child
    /// function and the induction hypothesis.
    pub fn case_goal(&self, arena: &mut TermArena, motive: TermHandle, case: usize) -> TermHandle {
        let carrier = self.carrier(arena);
        let children = self.children(arena);
        let label = self.label(arena, case);
        let function = variable(arena, 1);
        let node = arena.insert(Term::Sup {
            carrier,
            children,
            label,
            function,
        });
        let motive = shift(arena, motive, 0, 2);
        apply(arena, motive, node)
    }

    /// A field of the case under the same two binders, shifted past `inner`
    /// further binders.
    pub fn field(&self, arena: &mut TermArena, case: usize, field: u32, inner: u32) -> TermHandle {
        let function = variable(arena, inner + 1);
        let position = self.position(arena, case, field);
        apply(arena, function, position)
    }

    /// The induction hypothesis for that field.
    pub fn hypothesis(
        &self,
        arena: &mut TermArena,
        case: usize,
        field: u32,
        inner: u32,
    ) -> TermHandle {
        let hypothesis = variable(arena, inner);
        let position = self.position(arena, case, field);
        apply(arena, hypothesis, position)
    }

    /// The step of an `indW` over this declaration. `motive` is a function
    /// from the declaration in the caller's context. Branch `i` lives under
    /// two more binders, the child function and the induction hypothesis of
    /// case `i`, and proves `case_goal(motive, i)`.
    pub fn step(
        &self,
        arena: &mut TermArena,
        motive: TermHandle,
        branches: &[TermHandle],
    ) -> TermHandle {
        debug_assert_eq!(branches.len(), self.arities.len());
        let ty = self.ty(arena);
        let children = self.children(arena);
        let carrier = self.carrier(arena);
        // The type of a branch as a function of the label.
        let step_motive = {
            let label = variable(arena, 0);
            let positions = apply(arena, children, label);
            let function = pi(arena, positions, ty);
            let label = variable(arena, 1);
            let positions = apply(arena, children, label);
            let motive_under = shift(arena, motive, 0, 4);
            let function_bound = variable(arena, 1);
            let position = variable(arena, 0);
            let child = apply(arena, function_bound, position);
            let landing = apply(arena, motive_under, child);
            let hypothesis = pi(arena, positions, landing);
            let label = variable(arena, 2);
            let function_bound = variable(arena, 1);
            let node = arena.insert(Term::Sup {
                carrier,
                children,
                label,
                function: function_bound,
            });
            let motive_under = shift(arena, motive, 0, 4);
            let result = apply(arena, motive_under, node);
            let inner = pi(arena, hypothesis, result);
            let outer = pi(arena, function, inner);
            let domain = self.carrier(arena);
            lambda(arena, domain, outer)
        };
        let cases: Vec<_> = branches
            .iter()
            .enumerate()
            .map(|(case, &branch)| {
                let label = self.label(arena, case);
                let positions = apply(arena, children, label);
                let function = pi(arena, positions, ty);
                let motive_under = shift(arena, motive, 0, 3);
                let function_bound = variable(arena, 1);
                let position = variable(arena, 0);
                let child = apply(arena, function_bound, position);
                let landing = apply(arena, motive_under, child);
                let hypothesis = pi(arena, positions, landing);
                let branch = shift(arena, branch, 2, 1);
                let inner = lambda(arena, hypothesis, branch);
                lambda(arena, function, inner)
            })
            .collect();
        let label = variable(arena, 0);
        let selected = eliminate(arena, self.size(), step_motive, &cases, label);
        let domain = self.carrier(arena);
        lambda(arena, domain, selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversion::{Budget, convertible};
    use crate::signature::{Declaration, check_signature, judgment_assumption_closure};
    use crate::typing::{Context, check_type};

    fn constant(arena: &mut TermArena, declaration: u32) -> TermHandle {
        arena.insert(Term::Constant {
            declaration,
            levels: vec![],
        })
    }

    fn call(arena: &mut TermArena, function: u32, arguments: &[TermHandle]) -> TermHandle {
        let function = constant(arena, function);
        arena.apply_arguments(function, arguments)
    }

    const PEANO: u32 = 0;
    const ZERO: u32 = 1;
    const SUCC: u32 = 2;
    const PLUS: u32 = 3;

    /// `Peano`, its constructors and `plus` defined by cases on its first
    /// parameter, with the other parameter quantified in the motive.
    fn peano(arena: &mut TermArena) -> (PlainData, Vec<Declaration>) {
        let data = PlainData::new(vec![0, 1]).unwrap();
        let universe = type_zero(arena);
        let body = data.ty(arena);
        let mut declarations = vec![Declaration::definition(0, universe, body)];
        let ty = constant(arena, PEANO);
        for case in 0..2 {
            let constructor_type = data.constructor_type(arena, case, ty);
            let constructor = data.constructor(arena, case, ty);
            declarations.push(Declaration::definition(0, constructor_type, constructor));
        }
        let plus_type = {
            let inner = pi(arena, ty, ty);
            pi(arena, ty, inner)
        };
        let plus = {
            let rest = pi(arena, ty, ty);
            let motive = lambda(arena, ty, rest);
            let other = variable(arena, 0);
            let zero = lambda(arena, ty, other);
            let hypothesis = data.hypothesis(arena, 1, 0, 1);
            let other = variable(arena, 0);
            let smaller = apply(arena, hypothesis, other);
            let next = call(arena, SUCC, &[smaller]);
            let successor = lambda(arena, ty, next);
            let step = data.step(arena, motive, &[zero, successor]);
            let tree = variable(arena, 1);
            let recursion = arena.insert(Term::IndW { motive, step, tree });
            let other = variable(arena, 0);
            let body = apply(arena, recursion, other);
            let inner = lambda(arena, ty, body);
            lambda(arena, ty, inner)
        };
        declarations.push(Declaration::definition(0, plus_type, plus));
        (data, declarations)
    }

    fn associativity(arena: &mut TermArena, tree: u32, left: u32, right: u32) -> TermHandle {
        let ty = constant(arena, PEANO);
        let (tree, left, right) = (
            variable(arena, tree),
            variable(arena, left),
            variable(arena, right),
        );
        let inner = call(arena, PLUS, &[tree, left]);
        let lhs = call(arena, PLUS, &[inner, right]);
        let inner = call(arena, PLUS, &[left, right]);
        let rhs = call(arena, PLUS, &[tree, inner]);
        arena.insert(Term::Id {
            ty,
            left: lhs,
            right: rhs,
        })
    }

    #[test]
    fn peano_addition_is_associative_by_induction_without_assumptions() {
        let mut arena = TermArena::new();
        let mut budget = Budget::default();
        let (data, mut declarations) = peano(&mut arena);
        let ty = constant(&mut arena, PEANO);
        let statement = {
            let body = associativity(&mut arena, 2, 1, 0);
            let c = pi(&mut arena, ty, body);
            let b = pi(&mut arena, ty, c);
            pi(&mut arena, ty, b)
        };
        let proof = {
            let body = associativity(&mut arena, 2, 1, 0);
            let c = pi(&mut arena, ty, body);
            let b = pi(&mut arena, ty, c);
            let motive = lambda(&mut arena, ty, b);
            let base = {
                let (b, c) = (variable(&mut arena, 1), variable(&mut arena, 0));
                let value = call(&mut arena, PLUS, &[b, c]);
                let refl = arena.insert(Term::Refl { ty, value });
                let c = lambda(&mut arena, ty, refl);
                lambda(&mut arena, ty, c)
            };
            let successor = {
                let smaller = data.field(&mut arena, 1, 0, 2);
                let (b, c) = (variable(&mut arena, 1), variable(&mut arena, 0));
                let inner = call(&mut arena, PLUS, &[smaller, b]);
                let from = call(&mut arena, PLUS, &[inner, c]);
                let inner = call(&mut arena, PLUS, &[b, c]);
                let to = call(&mut arena, PLUS, &[smaller, inner]);
                let hypothesis = data.hypothesis(&mut arena, 1, 0, 2);
                let equality = arena.apply_arguments(hypothesis, &[b, c]);
                let from_one = shift(&mut arena, from, 0, 1);
                let from_two = shift(&mut arena, from, 0, 2);
                let bound = variable(&mut arena, 0);
                let domain = arena.insert(Term::Id {
                    ty,
                    left: from_one,
                    right: bound,
                });
                let left = call(&mut arena, SUCC, &[from_two]);
                let bound = variable(&mut arena, 1);
                let right = call(&mut arena, SUCC, &[bound]);
                let conclusion = arena.insert(Term::Id { ty, left, right });
                let inner = lambda(&mut arena, domain, conclusion);
                let congruence = lambda(&mut arena, ty, inner);
                let value = call(&mut arena, SUCC, &[from]);
                let base = arena.insert(Term::Refl { ty, value });
                let step = arena.insert(Term::IdElim {
                    motive: congruence,
                    base,
                    endpoint: to,
                    proof: equality,
                });
                let c = lambda(&mut arena, ty, step);
                lambda(&mut arena, ty, c)
            };
            let step = data.step(&mut arena, motive, &[base, successor]);
            let tree = variable(&mut arena, 2);
            let induction = arena.insert(Term::IndW { motive, step, tree });
            let (b, c) = (variable(&mut arena, 1), variable(&mut arena, 0));
            let body = arena.apply_arguments(induction, &[b, c]);
            let c = lambda(&mut arena, ty, body);
            let b = lambda(&mut arena, ty, c);
            lambda(&mut arena, ty, b)
        };
        declarations.push(Declaration::definition(0, statement, proof));
        let signature = check_signature(&mut arena, &declarations, &mut budget)
            .unwrap_or_else(|error| panic!("{error:?}"));
        assert!(judgment_assumption_closure(&arena, &signature, &[statement, proof]).is_empty());
        eprintln!(
            "peano associativity: {} conversion steps, {} terms",
            crate::conversion::DEFAULT_CONVERSION_STEPS - budget.remaining(),
            arena.len()
        );

        // Addition computes on closed numerals.
        let context = Context::empty().with_signature(signature);
        let zero = constant(&mut arena, ZERO);
        let one = call(&mut arena, SUCC, &[zero]);
        let two = call(&mut arena, SUCC, &[one]);
        let three = call(&mut arena, SUCC, &[two]);
        let sum = call(&mut arena, PLUS, &[two, one]);
        assert!(convertible(&mut arena, &context, sum, three, ty, &mut budget).unwrap());
        assert!(!convertible(&mut arena, &context, sum, two, ty, &mut budget).unwrap());
        let reflexivity = arena.insert(Term::Refl { ty, value: three });
        let equation = arena.insert(Term::Id {
            ty,
            left: sum,
            right: three,
        });
        check_type(&mut arena, &context, reflexivity, equation, &mut budget).unwrap();
    }

    #[test]
    fn wider_declarations_form_and_select_each_field() {
        // Three cases with zero, one and two fields: labels use the nested
        // sum and the two-field case selects a child by its position.
        let mut arena = TermArena::new();
        let mut budget = Budget::default();
        let data = PlainData::new(vec![0, 1, 2]).unwrap();
        let universe = type_zero(&mut arena);
        let body = data.ty(&mut arena);
        let mut declarations = vec![Declaration::definition(0, universe, body)];
        let ty = constant(&mut arena, 0);
        for case in 0..3 {
            let constructor_type = data.constructor_type(&mut arena, case, ty);
            let constructor = data.constructor(&mut arena, case, ty);
            declarations.push(Declaration::definition(0, constructor_type, constructor));
        }
        // `leftmost` returns the first field of the widest case, or itself.
        let leftmost_type = pi(&mut arena, ty, ty);
        let leftmost = {
            let motive = lambda(&mut arena, ty, ty);
            let leaf = constant(&mut arena, 1);
            let unary = data.field(&mut arena, 1, 0, 0);
            let binary = data.hypothesis(&mut arena, 2, 0, 0);
            let step = data.step(&mut arena, motive, &[leaf, unary, binary]);
            let tree = variable(&mut arena, 0);
            let recursion = arena.insert(Term::IndW { motive, step, tree });
            lambda(&mut arena, ty, recursion)
        };
        declarations.push(Declaration::definition(0, leftmost_type, leftmost));
        let signature = check_signature(&mut arena, &declarations, &mut budget)
            .unwrap_or_else(|error| panic!("{error:?}"));
        let context = Context::empty().with_signature(signature);
        let leaf = constant(&mut arena, 1);
        let wrapped = call(&mut arena, 2, &[leaf]);
        let pair = call(&mut arena, 3, &[wrapped, leaf]);
        let outer = call(&mut arena, 3, &[pair, wrapped]);
        let found = call(&mut arena, 4, &[outer]);
        assert!(convertible(&mut arena, &context, found, leaf, ty, &mut budget).unwrap());
        assert!(!convertible(&mut arena, &context, found, wrapped, ty, &mut budget).unwrap());
    }
}
