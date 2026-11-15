/*
    Predicative dependent typing, sort inference, and typed conversion judgments
    for the mathematical core (`Pi`, `Sigma`, `Two`, `Id`, `W`, and strict
    propositions).

    We infer sorts and types, check dependent pair and lambda continuations off
    the host call stack, and decide typed conversion with strict proof
    irrelevance gated solely on the shared type's `Strict` sort.
*/

use crate::mathematical_core::conversion::{Budget, weak_head_normalize};
use crate::mathematical_core::signature::Signature;
use crate::mathematical_core::substitution::{instantiate_levels, shift};
use crate::mathematical_core::term::{Level, Sort, Term, TermArena, TermHandle};

macro_rules! row {
    (Apply => $function:expr => $argument:expr) => {
        Term::Apply {
            function: $function,
            argument: $argument,
        }
    };
    (Pi => $domain:expr => $codomain:expr) => {
        Term::Pi {
            domain: $domain,
            codomain: $codomain,
        }
    };
    (TypeMismatch => $expected:expr => $actual:expr) => {
        CoreError::TypeMismatch {
            expected: $expected,
            actual: $actual,
        }
    };
}

#[derive(Clone, Debug, Default)]
pub struct Context {
    bindings: Vec<TermHandle>,
    level_arity: u32,
    signature: Signature,
}

impl Context {
    pub fn empty() -> Self {
        Self {
            bindings: Vec::new(),
            level_arity: 0,
            signature: Signature::new(),
        }
    }

    pub fn with_level_arity(level_arity: u32) -> Self {
        Self {
            bindings: Vec::new(),
            level_arity,
            signature: Signature::new(),
        }
    }

    pub fn level_arity(&self) -> u32 {
        self.level_arity
    }

    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    pub fn with_signature(&self, signature: Signature) -> Context {
        Context {
            bindings: self.bindings.clone(),
            level_arity: self.level_arity,
            signature,
        }
    }

    pub fn extend(&self, domain: TermHandle) -> Context {
        let mut bindings = self.bindings.clone();
        bindings.push(domain);
        Context {
            bindings,
            level_arity: self.level_arity,
            signature: self.signature.clone(),
        }
    }

    pub fn lookup(&self, arena: &mut TermArena, index: u32) -> Option<TermHandle> {
        let position = self
            .bindings
            .len()
            .checked_sub(usize::try_from(index).ok()?.checked_add(1)?)?;
        let stored = self.bindings[position];
        Some(shift(arena, stored, 0, index + 1))
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreError {
    DummyTerm,
    UnboundVariable {
        index: u32,
        context_depth: usize,
    },
    LevelOverflow,
    UnboundLevelParameter {
        index: u32,
        arity: u32,
    },
    NotASort {
        term: TermHandle,
        actual_type: TermHandle,
    },
    NotAFunction {
        function: TermHandle,
        actual_type: TermHandle,
    },
    NotAPair {
        pair: TermHandle,
        actual_type: TermHandle,
    },
    StrictCaseMotiveCodomain {
        codomain: TermHandle,
    },
    CaseMotiveCodomainNotAUniverse {
        codomain: TermHandle,
    },
    StrictIdentityDomain {
        domain: TermHandle,
    },
    NotAnIdentity {
        proof: TermHandle,
        actual_type: TermHandle,
    },
    IdentityMotiveCodomainNotAFunction {
        codomain: TermHandle,
    },
    StrictIdentityMotiveCodomain {
        codomain: TermHandle,
    },
    IdentityMotiveCodomainNotAUniverse {
        codomain: TermHandle,
    },
    StrictWCarrier {
        carrier: TermHandle,
    },
    StrictWChildrenCodomain {
        codomain: TermHandle,
    },
    WChildrenCodomainNotAUniverse {
        codomain: TermHandle,
    },
    NotAW {
        tree: TermHandle,
        actual_type: TermHandle,
    },
    StrictInductionMotiveCodomain {
        codomain: TermHandle,
    },
    InductionMotiveCodomainNotAUniverse {
        codomain: TermHandle,
    },
    StrictSquashDomain {
        domain: TermHandle,
    },
    NotASquash {
        scrutinee: TermHandle,
        actual_type: TermHandle,
    },
    SquashTargetNotStrict {
        proposition: TermHandle,
    },
    NonStrictBoxDomain {
        domain: TermHandle,
    },
    NotABox {
        scrutinee: TermHandle,
        actual_type: TermHandle,
    },
    BoxMotiveCodomainNotAUniverse {
        codomain: TermHandle,
    },
    UnknownDeclaration {
        declaration: u32,
        signature_len: usize,
    },
    DeclarationArityMismatch {
        declaration: u32,
        expected: u32,
        supplied: usize,
    },
    ArgumentTypeMismatch {
        expected: TermHandle,
        actual: TermHandle,
    },
    TypeMismatch {
        expected: TermHandle,
        actual: TermHandle,
    },
    StepCeiling,
}

pub fn infer_sort(
    arena: &mut TermArena,
    context: &Context,
    term: TermHandle,
    budget: &mut Budget,
) -> Result<Sort, CoreError> {
    let inferred = infer_type(arena, context, term, budget)?;
    inferred_sort(arena, context, term, inferred, budget)
}

fn inferred_sort(
    arena: &mut TermArena,
    context: &Context,
    term: TermHandle,
    inferred: TermHandle,
    budget: &mut Budget,
) -> Result<Sort, CoreError> {
    let head = weak_head_normalize(arena, context.signature(), inferred, budget)?;
    match arena.get(head) {
        Term::Sort(sort) => Ok(sort),
        _ => Err(CoreError::NotASort {
            term,
            actual_type: head,
        }),
    }
}

pub fn infer_type(
    arena: &mut TermArena,
    context: &Context,
    term: TermHandle,
    budget: &mut Budget,
) -> Result<TermHandle, CoreError> {
    let node = arena.get(term);
    match node {
        Term::Pi { .. }
        | Term::Sigma { .. }
        | Term::Squash { .. }
        | Term::Box { .. }
        | Term::Lambda { .. }
        | Term::Apply { .. }
        | Term::Pair { .. }
        | Term::Fst { .. }
        | Term::Snd { .. }
        | Term::Id { .. }
        | Term::Refl { .. }
        | Term::SquashIntro { .. }
        | Term::BoxIntro { .. }
        | Term::EmptyElim { .. }
        | Term::W { .. }
        | Term::Sup { .. }
        | Term::CaseTwo { .. }
        | Term::IndW { .. }
        | Term::BoxElim { .. }
        | Term::SquashElim { .. }
        | Term::IdElim { .. } => judgments::infer(arena, context, term, budget),
        node => infer_leaf_type(arena, context, node),
    }
}

fn infer_leaf_type(
    arena: &mut TermArena,
    context: &Context,
    node: Term,
) -> Result<TermHandle, CoreError> {
    match node {
        Term::Dummy => Err(CoreError::DummyTerm),
        Term::Variable(index) => context
            .lookup(arena, index)
            .ok_or(CoreError::UnboundVariable {
                index,
                context_depth: context.len(),
            }),
        Term::Sort(sort) => {
            check_level(context.level_arity(), &sort.level())?;
            let level = sort.level().successor().ok_or(CoreError::LevelOverflow)?;
            Ok(arena.insert(Term::Sort(Sort::Type(level))))
        }
        Term::Pi { .. }
        | Term::Sigma { .. }
        | Term::Squash { .. }
        | Term::Box { .. }
        | Term::Lambda { .. }
        | Term::Apply { .. }
        | Term::Pair { .. }
        | Term::Fst { .. }
        | Term::Snd { .. }
        | Term::Id { .. }
        | Term::Refl { .. }
        | Term::SquashIntro { .. }
        | Term::BoxIntro { .. }
        | Term::EmptyElim { .. }
        | Term::W { .. }
        | Term::Sup { .. }
        | Term::CaseTwo { .. }
        | Term::IndW { .. }
        | Term::BoxElim { .. }
        | Term::SquashElim { .. }
        | Term::IdElim { .. } => unreachable!("constructor inference owns these nodes"),
        Term::Two => Ok(arena.insert(Term::Sort(Sort::Type(Level::Constant(0))))),
        Term::TwoZero | Term::TwoOne => Ok(arena.insert(Term::Two)),
        Term::Empty => Ok(arena.insert(Term::Sort(Sort::Strict(Level::Constant(0))))),
        Term::Constant {
            declaration: index,
            levels,
        } => {
            let signature = context.signature();
            let Some(declaration) = signature.get(index) else {
                return Err(CoreError::UnknownDeclaration {
                    declaration: index,
                    signature_len: signature.len(),
                });
            };
            if levels.len() != declaration.level_arity as usize {
                return Err(CoreError::DeclarationArityMismatch {
                    declaration: index,
                    expected: declaration.level_arity,
                    supplied: levels.len(),
                });
            }
            for level in &levels {
                check_level(context.level_arity(), level)?;
            }
            instantiate_levels(arena, declaration.ty, &levels).map_err(|index| {
                CoreError::UnboundLevelParameter {
                    index,
                    arity: declaration.level_arity,
                }
            })
        }
    }
}

pub(crate) fn convert_retaining(
    arena: &mut TermArena,
    context: &Context,
    left: TermHandle,
    right: TermHandle,
    shared_type: TermHandle,
    budget: &mut Budget,
    retained: &impl Fn(u32) -> bool,
) -> Result<bool, CoreError> {
    judgments::convert(arena, context, left, right, shared_type, budget, retained)
}

fn check_level(arity: u32, level: &Level) -> Result<(), CoreError> {
    let mut pending: Vec<&Level> = Vec::new();
    let mut current = level;
    loop {
        match current {
            Level::Constant(_) => {}
            Level::Parameter(index) if *index < arity => {}
            Level::Parameter(index) => {
                return Err(CoreError::UnboundLevelParameter {
                    index: *index,
                    arity,
                });
            }
            Level::Successor(inner) => {
                current = inner;
                continue;
            }
            Level::Maximum(left, right) => {
                pending.push(right);
                current = left;
                continue;
            }
        }
        match pending.pop() {
            Some(next) => current = next,
            None => return Ok(()),
        }
    }
}

pub(crate) fn w_step_type(
    arena: &mut TermArena,
    carrier: TermHandle,
    children: TermHandle,
    motive: TermHandle,
) -> TermHandle {
    let children_at_one = shift(arena, children, 0, 1);
    let bound_a = arena.insert(Term::Variable(0));
    let children_domain = arena.insert(row!(Apply => children_at_one => bound_a));
    let carrier_at_two = shift(arena, carrier, 0, 2);
    let children_at_two = shift(arena, children, 0, 2);
    let w_at_two = arena.insert(Term::W {
        carrier: carrier_at_two,
        children: children_at_two,
    });
    let function_type = arena.insert(row!(Pi => children_domain => w_at_two));
    let bound_a = arena.insert(Term::Variable(1));
    let hypothesis_domain = arena.insert(row!(Apply => children_at_two => bound_a));
    let motive_at_three = shift(arena, motive, 0, 3);
    let bound_k = arena.insert(Term::Variable(1));
    let bound_b = arena.insert(Term::Variable(0));
    let child = arena.insert(row!(Apply => bound_k => bound_b));
    let hypothesis_codomain = arena.insert(row!(Apply => motive_at_three => child));
    let hypothesis = arena.insert(row!(Pi => hypothesis_domain => hypothesis_codomain));
    let carrier_at_three = shift(arena, carrier, 0, 3);
    let children_at_three = shift(arena, children, 0, 3);
    let bound_a = arena.insert(Term::Variable(2));
    let bound_k = arena.insert(Term::Variable(1));
    let sup = arena.insert(Term::Sup {
        carrier: carrier_at_three,
        children: children_at_three,
        label: bound_a,
        function: bound_k,
    });
    let result = arena.insert(row!(Apply => motive_at_three => sup));
    let inner = arena.insert(row!(Pi => hypothesis => result));
    let middle = arena.insert(row!(Pi => function_type => inner));
    arena.insert(row!(Pi => carrier => middle))
}

pub(crate) fn box_body_type(
    arena: &mut TermArena,
    payload: TermHandle,
    motive: TermHandle,
) -> TermHandle {
    let shifted_motive = shift(arena, motive, 0, 1);
    let shifted_payload = shift(arena, payload, 0, 1);
    let bound = arena.insert(Term::Variable(0));
    let boxed = arena.insert(Term::BoxIntro {
        ty: shifted_payload,
        value: bound,
    });
    let body_codomain = arena.insert(row!(Apply => shifted_motive => boxed));
    arena.insert(row!(Pi => payload => body_codomain))
}

mod judgments {
    use super::{Context, CoreError, box_body_type, infer_leaf_type, inferred_sort, w_step_type};
    use crate::mathematical_core::conversion::{Budget, weak_head_normalize};
    use crate::mathematical_core::substitution::{shift, substitute};
    use crate::mathematical_core::term::{Level, Sort, Term, TermArena, TermHandle};

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

    mod comparison {
        use super::super::{Context, CoreError, box_body_type, inferred_sort, w_step_type};
        use super::JudgmentValue;
        use crate::mathematical_core::conversion::{Budget, weak_head_normalize_retaining};
        use crate::mathematical_core::substitution::{shift, substitute};
        use crate::mathematical_core::term::{
            Level, Sort, Term, TermArena, TermHandle, levels_equal, sorts_equal,
        };

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
            pub(super) fn ordinary(
                left: TermHandle,
                right: TermHandle,
                shared_type: TermHandle,
            ) -> Self {
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
                        let sort =
                            inferred_sort(arena, context, query.shared_type, inferred, budget)?;
                        if sort.is_strict()
                            || arena.structurally_equal(query.terms.left, query.terms.right)
                        {
                            Step::Complete(true)
                        } else {
                            let left =
                                head(arena, context, query.terms.left, budget, query.retained)?;
                            let right =
                                head(arena, context, query.terms.right, budget, query.retained)?;
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
                        let sort = inferred_sort(
                            arena,
                            context,
                            types.left,
                            value.require_inferred(),
                            budget,
                        )?;
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
                        let inferred =
                            head(arena, context, value.require_inferred(), budget, retained)?;
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
                        let proof_type =
                            head(arena, context, value.require_inferred(), budget, retained)?;
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
                        let left_level = family_level(
                            arena,
                            context,
                            value.require_inferred(),
                            budget,
                            retained,
                        )?;
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
                        let right_level = family_level(
                            arena,
                            context,
                            value.require_inferred(),
                            budget,
                            retained,
                        )?;
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
                        let inferred =
                            head(arena, context, value.require_inferred(), budget, retained)?;
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
                        let inferred =
                            head(arena, context, value.require_inferred(), budget, retained)?;
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
                        let inferred =
                            head(arena, context, value.require_inferred(), budget, retained)?;
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
    }

    enum Compared {
        Endpoint {
            elimination: IdentityElimination,
            recorded: TermHandle,
        },
        IdentityMotive {
            elimination: IdentityElimination,
            proof_domain: TermHandle,
            motive_codomain: TermHandle,
            expected_domain: TermHandle,
            prefix: usize,
        },
        Motive {
            motive: TermHandle,
            domain: TermHandle,
            codomain: TermHandle,
            scrutinee_type: TermHandle,
            use_kind: MotiveUse,
        },
        WFamily {
            carrier: TermHandle,
            children: TermHandle,
            carrier_level: Level,
            domain: TermHandle,
            codomain: TermHandle,
            use_kind: WUse,
        },
        Argument {
            domain: TermHandle,
            codomain: TermHandle,
            argument: TermHandle,
            argument_type: TermHandle,
        },
        Annotation {
            domain: TermHandle,
            body: TermHandle,
            expected_domain: TermHandle,
            codomain: TermHandle,
        },
        Check {
            expected: TermHandle,
            actual: TermHandle,
        },
    }

    enum Pending<'a> {
        Compared(Compared),
        ConversionScope {
            prefix: usize,
        },
        Conversion(comparison::Work<'a>),
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

    enum Operation<'a> {
        Convert(comparison::Query<'a>),
        Infer(TermHandle),
        Check {
            term: TermHandle,
            expected: TermHandle,
        },
    }

    enum JudgmentValue {
        Inferred(TermHandle),
        Checked,
        Converted(bool),
    }

    impl JudgmentValue {
        fn require_inferred(self) -> TermHandle {
            match self {
                Self::Inferred(ty) => ty,
                Self::Checked | Self::Converted(_) => {
                    unreachable!("inference continuation consumes an inferred type")
                }
            }
        }

        fn require_converted(self) -> bool {
            match self {
                Self::Converted(equal) => equal,
                Self::Inferred(_) | Self::Checked => {
                    unreachable!("conversion continuation consumes a comparison")
                }
            }
        }

        fn require_checked(self) {
            match self {
                Self::Checked => {}
                Self::Inferred(_) | Self::Converted(_) => {
                    unreachable!("checking continuation consumes a completed check")
                }
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

    pub(super) fn convert(
        arena: &mut TermArena,
        context: &Context,
        left: TermHandle,
        right: TermHandle,
        shared_type: TermHandle,
        budget: &mut Budget,
        retained: &dyn Fn(u32) -> bool,
    ) -> Result<bool, CoreError> {
        let query = comparison::Query {
            terms: comparison::Terms { left, right },
            shared_type,
            retained,
        };
        drive(arena, context, Operation::Convert(query), budget)
            .map(JudgmentValue::require_converted)
    }

    fn drive<'a>(
        arena: &mut TermArena,
        context: &Context,
        operation: Operation<'a>,
        budget: &mut Budget,
    ) -> Result<JudgmentValue, CoreError> {
        let mut context = context.clone();
        let mut pending = Vec::new();
        let mut current = operation;
        loop {
            let mut value = match current {
                Operation::Convert(query) => {
                    pending.push(Pending::ConversionScope {
                        prefix: context.bindings.len(),
                    });
                    pending.push(Pending::Conversion(comparison::Work::SharedSort(query)));
                    current = Operation::Infer(query.shared_type);
                    continue;
                }
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
                    Some(Pending::ConversionScope { prefix }) => {
                        let equal = value.require_converted();
                        context.bindings.truncate(prefix);
                        value = JudgmentValue::Converted(equal);
                    }
                    Some(Pending::Conversion(work)) => {
                        match work.resume(arena, &mut context, value, budget)? {
                            comparison::Step::Infer { term, next } => {
                                pending.push(Pending::Conversion(next));
                                current = Operation::Infer(term);
                                break;
                            }
                            comparison::Step::Compare { query, next } => {
                                if let Some(next) = next {
                                    pending.push(Pending::Conversion(next));
                                }
                                current = Operation::Convert(query);
                                break;
                            }
                            comparison::Step::Complete(equal) => {
                                value = JudgmentValue::Converted(equal)
                            }
                        }
                    }
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
                        value = JudgmentValue::Inferred(
                            arena.insert(Term::Apply { function, argument }),
                        );
                    }
                    Some(Pending::IdentityEndpoint {
                        elimination,
                        recorded,
                    }) => {
                        value.require_checked();
                        let query = comparison::Query::ordinary(
                            recorded,
                            elimination.endpoint,
                            elimination.ty,
                        );
                        pending.push(Pending::Compared(Compared::Endpoint {
                            elimination,
                            recorded,
                        }));
                        current = Operation::Convert(query);
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
                    Some(Pending::Compared(conclusion)) => {
                        let equal = value.require_converted();
                        match conclusion {
                            Compared::Endpoint {
                                elimination,
                                recorded,
                            } => {
                                if !equal {
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
                            Compared::IdentityMotive {
                                elimination,
                                proof_domain,
                                motive_codomain,
                                expected_domain,
                                prefix,
                            } => {
                                if !equal {
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
                                        return Err(
                                            CoreError::IdentityMotiveCodomainNotAUniverse {
                                                codomain: motive_codomain,
                                            },
                                        );
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
                            Compared::Motive {
                                motive,
                                domain,
                                codomain,
                                scrutinee_type,
                                use_kind,
                            } => {
                                if !equal {
                                    return Err(CoreError::TypeMismatch {
                                        expected: scrutinee_type,
                                        actual: domain,
                                    });
                                }

                                let head = weak_head_normalize(
                                    arena,
                                    context.signature(),
                                    codomain,
                                    budget,
                                )?;
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
                                                return Err(
                                                    CoreError::StrictInductionMotiveCodomain {
                                                        codomain,
                                                    },
                                                );
                                            }
                                            _ => {
                                                return Err(
                                                CoreError::InductionMotiveCodomainNotAUniverse {
                                                    codomain,
                                                },
                                            );
                                            }
                                        }
                                        let step_type =
                                            w_step_type(arena, carrier, children, motive);
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
                            Compared::WFamily {
                                carrier,
                                children,
                                carrier_level,
                                domain,
                                codomain,
                                use_kind,
                            } => {
                                if !equal {
                                    return Err(CoreError::TypeMismatch {
                                        expected: carrier,
                                        actual: domain,
                                    });
                                }

                                let head = weak_head_normalize(
                                    arena,
                                    context.signature(),
                                    codomain,
                                    budget,
                                )?;
                                let children_level = match arena.get(head) {
                                    Term::Sort(Sort::Type(level)) => level,
                                    Term::Sort(Sort::Strict(_)) => {
                                        return Err(CoreError::StrictWChildrenCodomain {
                                            codomain,
                                        });
                                    }
                                    _ => {
                                        return Err(CoreError::WChildrenCodomainNotAUniverse {
                                            codomain,
                                        });
                                    }
                                };
                                let level = carrier_level.maximum(children_level);
                                match use_kind {
                                    WUse::Formation => {
                                        value = JudgmentValue::Inferred(
                                            arena.insert(Term::Sort(Sort::Type(level))),
                                        );
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
                            Compared::Argument {
                                domain,
                                codomain,
                                argument,
                                argument_type,
                            } => {
                                if !equal {
                                    return Err(CoreError::ArgumentTypeMismatch {
                                        expected: domain,
                                        actual: argument_type,
                                    });
                                }

                                value =
                                    JudgmentValue::Inferred(substitute(arena, codomain, argument));
                            }
                            Compared::Annotation {
                                domain,
                                body,
                                expected_domain,
                                codomain,
                            } => {
                                if !equal {
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
                            Compared::Check { expected, actual } => {
                                if !equal {
                                    return Err(CoreError::TypeMismatch { expected, actual });
                                }

                                value = JudgmentValue::Checked;
                                continue;
                            }
                        }
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
                                let head = weak_head_normalize(
                                    arena,
                                    context.signature(),
                                    inferred,
                                    budget,
                                )?;
                                let Term::Pi { domain, codomain } = arena.get(head) else {
                                    return Err(CoreError::NotAFunction {
                                        function,
                                        actual_type: head,
                                    });
                                };
                                if matches!(arena.node(argument), Term::Pair { .. }) {
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
                                let query =
                                    comparison::Query::ordinary(argument_type, domain, shared_type);
                                pending.push(Pending::Compared(Compared::Argument {
                                    domain,
                                    codomain,
                                    argument,
                                    argument_type,
                                }));
                                current = Operation::Convert(query);
                                break;
                            }
                            Pending::Projection { is_first, pair } => {
                                let head = weak_head_normalize(
                                    arena,
                                    context.signature(),
                                    inferred,
                                    budget,
                                )?;
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
                                let head = weak_head_normalize(
                                    arena,
                                    context.signature(),
                                    inferred,
                                    budget,
                                )?;
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
                                let query = comparison::Query::ordinary(
                                    proof_domain,
                                    expected_domain,
                                    shared_domain,
                                );
                                pending.push(Pending::Compared(Compared::IdentityMotive {
                                    elimination,
                                    proof_domain,
                                    motive_codomain,
                                    expected_domain,
                                    prefix,
                                }));
                                current = Operation::Convert(query);
                                break;
                            }
                            Pending::MotiveType {
                                motive,
                                scrutinee_type,
                                use_kind,
                            } => {
                                let head = weak_head_normalize(
                                    arena,
                                    context.signature(),
                                    inferred,
                                    budget,
                                )?;
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
                                let domain_sort =
                                    inferred_sort(arena, &context, domain, inferred, budget)?;
                                let shared_domain = arena.insert(Term::Sort(domain_sort));
                                let query = comparison::Query::ordinary(
                                    domain,
                                    scrutinee_type,
                                    shared_domain,
                                );
                                pending.push(Pending::Compared(Compared::Motive {
                                    motive,
                                    domain,
                                    codomain,
                                    scrutinee_type,
                                    use_kind,
                                }));
                                current = Operation::Convert(query);
                                break;
                            }
                            Pending::InductionTree { motive, step, tree } => {
                                let head = weak_head_normalize(
                                    arena,
                                    context.signature(),
                                    inferred,
                                    budget,
                                )?;
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
                                let head = weak_head_normalize(
                                    arena,
                                    context.signature(),
                                    inferred,
                                    budget,
                                )?;
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
                                let head = weak_head_normalize(
                                    arena,
                                    context.signature(),
                                    inferred,
                                    budget,
                                )?;
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
                                let head = weak_head_normalize(
                                    arena,
                                    context.signature(),
                                    inferred,
                                    budget,
                                )?;
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
                                let query =
                                    comparison::Query::ordinary(domain, carrier, shared_domain);
                                pending.push(Pending::Compared(Compared::WFamily {
                                    carrier,
                                    children,
                                    carrier_level,
                                    domain,
                                    codomain,
                                    use_kind,
                                }));
                                current = Operation::Convert(query);
                                break;
                            }
                            Pending::IdentityDomain { ty, use_kind } => {
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
                                    (Layer::Squash, Sort::Type(_))
                                    | (Layer::Box, Sort::Strict(_)) => {}
                                }
                                pending.push(Pending::LayerIntroduction { layer, ty });
                                current = Operation::Check {
                                    term: value,
                                    expected: ty,
                                };
                                break;
                            }
                            Pending::EmptyTarget { ty, scrutinee } => {
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
                                let domain_sort = inferred_sort(
                                    arena,
                                    &context,
                                    expected_domain,
                                    inferred,
                                    budget,
                                )?;
                                let shared_domain = arena.insert(Term::Sort(domain_sort));
                                let query = comparison::Query::ordinary(
                                    domain,
                                    expected_domain,
                                    shared_domain,
                                );
                                pending.push(Pending::Compared(Compared::Annotation {
                                    domain,
                                    body,
                                    expected_domain,
                                    codomain,
                                }));
                                current = Operation::Convert(query);
                                break;
                            }
                            Pending::CheckActual {
                                expected,
                                shared_type,
                            } => {
                                let query =
                                    comparison::Query::ordinary(inferred, expected, shared_type);
                                pending.push(Pending::Compared(Compared::Check {
                                    expected,
                                    actual: inferred,
                                }));
                                current = Operation::Convert(query);
                                break;
                            }
                            Pending::Compared(_)
                            | Pending::ConversionScope { .. }
                            | Pending::Conversion(_)
                            | Pending::CheckScope { .. }
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
}

pub fn check_type(
    arena: &mut TermArena,
    context: &Context,
    term: TermHandle,
    expected: TermHandle,
    budget: &mut Budget,
) -> Result<(), CoreError> {
    judgments::check(arena, context, term, expected, budget)
}

#[macro_export]
macro_rules! argument_error_pair_pattern {
    () => {
        $crate::CoreError::TypeMismatch { .. } | $crate::CoreError::ArgumentTypeMismatch { .. }
    };
}
