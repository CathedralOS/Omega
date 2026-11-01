//! Typed conversion premises scheduled by the common judgment owner.
//!
//! Preparing a later premise can itself fail or spend normalization budget, so
//! comparisons stay sequential. A false premise returns before constructing or
//! inspecting later obligations. Transparency belongs to each comparison;
//! ordinary typing premises never inherit a caller's retained-head policy.

use super::super::super::conversion::{Budget, weak_head_normalize_retaining};
use super::super::super::substitution::{shift, substitute};
use super::super::super::term::{
    Level, Sort, Term, TermArena, TermHandle, levels_equal, sorts_equal,
};
use super::super::{Context, CoreError, box_body_type, inferred_sort, w_step_type};
use super::JudgmentValue;

type Retention<'a> = &'a dyn Fn(u32) -> bool;

fn expand_all(_: u32) -> bool {
    false
}

#[derive(Clone, Copy)]
pub(super) struct Terms {
    pub left: TermHandle,
    pub right: TermHandle,
}

#[derive(Clone, Copy)]
pub(super) struct Query<'a> {
    pub terms: Terms,
    pub shared_type: TermHandle,
    pub retained: Retention<'a>,
}

impl Query<'_> {
    pub(super) fn ordinary(left: TermHandle, right: TermHandle, shared_type: TermHandle) -> Self {
        Self {
            terms: Terms { left, right },
            shared_type,
            retained: &expand_all,
        }
    }
}

impl Terms {
    fn at(self, shared_type: TermHandle, retained: Retention<'_>) -> Query<'_> {
        Query {
            terms: self,
            shared_type,
            retained,
        }
    }
}

pub(super) enum Step<'a> {
    Infer {
        term: TermHandle,
        next: Work<'a>,
    },
    Compare {
        query: Query<'a>,
        next: Option<Work<'a>>,
    },
    Complete(bool),
}

fn infer(term: TermHandle, next: Work<'_>) -> Step<'_> {
    Step::Infer { term, next }
}
fn compare<'a>(query: Query<'a>, next: Option<Work<'a>>) -> Step<'a> {
    Step::Compare { query, next }
}

pub(super) enum Second {
    Stored(TermHandle),
    Project(TermHandle),
}

impl Second {
    fn resolve(self, arena: &mut TermArena) -> TermHandle {
        match self {
            Self::Stored(value) => value,
            Self::Project(pair) => arena.insert(Term::Snd { pair }),
        }
    }
}

pub(super) enum TypeTail {
    Return,
    Binders {
        domain: TermHandle,
        codomains: Terms,
    },
    Identity {
        carrier: TermHandle,
        left: Terms,
        right: Terms,
    },
    Empty {
        scrutinees: Terms,
    },
    Squash {
        proposition: TermHandle,
        functions: Terms,
        scrutinees: Terms,
    },
}

pub(super) enum Elimination {
    Two {
        motive: TermHandle,
        zero: Terms,
        one: Terms,
        scrutinees: Terms,
    },
    Identity {
        motive: TermHandle,
        bases: Terms,
        endpoints: Terms,
        proofs: Terms,
    },
    W {
        motive: TermHandle,
        steps: Terms,
        trees: Terms,
    },
    Box {
        motive: TermHandle,
        bodies: Terms,
        scrutinees: Terms,
    },
}

pub(super) enum Work<'a> {
    SharedSort(Query<'a>),
    TypeSort {
        types: Terms,
        tail: TypeTail,
        retained: Retention<'a>,
    },
    TypesCompared {
        tail: TypeTail,
        retained: Retention<'a>,
    },
    BinderCodomainSort {
        codomains: Terms,
        retained: Retention<'a>,
    },
    ThenCompare(Query<'a>),
    PairFirst {
        left: Second,
        right: Second,
        codomain: TermHandle,
        substitution: TermHandle,
        retained: Retention<'a>,
    },
    FunctionType {
        functions: Terms,
        arguments: Terms,
        retained: Retention<'a>,
    },
    ProjectionType {
        pairs: Terms,
        retained: Retention<'a>,
    },
    MotiveType {
        motives: Terms,
        tail: Elimination,
        retained: Retention<'a>,
    },
    MotivesCompared {
        tail: Elimination,
        retained: Retention<'a>,
    },
    CaseScrutinees {
        motive: TermHandle,
        zero: Terms,
        one: Terms,
        retained: Retention<'a>,
    },
    CaseZero {
        motive: TermHandle,
        one: Terms,
        retained: Retention<'a>,
    },
    IdentityProofType {
        motive: TermHandle,
        bases: Terms,
        endpoints: Terms,
        proofs: Terms,
        retained: Retention<'a>,
    },
    IdentityEndpoints {
        motive: TermHandle,
        bases: Terms,
        proofs: Terms,
        ty: TermHandle,
        fixed: TermHandle,
        proof_type: TermHandle,
        retained: Retention<'a>,
    },
    IdentityProofs {
        motive: TermHandle,
        bases: Terms,
        ty: TermHandle,
        fixed: TermHandle,
        retained: Retention<'a>,
    },
    WLeftSort {
        carriers: Terms,
        families: Terms,
        retained: Retention<'a>,
    },
    WRightSort {
        carriers: Terms,
        families: Terms,
        left_sort: Sort,
        retained: Retention<'a>,
    },
    WCarriers {
        carrier: TermHandle,
        families: Terms,
        retained: Retention<'a>,
    },
    WLeftFamily {
        carrier: TermHandle,
        families: Terms,
        retained: Retention<'a>,
    },
    WRightFamily {
        carrier: TermHandle,
        families: Terms,
        left_level: Option<Level>,
        retained: Retention<'a>,
    },
    SupLabels {
        shared_type: TermHandle,
        children: TermHandle,
        label: TermHandle,
        functions: Terms,
        retained: Retention<'a>,
    },
    InductionTreeType {
        motive: TermHandle,
        steps: Terms,
        trees: Terms,
        retained: Retention<'a>,
    },
    InductionTrees {
        motive: TermHandle,
        steps: Terms,
        carrier: TermHandle,
        children: TermHandle,
        retained: Retention<'a>,
    },
    SquashType {
        proposition: TermHandle,
        functions: Terms,
        scrutinees: Terms,
        retained: Retention<'a>,
    },
    SquashScrutinees {
        proposition: TermHandle,
        functions: Terms,
        carrier: TermHandle,
        retained: Retention<'a>,
    },
    BoxType {
        motive: TermHandle,
        bodies: Terms,
        scrutinees: Terms,
        retained: Retention<'a>,
    },
    BoxScrutinees {
        motive: TermHandle,
        bodies: Terms,
        payload: TermHandle,
        retained: Retention<'a>,
    },
}

fn head(
    arena: &mut TermArena,
    context: &Context,
    term: TermHandle,
    budget: &mut Budget,
    retained: Retention<'_>,
) -> Result<TermHandle, CoreError> {
    weak_head_normalize_retaining(arena, context.signature(), term, budget, retained)
}

fn type_comparison(types: Terms, tail: TypeTail, retained: Retention<'_>) -> Step<'_> {
    infer(
        types.left,
        Work::TypeSort {
            types,
            tail,
            retained,
        },
    )
}

fn motives(motives: Terms, tail: Elimination, retained: Retention<'_>) -> Step<'_> {
    infer(
        motives.left,
        Work::MotiveType {
            motives,
            tail,
            retained,
        },
    )
}

fn family_level(
    arena: &mut TermArena,
    context: &Context,
    inferred: TermHandle,
    budget: &mut Budget,
    retained: Retention<'_>,
) -> Result<Option<Level>, CoreError> {
    let inferred = head(arena, context, inferred, budget, retained)?;
    let Term::Pi { codomain, .. } = arena.get(inferred) else {
        return Ok(None);
    };
    let codomain = head(arena, context, codomain, budget, retained)?;
    Ok(match arena.get(codomain) {
        Term::Sort(Sort::Type(level)) => Some(level),
        _ => None,
    })
}

fn enter_heads<'a>(
    arena: &mut TermArena,
    context: &mut Context,
    query: Query<'a>,
    budget: &mut Budget,
) -> Result<Step<'a>, CoreError> {
    let Query {
        terms: Terms { left, right },
        shared_type,
        retained,
    } = query;
    Ok(match (arena.get(left), arena.get(right)) {
        (Term::Sort(left), Term::Sort(right)) => Step::Complete(sorts_equal(&left, &right)),
        (Term::Two, Term::Two)
        | (Term::TwoZero, Term::TwoZero)
        | (Term::TwoOne, Term::TwoOne)
        | (Term::Empty, Term::Empty) => Step::Complete(true),
        (Term::Variable(left), Term::Variable(right)) => Step::Complete(left == right),
        (
            Term::Constant {
                declaration: left,
                levels: ll,
            },
            Term::Constant {
                declaration: right,
                levels: rl,
            },
        ) => Step::Complete(
            left == right
                && ll.len() == rl.len()
                && ll
                    .iter()
                    .zip(&rl)
                    .all(|(left, right)| levels_equal(left, right)),
        ),
        (
            Term::Pi {
                domain: ld,
                codomain: lc,
            },
            Term::Pi {
                domain: rd,
                codomain: rc,
            },
        )
        | (
            Term::Sigma {
                domain: ld,
                codomain: lc,
            },
            Term::Sigma {
                domain: rd,
                codomain: rc,
            },
        ) => type_comparison(
            Terms {
                left: ld,
                right: rd,
            },
            TypeTail::Binders {
                domain: ld,
                codomains: Terms {
                    left: lc,
                    right: rc,
                },
            },
            retained,
        ),
        (Term::Lambda { body: lb, .. }, Term::Lambda { body: rb, .. }) => {
            let ty = head(arena, context, shared_type, budget, retained)?;
            match arena.get(ty) {
                Term::Pi { domain, codomain } => {
                    context.bindings.push(domain);
                    compare(
                        Terms {
                            left: lb,
                            right: rb,
                        }
                        .at(codomain, retained),
                        None,
                    )
                }
                _ => Step::Complete(false),
            }
        }
        (
            Term::Pair {
                first: lf,
                second: ls,
            },
            Term::Pair {
                first: rf,
                second: rs,
            },
        ) => {
            let ty = head(arena, context, shared_type, budget, retained)?;
            match arena.get(ty) {
                Term::Sigma { domain, codomain } => compare(
                    Terms {
                        left: lf,
                        right: rf,
                    }
                    .at(domain, retained),
                    Some(Work::PairFirst {
                        left: Second::Stored(ls),
                        right: Second::Stored(rs),
                        codomain,
                        substitution: lf,
                        retained,
                    }),
                ),
                _ => Step::Complete(false),
            }
        }
        (
            Term::Apply {
                function: lf,
                argument: la,
            },
            Term::Apply {
                function: rf,
                argument: ra,
            },
        ) => infer(
            lf,
            Work::FunctionType {
                functions: Terms {
                    left: lf,
                    right: rf,
                },
                arguments: Terms {
                    left: la,
                    right: ra,
                },
                retained,
            },
        ),
        (Term::Fst { pair: left }, Term::Fst { pair: right })
        | (Term::Snd { pair: left }, Term::Snd { pair: right }) => infer(
            left,
            Work::ProjectionType {
                pairs: Terms { left, right },
                retained,
            },
        ),
        (
            Term::CaseTwo {
                motive: lm,
                zero_branch: lz,
                one_branch: lo,
                scrutinee: ls,
            },
            Term::CaseTwo {
                motive: rm,
                zero_branch: rz,
                one_branch: ro,
                scrutinee: rs,
            },
        ) => motives(
            Terms {
                left: lm,
                right: rm,
            },
            Elimination::Two {
                motive: lm,
                zero: Terms {
                    left: lz,
                    right: rz,
                },
                one: Terms {
                    left: lo,
                    right: ro,
                },
                scrutinees: Terms {
                    left: ls,
                    right: rs,
                },
            },
            retained,
        ),
        (
            Term::Id {
                ty: lt,
                left: ll,
                right: lr,
            },
            Term::Id {
                ty: rt,
                left: rl,
                right: rr,
            },
        ) => type_comparison(
            Terms {
                left: lt,
                right: rt,
            },
            TypeTail::Identity {
                carrier: lt,
                left: Terms {
                    left: ll,
                    right: rl,
                },
                right: Terms {
                    left: lr,
                    right: rr,
                },
            },
            retained,
        ),
        (Term::Refl { value: lv, .. }, Term::Refl { value: rv, .. }) => {
            let ty = head(arena, context, shared_type, budget, retained)?;
            match arena.get(ty) {
                Term::Id { ty, .. } => compare(
                    Terms {
                        left: lv,
                        right: rv,
                    }
                    .at(ty, retained),
                    None,
                ),
                _ => Step::Complete(false),
            }
        }
        (
            Term::IdElim {
                motive: lm,
                base: lb,
                endpoint: le,
                proof: lp,
            },
            Term::IdElim {
                motive: rm,
                base: rb,
                endpoint: re,
                proof: rp,
            },
        ) => motives(
            Terms {
                left: lm,
                right: rm,
            },
            Elimination::Identity {
                motive: lm,
                bases: Terms {
                    left: lb,
                    right: rb,
                },
                endpoints: Terms {
                    left: le,
                    right: re,
                },
                proofs: Terms {
                    left: lp,
                    right: rp,
                },
            },
            retained,
        ),
        (
            Term::W {
                carrier: lc,
                children: lf,
            },
            Term::W {
                carrier: rc,
                children: rf,
            },
        ) => infer(
            lc,
            Work::WLeftSort {
                carriers: Terms {
                    left: lc,
                    right: rc,
                },
                families: Terms {
                    left: lf,
                    right: rf,
                },
                retained,
            },
        ),
        (
            Term::Sup {
                label: ll,
                function: lf,
                ..
            },
            Term::Sup {
                label: rl,
                function: rf,
                ..
            },
        ) => {
            let ty = head(arena, context, shared_type, budget, retained)?;
            match arena.get(ty) {
                Term::W { carrier, children } => compare(
                    Terms {
                        left: ll,
                        right: rl,
                    }
                    .at(carrier, retained),
                    Some(Work::SupLabels {
                        shared_type: ty,
                        children,
                        label: ll,
                        functions: Terms {
                            left: lf,
                            right: rf,
                        },
                        retained,
                    }),
                ),
                _ => Step::Complete(false),
            }
        }
        (
            Term::IndW {
                motive: lm,
                step: ls,
                tree: lt,
            },
            Term::IndW {
                motive: rm,
                step: rs,
                tree: rt,
            },
        ) => motives(
            Terms {
                left: lm,
                right: rm,
            },
            Elimination::W {
                motive: lm,
                steps: Terms {
                    left: ls,
                    right: rs,
                },
                trees: Terms {
                    left: lt,
                    right: rt,
                },
            },
            retained,
        ),
        (Term::Squash { ty: left }, Term::Squash { ty: right })
        | (Term::Box { ty: left }, Term::Box { ty: right }) => {
            type_comparison(Terms { left, right }, TypeTail::Return, retained)
        }
        (Term::SquashIntro { value: lv, .. }, Term::SquashIntro { value: rv, .. }) => {
            let ty = head(arena, context, shared_type, budget, retained)?;
            match arena.get(ty) {
                Term::Squash { ty } => compare(
                    Terms {
                        left: lv,
                        right: rv,
                    }
                    .at(ty, retained),
                    None,
                ),
                _ => Step::Complete(false),
            }
        }
        (Term::BoxIntro { value: lv, .. }, Term::BoxIntro { value: rv, .. }) => {
            let ty = head(arena, context, shared_type, budget, retained)?;
            match arena.get(ty) {
                Term::Box { ty } => compare(
                    Terms {
                        left: lv,
                        right: rv,
                    }
                    .at(ty, retained),
                    None,
                ),
                _ => Step::Complete(false),
            }
        }
        (
            Term::EmptyElim {
                ty: lt,
                scrutinee: ls,
            },
            Term::EmptyElim {
                ty: rt,
                scrutinee: rs,
            },
        ) => type_comparison(
            Terms {
                left: lt,
                right: rt,
            },
            TypeTail::Empty {
                scrutinees: Terms {
                    left: ls,
                    right: rs,
                },
            },
            retained,
        ),
        (
            Term::SquashElim {
                proposition: lp,
                function: lf,
                scrutinee: ls,
            },
            Term::SquashElim {
                proposition: rp,
                function: rf,
                scrutinee: rs,
            },
        ) => type_comparison(
            Terms {
                left: lp,
                right: rp,
            },
            TypeTail::Squash {
                proposition: lp,
                functions: Terms {
                    left: lf,
                    right: rf,
                },
                scrutinees: Terms {
                    left: ls,
                    right: rs,
                },
            },
            retained,
        ),
        (
            Term::BoxElim {
                motive: lm,
                body: lb,
                scrutinee: ls,
            },
            Term::BoxElim {
                motive: rm,
                body: rb,
                scrutinee: rs,
            },
        ) => motives(
            Terms {
                left: lm,
                right: rm,
            },
            Elimination::Box {
                motive: lm,
                bodies: Terms {
                    left: lb,
                    right: rb,
                },
                scrutinees: Terms {
                    left: ls,
                    right: rs,
                },
            },
            retained,
        ),
        (Term::Pair { first, second }, _) => {
            let ty = head(arena, context, shared_type, budget, retained)?;
            match arena.get(ty) {
                Term::Sigma { domain, codomain } => {
                    let other_first = arena.insert(Term::Fst { pair: right });
                    compare(
                        Terms {
                            left: first,
                            right: other_first,
                        }
                        .at(domain, retained),
                        Some(Work::PairFirst {
                            left: Second::Stored(second),
                            right: Second::Project(right),
                            codomain,
                            substitution: other_first,
                            retained,
                        }),
                    )
                }
                _ => Step::Complete(false),
            }
        }
        (_, Term::Pair { first, second }) => {
            let ty = head(arena, context, shared_type, budget, retained)?;
            match arena.get(ty) {
                Term::Sigma { domain, codomain } => {
                    let other_first = arena.insert(Term::Fst { pair: left });
                    compare(
                        Terms {
                            left: other_first,
                            right: first,
                        }
                        .at(domain, retained),
                        Some(Work::PairFirst {
                            left: Second::Project(left),
                            right: Second::Stored(second),
                            codomain,
                            substitution: other_first,
                            retained,
                        }),
                    )
                }
                _ => Step::Complete(false),
            }
        }
        (Term::Lambda { .. }, _) | (_, Term::Lambda { .. }) => {
            let ty = head(arena, context, shared_type, budget, retained)?;
            match arena.get(ty) {
                Term::Pi { domain, codomain } => {
                    let (body, other) = match (arena.get(left), arena.get(right)) {
                        (Term::Lambda { body, .. }, _) => (body, right),
                        (_, Term::Lambda { body, .. }) => (body, left),
                        _ => unreachable!("one lambda is present"),
                    };
                    context.bindings.push(domain);
                    let lifted = shift(arena, other, 0, 1);
                    let fresh = arena.insert(Term::Variable(0));
                    let applied = arena.insert(Term::Apply {
                        function: lifted,
                        argument: fresh,
                    });
                    compare(
                        Terms {
                            left: body,
                            right: applied,
                        }
                        .at(codomain, retained),
                        None,
                    )
                }
                _ => Step::Complete(false),
            }
        }
        _ => Step::Complete(false),
    })
}

impl<'a> Work<'a> {
    pub(super) fn resume(
        self,
        arena: &mut TermArena,
        context: &mut Context,
        value: JudgmentValue,
        budget: &mut Budget,
    ) -> Result<Step<'a>, CoreError> {
        Ok(match self {
            Self::SharedSort(query) => {
                let inferred = value.require_inferred();
                let sort = inferred_sort(arena, context, query.shared_type, inferred, budget)?;
                // Malformed shared types refuse before reflexivity, strict
                // collapse, or either term is inspected.
                if sort.is_strict() || arena.structurally_equal(query.terms.left, query.terms.right)
                {
                    Step::Complete(true)
                } else {
                    let left = head(arena, context, query.terms.left, budget, query.retained)?;
                    let right = head(arena, context, query.terms.right, budget, query.retained)?;
                    if arena.structurally_equal(left, right) {
                        Step::Complete(true)
                    } else {
                        return enter_heads(
                            arena,
                            context,
                            Query {
                                terms: Terms { left, right },
                                ..query
                            },
                            budget,
                        );
                    }
                }
            }
            Self::TypeSort {
                types,
                tail,
                retained,
            } => {
                let sort =
                    inferred_sort(arena, context, types.left, value.require_inferred(), budget)?;
                let shared = arena.insert(Term::Sort(sort));
                let next = match tail {
                    TypeTail::Return => None,
                    tail => Some(Self::TypesCompared { tail, retained }),
                };
                compare(types.at(shared, retained), next)
            }
            Self::TypesCompared { tail, retained } => {
                if !value.require_converted() {
                    return Ok(Step::Complete(false));
                }
                match tail {
                    TypeTail::Return => Step::Complete(true),
                    TypeTail::Binders { domain, codomains } => {
                        context.bindings.push(domain);
                        infer(
                            codomains.left,
                            Self::BinderCodomainSort {
                                codomains,
                                retained,
                            },
                        )
                    }
                    TypeTail::Identity {
                        carrier,
                        left,
                        right,
                    } => compare(
                        left.at(carrier, retained),
                        Some(Self::ThenCompare(right.at(carrier, retained))),
                    ),
                    TypeTail::Empty { scrutinees } => {
                        let empty = arena.insert(Term::Empty);
                        compare(scrutinees.at(empty, retained), None)
                    }
                    TypeTail::Squash {
                        proposition,
                        functions,
                        scrutinees,
                    } => infer(
                        scrutinees.left,
                        Self::SquashType {
                            proposition,
                            functions,
                            scrutinees,
                            retained,
                        },
                    ),
                }
            }
            Self::BinderCodomainSort {
                codomains,
                retained,
            } => {
                let sort = inferred_sort(
                    arena,
                    context,
                    codomains.left,
                    value.require_inferred(),
                    budget,
                )?;
                let shared = arena.insert(Term::Sort(sort));
                compare(codomains.at(shared, retained), None)
            }
            Self::ThenCompare(query) => {
                if value.require_converted() {
                    compare(query, None)
                } else {
                    Step::Complete(false)
                }
            }
            Self::PairFirst {
                left,
                right,
                codomain,
                substitution,
                retained,
            } => {
                if !value.require_converted() {
                    return Ok(Step::Complete(false));
                }
                let left = left.resolve(arena);
                let right = right.resolve(arena);
                let second_type = substitute(arena, codomain, substitution);
                compare(Terms { left, right }.at(second_type, retained), None)
            }
            Self::FunctionType {
                functions,
                arguments,
                retained,
            } => {
                let function_type = value.require_inferred();
                let type_head = head(arena, context, function_type, budget, retained)?;
                match arena.get(type_head) {
                    Term::Pi { domain, .. } => compare(
                        arguments.at(domain, retained),
                        Some(Self::ThenCompare(functions.at(function_type, retained))),
                    ),
                    _ => Step::Complete(false),
                }
            }
            Self::ProjectionType { pairs, retained } => {
                let inferred = head(arena, context, value.require_inferred(), budget, retained)?;
                match arena.get(inferred) {
                    Term::Sigma { .. } => compare(pairs.at(inferred, retained), None),
                    _ => Step::Complete(false),
                }
            }
            Self::MotiveType {
                motives,
                tail,
                retained,
            } => compare(
                motives.at(value.require_inferred(), retained),
                Some(Self::MotivesCompared { tail, retained }),
            ),
            Self::MotivesCompared { tail, retained } => {
                if !value.require_converted() {
                    return Ok(Step::Complete(false));
                }
                match tail {
                    Elimination::Two {
                        motive,
                        zero,
                        one,
                        scrutinees,
                    } => {
                        let two = arena.insert(Term::Two);
                        compare(
                            scrutinees.at(two, retained),
                            Some(Self::CaseScrutinees {
                                motive,
                                zero,
                                one,
                                retained,
                            }),
                        )
                    }
                    Elimination::Identity {
                        motive,
                        bases,
                        endpoints,
                        proofs,
                    } => infer(
                        proofs.left,
                        Self::IdentityProofType {
                            motive,
                            bases,
                            endpoints,
                            proofs,
                            retained,
                        },
                    ),
                    Elimination::W {
                        motive,
                        steps,
                        trees,
                    } => infer(
                        trees.left,
                        Self::InductionTreeType {
                            motive,
                            steps,
                            trees,
                            retained,
                        },
                    ),
                    Elimination::Box {
                        motive,
                        bodies,
                        scrutinees,
                    } => infer(
                        scrutinees.left,
                        Self::BoxType {
                            motive,
                            bodies,
                            scrutinees,
                            retained,
                        },
                    ),
                }
            }
            Self::CaseScrutinees {
                motive,
                zero,
                one,
                retained,
            } => {
                if !value.require_converted() {
                    return Ok(Step::Complete(false));
                }
                let constructor = arena.insert(Term::TwoZero);
                let shared = arena.insert(Term::Apply {
                    function: motive,
                    argument: constructor,
                });
                compare(
                    zero.at(shared, retained),
                    Some(Self::CaseZero {
                        motive,
                        one,
                        retained,
                    }),
                )
            }
            Self::CaseZero {
                motive,
                one,
                retained,
            } => {
                if !value.require_converted() {
                    return Ok(Step::Complete(false));
                }
                let constructor = arena.insert(Term::TwoOne);
                let shared = arena.insert(Term::Apply {
                    function: motive,
                    argument: constructor,
                });
                compare(one.at(shared, retained), None)
            }
            Self::IdentityProofType {
                motive,
                bases,
                endpoints,
                proofs,
                retained,
            } => {
                let proof_type = head(arena, context, value.require_inferred(), budget, retained)?;
                match arena.get(proof_type) {
                    Term::Id {
                        ty, left: fixed, ..
                    } => compare(
                        endpoints.at(ty, retained),
                        Some(Self::IdentityEndpoints {
                            motive,
                            bases,
                            proofs,
                            ty,
                            fixed,
                            proof_type,
                            retained,
                        }),
                    ),
                    _ => Step::Complete(false),
                }
            }
            Self::IdentityEndpoints {
                motive,
                bases,
                proofs,
                ty,
                fixed,
                proof_type,
                retained,
            } => {
                if !value.require_converted() {
                    return Ok(Step::Complete(false));
                }
                compare(
                    proofs.at(proof_type, retained),
                    Some(Self::IdentityProofs {
                        motive,
                        bases,
                        ty,
                        fixed,
                        retained,
                    }),
                )
            }
            Self::IdentityProofs {
                motive,
                bases,
                ty,
                fixed,
                retained,
            } => {
                if !value.require_converted() {
                    return Ok(Step::Complete(false));
                }
                let refl = arena.insert(Term::Refl { ty, value: fixed });
                let fixed = arena.insert(Term::Apply {
                    function: motive,
                    argument: fixed,
                });
                let shared = arena.insert(Term::Apply {
                    function: fixed,
                    argument: refl,
                });
                compare(bases.at(shared, retained), None)
            }
            Self::WLeftSort {
                carriers,
                families,
                retained,
            } => {
                let left_sort = inferred_sort(
                    arena,
                    context,
                    carriers.left,
                    value.require_inferred(),
                    budget,
                )?;
                infer(
                    carriers.right,
                    Self::WRightSort {
                        carriers,
                        families,
                        left_sort,
                        retained,
                    },
                )
            }
            Self::WRightSort {
                carriers,
                families,
                left_sort,
                retained,
            } => {
                let right_sort = inferred_sort(
                    arena,
                    context,
                    carriers.right,
                    value.require_inferred(),
                    budget,
                )?;
                if !sorts_equal(&left_sort, &right_sort) {
                    return Ok(Step::Complete(false));
                }
                let shared = arena.insert(Term::Sort(left_sort));
                compare(
                    carriers.at(shared, retained),
                    Some(Self::WCarriers {
                        carrier: carriers.left,
                        families,
                        retained,
                    }),
                )
            }
            Self::WCarriers {
                carrier,
                families,
                retained,
            } => {
                if !value.require_converted() {
                    return Ok(Step::Complete(false));
                }
                infer(
                    families.left,
                    Self::WLeftFamily {
                        carrier,
                        families,
                        retained,
                    },
                )
            }
            Self::WLeftFamily {
                carrier,
                families,
                retained,
            } => {
                let left_level =
                    family_level(arena, context, value.require_inferred(), budget, retained)?;
                // The old judgment observes both family types even if the
                // first has no suitable codomain. Preserve that error order.
                infer(
                    families.right,
                    Self::WRightFamily {
                        carrier,
                        families,
                        left_level,
                        retained,
                    },
                )
            }
            Self::WRightFamily {
                carrier,
                families,
                left_level,
                retained,
            } => {
                let right_level =
                    family_level(arena, context, value.require_inferred(), budget, retained)?;
                let (Some(level), Some(other)) = (left_level, right_level) else {
                    return Ok(Step::Complete(false));
                };
                if !levels_equal(&level, &other) {
                    return Ok(Step::Complete(false));
                }
                let codomain = arena.insert(Term::Sort(Sort::Type(level)));
                let shared = arena.insert(Term::Pi {
                    domain: carrier,
                    codomain,
                });
                compare(families.at(shared, retained), None)
            }
            Self::SupLabels {
                shared_type,
                children,
                label,
                functions,
                retained,
            } => {
                if !value.require_converted() {
                    return Ok(Step::Complete(false));
                }
                let domain = arena.insert(Term::Apply {
                    function: children,
                    argument: label,
                });
                let codomain = shift(arena, shared_type, 0, 1);
                let shared = arena.insert(Term::Pi { domain, codomain });
                compare(functions.at(shared, retained), None)
            }
            Self::InductionTreeType {
                motive,
                steps,
                trees,
                retained,
            } => {
                let inferred = head(arena, context, value.require_inferred(), budget, retained)?;
                match arena.get(inferred) {
                    Term::W { carrier, children } => compare(
                        trees.at(inferred, retained),
                        Some(Self::InductionTrees {
                            motive,
                            steps,
                            carrier,
                            children,
                            retained,
                        }),
                    ),
                    _ => Step::Complete(false),
                }
            }
            Self::InductionTrees {
                motive,
                steps,
                carrier,
                children,
                retained,
            } => {
                if !value.require_converted() {
                    return Ok(Step::Complete(false));
                }
                let shared = w_step_type(arena, carrier, children, motive);
                compare(steps.at(shared, retained), None)
            }
            Self::SquashType {
                proposition,
                functions,
                scrutinees,
                retained,
            } => {
                let inferred = head(arena, context, value.require_inferred(), budget, retained)?;
                match arena.get(inferred) {
                    Term::Squash { ty: carrier } => compare(
                        scrutinees.at(inferred, retained),
                        Some(Self::SquashScrutinees {
                            proposition,
                            functions,
                            carrier,
                            retained,
                        }),
                    ),
                    _ => Step::Complete(false),
                }
            }
            Self::SquashScrutinees {
                proposition,
                functions,
                carrier,
                retained,
            } => {
                if !value.require_converted() {
                    return Ok(Step::Complete(false));
                }
                let codomain = shift(arena, proposition, 0, 1);
                let shared = arena.insert(Term::Pi {
                    domain: carrier,
                    codomain,
                });
                compare(functions.at(shared, retained), None)
            }
            Self::BoxType {
                motive,
                bodies,
                scrutinees,
                retained,
            } => {
                let inferred = head(arena, context, value.require_inferred(), budget, retained)?;
                match arena.get(inferred) {
                    Term::Box { ty: payload } => compare(
                        scrutinees.at(inferred, retained),
                        Some(Self::BoxScrutinees {
                            motive,
                            bodies,
                            payload,
                            retained,
                        }),
                    ),
                    _ => Step::Complete(false),
                }
            }
            Self::BoxScrutinees {
                motive,
                bodies,
                payload,
                retained,
            } => {
                if !value.require_converted() {
                    return Ok(Step::Complete(false));
                }
                let shared = box_body_type(arena, payload, motive);
                compare(bodies.at(shared, retained), None)
            }
        })
    }
}
