//! One continuation owner for oriented evidence and its ordered alternatives.
//!
//! Every coercion question spends the original shared construction allowance
//! once, before structural equality or search. Refutation, shape-directed
//! coercion and connective fallback retain their original order and partial
//! allocations. A failed child keeps the allowance it spent; no result or
//! authority is reused across a different premise, goal, or evidence term.

use super::*;

#[derive(Clone, Copy)]
struct Request {
    premise: TermHandle,
    goal: TermHandle,
    evidence: TermHandle,
}

impl Request {
    fn new(premise: TermHandle, goal: TermHandle, evidence: TermHandle) -> Self {
        Self {
            premise,
            goal,
            evidence,
        }
    }
}

#[derive(Clone, Copy)]
struct Cases {
    motive: TermHandle,
    zero: TermHandle,
    one: TermHandle,
    goal_zero: TermHandle,
    goal_one: TermHandle,
    evidence: TermHandle,
}

enum Work {
    Coerce(Request),
    Shape(Request),
    Refute(Request),
    Shuffle(Request),
    ShuffleRest(Request),
    ShuffleGoal(Request),
    Cases(Cases),
}

enum Pending {
    Fallback(Request),
    Refutation(Request),
    RefuteFirst {
        request: Request,
        codomain: TermHandle,
    },
    FunctionArgument {
        evidence: TermHandle,
        codomain: TermHandle,
        goal_domain: TermHandle,
        goal_codomain: TermHandle,
    },
    FunctionBody {
        domain: TermHandle,
    },
    ProductFirst {
        request: Request,
        domain: TermHandle,
        codomain: TermHandle,
        goal_domain: TermHandle,
        goal_codomain: TermHandle,
    },
    PairSecond {
        first: TermHandle,
    },
    CaseZero(Cases),
    CaseOne {
        cases: Cases,
        zero: TermHandle,
    },
    ShuffleFirst {
        request: Request,
        codomain: TermHandle,
    },
    ShuffleSecond(Request),
    GoalFirst {
        request: Request,
        codomain: TermHandle,
    },
    TagZero {
        request: Request,
        one: TermHandle,
    },
    TagOne,
}

pub(super) fn construct(
    d: &mut Denotation,
    premise: TermHandle,
    goal: TermHandle,
    evidence: TermHandle,
    remaining: &mut u64,
) -> Result<Option<TermHandle>, BoundedDenotationError> {
    let mut pending = Vec::new();
    let mut work = Work::Coerce(Request::new(premise, goal, evidence));
    'drive: loop {
        let mut value = match work {
            Work::Coerce(request) => {
                if let Some(rest) = remaining.checked_sub(1) {
                    *remaining = rest;
                    pending.push(Pending::Fallback(request));
                    if d.arena.structurally_equal(request.premise, request.goal) {
                        Some(request.evidence)
                    } else if let Some(inhabited) = d.closed_inhabitant(request.goal)? {
                        Some(inhabited)
                    } else {
                        pending.push(Pending::Refutation(request));
                        work = Work::Refute(request);
                        continue 'drive;
                    }
                } else {
                    // Exhaustion declines immediately. It must not begin the
                    // fallback or reset the allowance for another alternative.
                    None
                }
            }
            Work::Refute(request) => {
                if let Term::Sigma { domain, codomain } = d.arena.get(request.premise)
                    && !d.occurs_free(codomain, 0)
                {
                    let first = d.arena.insert(Term::Fst {
                        pair: request.evidence,
                    });
                    pending.push(Pending::RefuteFirst { request, codomain });
                    work = Work::Refute(Request::new(domain, request.goal, first));
                    continue 'drive;
                }
                d.closed_relation_refutation(request.premise, request.evidence, request.goal)?
            }
            Work::Shape(request) => {
                match (d.arena.get(request.premise), d.arena.get(request.goal)) {
                    (
                        Term::Id { ty, left, right },
                        Term::Id {
                            ty: goal_ty,
                            left: goal_left,
                            right: goal_right,
                        },
                    ) => {
                        if d.arena.structurally_equal(ty, goal_ty)
                            && d.arena.structurally_equal(goal_left, right)
                            && d.arena.structurally_equal(goal_right, left)
                        {
                            Some(d.symmetry(ty, left, right, request.evidence))
                        } else {
                            d.boolean_identity_transport(
                                request.premise,
                                request.goal,
                                request.evidence,
                            )?
                        }
                    }
                    (Term::Id { ty, .. }, _) if d.arena.structurally_equal(ty, d.two) => {
                        d.boolean_goal_transport(request.premise, request.goal, request.evidence)?
                    }
                    (
                        Term::Pi { domain, codomain },
                        Term::Pi {
                            domain: goal_domain,
                            codomain: goal_codomain,
                        },
                    ) => {
                        if d.occurs_free(codomain, 0) || d.occurs_free(goal_codomain, 0) {
                            None
                        } else {
                            let hypothesis = d.arena.insert(Term::Variable(0));
                            pending.push(Pending::FunctionArgument {
                                evidence: request.evidence,
                                codomain,
                                goal_domain,
                                goal_codomain,
                            });
                            work = Work::Coerce(Request::new(goal_domain, domain, hypothesis));
                            continue 'drive;
                        }
                    }
                    (
                        Term::Sigma { domain, codomain },
                        Term::Sigma {
                            domain: goal_domain,
                            codomain: goal_codomain,
                        },
                    ) => {
                        let fst = d.arena.insert(Term::Fst {
                            pair: request.evidence,
                        });
                        pending.push(Pending::ProductFirst {
                            request,
                            domain,
                            codomain,
                            goal_domain,
                            goal_codomain,
                        });
                        work = Work::Coerce(Request::new(domain, goal_domain, fst));
                        continue 'drive;
                    }
                    _ => None,
                }
            }
            Work::Shuffle(request) => {
                if let Term::Sigma { domain, codomain } = d.arena.get(request.premise)
                    && !d.occurs_free(codomain, 0)
                {
                    let projection = d.arena.insert(Term::Fst {
                        pair: request.evidence,
                    });
                    pending.push(Pending::ShuffleFirst { request, codomain });
                    work = Work::Coerce(Request::new(domain, request.goal, projection));
                } else {
                    work = Work::ShuffleRest(request);
                }
                continue 'drive;
            }
            Work::ShuffleRest(request) => {
                if let Some((motive, zero, one)) = d.tagged_sum_parts(request.premise) {
                    work = Work::Cases(elimination_cases(d, request, motive, zero, one));
                } else {
                    work = Work::ShuffleGoal(request);
                }
                continue 'drive;
            }
            Work::ShuffleGoal(request) => {
                if let Term::Sigma { domain, codomain } = d.arena.get(request.goal)
                    && !d.occurs_free(codomain, 0)
                {
                    pending.push(Pending::GoalFirst { request, codomain });
                    work = Work::Coerce(Request::new(request.premise, domain, request.evidence));
                    continue 'drive;
                }
                if let Some((_, zero, one)) = d.tagged_sum_parts(request.goal) {
                    pending.push(Pending::TagZero { request, one });
                    work = Work::Coerce(Request::new(request.premise, zero, request.evidence));
                    continue 'drive;
                }
                None
            }
            Work::Cases(cases) => {
                let hypothesis = d.arena.insert(Term::Variable(0));
                pending.push(Pending::CaseZero(cases));
                work = Work::Coerce(Request::new(cases.zero, cases.goal_zero, hypothesis));
                continue 'drive;
            }
        };
        loop {
            match pending.pop() {
                Some(Pending::Fallback(request)) => {
                    if value.is_none() {
                        work = Work::Shuffle(request);
                        continue 'drive;
                    }
                }
                Some(Pending::Refutation(request)) => {
                    if value.is_none() {
                        work = Work::Shape(request);
                        continue 'drive;
                    }
                }
                Some(Pending::RefuteFirst { request, codomain }) => {
                    if value.is_none() {
                        let second = d.arena.insert(Term::Snd {
                            pair: request.evidence,
                        });
                        work = Work::Refute(Request::new(codomain, request.goal, second));
                        continue 'drive;
                    }
                }
                Some(Pending::FunctionArgument {
                    evidence,
                    codomain,
                    goal_domain,
                    goal_codomain,
                }) => {
                    if let Some(argument) = value {
                        let function = shift(&mut d.arena, evidence, 0, 1);
                        let applied = d.arena.insert(Term::Apply { function, argument });
                        pending.push(Pending::FunctionBody {
                            domain: goal_domain,
                        });
                        work = Work::Coerce(Request::new(codomain, goal_codomain, applied));
                        continue 'drive;
                    }
                }
                Some(Pending::FunctionBody { domain }) => {
                    value = value.map(|body| d.arena.insert(Term::Lambda { domain, body }));
                }
                Some(Pending::ProductFirst {
                    request,
                    domain,
                    codomain,
                    goal_domain,
                    goal_codomain,
                }) => {
                    if let Some(first) = value {
                        let cases = match (d.arena.get(codomain), d.arena.get(goal_codomain)) {
                            (
                                Term::CaseTwo {
                                    motive,
                                    zero_branch: zero,
                                    one_branch: one,
                                    scrutinee,
                                },
                                Term::CaseTwo {
                                    motive: goal_motive,
                                    zero_branch: goal_zero,
                                    one_branch: goal_one,
                                    scrutinee: goal_scrutinee,
                                },
                            ) if d.arena.structurally_equal(motive, goal_motive)
                                && d.arena.structurally_equal(domain, goal_domain)
                                && matches!(d.arena.get(scrutinee), Term::Variable(0))
                                && matches!(d.arena.get(goal_scrutinee), Term::Variable(0))
                                && [motive, zero, one, goal_zero, goal_one]
                                    .into_iter()
                                    .all(|payload| !d.occurs_free(payload, 0)) =>
                            {
                                Some(selected_cases(
                                    d,
                                    motive,
                                    zero,
                                    one,
                                    goal_zero,
                                    goal_one,
                                    request.evidence,
                                ))
                            }
                            _ => None,
                        };
                        if let Some(cases) = cases {
                            pending.push(Pending::PairSecond { first });
                            work = Work::Cases(cases);
                            continue 'drive;
                        }
                        if !d.occurs_free(codomain, 0) && !d.occurs_free(goal_codomain, 0) {
                            let snd = d.arena.insert(Term::Snd {
                                pair: request.evidence,
                            });
                            pending.push(Pending::PairSecond { first });
                            work = Work::Coerce(Request::new(codomain, goal_codomain, snd));
                            continue 'drive;
                        }
                        value = None;
                    }
                }
                Some(Pending::PairSecond { first }) => {
                    value = value.map(|second| d.arena.insert(Term::Pair { first, second }));
                }
                Some(Pending::CaseZero(cases)) => {
                    if let Some(coerced) = value {
                        let zero = d.arena.insert(Term::Lambda {
                            domain: cases.zero,
                            body: coerced,
                        });
                        let hypothesis = d.arena.insert(Term::Variable(0));
                        pending.push(Pending::CaseOne { cases, zero });
                        work = Work::Coerce(Request::new(cases.one, cases.goal_one, hypothesis));
                        continue 'drive;
                    }
                }
                Some(Pending::CaseOne { cases, zero }) => {
                    if let Some(coerced) = value {
                        let one = d.arena.insert(Term::Lambda {
                            domain: cases.one,
                            body: coerced,
                        });
                        let scrutinee = d.arena.insert(Term::Fst {
                            pair: cases.evidence,
                        });
                        let eliminator = d.arena.insert(Term::CaseTwo {
                            motive: cases.motive,
                            zero_branch: zero,
                            one_branch: one,
                            scrutinee,
                        });
                        let argument = d.arena.insert(Term::Snd {
                            pair: cases.evidence,
                        });
                        value = Some(d.arena.insert(Term::Apply {
                            function: eliminator,
                            argument,
                        }));
                    }
                }
                Some(Pending::ShuffleFirst { request, codomain }) => {
                    if value.is_none() {
                        let projection = d.arena.insert(Term::Snd {
                            pair: request.evidence,
                        });
                        pending.push(Pending::ShuffleSecond(request));
                        work = Work::Coerce(Request::new(codomain, request.goal, projection));
                        continue 'drive;
                    }
                }
                Some(Pending::ShuffleSecond(request)) => {
                    if value.is_none() {
                        work = Work::ShuffleRest(request);
                        continue 'drive;
                    }
                }
                Some(Pending::GoalFirst { request, codomain }) => {
                    if let Some(first) = value {
                        pending.push(Pending::PairSecond { first });
                        work =
                            Work::Coerce(Request::new(request.premise, codomain, request.evidence));
                        continue 'drive;
                    }
                }
                Some(Pending::TagZero { request, one }) => {
                    if let Some(payload) = value {
                        value = Some(d.arena.insert(Term::Pair {
                            first: d.two_zero,
                            second: payload,
                        }));
                    } else {
                        pending.push(Pending::TagOne);
                        work = Work::Coerce(Request::new(request.premise, one, request.evidence));
                        continue 'drive;
                    }
                }
                Some(Pending::TagOne) => {
                    value = value.map(|payload| {
                        d.arena.insert(Term::Pair {
                            first: d.two_one,
                            second: payload,
                        })
                    });
                }
                None => return Ok(value),
            }
        }
    }
}

/// The matched tagged families retain their own arm types and shift the
/// target family under the motive's argument binder, exactly before either
/// branch is tried. The branch driver below is shared with sum elimination.
fn selected_cases(
    d: &mut Denotation,
    motive: TermHandle,
    zero: TermHandle,
    one: TermHandle,
    goal_zero: TermHandle,
    goal_one: TermHandle,
    evidence: TermHandle,
) -> Cases {
    let tag = d.arena.insert(Term::Variable(0));
    let premise_family = d.arena.insert(Term::CaseTwo {
        motive,
        zero_branch: zero,
        one_branch: one,
        scrutinee: tag,
    });
    let shifted_tag = d.arena.insert(Term::Variable(1));
    let shifted_goal_zero = shift(&mut d.arena, goal_zero, 0, 1);
    let shifted_goal_one = shift(&mut d.arena, goal_one, 0, 1);
    let shifted_motive = shift(&mut d.arena, motive, 0, 1);
    let goal_family = d.arena.insert(Term::CaseTwo {
        motive: shifted_motive,
        zero_branch: shifted_goal_zero,
        one_branch: shifted_goal_one,
        scrutinee: shifted_tag,
    });
    let body = d.arena.insert(Term::Pi {
        domain: premise_family,
        codomain: goal_family,
    });
    let motive = d.arena.insert(Term::Lambda {
        domain: d.two,
        body,
    });
    Cases {
        motive,
        zero,
        one,
        goal_zero,
        goal_one,
        evidence,
    }
}

fn elimination_cases(
    d: &mut Denotation,
    request: Request,
    motive: TermHandle,
    zero: TermHandle,
    one: TermHandle,
) -> Cases {
    let tag = d.arena.insert(Term::Variable(0));
    let premise_family = d.arena.insert(Term::CaseTwo {
        motive,
        zero_branch: zero,
        one_branch: one,
        scrutinee: tag,
    });
    let shifted_goal = shift(&mut d.arena, request.goal, 0, 2);
    let body = d.arena.insert(Term::Pi {
        domain: premise_family,
        codomain: shifted_goal,
    });
    let motive = d.arena.insert(Term::Lambda {
        domain: d.two,
        body,
    });
    let goal = shift(&mut d.arena, request.goal, 0, 1);
    Cases {
        motive,
        zero,
        one,
        goal_zero: goal,
        goal_one: goal,
        evidence: request.evidence,
    }
}
