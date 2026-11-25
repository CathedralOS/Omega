/*
    Predicative dependent typing, sort inference, and typed conversion judgments
    for the mathematical core (`Pi`, `Sigma`, `Two`, `Id`, native inductives, and strict
    propositions).

    We infer sorts and types, check dependent pair and lambda continuations off
    the host call stack, and decide typed conversion with strict proof
    irrelevance gated solely on the shared type's `Strict` sort.
*/

use crate::conversion::{Budget, weak_head_normalize};
use crate::signature::Signature;
use crate::substitution::{instantiate_levels, shift};
use crate::term::{Level, Sort, Term, TermArena, TermHandle};

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
    InvalidInductiveDeclaration,
    InductiveSortNotRelevant,
    InductiveParameterMismatch,
    InductiveUniverseViolation,
    NonStrictlyPositiveOccurrence,
    InvalidConstructorReturnType,
    InvalidRecursorType,
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
        | Term::CaseTwo { .. }
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
        | Term::CaseTwo { .. }
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
    use super::{Context, CoreError, box_body_type, infer_leaf_type, inferred_sort};
    use crate::conversion::{Budget, weak_head_normalize};
    use crate::substitution::{shift, substitute};
    use crate::term::{Sort, Term, TermArena, TermHandle};
    use std::collections::HashMap;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct InferenceScope(usize);

    /// One drive keeps its signature, universe scope and initial bindings.
    /// Inference and comparison can both extend that context. Distinguish
    /// their actual local bindings even at the same depth.
    #[derive(Default)]
    struct InferredTypes {
        initial_bindings: usize,
        scopes: HashMap<(InferenceScope, TermHandle), InferenceScope>,
        active_bindings: Vec<(TermHandle, InferenceScope)>,
        types: HashMap<(InferenceScope, TermHandle), TermHandle>,
    }

    impl InferredTypes {
        fn under(context: &Context) -> Self {
            Self {
                initial_bindings: context.len(),
                ..Self::default()
            }
        }

        fn scope(&mut self, context: &Context) -> InferenceScope {
            let bindings = &context.bindings[self.initial_bindings..];
            let mut common = 0;
            while common < self.active_bindings.len()
                && common < bindings.len()
                && self.active_bindings[common].0 == bindings[common]
            {
                common += 1;
            }
            self.active_bindings.truncate(common);
            let mut scope = self
                .active_bindings
                .last()
                .map_or(InferenceScope(0), |&(_, scope)| scope);
            for &binding in &bindings[common..] {
                let next = InferenceScope(self.scopes.len() + 1);
                scope = *self.scopes.entry((scope, binding)).or_insert(next);
                self.active_bindings.push((binding, scope));
            }
            scope
        }
    }

    #[test]
    fn only_binders_introduced_during_inference_receive_scope_keys() {
        let mut arena = TermArena::new();
        let two = arena.insert(Term::Two);
        let empty = arena.insert(Term::Empty);
        let mut context = Context::empty();
        context.bindings.resize(2_048, two);
        let mut inferred = InferredTypes::under(&context);
        let base = inferred.scope(&context);
        assert!(inferred.scopes.is_empty());

        let first = inferred.scope(&context.extend(two));
        let second = inferred.scope(&context.extend(empty));
        assert_ne!(first, second);
        assert_eq!(inferred.scope(&context.extend(two)), first);
        assert_eq!(inferred.scope(&context), base);
        assert_eq!(inferred.scopes.len(), 2);
    }

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
        Box {
            payload: TermHandle,
            body: TermHandle,
            scrutinee: TermHandle,
        },
    }

    mod comparison {
        use super::super::{Context, CoreError, box_body_type, inferred_sort};
        use super::JudgmentValue;
        use crate::conversion::{Budget, weak_head_normalize_retaining};
        use crate::substitution::{shift, substitute};
        use crate::term::{Term, TermArena, TermHandle, levels_equal, sorts_equal};

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
                            || arena.structurally_equal_dag(query.terms.left, query.terms.right)
                        {
                            Step::Complete(true)
                        } else {
                            let left =
                                head(arena, context, query.terms.left, budget, query.retained)?;
                            let right =
                                head(arena, context, query.terms.right, budget, query.retained)?;
                            if arena.structurally_equal_dag(left, right) {
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
        RememberInference {
            scope: InferenceScope,
            term: TermHandle,
        },
        Compared(Compared),
        ConversionScope {
            prefix: usize,
        },
        Conversion(comparison::Work<'a>),
        ConversionType(comparison::Query<'a>),
        ConversionLeft {
            query: comparison::Query<'a>,
            sort: Sort,
            shared_sort: TermHandle,
        },
        ConversionRight {
            query: comparison::Query<'a>,
            shared_sort: TermHandle,
        },
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
        CheckLambdaAnnotation {
            domain: TermHandle,
            body: TermHandle,
            expected_domain: TermHandle,
            codomain: TermHandle,
            shared_domain: TermHandle,
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
        ReusedInference(TermHandle),
        CheckConversion(comparison::Query<'a>),
        Convert(comparison::Query<'a>),
        Infer(TermHandle),
        Check {
            term: TermHandle,
            expected: TermHandle,
        },
        // Only a completed expected-type check or a checked binder introduction
        // constructs this state. Substitution preserves formation after the first
        // pair component checks; it does not establish that component's type.
        CheckFormed {
            term: TermHandle,
            expected: TermHandle,
            sort: Option<Sort>,
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
        // A public conversion question owns both operand judgments. Internal
        // comparisons already have their premises from typing continuations.
        drive(arena, context, Operation::CheckConversion(query), budget)
            .map(JudgmentValue::require_converted)
    }

    fn drive<'a>(
        arena: &mut TermArena,
        context: &Context,
        operation: Operation<'a>,
        budget: &mut Budget,
    ) -> Result<JudgmentValue, CoreError> {
        let mut context = context.clone();
        let mut inferred_types = InferredTypes::under(&context);
        let mut pending = Vec::new();
        let mut current = operation;
        loop {
            if let Operation::Infer(term) = &current {
                let term = *term;
                let scope = if arena.required_bindings(term) == Some(0) {
                    InferenceScope(0)
                } else {
                    inferred_types.scope(&context)
                };
                if let Some(&ty) = inferred_types.types.get(&(scope, term)) {
                    current = Operation::ReusedInference(ty);
                } else {
                    pending.push(Pending::RememberInference { scope, term });
                }
            }
            let mut value = match current {
                Operation::ReusedInference(ty) => JudgmentValue::Inferred(ty),
                Operation::CheckConversion(query) => {
                    pending.push(Pending::ConversionType(query));
                    current = Operation::Infer(query.shared_type);
                    continue;
                }
                Operation::Convert(query) => {
                    pending.push(Pending::ConversionScope {
                        prefix: context.bindings.len(),
                    });
                    pending.push(Pending::Conversion(comparison::Work::SharedSort(query)));
                    current = Operation::Infer(query.shared_type);
                    continue;
                }
                Operation::Check { term, expected } => {
                    pending.push(Pending::CheckExpected { term, expected });
                    current = Operation::Infer(expected);
                    continue;
                }
                Operation::CheckFormed {
                    term,
                    expected,
                    sort,
                } => {
                    pending.push(Pending::CheckScope {
                        prefix: context.bindings.len(),
                    });
                    if let Term::Pair { first, second } = arena.get(term) {
                        let head =
                            weak_head_normalize(arena, context.signature(), expected, budget)?;
                        if let Term::Sigma { domain, codomain } = arena.get(head) {
                            pending.push(Pending::CheckPairSecond {
                                first,
                                second,
                                codomain,
                                prefix: context.bindings.len(),
                            });
                            current = Operation::CheckFormed {
                                term: first,
                                expected: domain,
                                sort: None,
                            };
                            continue;
                        }
                    }
                    if let Term::Lambda { domain, body } = arena.get(term) {
                        let head =
                            weak_head_normalize(arena, context.signature(), expected, budget)?;
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
                            continue;
                        }
                    }
                    let Some(expected_sort) = sort else {
                        current = Operation::Check { term, expected };
                        continue;
                    };
                    let shared_type = arena.insert(Term::Sort(expected_sort));
                    pending.push(Pending::CheckActual {
                        expected,
                        shared_type,
                    });
                    current = Operation::Infer(term);
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
                    Some(Pending::RememberInference { scope, term }) => {
                        let ty = value.require_inferred();
                        inferred_types.types.insert((scope, term), ty);
                        value = JudgmentValue::Inferred(ty);
                    }
                    Some(Pending::ConversionType(query)) => {
                        let shared_sort = weak_head_normalize(
                            arena,
                            context.signature(),
                            value.require_inferred(),
                            budget,
                        )?;
                        let sort =
                            inferred_sort(arena, &context, query.shared_type, shared_sort, budget)?;
                        pending.push(Pending::ConversionLeft {
                            query,
                            sort: sort.clone(),
                            shared_sort,
                        });
                        current = Operation::CheckFormed {
                            term: query.terms.left,
                            expected: query.shared_type,
                            sort: Some(sort),
                        };
                        break;
                    }
                    Some(Pending::ConversionLeft {
                        query,
                        sort,
                        shared_sort,
                    }) => {
                        value.require_checked();
                        pending.push(Pending::ConversionRight { query, shared_sort });
                        current = Operation::CheckFormed {
                            term: query.terms.right,
                            expected: query.shared_type,
                            sort: Some(sort),
                        };
                        break;
                    }
                    Some(Pending::ConversionRight { query, shared_sort }) => {
                        value.require_checked();
                        pending.push(Pending::ConversionScope {
                            prefix: context.bindings.len(),
                        });
                        pending.push(Pending::Conversion(comparison::Work::SharedSort(query)));
                        // Formation already normalized the shared type's inferred
                        // sort. Resume the existing comparison with that result,
                        // rather than spending its reductions a second time.
                        value = JudgmentValue::Inferred(shared_sort);
                    }
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
                        current = Operation::CheckFormed {
                            term: second,
                            expected: substitute(arena, codomain, first),
                            sort: None,
                        };
                        break;
                    }
                    Some(Pending::CheckLambdaAnnotation {
                        domain,
                        body,
                        expected_domain,
                        codomain,
                        shared_domain,
                    }) => {
                        value.require_checked();
                        let query =
                            comparison::Query::ordinary(domain, expected_domain, shared_domain);
                        pending.push(Pending::Compared(Compared::Annotation {
                            domain,
                            body,
                            expected_domain,
                            codomain,
                        }));
                        current = Operation::Convert(query);
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
                                let sort =
                                    inferred_sort(arena, &context, expected, inferred, budget)?;
                                current = Operation::CheckFormed {
                                    term,
                                    expected,
                                    sort: Some(sort),
                                };
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
                                pending.push(Pending::CheckLambdaAnnotation {
                                    domain,
                                    body,
                                    expected_domain,
                                    codomain,
                                    shared_domain,
                                });
                                // The expected domain has already formed. Exact syntax
                                // retains that premise; conversion alone cannot form a
                                // different annotation whose reduction hides a bad argument.
                                if arena.structurally_equal(domain, expected_domain) {
                                    value = JudgmentValue::Checked;
                                    continue;
                                }
                                current = Operation::Check {
                                    term: domain,
                                    expected: shared_domain,
                                };
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
                            Pending::RememberInference { .. }
                            | Pending::Compared(_)
                            | Pending::ConversionType(_)
                            | Pending::ConversionLeft { .. }
                            | Pending::ConversionRight { .. }
                            | Pending::ConversionScope { .. }
                            | Pending::Conversion(_)
                            | Pending::CheckScope { .. }
                            | Pending::CheckPairSecond { .. }
                            | Pending::CheckLambdaAnnotation { .. }
                            | Pending::ApplyPair { .. }
                            | Pending::IdentityLeft { .. }
                            | Pending::IdentityRight { .. }
                            | Pending::ReflexivityValue { .. }
                            | Pending::LayerIntroduction { .. }
                            | Pending::ReturnType { .. }
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

#[cfg(test)]
mod inference_reuse_tests {
    use super::*;

    fn equalities(
        arena: &mut TermArena,
        count: u32,
        outer_index: u32,
        last_is_zero: bool,
    ) -> TermHandle {
        let two = arena.insert(Term::Two);
        let zero = arena.insert(Term::TwoZero);
        let mut tail = None;
        for position in (0..count).rev() {
            let value = arena.insert(Term::Variable(outer_index + position));
            let atom = arena.insert(Term::Id {
                ty: two,
                left: value,
                right: if last_is_zero && position + 1 == count {
                    zero
                } else {
                    value
                },
            });
            tail = Some(if let Some(codomain) = tail {
                arena.insert(Term::Sigma {
                    domain: atom,
                    codomain,
                })
            } else {
                atom
            });
        }
        tail.expect("the fixture has at least one field")
    }

    fn copied_fields(arena: &mut TermArena, count: u32) -> TermHandle {
        let mut remaining = arena.insert(Term::Variable(0));
        let mut fields = Vec::new();
        for _ in 1..count {
            fields.push(arena.insert(Term::Fst { pair: remaining }));
            remaining = arena.insert(Term::Snd { pair: remaining });
        }
        while let Some(first) = fields.pop() {
            remaining = arena.insert(Term::Pair {
                first,
                second: remaining,
            });
        }
        remaining
    }

    #[test]
    fn shared_tuple_fields_do_not_rebuild_projection_types() {
        const FIELDS: u32 = 64;
        let mut arena = TermArena::new();
        let two = arena.insert(Term::Two);
        let input = equalities(&mut arena, FIELDS, 0, false);
        let context = Context::empty().extend(two).extend(input);
        let expected = equalities(&mut arena, FIELDS, 1, false);
        let term = copied_fields(&mut arena, FIELDS);
        let before = arena.len();
        check_type(&mut arena, &context, term, expected, &mut Budget::default()).unwrap();
        let growth = arena.len() - before;
        assert!(
            growth <= 32 * usize::try_from(FIELDS * FIELDS).unwrap(),
            "checking {FIELDS} shared fields allocated {growth} extra terms"
        );

        let changed = equalities(&mut arena, FIELDS, 1, true);
        assert!(matches!(
            check_type(&mut arena, &context, term, changed, &mut Budget::default()),
            Err(CoreError::TypeMismatch { .. })
        ));
    }

    #[test]
    fn shared_variables_keep_their_exact_binding_scope() {
        let mut arena = TermArena::new();
        let two = arena.insert(Term::Two);
        let empty = arena.insert(Term::Empty);
        let value = arena.insert(Term::Variable(0));
        let first = arena.insert(Term::Lambda {
            domain: two,
            body: value,
        });
        let second = arena.insert(Term::Lambda {
            domain: empty,
            body: value,
        });
        let term = arena.insert(Term::Pair { first, second });
        let domain = arena.insert(Term::Pi {
            domain: two,
            codomain: two,
        });
        for (result, accepted) in [(empty, true), (two, false)] {
            let codomain = arena.insert(Term::Pi {
                domain: empty,
                codomain: result,
            });
            let expected = arena.insert(Term::Sigma { domain, codomain });
            let checked = check_type(
                &mut arena,
                &Context::empty(),
                term,
                expected,
                &mut Budget::default(),
            );
            if accepted {
                checked.unwrap();
            } else {
                assert!(matches!(checked, Err(CoreError::TypeMismatch { .. })));
            }
        }
    }

    #[test]
    fn failed_inference_and_checking_do_not_supply_reusable_success() {
        let mut arena = TermArena::new();
        let two = arena.insert(Term::Two);
        let zero = arena.insert(Term::TwoZero);
        let empty = arena.insert(Term::Empty);
        let function = arena.insert(Term::Lambda {
            domain: empty,
            body: zero,
        });
        let invalid = arena.insert(Term::Apply {
            function,
            argument: zero,
        });
        let context = Context::empty();
        for _ in 0..2 {
            assert!(matches!(
                infer_type(&mut arena, &context, invalid, &mut Budget::default()),
                Err(CoreError::ArgumentTypeMismatch { .. })
            ));
            assert!(matches!(
                check_type(&mut arena, &context, invalid, two, &mut Budget::default()),
                Err(CoreError::ArgumentTypeMismatch { .. })
            ));
            check_type(&mut arena, &context, zero, two, &mut Budget::default()).unwrap();
        }
    }
}
