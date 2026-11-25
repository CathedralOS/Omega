//! A plain, unindexed `data` declaration as a unified native inductive family.
//!
//! Each case contributes one constructor taking its recursive fields in order,
//! and the kernel derives the dependent recursor (`I.rec`) and its 1-step
//! constructor computation rules directly from the admitted block.

use crate::inductive::declare_inductive_family;
use crate::signature::Declaration;
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

impl PlainData {
    /// A declaration needs at least one case.
    pub fn new(arities: Vec<u32>) -> Option<Self> {
        (!arities.is_empty() && u32::try_from(arities.len()).is_ok()).then_some(Self { arities })
    }

    pub fn cases(&self) -> usize {
        self.arities.len()
    }

    pub fn arity(&self, case: usize) -> u32 {
        self.arities[case]
    }

    /// The index of the data's own entry when its declarations start at `first`.
    pub fn data_entry(first: u32) -> u32 {
        first
    }

    /// The index of a case's constructor, given the data's own entry.
    pub fn constructor_entry(data: u32, case: usize) -> u32 {
        data + 1 + case as u32
    }

    /// The index of the induction principle (recursor), given the data's own entry.
    pub fn induction_entry(&self, data: u32) -> u32 {
        data + 1 + self.arities.len() as u32
    }

    /// Every declaration of the data, in order, for a signature in which the
    /// first of them has index `first`: the inductive type itself, one
    /// constructor per case, and the kernel-derived dependent recursor.
    pub fn declarations(&self, arena: &mut TermArena, first: u32) -> Vec<Declaration> {
        let universe = type_zero(arena);
        let data = arena.insert(Term::Constant {
            declaration: Self::data_entry(first),
            levels: Vec::new(),
        });
        let constructor_types: Vec<TermHandle> = self
            .arities
            .iter()
            .map(|&arity| {
                let mut ty = data;
                for _ in 0..arity {
                    ty = pi(arena, data, ty);
                }
                ty
            })
            .collect();
        declare_inductive_family(arena, first, 0, 0, universe, &constructor_types)
    }

    /// The type of branch `case` of an induction over the data at entry
    /// `data` whose motive is `motive`, a function from the data to a type in
    /// the caller's context. The branch takes the case's fields in order,
    /// then the motive at each field, and lands at the motive of the case.
    pub fn branch_type(
        &self,
        arena: &mut TermArena,
        data: u32,
        motive: TermHandle,
        case: usize,
    ) -> TermHandle {
        let binders = self.case_binders(arena, data, motive, case);
        let depth = 2 * self.arities[case];
        let motive_under = shift(arena, motive, 0, depth);
        let value = self.case_value(arena, data, case, 0);
        let conclusion = apply(arena, motive_under, value);
        binders
            .into_iter()
            .rev()
            .fold(conclusion, |codomain, domain| pi(arena, domain, codomain))
    }

    /// The types of the binders a branch body lives under, each under the
    /// ones before it: the case's fields, then the motive at each field.
    pub fn case_binders(
        &self,
        arena: &mut TermArena,
        data: u32,
        motive: TermHandle,
        case: usize,
    ) -> Vec<TermHandle> {
        let arity = self.arities[case];
        let ty = arena.insert(Term::Constant {
            declaration: data,
            levels: Vec::new(),
        });
        let mut binders = vec![ty; arity as usize];
        for field in 0..arity {
            let motive = shift(arena, motive, 0, arity + field);
            let value = variable(arena, arity - 1);
            binders.push(apply(arena, motive, value));
        }
        binders
    }

    /// A field of the case, seen from a branch body past `inner` further
    /// binders.
    pub fn field(&self, arena: &mut TermArena, case: usize, field: u32, inner: u32) -> TermHandle {
        variable(arena, inner + 2 * self.arities[case] - 1 - field)
    }

    /// The induction hypothesis for that field, seen from the same place.
    pub fn hypothesis(
        &self,
        arena: &mut TermArena,
        case: usize,
        field: u32,
        inner: u32,
    ) -> TermHandle {
        variable(arena, inner + self.arities[case] - 1 - field)
    }

    /// The value of the case over its fields, seen from the same place.
    pub fn case_value(
        &self,
        arena: &mut TermArena,
        data: u32,
        case: usize,
        inner: u32,
    ) -> TermHandle {
        let constructor = arena.insert(Term::Constant {
            declaration: Self::constructor_entry(data, case),
            levels: Vec::new(),
        });
        let fields: Vec<_> = (0..self.arities[case])
            .map(|field| self.field(arena, case, field, inner))
            .collect();
        arena.apply_arguments(constructor, &fields)
    }

    /// Wraps a branch body in its binders. `motive` is in the caller's
    /// context; `body` lives under the binders of `case_binders`.
    pub fn branch(
        &self,
        arena: &mut TermArena,
        data: u32,
        motive: TermHandle,
        case: usize,
        body: TermHandle,
    ) -> TermHandle {
        let binders = self.case_binders(arena, data, motive, case);
        binders
            .into_iter()
            .rev()
            .fold(body, |body, domain| lambda(arena, domain, body))
    }

    /// The induction over `subject` with one closed-over branch per case,
    /// each built by `branch`. Its type is `motive(subject)`.
    pub fn induct(
        &self,
        arena: &mut TermArena,
        data: u32,
        motive: TermHandle,
        branches: &[TermHandle],
        subject: TermHandle,
    ) -> TermHandle {
        debug_assert_eq!(branches.len(), self.cases());
        let induction = arena.insert(Term::Constant {
            declaration: self.induction_entry(data),
            levels: vec![Level::Constant(0)],
        });
        let mut term = apply(arena, induction, motive);
        for &branch in branches {
            term = apply(arena, term, branch);
        }
        apply(arena, term, subject)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversion::{Budget, convertible};
    use crate::signature::{check_signature, judgment_assumption_closure};
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
    const PLUS: u32 = 4;

    /// `Peano`'s declarations and `plus`, defined by cases on its first
    /// parameter with the other parameter quantified in the motive.
    fn peano(arena: &mut TermArena) -> (PlainData, Vec<Declaration>) {
        let data = PlainData::new(vec![0, 1]).unwrap();
        let mut declarations = data.declarations(arena, 0);
        assert_eq!(PlainData::data_entry(0), PEANO);
        assert_eq!(PlainData::constructor_entry(PEANO, 1), SUCC);
        assert_eq!(declarations.len() as u32, PLUS);
        let ty = constant(arena, PEANO);
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
            let body = lambda(arena, ty, next);
            let successor = data.branch(arena, PEANO, motive, 1, body);
            let subject = variable(arena, 1);
            let recursion = data.induct(arena, PEANO, motive, &[zero, successor], subject);
            let other = variable(arena, 0);
            let body = apply(arena, recursion, other);
            let inner = lambda(arena, ty, body);
            lambda(arena, ty, inner)
        };
        declarations.push(Declaration::definition(0, plus_type, plus));
        (data, declarations)
    }

    fn checked(
        arena: &mut TermArena,
        declarations: &[Declaration],
        budget: &mut Budget,
    ) -> Context {
        let signature = check_signature(arena, declarations, budget)
            .unwrap_or_else(|error| panic!("{error:?}"));
        Context::empty().with_signature(signature)
    }

    #[test]
    fn a_function_by_cases_computes_on_a_constructor_over_any_fields() {
        let mut arena = TermArena::new();
        let mut budget = Budget::default();
        let (_, declarations) = peano(&mut arena);
        let ty = constant(&mut arena, PEANO);
        let context = checked(&mut arena, &declarations, &mut budget)
            .extend(ty)
            .extend(ty);
        let (m, b) = (variable(&mut arena, 1), variable(&mut arena, 0));
        let successor = call(&mut arena, SUCC, &[m]);
        let unfolded = call(&mut arena, PLUS, &[successor, b]);
        let smaller = call(&mut arena, PLUS, &[m, b]);
        let expected = call(&mut arena, SUCC, &[smaller]);
        assert!(convertible(&mut arena, &context, unfolded, expected, ty, &mut budget).unwrap());
        let zero = constant(&mut arena, ZERO);
        let from_zero = call(&mut arena, PLUS, &[zero, b]);
        assert!(convertible(&mut arena, &context, from_zero, b, ty, &mut budget).unwrap());
        assert!(!convertible(&mut arena, &context, from_zero, m, ty, &mut budget).unwrap());
        let stuck = call(&mut arena, PLUS, &[m, zero]);
        assert!(!convertible(&mut arena, &context, stuck, m, ty, &mut budget).unwrap());
    }

    /// `Succ(from) = Succ(to)` from a proof of `from = to`, in the proof's
    /// own context.
    fn under_successor(
        arena: &mut TermArena,
        from: TermHandle,
        to: TermHandle,
        equality: TermHandle,
    ) -> TermHandle {
        let ty = constant(arena, PEANO);
        let from_one = shift(arena, from, 0, 1);
        let from_two = shift(arena, from, 0, 2);
        let bound = variable(arena, 0);
        let domain = arena.insert(Term::Id {
            ty,
            left: from_one,
            right: bound,
        });
        let left = call(arena, SUCC, &[from_two]);
        let bound = variable(arena, 1);
        let right = call(arena, SUCC, &[bound]);
        let conclusion = arena.insert(Term::Id { ty, left, right });
        let inner = lambda(arena, domain, conclusion);
        let motive = lambda(arena, ty, inner);
        let value = call(arena, SUCC, &[from]);
        let base = arena.insert(Term::Refl { ty, value });
        arena.insert(Term::IdElim {
            motive,
            base,
            endpoint: to,
            proof: equality,
        })
    }

    #[test]
    fn a_law_that_returns_its_own_argument_is_proved_by_induction() {
        let mut arena = TermArena::new();
        let mut budget = Budget::default();
        let (data, mut declarations) = peano(&mut arena);
        let ty = constant(&mut arena, PEANO);
        let law = |arena: &mut TermArena, subject: u32| {
            let subject = variable(arena, subject);
            let zero = constant(arena, ZERO);
            let left = call(arena, PLUS, &[subject, zero]);
            arena.insert(Term::Id {
                ty,
                left,
                right: subject,
            })
        };
        let statement = {
            let body = law(&mut arena, 0);
            pi(&mut arena, ty, body)
        };
        let proof = {
            let body = law(&mut arena, 0);
            let motive = lambda(&mut arena, ty, body);
            let zero = constant(&mut arena, ZERO);
            let base = arena.insert(Term::Refl { ty, value: zero });
            let successor = {
                let smaller = data.field(&mut arena, 1, 0, 0);
                let zero = constant(&mut arena, ZERO);
                let from = call(&mut arena, PLUS, &[smaller, zero]);
                let hypothesis = data.hypothesis(&mut arena, 1, 0, 0);
                let body = under_successor(&mut arena, from, smaller, hypothesis);
                data.branch(&mut arena, PEANO, motive, 1, body)
            };
            let subject = variable(&mut arena, 0);
            let body = data.induct(&mut arena, PEANO, motive, &[base, successor], subject);
            lambda(&mut arena, ty, body)
        };
        declarations.push(Declaration::definition(0, statement, proof));
        let before = budget.remaining();
        let signature = check_signature(&mut arena, &declarations, &mut budget)
            .unwrap_or_else(|error| panic!("{error:?}"));
        assert!(judgment_assumption_closure(&arena, &signature, &[statement, proof]).is_empty());
        eprintln!(
            "peano declarations and right identity: {} conversion steps, {} terms",
            before - budget.remaining(),
            arena.len()
        );
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
                let step = under_successor(&mut arena, from, to, equality);
                let c = lambda(&mut arena, ty, step);
                let body = lambda(&mut arena, ty, c);
                data.branch(&mut arena, PEANO, motive, 1, body)
            };
            let tree = variable(&mut arena, 2);
            let induction = data.induct(&mut arena, PEANO, motive, &[base, successor], tree);
            let (b, c) = (variable(&mut arena, 1), variable(&mut arena, 0));
            let body = arena.apply_arguments(induction, &[b, c]);
            let c = lambda(&mut arena, ty, body);
            let b = lambda(&mut arena, ty, c);
            lambda(&mut arena, ty, b)
        };
        declarations.push(Declaration::definition(0, statement, proof));
        let before = budget.remaining();
        let signature = check_signature(&mut arena, &declarations, &mut budget)
            .unwrap_or_else(|error| panic!("{error:?}"));
        assert!(judgment_assumption_closure(&arena, &signature, &[statement, proof]).is_empty());
        eprintln!(
            "peano associativity: {} conversion steps, {} terms",
            before - budget.remaining(),
            arena.len()
        );

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
        let mut arena = TermArena::new();
        let mut budget = Budget::default();
        let data = PlainData::new(vec![0, 2, 3]).unwrap();
        let mut declarations = data.declarations(&mut arena, 0);
        let tree = PlainData::data_entry(0);
        let ty = constant(&mut arena, tree);
        let (leaf, two, three) = (tree + 1, tree + 2, tree + 3);
        let pick = declarations.len() as u32;
        let pick_type = pi(&mut arena, ty, ty);
        let body = {
            let motive = lambda(&mut arena, ty, ty);
            let leaf_branch = constant(&mut arena, leaf);
            let first = data.field(&mut arena, 1, 0, 0);
            let first = data.branch(&mut arena, tree, motive, 1, first);
            let last = data.hypothesis(&mut arena, 2, 2, 0);
            let last = data.branch(&mut arena, tree, motive, 2, last);
            let subject = variable(&mut arena, 0);
            let recursion = data.induct(
                &mut arena,
                tree,
                motive,
                &[leaf_branch, first, last],
                subject,
            );
            lambda(&mut arena, ty, recursion)
        };
        declarations.push(Declaration::definition(0, pick_type, body));
        let context = checked(&mut arena, &declarations, &mut budget)
            .extend(ty)
            .extend(ty);
        let (x, y) = (variable(&mut arena, 1), variable(&mut arena, 0));
        let pair = call(&mut arena, two, &[x, y]);
        let picked = call(&mut arena, pick, &[pair]);
        assert!(convertible(&mut arena, &context, picked, x, ty, &mut budget).unwrap());
        assert!(!convertible(&mut arena, &context, picked, y, ty, &mut budget).unwrap());
        let triple = call(&mut arena, three, &[x, y, pair]);
        let picked = call(&mut arena, pick, &[triple]);
        assert!(convertible(&mut arena, &context, picked, x, ty, &mut budget).unwrap());
        let leaf_value = constant(&mut arena, leaf);
        let picked = call(&mut arena, pick, &[leaf_value]);
        assert!(convertible(&mut arena, &context, picked, leaf_value, ty, &mut budget).unwrap());
    }

    #[test]
    fn a_declaration_checks_within_one_default_budget() {
        for arities in [
            vec![0, 1],
            vec![0, 2],
            vec![0, 1, 1],
            vec![0, 2, 3],
            vec![1, 1, 1, 1],
            vec![0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 3, 3],
        ] {
            let mut arena = TermArena::new();
            let data = PlainData::new(arities.clone()).unwrap();
            let declarations = data.declarations(&mut arena, 0);
            let mut budget = Budget::default();
            let before = budget.remaining();
            check_signature(&mut arena, &declarations, &mut budget)
                .unwrap_or_else(|error| panic!("{arities:?}: {error:?}"));
            let spent = before - budget.remaining();
            assert_eq!(
                spent, 0,
                "{arities:?}: native inductive decl should take 0 steps"
            );
        }
    }

    #[test]
    fn a_declaration_with_one_case_forms() {
        for arities in [vec![0], vec![1], vec![2]] {
            let mut arena = TermArena::new();
            let data = PlainData::new(arities.clone()).unwrap();
            let declarations = data.declarations(&mut arena, 0);
            check_signature(&mut arena, &declarations, &mut Budget::default())
                .unwrap_or_else(|error| panic!("{arities:?}: {error:?}"));
        }
    }
}
