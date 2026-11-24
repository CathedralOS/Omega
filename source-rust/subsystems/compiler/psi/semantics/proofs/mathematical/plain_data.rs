//! A plain, unindexed `data` declaration as canonical trees of a W type.
//!
//! Each case contributes one label and one child position per field that
//! holds the data itself. The labels and the positions of one case are finite
//! types built from `Two`, `Box` and `Empty`. A raw tree `sup(label, k)`
//! exists for every child function `k`, and the kernel does not convert `k`
//! to the function a constructor builds from `k`'s values: W types have no
//! function extensionality and the position types no eta rule. So the raw
//! trees are not the data. A law such as `plus(a, Zero) = a` would be false
//! of them up to conversion.
//!
//! The data is a raw tree paired with evidence that it is canonical:
//! `canonical(sup(case, k))` holds one raw tree per field, an identity
//! between the constructor's child function over those trees and `k`, and
//! `canonical` of each field. Eliminating that identity replaces `k` by the
//! constructor's own function, so the induction principle is stated over
//! constructor applications and computes on them, for any fields, by `indW`,
//! identity elimination on `refl`, beta and pair eta. This adds no rule and
//! no assumption. The step of that recursion, and the step of the
//! canonical-tree condition, are declarations of their own: the kernel checks
//! each once and thereafter reads its type from the declaration, where an
//! inline step would be checked again at every recursion it cannot finish.
//! The
//! [inductive profile](../../../../../../wiki/spec/proofs/inductive_profile.md#derived-indexed-families)
//! builds indexed families the same way, as a tree paired with a
//! recursively defined condition.

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

/// Builds a term with each binder named by the depth it was opened at, so a
/// subterm written under more binders still names the same variable. Depth
/// counts from where the build started; a term of the caller's context is
/// shifted by the current depth before it is used.
struct Scope<'a> {
    arena: &'a mut TermArena,
    depth: u32,
}

#[derive(Clone, Copy)]
struct Name(u32);

type Bound<'f> = &'f dyn Fn(&mut Scope<'_>) -> TermHandle;
type Indexed<'f> = &'f dyn Fn(&mut Scope<'_>, u32) -> TermHandle;

impl Scope<'_> {
    fn var(&mut self, name: Name) -> TermHandle {
        variable(self.arena, self.depth - 1 - name.0)
    }

    fn bind(&mut self, body: impl FnOnce(&mut Self, Name) -> TermHandle) -> TermHandle {
        let name = Name(self.depth);
        self.depth += 1;
        let body = body(self, name);
        self.depth -= 1;
        body
    }

    fn lambda(
        &mut self,
        domain: TermHandle,
        body: impl FnOnce(&mut Self, Name) -> TermHandle,
    ) -> TermHandle {
        let body = self.bind(body);
        lambda(self.arena, domain, body)
    }

    fn pi(
        &mut self,
        domain: TermHandle,
        codomain: impl FnOnce(&mut Self, Name) -> TermHandle,
    ) -> TermHandle {
        let codomain = self.bind(codomain);
        pi(self.arena, domain, codomain)
    }

    fn sigma(
        &mut self,
        domain: TermHandle,
        codomain: impl FnOnce(&mut Self, Name) -> TermHandle,
    ) -> TermHandle {
        let codomain = self.bind(codomain);
        self.arena.insert(Term::Sigma { domain, codomain })
    }

    fn constant(&mut self, declaration: u32) -> TermHandle {
        self.arena.insert(Term::Constant {
            declaration,
            levels: Vec::new(),
        })
    }

    fn apply(&mut self, function: TermHandle, argument: TermHandle) -> TermHandle {
        apply(self.arena, function, argument)
    }

    fn pair(&mut self, first: TermHandle, second: TermHandle) -> TermHandle {
        self.arena.insert(Term::Pair { first, second })
    }

    /// `term` after `skip` second projections, then the first projection
    /// when `first` is set: one component of a right-nested tuple.
    fn component(&mut self, mut term: TermHandle, skip: u32, first: bool) -> TermHandle {
        for _ in 0..skip {
            term = self.arena.insert(Term::Snd { pair: term });
        }
        if first {
            term = self.arena.insert(Term::Fst { pair: term });
        }
        term
    }
}

/// The declarations of one `data`, by their indices in the signature.
#[derive(Clone, Copy)]
struct Entries {
    labels: u32,
    children: u32,
    tree: u32,
    canonical_step: u32,
    canonical: u32,
    data: u32,
    induction_step: u32,
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

    /// The index of the data's own entry when its declarations start at
    /// `first`: the case labels, their child positions, the raw trees and
    /// the canonical-tree condition with its step come before it.
    pub fn data_entry(first: u32) -> u32 {
        first + 5
    }

    /// The index of a case's constructor, given the data's own entry.
    pub fn constructor_entry(data: u32, case: usize) -> u32 {
        data + 1 + case as u32
    }

    /// The index of the induction principle, given the data's own entry.
    /// Its step over raw trees comes just before it.
    pub fn induction_entry(&self, data: u32) -> u32 {
        data + 2 + self.size()
    }

    fn entries(&self, data: u32) -> Entries {
        Entries {
            labels: data - 5,
            children: data - 4,
            tree: data - 3,
            canonical_step: data - 2,
            canonical: data - 1,
            data,
            induction_step: data + 1 + self.size(),
        }
    }

    /// The closed type of case labels: the body of the labels' declaration.
    fn carrier(&self, arena: &mut TermArena) -> TermHandle {
        finite(arena, self.size())
    }

    /// The closed family giving each label its child positions: the body of
    /// the positions' declaration.
    fn children(&self, arena: &mut TermArena) -> TermHandle {
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

    /// The type of a child function of a case: its positions to raw trees.
    fn child_type(&self, scope: &mut Scope<'_>, entries: Entries, case: usize) -> TermHandle {
        let positions = finite(scope.arena, self.arities[case]);
        let tree = scope.constant(entries.tree);
        pi(scope.arena, positions, tree)
    }

    /// The child function a constructor builds from one raw tree per field.
    fn child_function(
        &self,
        scope: &mut Scope<'_>,
        entries: Entries,
        case: usize,
        field: Indexed<'_>,
    ) -> TermHandle {
        let arity = self.arities[case];
        let positions = finite(scope.arena, arity);
        scope.lambda(positions, |scope, position| {
            let fields: Vec<_> = (0..arity).map(|index| field(scope, index)).collect();
            if arity == 1 {
                return fields[0];
            }
            let domain = finite(scope.arena, arity);
            let tree = scope.constant(entries.tree);
            let motive = lambda(scope.arena, domain, tree);
            let position = scope.var(position);
            eliminate(scope.arena, arity, motive, &fields, position)
        })
    }

    fn node(
        &self,
        scope: &mut Scope<'_>,
        entries: Entries,
        case: usize,
        function: TermHandle,
    ) -> TermHandle {
        let carrier = scope.constant(entries.labels);
        let children = scope.constant(entries.children);
        let label = element(scope.arena, self.size(), case as u32);
        scope.arena.insert(Term::Sup {
            carrier,
            children,
            label,
            function,
        })
    }

    /// The right-nested product of one type per field of a case.
    fn product(scope: &mut Scope<'_>, arity: u32, index: u32, factor: Indexed<'_>) -> TermHandle {
        let domain = factor(scope, index);
        if index + 1 == arity {
            return domain;
        }
        scope.sigma(domain, |scope, _| {
            Self::product(scope, arity, index + 1, factor)
        })
    }

    /// The evidence that `sup(case, child)` is canonical. For a case with no
    /// field it is the identity of the constructor's child function with
    /// `child`. Otherwise it is one raw tree per field, that identity for the
    /// child function over those trees, and `condition(field)` for each field.
    fn evidence_type(
        &self,
        scope: &mut Scope<'_>,
        entries: Entries,
        case: usize,
        child: Bound<'_>,
        condition: Indexed<'_>,
        fields: &mut Vec<Name>,
    ) -> TermHandle {
        let arity = self.arities[case];
        if (fields.len() as u32) < arity {
            let tree = scope.constant(entries.tree);
            return scope.sigma(tree, |scope, field| {
                fields.push(field);
                self.evidence_type(scope, entries, case, child, condition, fields)
            });
        }
        let ty = self.child_type(scope, entries, case);
        let names = fields.clone();
        let left = self.child_function(scope, entries, case, &|scope, index| {
            scope.var(names[index as usize])
        });
        let right = child(scope);
        let identity = scope.arena.insert(Term::Id { ty, left, right });
        if arity == 0 {
            return identity;
        }
        scope.sigma(identity, |scope, _| {
            Self::product(scope, arity, 0, condition)
        })
    }

    /// `canonical : tree -> Type`, by recursion on the tree.
    fn canonical(&self, scope: &mut Scope<'_>, entries: Entries) -> TermHandle {
        let tree = scope.constant(entries.tree);
        let universe = type_zero(scope.arena);
        let motive = lambda(scope.arena, tree, universe);
        scope.lambda(tree, |scope, subject| {
            let step = scope.constant(entries.canonical_step);
            let tree = scope.var(subject);
            scope.arena.insert(Term::IndW { motive, step, tree })
        })
    }

    /// The step of `canonical`, with its type: for a label, a child function
    /// and the condition of each child, the evidence that the node is
    /// canonical. It is a declaration of its own so that a recursion the
    /// kernel cannot finish names it by a constant; written inline, the
    /// kernel would check the whole step again each time it met one.
    fn canonical_step(&self, scope: &mut Scope<'_>, entries: Entries) -> (TermHandle, TermHandle) {
        let universe = type_zero(scope.arena);
        let over = |scope: &mut Scope<'_>, label: Name| {
            let children = scope.constant(entries.children);
            let label_term = scope.var(label);
            let positions = scope.apply(children, label_term);
            let tree = scope.constant(entries.tree);
            let child = pi(scope.arena, positions, tree);
            scope.pi(child, |scope, _| {
                let children = scope.constant(entries.children);
                let label_term = scope.var(label);
                let positions = scope.apply(children, label_term);
                let conditions = pi(scope.arena, positions, universe);
                scope.pi(conditions, |_, _| universe)
            })
        };
        let labels = scope.constant(entries.labels);
        let ty = scope.pi(labels, |scope, label| over(scope, label));
        let step = scope.lambda(labels, |scope, label| {
            let labels = scope.constant(entries.labels);
            let step_motive = scope.lambda(labels, |scope, label| over(scope, label));
            let branches: Vec<_> = (0..self.cases())
                .map(|case| {
                    let child = self.child_type(scope, entries, case);
                    scope.lambda(child, |scope, child| {
                        let positions = finite(scope.arena, self.arities[case]);
                        let conditions = pi(scope.arena, positions, universe);
                        scope.lambda(conditions, |scope, conditions| {
                            self.evidence_type(
                                scope,
                                entries,
                                case,
                                &|scope| scope.var(child),
                                &|scope, field| {
                                    let conditions = scope.var(conditions);
                                    let position = element(scope.arena, self.arities[case], field);
                                    scope.apply(conditions, position)
                                },
                                &mut Vec::new(),
                            )
                        })
                    })
                })
                .collect();
            let label = scope.var(label);
            eliminate(scope.arena, self.size(), step_motive, &branches, label)
        });
        (ty, step)
    }

    /// The value of a case over one value of the data per field.
    fn constructed(
        &self,
        scope: &mut Scope<'_>,
        entries: Entries,
        case: usize,
        field: Indexed<'_>,
    ) -> TermHandle {
        let arity = self.arities[case];
        let raw: Indexed<'_> = &|scope, index| {
            let field = field(scope, index);
            scope.component(field, 0, true)
        };
        let function = self.child_function(scope, entries, case, raw);
        let tree = self.node(scope, entries, case, function);
        let ty = self.child_type(scope, entries, case);
        let value = self.child_function(scope, entries, case, raw);
        let mut evidence = scope.arena.insert(Term::Refl { ty, value });
        if arity > 0 {
            let last = field(scope, arity - 1);
            let mut conditions = scope.component(last, 1, false);
            for index in (0..arity - 1).rev() {
                let field = field(scope, index);
                let condition = scope.component(field, 1, false);
                conditions = scope.pair(condition, conditions);
            }
            evidence = scope.pair(evidence, conditions);
            for index in (0..arity).rev() {
                let tree = raw(scope, index);
                evidence = scope.pair(tree, evidence);
            }
        }
        scope.pair(tree, evidence)
    }

    fn constructor(&self, scope: &mut Scope<'_>, entries: Entries, case: usize) -> TermHandle {
        fn fields(
            data: &PlainData,
            scope: &mut Scope<'_>,
            entries: Entries,
            case: usize,
            names: &mut Vec<Name>,
        ) -> TermHandle {
            if (names.len() as u32) < data.arities[case] {
                let domain = scope.constant(entries.data);
                return scope.lambda(domain, |scope, field| {
                    names.push(field);
                    fields(data, scope, entries, case, names)
                });
            }
            let names = names.clone();
            data.constructed(scope, entries, case, &|scope, index| {
                scope.var(names[index as usize])
            })
        }
        fields(self, scope, entries, case, &mut Vec::new())
    }

    /// What one case of an induction with this motive must supply: for every
    /// value of each field and the motive at each of them, the motive at the
    /// case over those fields.
    fn branch_type_in(
        &self,
        scope: &mut Scope<'_>,
        entries: Entries,
        case: usize,
        motive: Bound<'_>,
        fields: &mut Vec<Name>,
        hypotheses: u32,
    ) -> TermHandle {
        let arity = self.arities[case];
        if (fields.len() as u32) < arity {
            let domain = scope.constant(entries.data);
            return scope.pi(domain, |scope, field| {
                fields.push(field);
                self.branch_type_in(scope, entries, case, motive, fields, hypotheses)
            });
        }
        if hypotheses < arity {
            let motive_term = motive(scope);
            let field = scope.var(fields[hypotheses as usize]);
            let domain = scope.apply(motive_term, field);
            return scope.pi(domain, |scope, _| {
                self.branch_type_in(scope, entries, case, motive, fields, hypotheses + 1)
            });
        }
        let motive = motive(scope);
        let arguments: Vec<_> = fields.iter().map(|&field| scope.var(field)).collect();
        let constructor = scope.constant(Self::constructor_entry(entries.data, case));
        let value = scope.arena.apply_arguments(constructor, &arguments);
        scope.apply(motive, value)
    }

    /// For every position of a child function and every proof that the child
    /// there is canonical, the motive at that child.
    fn hypothesis_type(
        &self,
        scope: &mut Scope<'_>,
        entries: Entries,
        positions: TermHandle,
        child: Bound<'_>,
        motive: Bound<'_>,
    ) -> TermHandle {
        scope.pi(positions, |scope, position| {
            let function = child(scope);
            let position_term = scope.var(position);
            let subtree = scope.apply(function, position_term);
            let canonical = scope.constant(entries.canonical);
            let condition = scope.apply(canonical, subtree);
            scope.pi(condition, |scope, evidence| {
                let function = child(scope);
                let position_term = scope.var(position);
                let subtree = scope.apply(function, position_term);
                let evidence = scope.var(evidence);
                let value = scope.pair(subtree, evidence);
                let motive = motive(scope);
                scope.apply(motive, value)
            })
        })
    }

    /// The canonical-tree condition of each field's child, as one product.
    fn conditions_type(
        &self,
        scope: &mut Scope<'_>,
        entries: Entries,
        case: usize,
        child: Bound<'_>,
    ) -> TermHandle {
        let arity = self.arities[case];
        Self::product(scope, arity, 0, &|scope, field| {
            let function = child(scope);
            let position = element(scope.arena, arity, field);
            let subtree = scope.apply(function, position);
            let canonical = scope.constant(entries.canonical);
            scope.apply(canonical, subtree)
        })
    }

    /// One case of the induction's step over raw trees. The evidence says the
    /// child function is the constructor's own over some fields; eliminating
    /// that identity leaves exactly the case over those fields, which the
    /// caller's branch answers.
    fn step_branch(
        &self,
        scope: &mut Scope<'_>,
        entries: Entries,
        case: usize,
        motive: Bound<'_>,
        branch: Name,
    ) -> TermHandle {
        let arity = self.arities[case];
        let child_type = self.child_type(scope, entries, case);
        scope.lambda(child_type, |scope, child| {
            let positions = finite(scope.arena, arity);
            let hypothesis =
                self.hypothesis_type(scope, entries, positions, &|scope| scope.var(child), motive);
            scope.lambda(hypothesis, |scope, hypothesis| {
                let function = scope.var(child);
                let tree = self.node(scope, entries, case, function);
                let canonical = scope.constant(entries.canonical);
                let evidence_type = scope.apply(canonical, tree);
                scope.lambda(evidence_type, |scope, evidence| {
                    // The raw tree of each field, read out of the evidence.
                    let raw: Indexed<'_> = &|scope, index| {
                        let evidence = scope.var(evidence);
                        scope.component(evidence, index, true)
                    };
                    let own: Bound<'_> = &|scope| self.child_function(scope, entries, case, raw);
                    let eliminator_motive = scope.lambda(child_type, |scope, other| {
                        let left = own(scope);
                        let right = scope.var(other);
                        let identity = scope.arena.insert(Term::Id {
                            ty: child_type,
                            left,
                            right,
                        });
                        scope.lambda(identity, |scope, proof| {
                            let conclusion = |scope: &mut Scope<'_>, tail: TermHandle| {
                                let mut evidence = tail;
                                for index in (0..arity).rev() {
                                    let tree = raw(scope, index);
                                    evidence = scope.pair(tree, evidence);
                                }
                                let function = scope.var(other);
                                let tree = self.node(scope, entries, case, function);
                                let value = scope.pair(tree, evidence);
                                let motive = motive(scope);
                                scope.apply(motive, value)
                            };
                            if arity == 0 {
                                let proof = scope.var(proof);
                                return conclusion(scope, proof);
                            }
                            let positions = finite(scope.arena, arity);
                            let hypothesis = self.hypothesis_type(
                                scope,
                                entries,
                                positions,
                                &|scope| scope.var(other),
                                motive,
                            );
                            scope.pi(hypothesis, |scope, _| {
                                let conditions =
                                    self.conditions_type(scope, entries, case, &|scope| {
                                        scope.var(other)
                                    });
                                scope.pi(conditions, |scope, conditions| {
                                    let proof = scope.var(proof);
                                    let conditions = scope.var(conditions);
                                    let tail = scope.pair(proof, conditions);
                                    conclusion(scope, tail)
                                })
                            })
                        })
                    });
                    let endpoint = scope.var(child);
                    if arity == 0 {
                        let base = scope.var(branch);
                        let proof = scope.var(evidence);
                        return scope.arena.insert(Term::IdElim {
                            motive: eliminator_motive,
                            base,
                            endpoint,
                            proof,
                        });
                    }
                    let positions = finite(scope.arena, arity);
                    let own_hypothesis =
                        self.hypothesis_type(scope, entries, positions, own, motive);
                    let base = scope.lambda(own_hypothesis, |scope, hypothesis| {
                        let conditions = self.conditions_type(scope, entries, case, own);
                        scope.lambda(conditions, |scope, conditions| {
                            let condition = |scope: &mut Scope<'_>, index: u32| {
                                let conditions = scope.var(conditions);
                                scope.component(conditions, index, index + 1 < arity)
                            };
                            let mut term = scope.var(branch);
                            for index in 0..arity {
                                let tree = raw(scope, index);
                                let evidence = condition(scope, index);
                                let field = scope.pair(tree, evidence);
                                term = scope.apply(term, field);
                            }
                            for index in 0..arity {
                                let hypothesis = scope.var(hypothesis);
                                let position = element(scope.arena, arity, index);
                                let at_position = scope.apply(hypothesis, position);
                                let evidence = condition(scope, index);
                                let result = scope.apply(at_position, evidence);
                                term = scope.apply(term, result);
                            }
                            term
                        })
                    });
                    let evidence_term = scope.var(evidence);
                    let proof = scope.component(evidence_term, arity, true);
                    let eliminated = scope.arena.insert(Term::IdElim {
                        motive: eliminator_motive,
                        base,
                        endpoint,
                        proof,
                    });
                    let hypothesis = scope.var(hypothesis);
                    let with_hypothesis = scope.apply(eliminated, hypothesis);
                    let evidence_term = scope.var(evidence);
                    let conditions = scope.component(evidence_term, arity + 1, false);
                    scope.apply(with_hypothesis, conditions)
                })
            })
        })
    }

    /// What a step over raw trees must supply at one label: for a child
    /// function, the motive at each canonical child and the evidence that
    /// the node is canonical, the motive at the node with that evidence.
    fn step_type(
        &self,
        scope: &mut Scope<'_>,
        entries: Entries,
        motive: Name,
        label: Name,
    ) -> TermHandle {
        let children = scope.constant(entries.children);
        let label_term = scope.var(label);
        let positions = scope.apply(children, label_term);
        let tree = scope.constant(entries.tree);
        let child_type = pi(scope.arena, positions, tree);
        scope.pi(child_type, |scope, child| {
            let children = scope.constant(entries.children);
            let label_term = scope.var(label);
            let positions = scope.apply(children, label_term);
            let hypothesis = self.hypothesis_type(
                scope,
                entries,
                positions,
                &|scope| scope.var(child),
                &|scope| scope.var(motive),
            );
            scope.pi(hypothesis, |scope, _| {
                let node = |scope: &mut Scope<'_>| {
                    let carrier = scope.constant(entries.labels);
                    let children = scope.constant(entries.children);
                    let label = scope.var(label);
                    let function = scope.var(child);
                    scope.arena.insert(Term::Sup {
                        carrier,
                        children,
                        label,
                        function,
                    })
                };
                let tree = node(scope);
                let canonical = scope.constant(entries.canonical);
                let condition = scope.apply(canonical, tree);
                scope.pi(condition, |scope, evidence| {
                    let tree = node(scope);
                    let evidence = scope.var(evidence);
                    let value = scope.pair(tree, evidence);
                    let motive = scope.var(motive);
                    scope.apply(motive, value)
                })
            })
        })
    }

    /// Binds the motive and one branch per case, then builds `inner` under
    /// them, as a type when `quantify` is set and as a function otherwise.
    fn over_branches(
        &self,
        scope: &mut Scope<'_>,
        entries: Entries,
        quantify: bool,
        inner: &dyn Fn(&mut Scope<'_>, Name, &[Name]) -> TermHandle,
    ) -> TermHandle {
        fn branches(
            this: &PlainData,
            scope: &mut Scope<'_>,
            entries: Entries,
            quantify: bool,
            motive: Name,
            bound: &mut Vec<Name>,
            inner: &dyn Fn(&mut Scope<'_>, Name, &[Name]) -> TermHandle,
        ) -> TermHandle {
            let case = bound.len();
            if case == this.cases() {
                let bound = bound.clone();
                return inner(scope, motive, &bound);
            }
            let branch = this.branch_type_in(
                scope,
                entries,
                case,
                &|scope| scope.var(motive),
                &mut Vec::new(),
                0,
            );
            let rest = |scope: &mut Scope<'_>, branch: Name| {
                bound.push(branch);
                branches(this, scope, entries, quantify, motive, bound, inner)
            };
            if quantify {
                scope.pi(branch, rest)
            } else {
                scope.lambda(branch, rest)
            }
        }
        let data = scope.constant(entries.data);
        let universe = type_zero(scope.arena);
        let motive_type = pi(scope.arena, data, universe);
        let rest = |scope: &mut Scope<'_>, motive: Name| {
            branches(
                self,
                scope,
                entries,
                quantify,
                motive,
                &mut Vec::new(),
                inner,
            )
        };
        if quantify {
            scope.pi(motive_type, rest)
        } else {
            scope.lambda(motive_type, rest)
        }
    }

    /// The step of the induction over raw trees, with its type. Like the
    /// canonical-tree condition's step it is a declaration of its own: a
    /// recursion the kernel cannot finish then names the step by this
    /// constant applied to the motive and the branches, and the kernel reads
    /// its type from the declaration without checking the step again.
    fn induction_step(&self, scope: &mut Scope<'_>, entries: Entries) -> (TermHandle, TermHandle) {
        let ty = self.over_branches(scope, entries, true, &|scope, motive, _| {
            let labels = scope.constant(entries.labels);
            scope.pi(labels, |scope, label| {
                self.step_type(scope, entries, motive, label)
            })
        });
        let step = self.over_branches(scope, entries, false, &|scope, motive, branches| {
            let labels = scope.constant(entries.labels);
            scope.lambda(labels, |scope, label| {
                let labels = scope.constant(entries.labels);
                let step_motive = scope.lambda(labels, |scope, label| {
                    self.step_type(scope, entries, motive, label)
                });
                let cases: Vec<_> = (0..self.cases())
                    .map(|case| {
                        self.step_branch(
                            scope,
                            entries,
                            case,
                            &|scope| scope.var(motive),
                            branches[case],
                        )
                    })
                    .collect();
                let label = scope.var(label);
                eliminate(scope.arena, self.size(), step_motive, &cases, label)
            })
        });
        (ty, step)
    }

    /// The induction principle and its type:
    /// `(P : data -> Type) -> branch_0 -> .. -> (d : data) -> P d`.
    fn induction(&self, scope: &mut Scope<'_>, entries: Entries) -> (TermHandle, TermHandle) {
        let ty = self.over_branches(scope, entries, true, &|scope, motive, _| {
            let data = scope.constant(entries.data);
            scope.pi(data, |scope, subject| {
                let motive = scope.var(motive);
                let subject = scope.var(subject);
                scope.apply(motive, subject)
            })
        });
        let body = self.over_branches(scope, entries, false, &|scope, motive, branches| {
            let data = scope.constant(entries.data);
            scope.lambda(data, |scope, subject| {
                let tree = scope.constant(entries.tree);
                // Over raw trees: for every proof the tree is canonical, the
                // caller's motive at the tree with that proof.
                let tree_motive = scope.lambda(tree, |scope, tree| {
                    let canonical = scope.constant(entries.canonical);
                    let tree_term = scope.var(tree);
                    let condition = scope.apply(canonical, tree_term);
                    scope.pi(condition, |scope, evidence| {
                        let tree = scope.var(tree);
                        let evidence = scope.var(evidence);
                        let value = scope.pair(tree, evidence);
                        let motive = scope.var(motive);
                        scope.apply(motive, value)
                    })
                });
                let mut step = scope.constant(entries.induction_step);
                let motive_term = scope.var(motive);
                step = scope.apply(step, motive_term);
                for &branch in branches {
                    let branch = scope.var(branch);
                    step = scope.apply(step, branch);
                }
                let subject_term = scope.var(subject);
                let tree = scope.component(subject_term, 0, true);
                let recursion = scope.arena.insert(Term::IndW {
                    motive: tree_motive,
                    step,
                    tree,
                });
                let subject_term = scope.var(subject);
                let evidence = scope.component(subject_term, 1, false);
                scope.apply(recursion, evidence)
            })
        });
        (ty, body)
    }

    /// Every declaration of the data, in order, for a signature in which the
    /// first of them has index `first`: the case labels, their child
    /// positions, the raw trees, the canonical-tree condition's step and the
    /// condition, the data itself, one constructor per case, the induction's
    /// step over raw trees and the induction principle.
    pub fn declarations(&self, arena: &mut TermArena, first: u32) -> Vec<Declaration> {
        let entries = self.entries(Self::data_entry(first));
        let universe = type_zero(arena);
        let labels = self.carrier(arena);
        let positions = self.children(arena);
        let mut scope = Scope { arena, depth: 0 };
        let carrier = scope.constant(entries.labels);
        let family = pi(scope.arena, carrier, universe);
        let children = scope.constant(entries.children);
        let trees = scope.arena.insert(Term::W { carrier, children });
        let mut declarations = vec![
            Declaration::definition(0, universe, labels),
            Declaration::definition(0, family, positions),
            Declaration::definition(0, universe, trees),
        ];
        let tree = scope.constant(entries.tree);
        let canonical_type = pi(scope.arena, tree, universe);
        let (step_type, step) = self.canonical_step(&mut scope, entries);
        declarations.push(Declaration::definition(0, step_type, step));
        let canonical = self.canonical(&mut scope, entries);
        declarations.push(Declaration::definition(0, canonical_type, canonical));
        let data = scope.sigma(tree, |scope, tree| {
            let canonical = scope.constant(entries.canonical);
            let tree = scope.var(tree);
            scope.apply(canonical, tree)
        });
        declarations.push(Declaration::definition(0, universe, data));
        let data = scope.constant(entries.data);
        for case in 0..self.cases() {
            let mut ty = data;
            for _ in 0..self.arities[case] {
                ty = pi(scope.arena, data, ty);
            }
            let constructor = self.constructor(&mut scope, entries, case);
            declarations.push(Declaration::definition(0, ty, constructor));
        }
        let (step_type, step) = self.induction_step(&mut scope, entries);
        declarations.push(Declaration::definition(0, step_type, step));
        let (ty, induction) = self.induction(&mut scope, entries);
        declarations.push(Declaration::definition(0, ty, induction));
        declarations
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
        let mut scope = Scope { arena, depth: 0 };
        self.branch_type_in(
            &mut scope,
            self.entries(data),
            case,
            &|scope| shift(scope.arena, motive, 0, scope.depth),
            &mut Vec::new(),
            0,
        )
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
            levels: Vec::new(),
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

    const PEANO: u32 = 5;
    const ZERO: u32 = 6;
    const SUCC: u32 = 7;
    const PLUS: u32 = 10;

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
        // Under two arbitrary values `m` and `b`.
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
        // It does not compute on a value that is no constructor.
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
        // `plus(a, Zero) = a`. The successor case needs
        // `Succ(plus(m, Zero)) = Succ(m)`, where `Succ(m)` is the very value
        // the case is about. Raw trees cannot state that: a tree of the
        // successor case is `sup(succ, k)` for any child function `k`, which
        // the kernel does not convert to `Succ(k(position))`. Here a value
        // also carries the evidence that `k` is the constructor's own.
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
        // Three cases with no, two and three fields: more than two labels,
        // and child positions of both wider kinds.
        let mut arena = TermArena::new();
        let mut budget = Budget::default();
        let data = PlainData::new(vec![0, 2, 3]).unwrap();
        let mut declarations = data.declarations(&mut arena, 0);
        let tree = PlainData::data_entry(0);
        let ty = constant(&mut arena, tree);
        let (leaf, two, three) = (tree + 1, tree + 2, tree + 3);
        // `pick`: a leaf for a leaf, the first field of the two-field case,
        // the recursion's result for the last field of the three-field case.
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
        // The last field is the pair, whose own pick is `x`.
        assert!(convertible(&mut arena, &context, picked, x, ty, &mut budget).unwrap());
        let leaf_value = constant(&mut arena, leaf);
        let picked = call(&mut arena, pick, &[leaf_value]);
        assert!(convertible(&mut arena, &context, picked, leaf_value, ty, &mut budget).unwrap());
    }

    #[test]
    fn a_declaration_checks_within_one_default_budget() {
        // What the kernel spends on a data's own declarations, mostly on its
        // induction principle. The cost grows with the number of cases and
        // fields; each shape below must stay under its ceiling, and the
        // largest under the default budget of one check.
        for (arities, ceiling) in [
            (vec![0, 1], 1_000),
            (vec![0, 2], 2_500),
            (vec![0, 1, 1], 4_500),
            (vec![0, 2, 3], 18_000),
            (vec![1, 1, 1, 1], 13_000),
        ] {
            let mut arena = TermArena::new();
            let data = PlainData::new(arities.clone()).unwrap();
            let declarations = data.declarations(&mut arena, 0);
            let mut budget = Budget::default();
            let before = budget.remaining();
            check_signature(&mut arena, &declarations, &mut budget)
                .unwrap_or_else(|error| panic!("{arities:?}: {error:?}"));
            let spent = before - budget.remaining();
            eprintln!(
                "{arities:?}: {spent} conversion steps, {} terms",
                arena.len()
            );
            assert!(spent <= ceiling, "{arities:?}: {spent} > {ceiling}");
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
