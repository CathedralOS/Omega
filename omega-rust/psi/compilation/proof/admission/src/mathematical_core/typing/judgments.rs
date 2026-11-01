//! One continuation owner for constructor inference and bidirectional checking.
//!
//! A worklist per constructor still nests native calls when a lambda body is a
//! pair, a binder domain is an application, or a projection selects another
//! constructor, and a dependent pair argument switches from inference to
//! componentwise checking. These frames compose those judgments under one
//! mutable binder prefix. Returning from a check restores only that check's
//! binders; siblings never inherit them. Inductive and strict-layer rules
//! schedule their checked premises here too. Typed conversion still owns its
//! separate comparison rules and is the remaining recursive judgment route.

use super::super::conversion::{Budget, convertible, weak_head_normalize};
use super::super::substitution::{shift, substitute};
use super::super::term::{Level, Sort, Term, TermArena, TermHandle};
use super::{Context, CoreError, box_body_type, infer_leaf_type, inferred_sort, w_step_type};

enum Formation {
    Pi,
    Sigma,
}

enum Layer {
    Squash,
    Box,
}

enum IdentityUse {
    Formation { left: TermHandle, right: TermHandle },
    Reflexivity { value: TermHandle },
}

enum WUse {
    Formation,
    Constructor {
        label: TermHandle,
        function: TermHandle,
    },
}

struct IdentityElimination {
    motive: TermHandle,
    base: TermHandle,
    endpoint: TermHandle,
    proof: TermHandle,
    ty: TermHandle,
    fixed: TermHandle,
}

enum MotiveUse {
    Identity(IdentityElimination),
    Two {
        zero_branch: TermHandle,
        one_branch: TermHandle,
        scrutinee: TermHandle,
    },
    W {
        carrier: TermHandle,
        children: TermHandle,
        tree: TermHandle,
        step: TermHandle,
    },
    Box {
        payload: TermHandle,
        body: TermHandle,
        scrutinee: TermHandle,
    },
}

enum Pending {
    IdentityProof {
        motive: TermHandle,
        base: TermHandle,
        endpoint: TermHandle,
        proof: TermHandle,
    },
    IdentityEndpoint {
        elimination: IdentityElimination,
        recorded: TermHandle,
    },
    IdentityMotiveDomain {
        elimination: IdentityElimination,
        proof_domain: TermHandle,
        motive_codomain: TermHandle,
        expected_domain: TermHandle,
        prefix: usize,
    },
    IdentityBase {
        motive: TermHandle,
        endpoint: TermHandle,
        proof: TermHandle,
    },

    MotiveType {
        motive: TermHandle,
        scrutinee_type: TermHandle,
        use_kind: MotiveUse,
    },
    MotiveDomain {
        motive: TermHandle,
        domain: TermHandle,
        codomain: TermHandle,
        scrutinee_type: TermHandle,
        use_kind: MotiveUse,
    },
    CaseScrutinee {
        motive: TermHandle,
        zero_branch: TermHandle,
        one_branch: TermHandle,
        scrutinee: TermHandle,
    },
    CaseZero {
        motive: TermHandle,
        one_branch: TermHandle,
        scrutinee: TermHandle,
    },
    ReturnApplication {
        function: TermHandle,
        argument: TermHandle,
    },
    InductionTree {
        motive: TermHandle,
        step: TermHandle,
        tree: TermHandle,
    },
    BoxScrutinee {
        motive: TermHandle,
        body: TermHandle,
        scrutinee: TermHandle,
    },
    SquashScrutinee {
        proposition: TermHandle,
        function: TermHandle,
        scrutinee: TermHandle,
    },
    SquashTarget {
        proposition: TermHandle,
        function: TermHandle,
        carrier: TermHandle,
    },

    WCarrier {
        carrier: TermHandle,
        children: TermHandle,
        use_kind: WUse,
    },
    WChildren {
        carrier: TermHandle,
        children: TermHandle,
        carrier_level: Level,
        use_kind: WUse,
    },
    WDomain {
        carrier: TermHandle,
        children: TermHandle,
        carrier_level: Level,
        domain: TermHandle,
        codomain: TermHandle,
        use_kind: WUse,
    },
    SupLabel {
        carrier: TermHandle,
        children: TermHandle,
        label: TermHandle,
        function: TermHandle,
    },

    IdentityDomain {
        ty: TermHandle,
        use_kind: IdentityUse,
    },
    IdentityLeft {
        ty: TermHandle,
        right: TermHandle,
        sort: Sort,
    },
    IdentityRight {
        sort: Sort,
    },
    ReflexivityValue {
        ty: TermHandle,
        value: TermHandle,
    },
    LayerDomain {
        layer: Layer,
        ty: TermHandle,
        value: TermHandle,
    },
    LayerIntroduction {
        layer: Layer,
        ty: TermHandle,
    },
    EmptyTarget {
        ty: TermHandle,
        scrutinee: TermHandle,
    },
    ReturnType {
        ty: TermHandle,
    },

    CheckScope {
        prefix: usize,
    },
    CheckExpected {
        term: TermHandle,
        expected: TermHandle,
    },
    CheckActual {
        expected: TermHandle,
        shared_type: TermHandle,
    },
    CheckPairSecond {
        first: TermHandle,
        second: TermHandle,
        codomain: TermHandle,
        prefix: usize,
    },
    CheckLambdaDomain {
        domain: TermHandle,
        body: TermHandle,
        expected_domain: TermHandle,
        codomain: TermHandle,
    },
    ApplyPair {
        argument: TermHandle,
        codomain: TermHandle,
    },

    FormationDomain {
        formation: Formation,
        domain: TermHandle,
        codomain: TermHandle,
    },
    FormationCodomain {
        formation: Formation,
        codomain: TermHandle,
        domain_sort: Sort,
        prefix: usize,
    },
    Layer {
        layer: Layer,
        domain: TermHandle,
    },
    LambdaDomain {
        domain: TermHandle,
        body: TermHandle,
    },
    LambdaBody {
        domain: TermHandle,
        prefix: usize,
    },
    PairFirst {
        second: TermHandle,
    },
    PairSecond {
        first_type: TermHandle,
    },
    ApplyFunction {
        function: TermHandle,
        argument: TermHandle,
    },
    ApplyArgument {
        domain: TermHandle,
        codomain: TermHandle,
        argument: TermHandle,
    },
    ApplyDomain {
        domain: TermHandle,
        codomain: TermHandle,
        argument: TermHandle,
        argument_type: TermHandle,
    },
    Projection {
        is_first: bool,
        pair: TermHandle,
    },
}

enum Operation {
    Infer(TermHandle),
    Check {
        term: TermHandle,
        expected: TermHandle,
    },
}

enum JudgmentValue {
    Inferred(TermHandle),
    Checked,
}

impl JudgmentValue {
    fn require_inferred(self) -> TermHandle {
        match self {
            Self::Inferred(ty) => ty,
            Self::Checked => unreachable!("inference continuation consumes an inferred type"),
        }
    }

    fn require_checked(self) {
        match self {
            Self::Checked => {}
            Self::Inferred(_) => unreachable!("checking continuation consumes a completed check"),
        }
    }
}

pub(super) fn infer(
    arena: &mut TermArena,
    context: &Context,
    term: TermHandle,
    budget: &mut Budget,
) -> Result<TermHandle, CoreError> {
    drive(arena, context, Operation::Infer(term), budget).map(JudgmentValue::require_inferred)
}

pub(super) fn check(
    arena: &mut TermArena,
    context: &Context,
    term: TermHandle,
    expected: TermHandle,
    budget: &mut Budget,
) -> Result<(), CoreError> {
    drive(arena, context, Operation::Check { term, expected }, budget)
        .map(JudgmentValue::require_checked)
}

fn drive(
    arena: &mut TermArena,
    context: &Context,
    operation: Operation,
    budget: &mut Budget,
) -> Result<JudgmentValue, CoreError> {
    let mut context = context.clone();
    let mut pending = Vec::new();
    let mut current = operation;
    loop {
        let mut value = match current {
            Operation::Check { term, expected } => {
                pending.push(Pending::CheckScope {
                    prefix: context.bindings.len(),
                });
                pending.push(Pending::CheckExpected { term, expected });
                current = Operation::Infer(expected);
                continue;
            }
            Operation::Infer(term) => JudgmentValue::Inferred(match arena.get(term) {
                Term::Pi { domain, codomain } => {
                    pending.push(Pending::FormationDomain {
                        formation: Formation::Pi,
                        domain,
                        codomain,
                    });
                    current = Operation::Infer(domain);
                    continue;
                }
                Term::Sigma { domain, codomain } => {
                    pending.push(Pending::FormationDomain {
                        formation: Formation::Sigma,
                        domain,
                        codomain,
                    });
                    current = Operation::Infer(domain);
                    continue;
                }
                Term::Squash { ty } => {
                    pending.push(Pending::Layer {
                        layer: Layer::Squash,
                        domain: ty,
                    });
                    current = Operation::Infer(ty);
                    continue;
                }
                Term::Box { ty } => {
                    pending.push(Pending::Layer {
                        layer: Layer::Box,
                        domain: ty,
                    });
                    current = Operation::Infer(ty);
                    continue;
                }
                Term::Lambda { domain, body } => {
                    pending.push(Pending::LambdaDomain { domain, body });
                    current = Operation::Infer(domain);
                    continue;
                }
                Term::Pair { first, second } => {
                    pending.push(Pending::PairFirst { second });
                    current = Operation::Infer(first);
                    continue;
                }
                Term::Apply { function, argument } => {
                    pending.push(Pending::ApplyFunction { function, argument });
                    current = Operation::Infer(function);
                    continue;
                }
                Term::Fst { pair } => {
                    pending.push(Pending::Projection {
                        is_first: true,
                        pair,
                    });
                    current = Operation::Infer(pair);
                    continue;
                }
                Term::Snd { pair } => {
                    pending.push(Pending::Projection {
                        is_first: false,
                        pair,
                    });
                    current = Operation::Infer(pair);
                    continue;
                }
                Term::Id { ty, left, right } => {
                    pending.push(Pending::IdentityDomain {
                        ty,
                        use_kind: IdentityUse::Formation { left, right },
                    });
                    current = Operation::Infer(ty);
                    continue;
                }
                Term::Refl { ty, value } => {
                    pending.push(Pending::IdentityDomain {
                        ty,
                        use_kind: IdentityUse::Reflexivity { value },
                    });
                    current = Operation::Infer(ty);
                    continue;
                }
                Term::SquashIntro { ty, value } => {
                    pending.push(Pending::LayerDomain {
                        layer: Layer::Squash,
                        ty,
                        value,
                    });
                    current = Operation::Infer(ty);
                    continue;
                }
                Term::BoxIntro { ty, value } => {
                    pending.push(Pending::LayerDomain {
                        layer: Layer::Box,
                        ty,
                        value,
                    });
                    current = Operation::Infer(ty);
                    continue;
                }
                Term::EmptyElim { ty, scrutinee } => {
                    pending.push(Pending::EmptyTarget { ty, scrutinee });
                    current = Operation::Infer(ty);
                    continue;
                }
                Term::W { carrier, children } => {
                    pending.push(Pending::WCarrier {
                        carrier,
                        children,
                        use_kind: WUse::Formation,
                    });
                    current = Operation::Infer(carrier);
                    continue;
                }
                Term::Sup {
                    carrier,
                    children,
                    label,
                    function,
                } => {
                    pending.push(Pending::WCarrier {
                        carrier,
                        children,
                        use_kind: WUse::Constructor { label, function },
                    });
                    current = Operation::Infer(carrier);
                    continue;
                }
                Term::CaseTwo {
                    motive,
                    zero_branch,
                    one_branch,
                    scrutinee,
                } => {
                    let two = arena.insert(Term::Two);
                    pending.push(Pending::MotiveType {
                        motive,
                        scrutinee_type: two,
                        use_kind: MotiveUse::Two {
                            zero_branch,
                            one_branch,
                            scrutinee,
                        },
                    });
                    current = Operation::Infer(motive);
                    continue;
                }
                Term::IndW { motive, step, tree } => {
                    pending.push(Pending::InductionTree { motive, step, tree });
                    current = Operation::Infer(tree);
                    continue;
                }
                Term::BoxElim {
                    motive,
                    body,
                    scrutinee,
                } => {
                    pending.push(Pending::BoxScrutinee {
                        motive,
                        body,
                        scrutinee,
                    });
                    current = Operation::Infer(scrutinee);
                    continue;
                }
                Term::SquashElim {
                    proposition,
                    function,
                    scrutinee,
                } => {
                    pending.push(Pending::SquashScrutinee {
                        proposition,
                        function,
                        scrutinee,
                    });
                    current = Operation::Infer(scrutinee);
                    continue;
                }
                Term::IdElim {
                    motive,
                    base,
                    endpoint,
                    proof,
                } => {
                    pending.push(Pending::IdentityProof {
                        motive,
                        base,
                        endpoint,
                        proof,
                    });
                    current = Operation::Infer(proof);
                    continue;
                }
                node => infer_leaf_type(arena, &context, node)?,
            }),
        };
        loop {
            match pending.pop() {
                Some(Pending::CheckScope { prefix }) => {
                    value.require_checked();
                    context.bindings.truncate(prefix);
                    value = JudgmentValue::Checked;
                }
                Some(Pending::CheckPairSecond {
                    first,
                    second,
                    codomain,
                    prefix,
                }) => {
                    value.require_checked();
                    context.bindings.truncate(prefix);
                    current = Operation::Check {
                        term: second,
                        expected: substitute(arena, codomain, first),
                    };
                    break;
                }
                Some(Pending::ApplyPair { argument, codomain }) => {
                    value.require_checked();
                    value = JudgmentValue::Inferred(substitute(arena, codomain, argument));
                }
                Some(Pending::IdentityLeft { ty, right, sort }) => {
                    value.require_checked();
                    pending.push(Pending::IdentityRight { sort });
                    current = Operation::Check {
                        term: right,
                        expected: ty,
                    };
                    break;
                }
                Some(Pending::IdentityRight { sort }) => {
                    value.require_checked();
                    value = JudgmentValue::Inferred(arena.insert(Term::Sort(sort)));
                }
                Some(Pending::ReflexivityValue {
                    ty,
                    value: endpoint,
                }) => {
                    value.require_checked();
                    value = JudgmentValue::Inferred(arena.insert(Term::Id {
                        ty,
                        left: endpoint,
                        right: endpoint,
                    }));
                }
                Some(Pending::LayerIntroduction { layer, ty }) => {
                    value.require_checked();
                    value = JudgmentValue::Inferred(arena.insert(match layer {
                        Layer::Squash => Term::Squash { ty },
                        Layer::Box => Term::Box { ty },
                    }));
                }
                Some(Pending::ReturnType { ty }) => {
                    value.require_checked();
                    value = JudgmentValue::Inferred(ty);
                }
                Some(Pending::SupLabel {
                    carrier,
                    children,
                    label,
                    function,
                }) => {
                    value.require_checked();
                    let domain = arena.insert(Term::Apply {
                        function: children,
                        argument: label,
                    });
                    let w_type = arena.insert(Term::W { carrier, children });
                    let codomain = shift(arena, w_type, 0, 1);
                    let function_type = arena.insert(Term::Pi { domain, codomain });
                    pending.push(Pending::ReturnType { ty: w_type });
                    current = Operation::Check {
                        term: function,
                        expected: function_type,
                    };
                    break;
                }
                Some(Pending::CaseScrutinee {
                    motive,
                    zero_branch,
                    one_branch,
                    scrutinee,
                }) => {
                    value.require_checked();
                    let zero = arena.insert(Term::TwoZero);
                    let zero_type = arena.insert(Term::Apply {
                        function: motive,
                        argument: zero,
                    });
                    pending.push(Pending::CaseZero {
                        motive,
                        one_branch,
                        scrutinee,
                    });
                    current = Operation::Check {
                        term: zero_branch,
                        expected: zero_type,
                    };
                    break;
                }
                Some(Pending::CaseZero {
                    motive,
                    one_branch,
                    scrutinee,
                }) => {
                    value.require_checked();
                    let one = arena.insert(Term::TwoOne);
                    let one_type = arena.insert(Term::Apply {
                        function: motive,
                        argument: one,
                    });
                    pending.push(Pending::ReturnApplication {
                        function: motive,
                        argument: scrutinee,
                    });
                    current = Operation::Check {
                        term: one_branch,
                        expected: one_type,
                    };
                    break;
                }
                Some(Pending::ReturnApplication { function, argument }) => {
                    value.require_checked();
                    value =
                        JudgmentValue::Inferred(arena.insert(Term::Apply { function, argument }));
                }
                Some(Pending::IdentityEndpoint {
                    elimination,
                    recorded,
                }) => {
                    value.require_checked();
                    if !convertible(
                        arena,
                        &context,
                        recorded,
                        elimination.endpoint,
                        elimination.ty,
                        budget,
                    )? {
                        return Err(CoreError::TypeMismatch {
                            expected: recorded,
                            actual: elimination.endpoint,
                        });
                    }
                    let motive = elimination.motive;
                    pending.push(Pending::MotiveType {
                        motive,
                        scrutinee_type: elimination.ty,
                        use_kind: MotiveUse::Identity(elimination),
                    });
                    current = Operation::Infer(motive);
                    break;
                }
                Some(Pending::IdentityBase {
                    motive,
                    endpoint,
                    proof,
                }) => {
                    value.require_checked();
                    let at_endpoint = arena.insert(Term::Apply {
                        function: motive,
                        argument: endpoint,
                    });
                    value = JudgmentValue::Inferred(arena.insert(Term::Apply {
                        function: at_endpoint,
                        argument: proof,
                    }));
                }
                Some(frame) => {
                    let mut inferred = value.require_inferred();
                    match frame {
                        Pending::FormationDomain {
                            formation,
                            domain,
                            codomain,
                        } => {
                            let domain_sort =
                                inferred_sort(arena, &context, domain, inferred, budget)?;
                            let prefix = context.bindings.len();
                            context.bindings.push(domain);
                            pending.push(Pending::FormationCodomain {
                                formation,
                                codomain,
                                domain_sort,
                                prefix,
                            });
                            current = Operation::Infer(codomain);
                            break;
                        }
                        Pending::FormationCodomain {
                            formation,
                            codomain,
                            domain_sort,
                            prefix,
                        } => {
                            let codomain_sort =
                                inferred_sort(arena, &context, codomain, inferred, budget)?;
                            context.bindings.truncate(prefix);
                            let level = domain_sort.level().maximum(codomain_sort.level());
                            let sort = match formation {
                                Formation::Pi => match codomain_sort {
                                    Sort::Strict(_) => Sort::Strict(level),
                                    Sort::Type(_) => Sort::Type(level),
                                },
                                Formation::Sigma => match (domain_sort, codomain_sort) {
                                    (Sort::Strict(_), Sort::Strict(_)) => Sort::Strict(level),
                                    _ => Sort::Type(level),
                                },
                            };
                            inferred = arena.insert(Term::Sort(sort));
                        }
                        Pending::Layer { layer, domain } => {
                            let domain_sort =
                                inferred_sort(arena, &context, domain, inferred, budget)?;
                            let sort = match (layer, domain_sort) {
                                (Layer::Squash, Sort::Type(level)) => Sort::Strict(level),
                                (Layer::Squash, Sort::Strict(_)) => {
                                    return Err(CoreError::StrictSquashDomain { domain });
                                }
                                (Layer::Box, Sort::Strict(level)) => Sort::Type(level),
                                (Layer::Box, Sort::Type(_)) => {
                                    return Err(CoreError::NonStrictBoxDomain { domain });
                                }
                            };
                            inferred = arena.insert(Term::Sort(sort));
                        }
                        Pending::LambdaDomain { domain, body } => {
                            inferred_sort(arena, &context, domain, inferred, budget)?;
                            let prefix = context.bindings.len();
                            context.bindings.push(domain);
                            pending.push(Pending::LambdaBody { domain, prefix });
                            current = Operation::Infer(body);
                            break;
                        }
                        Pending::LambdaBody { domain, prefix } => {
                            context.bindings.truncate(prefix);
                            inferred = arena.insert(Term::Pi {
                                domain,
                                codomain: inferred,
                            });
                        }
                        Pending::PairFirst { second } => {
                            pending.push(Pending::PairSecond {
                                first_type: inferred,
                            });
                            current = Operation::Infer(second);
                            break;
                        }
                        Pending::PairSecond { first_type } => {
                            let codomain = shift(arena, inferred, 0, 1);
                            inferred = arena.insert(Term::Sigma {
                                domain: first_type,
                                codomain,
                            });
                        }
                        Pending::ApplyFunction { function, argument } => {
                            let head =
                                weak_head_normalize(arena, context.signature(), inferred, budget)?;
                            let Term::Pi { domain, codomain } = arena.get(head) else {
                                return Err(CoreError::NotAFunction {
                                    function,
                                    actual_type: head,
                                });
                            };
                            if matches!(arena.node(argument), Term::Pair { .. }) {
                                // Inference has no expected dependent codomain. Keep
                                // the componentwise pair-checking premise distinct.
                                pending.push(Pending::ApplyPair { argument, codomain });
                                current = Operation::Check {
                                    term: argument,
                                    expected: domain,
                                };
                                break;
                            } else {
                                pending.push(Pending::ApplyArgument {
                                    domain,
                                    codomain,
                                    argument,
                                });
                                current = Operation::Infer(argument);
                                break;
                            }
                        }
                        Pending::ApplyArgument {
                            domain,
                            codomain,
                            argument,
                        } => {
                            pending.push(Pending::ApplyDomain {
                                domain,
                                codomain,
                                argument,
                                argument_type: inferred,
                            });
                            current = Operation::Infer(domain);
                            break;
                        }
                        Pending::ApplyDomain {
                            domain,
                            codomain,
                            argument,
                            argument_type,
                        } => {
                            let domain_sort =
                                inferred_sort(arena, &context, domain, inferred, budget)?;
                            let shared_type = arena.insert(Term::Sort(domain_sort));
                            if !convertible(
                                arena,
                                &context,
                                argument_type,
                                domain,
                                shared_type,
                                budget,
                            )? {
                                return Err(CoreError::ArgumentTypeMismatch {
                                    expected: domain,
                                    actual: argument_type,
                                });
                            }
                            inferred = substitute(arena, codomain, argument);
                        }
                        Pending::Projection { is_first, pair } => {
                            let head =
                                weak_head_normalize(arena, context.signature(), inferred, budget)?;
                            let Term::Sigma { domain, codomain } = arena.get(head) else {
                                return Err(CoreError::NotAPair {
                                    pair,
                                    actual_type: head,
                                });
                            };
                            inferred = if is_first {
                                domain
                            } else {
                                let projected = arena.insert(Term::Fst { pair });
                                substitute(arena, codomain, projected)
                            };
                        }
                        Pending::IdentityProof {
                            motive,
                            base,
                            endpoint,
                            proof,
                        } => {
                            let head =
                                weak_head_normalize(arena, context.signature(), inferred, budget)?;
                            let Term::Id {
                                ty,
                                left: fixed,
                                right: recorded,
                            } = arena.get(head)
                            else {
                                return Err(CoreError::NotAnIdentity {
                                    proof,
                                    actual_type: head,
                                });
                            };
                            let elimination = IdentityElimination {
                                motive,
                                base,
                                endpoint,
                                proof,
                                ty,
                                fixed,
                            };
                            pending.push(Pending::IdentityEndpoint {
                                elimination,
                                recorded,
                            });
                            current = Operation::Check {
                                term: endpoint,
                                expected: ty,
                            };
                            break;
                        }
                        Pending::IdentityMotiveDomain {
                            elimination,
                            proof_domain,
                            motive_codomain,
                            expected_domain,
                            prefix,
                        } => {
                            let sort =
                                inferred_sort(arena, &context, proof_domain, inferred, budget)?;
                            let shared_domain = arena.insert(Term::Sort(sort));
                            if !convertible(
                                arena,
                                &context,
                                proof_domain,
                                expected_domain,
                                shared_domain,
                                budget,
                            )? {
                                return Err(CoreError::TypeMismatch {
                                    expected: expected_domain,
                                    actual: proof_domain,
                                });
                            }
                            context.bindings.truncate(prefix);
                            let head = weak_head_normalize(
                                arena,
                                context.signature(),
                                motive_codomain,
                                budget,
                            )?;
                            match arena.get(head) {
                                Term::Sort(Sort::Type(_)) => {}
                                Term::Sort(Sort::Strict(_)) => {
                                    return Err(CoreError::StrictIdentityMotiveCodomain {
                                        codomain: motive_codomain,
                                    });
                                }
                                _ => {
                                    return Err(CoreError::IdentityMotiveCodomainNotAUniverse {
                                        codomain: motive_codomain,
                                    });
                                }
                            }
                            let refl = arena.insert(Term::Refl {
                                ty: elimination.ty,
                                value: elimination.fixed,
                            });
                            let at_fixed = arena.insert(Term::Apply {
                                function: elimination.motive,
                                argument: elimination.fixed,
                            });
                            let base_type = arena.insert(Term::Apply {
                                function: at_fixed,
                                argument: refl,
                            });
                            pending.push(Pending::IdentityBase {
                                motive: elimination.motive,
                                endpoint: elimination.endpoint,
                                proof: elimination.proof,
                            });
                            current = Operation::Check {
                                term: elimination.base,
                                expected: base_type,
                            };
                            break;
                        }
                        Pending::MotiveType {
                            motive,
                            scrutinee_type,
                            use_kind,
                        } => {
                            let head =
                                weak_head_normalize(arena, context.signature(), inferred, budget)?;
                            let Term::Pi { domain, codomain } = arena.get(head) else {
                                return Err(CoreError::NotAFunction {
                                    function: motive,
                                    actual_type: head,
                                });
                            };
                            pending.push(Pending::MotiveDomain {
                                motive,
                                domain,
                                codomain,
                                scrutinee_type,
                                use_kind,
                            });
                            current = Operation::Infer(domain);
                            break;
                        }
                        Pending::MotiveDomain {
                            motive,
                            domain,
                            codomain,
                            scrutinee_type,
                            use_kind,
                        } => {
                            // Every eliminator first admits the motive's exact
                            // input carrier. A result-sort rule never relocates it.
                            let domain_sort =
                                inferred_sort(arena, &context, domain, inferred, budget)?;
                            let shared_domain = arena.insert(Term::Sort(domain_sort));
                            if !convertible(
                                arena,
                                &context,
                                domain,
                                scrutinee_type,
                                shared_domain,
                                budget,
                            )? {
                                return Err(CoreError::TypeMismatch {
                                    expected: scrutinee_type,
                                    actual: domain,
                                });
                            }
                            let head =
                                weak_head_normalize(arena, context.signature(), codomain, budget)?;
                            match use_kind {
                                MotiveUse::Identity(elimination) => {
                                    let Term::Pi {
                                        domain: proof_domain,
                                        codomain: motive_codomain,
                                    } = arena.get(head)
                                    else {
                                        return Err(
                                            CoreError::IdentityMotiveCodomainNotAFunction {
                                                codomain,
                                            },
                                        );
                                    };
                                    // The second domain is checked under exactly
                                    // the motive's endpoint binder, never a new
                                    // or relocated identity carrier.
                                    let prefix = context.bindings.len();
                                    context.bindings.push(domain);
                                    let shifted_ty = shift(arena, elimination.ty, 0, 1);
                                    let shifted_fixed = shift(arena, elimination.fixed, 0, 1);
                                    let bound = arena.insert(Term::Variable(0));
                                    let expected_domain = arena.insert(Term::Id {
                                        ty: shifted_ty,
                                        left: shifted_fixed,
                                        right: bound,
                                    });
                                    pending.push(Pending::IdentityMotiveDomain {
                                        elimination,
                                        proof_domain,
                                        motive_codomain,
                                        expected_domain,
                                        prefix,
                                    });
                                    current = Operation::Infer(proof_domain);
                                }
                                MotiveUse::Two {
                                    zero_branch,
                                    one_branch,
                                    scrutinee,
                                } => {
                                    match arena.get(head) {
                                        Term::Sort(Sort::Type(_)) => {}
                                        Term::Sort(Sort::Strict(_)) => {
                                            return Err(CoreError::StrictCaseMotiveCodomain {
                                                codomain,
                                            });
                                        }
                                        _ => {
                                            return Err(
                                                CoreError::CaseMotiveCodomainNotAUniverse {
                                                    codomain,
                                                },
                                            );
                                        }
                                    }
                                    let two = arena.insert(Term::Two);
                                    pending.push(Pending::CaseScrutinee {
                                        motive,
                                        zero_branch,
                                        one_branch,
                                        scrutinee,
                                    });
                                    current = Operation::Check {
                                        term: scrutinee,
                                        expected: two,
                                    };
                                }
                                MotiveUse::W {
                                    carrier,
                                    children,
                                    tree,
                                    step,
                                } => {
                                    match arena.get(head) {
                                        Term::Sort(Sort::Type(_)) => {}
                                        Term::Sort(Sort::Strict(_)) => {
                                            return Err(CoreError::StrictInductionMotiveCodomain {
                                                codomain,
                                            });
                                        }
                                        _ => {
                                            return Err(
                                                CoreError::InductionMotiveCodomainNotAUniverse {
                                                    codomain,
                                                },
                                            );
                                        }
                                    }
                                    let step_type = w_step_type(arena, carrier, children, motive);
                                    pending.push(Pending::ReturnApplication {
                                        function: motive,
                                        argument: tree,
                                    });
                                    current = Operation::Check {
                                        term: step,
                                        expected: step_type,
                                    };
                                }
                                MotiveUse::Box {
                                    payload,
                                    body,
                                    scrutinee,
                                } => {
                                    if !matches!(arena.node(head), Term::Sort(_)) {
                                        return Err(CoreError::BoxMotiveCodomainNotAUniverse {
                                            codomain,
                                        });
                                    }
                                    let body_type = box_body_type(arena, payload, motive);
                                    pending.push(Pending::ReturnApplication {
                                        function: motive,
                                        argument: scrutinee,
                                    });
                                    current = Operation::Check {
                                        term: body,
                                        expected: body_type,
                                    };
                                }
                            }
                            break;
                        }
                        Pending::InductionTree { motive, step, tree } => {
                            let head =
                                weak_head_normalize(arena, context.signature(), inferred, budget)?;
                            let Term::W { carrier, children } = arena.get(head) else {
                                return Err(CoreError::NotAW {
                                    tree,
                                    actual_type: head,
                                });
                            };
                            pending.push(Pending::MotiveType {
                                motive,
                                scrutinee_type: head,
                                use_kind: MotiveUse::W {
                                    carrier,
                                    children,
                                    tree,
                                    step,
                                },
                            });
                            current = Operation::Infer(motive);
                            break;
                        }
                        Pending::BoxScrutinee {
                            motive,
                            body,
                            scrutinee,
                        } => {
                            let head =
                                weak_head_normalize(arena, context.signature(), inferred, budget)?;
                            let Term::Box { ty: payload } = arena.get(head) else {
                                return Err(CoreError::NotABox {
                                    scrutinee,
                                    actual_type: head,
                                });
                            };
                            pending.push(Pending::MotiveType {
                                motive,
                                scrutinee_type: head,
                                use_kind: MotiveUse::Box {
                                    payload,
                                    body,
                                    scrutinee,
                                },
                            });
                            current = Operation::Infer(motive);
                            break;
                        }
                        Pending::SquashScrutinee {
                            proposition,
                            function,
                            scrutinee,
                        } => {
                            let head =
                                weak_head_normalize(arena, context.signature(), inferred, budget)?;
                            let Term::Squash { ty: carrier } = arena.get(head) else {
                                return Err(CoreError::NotASquash {
                                    scrutinee,
                                    actual_type: head,
                                });
                            };
                            pending.push(Pending::SquashTarget {
                                proposition,
                                function,
                                carrier,
                            });
                            current = Operation::Infer(proposition);
                            break;
                        }
                        Pending::SquashTarget {
                            proposition,
                            function,
                            carrier,
                        } => {
                            let sort =
                                inferred_sort(arena, &context, proposition, inferred, budget)?;
                            if !sort.is_strict() {
                                return Err(CoreError::SquashTargetNotStrict { proposition });
                            }
                            let codomain = shift(arena, proposition, 0, 1);
                            let function_type = arena.insert(Term::Pi {
                                domain: carrier,
                                codomain,
                            });
                            pending.push(Pending::ReturnType { ty: proposition });
                            current = Operation::Check {
                                term: function,
                                expected: function_type,
                            };
                            break;
                        }
                        Pending::WCarrier {
                            carrier,
                            children,
                            use_kind,
                        } => {
                            let carrier_sort =
                                inferred_sort(arena, &context, carrier, inferred, budget)?;
                            let Sort::Type(carrier_level) = carrier_sort else {
                                return Err(CoreError::StrictWCarrier { carrier });
                            };
                            pending.push(Pending::WChildren {
                                carrier,
                                children,
                                carrier_level,
                                use_kind,
                            });
                            current = Operation::Infer(children);
                            break;
                        }
                        Pending::WChildren {
                            carrier,
                            children,
                            carrier_level,
                            use_kind,
                        } => {
                            let head =
                                weak_head_normalize(arena, context.signature(), inferred, budget)?;
                            let Term::Pi { domain, codomain } = arena.get(head) else {
                                return Err(CoreError::NotAFunction {
                                    function: children,
                                    actual_type: head,
                                });
                            };
                            pending.push(Pending::WDomain {
                                carrier,
                                children,
                                carrier_level,
                                domain,
                                codomain,
                                use_kind,
                            });
                            current = Operation::Infer(domain);
                            break;
                        }
                        Pending::WDomain {
                            carrier,
                            children,
                            carrier_level,
                            domain,
                            codomain,
                            use_kind,
                        } => {
                            let domain_sort =
                                inferred_sort(arena, &context, domain, inferred, budget)?;
                            let shared_domain = arena.insert(Term::Sort(domain_sort));
                            if !convertible(
                                arena,
                                &context,
                                domain,
                                carrier,
                                shared_domain,
                                budget,
                            )? {
                                return Err(CoreError::TypeMismatch {
                                    expected: carrier,
                                    actual: domain,
                                });
                            }
                            let head =
                                weak_head_normalize(arena, context.signature(), codomain, budget)?;
                            let children_level = match arena.get(head) {
                                Term::Sort(Sort::Type(level)) => level,
                                Term::Sort(Sort::Strict(_)) => {
                                    return Err(CoreError::StrictWChildrenCodomain { codomain });
                                }
                                _ => {
                                    return Err(CoreError::WChildrenCodomainNotAUniverse {
                                        codomain,
                                    });
                                }
                            };
                            // Both W formation and every sup annotation owe the
                            // complete relevant carrier/family judgment first.
                            let level = carrier_level.maximum(children_level);
                            match use_kind {
                                WUse::Formation => {
                                    inferred = arena.insert(Term::Sort(Sort::Type(level)))
                                }
                                WUse::Constructor { label, function } => {
                                    pending.push(Pending::SupLabel {
                                        carrier,
                                        children,
                                        label,
                                        function,
                                    });
                                    current = Operation::Check {
                                        term: label,
                                        expected: carrier,
                                    };
                                    break;
                                }
                            }
                        }
                        Pending::IdentityDomain { ty, use_kind } => {
                            // Relevant identity keeps its carrier and both
                            // endpoints; strict proof irrelevance supplies none.
                            let sort = inferred_sort(arena, &context, ty, inferred, budget)?;
                            if sort.is_strict() {
                                return Err(CoreError::StrictIdentityDomain { domain: ty });
                            }
                            match use_kind {
                                IdentityUse::Formation { left, right } => {
                                    pending.push(Pending::IdentityLeft { ty, right, sort });
                                    current = Operation::Check {
                                        term: left,
                                        expected: ty,
                                    };
                                }
                                IdentityUse::Reflexivity { value } => {
                                    pending.push(Pending::ReflexivityValue { ty, value });
                                    current = Operation::Check {
                                        term: value,
                                        expected: ty,
                                    };
                                }
                            }
                            break;
                        }
                        Pending::LayerDomain { layer, ty, value } => {
                            let sort = inferred_sort(arena, &context, ty, inferred, budget)?;
                            match (&layer, sort) {
                                (Layer::Squash, Sort::Strict(_)) => {
                                    return Err(CoreError::StrictSquashDomain { domain: ty });
                                }
                                (Layer::Box, Sort::Type(_)) => {
                                    return Err(CoreError::NonStrictBoxDomain { domain: ty });
                                }
                                (Layer::Squash, Sort::Type(_)) | (Layer::Box, Sort::Strict(_)) => {}
                            }
                            pending.push(Pending::LayerIntroduction { layer, ty });
                            current = Operation::Check {
                                term: value,
                                expected: ty,
                            };
                            break;
                        }
                        Pending::EmptyTarget { ty, scrutinee } => {
                            // Empty elimination admits either target sort, but
                            // still forms the target before checking its proof.
                            inferred_sort(arena, &context, ty, inferred, budget)?;
                            let empty = arena.insert(Term::Empty);
                            pending.push(Pending::ReturnType { ty });
                            current = Operation::Check {
                                term: scrutinee,
                                expected: empty,
                            };
                            break;
                        }
                        Pending::CheckExpected { term, expected } => {
                            let expected_sort =
                                inferred_sort(arena, &context, expected, inferred, budget)?;
                            if let Term::Pair { first, second } = arena.get(term) {
                                let head = weak_head_normalize(
                                    arena,
                                    context.signature(),
                                    expected,
                                    budget,
                                )?;
                                if let Term::Sigma { domain, codomain } = arena.get(head) {
                                    pending.push(Pending::CheckPairSecond {
                                        first,
                                        second,
                                        codomain,
                                        prefix: context.bindings.len(),
                                    });
                                    current = Operation::Check {
                                        term: first,
                                        expected: domain,
                                    };
                                    break;
                                }
                            }
                            if let Term::Lambda { domain, body } = arena.get(term) {
                                let head = weak_head_normalize(
                                    arena,
                                    context.signature(),
                                    expected,
                                    budget,
                                )?;
                                if let Term::Pi {
                                    domain: expected_domain,
                                    codomain,
                                } = arena.get(head)
                                {
                                    pending.push(Pending::CheckLambdaDomain {
                                        domain,
                                        body,
                                        expected_domain,
                                        codomain,
                                    });
                                    current = Operation::Infer(expected_domain);
                                    break;
                                }
                            }
                            let shared_type = arena.insert(Term::Sort(expected_sort));
                            pending.push(Pending::CheckActual {
                                expected,
                                shared_type,
                            });
                            current = Operation::Infer(term);
                            break;
                        }
                        Pending::CheckLambdaDomain {
                            domain,
                            body,
                            expected_domain,
                            codomain,
                        } => {
                            let domain_sort =
                                inferred_sort(arena, &context, expected_domain, inferred, budget)?;
                            let shared_domain = arena.insert(Term::Sort(domain_sort));
                            if !convertible(
                                arena,
                                &context,
                                domain,
                                expected_domain,
                                shared_domain,
                                budget,
                            )? {
                                return Err(CoreError::TypeMismatch {
                                    expected: expected_domain,
                                    actual: domain,
                                });
                            }
                            context.bindings.push(domain);
                            current = Operation::Check {
                                term: body,
                                expected: codomain,
                            };
                            break;
                        }
                        Pending::CheckActual {
                            expected,
                            shared_type,
                        } => {
                            if !convertible(
                                arena,
                                &context,
                                inferred,
                                expected,
                                shared_type,
                                budget,
                            )? {
                                return Err(CoreError::TypeMismatch {
                                    expected,
                                    actual: inferred,
                                });
                            }
                            value = JudgmentValue::Checked;
                            continue;
                        }
                        Pending::CheckScope { .. }
                        | Pending::CheckPairSecond { .. }
                        | Pending::ApplyPair { .. }
                        | Pending::IdentityLeft { .. }
                        | Pending::IdentityRight { .. }
                        | Pending::ReflexivityValue { .. }
                        | Pending::LayerIntroduction { .. }
                        | Pending::ReturnType { .. }
                        | Pending::SupLabel { .. }
                        | Pending::CaseScrutinee { .. }
                        | Pending::CaseZero { .. }
                        | Pending::ReturnApplication { .. }
                        | Pending::IdentityEndpoint { .. }
                        | Pending::IdentityBase { .. } => {
                            unreachable!(
                                "checking continuations consume a completed checking premise"
                            )
                        }
                    }
                    value = JudgmentValue::Inferred(inferred);
                }
                None => return Ok(value),
            }
        }
    }
}
