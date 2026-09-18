//! Mathematical derivation certificates: the untrusted producer's complete
//! elaborated judgment `Γ ⊢ t : T`.
//!
//! A certificate is data, not authority: it carries the claimed context, the
//! evidence term, and the claimed type, and this module re-decides the whole
//! judgment through the kernel. There is no producer success flag to trust.
//! Context bindings are checked for formation under their own prefix before
//! use — a certificate cannot smuggle a meaningful variable type past the
//! checker by naming a binding that is not itself a type. The judgment then
//! runs through `check_type`, so malformed terms, wrong claimed types,
//! capture-changing substitutions, and exhausted conversion budgets all
//! reject as typed errors.
//!
//! Binding the claimed type to a verifier-reconstructed obligation stays with
//! the caller, exactly as `accept_certificate` leaves goal reconstruction to
//! `verify_obligation`: a certificate only ever establishes "this judgment
//! holds", never "this is the obligation you wanted discharged".

use std::collections::BTreeSet;

use super::conversion::Budget;
use super::signature::{Declaration, Signature, check_signature, judgment_assumption_closure};
use super::term::{TermArena, TermHandle};
use super::typing::{Context, CoreError, check_type, infer_sort};

/// One complete mathematical judgment supplied as producer evidence.
///
/// All handles resolve in a single [`TermArena`]; the wire form in
/// `terminal-codec` is the canonical portable shape. `level_arity` is the
/// judgment's universe scope: the claim is parametric over that many level
/// parameters, so `Level::Parameter(i)` is in scope exactly when
/// `i < level_arity`. `context` holds binder types ordered outermost-first
/// — the order [`Context::extend`] consumes — so `context.last()` is the
/// innermost binding that de Bruijn index 0 names.
///
/// `signature` is the ambient declaration environment the judgment's
/// `Constant` references resolve against — every declaration the
/// evidence or its claims name must appear here, because the signature
/// is producer evidence too: [`verify_mathematical_certificate`]
/// re-decides it through [`check_signature`] before the judgment runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathematicalCertificate {
    /// The declarations the judgment is checked under, in signature
    /// order: each is checked under the ones before it, so a certificate
    /// cannot smuggle a recursive or forward-referencing declaration.
    pub signature: Vec<Declaration>,
    /// The number of universe parameters the judgment is polymorphic over.
    /// `0` is a closed judgment; the kernel checks the judgment for all
    /// instantiations of the parameters, never for a guessed one.
    pub level_arity: u32,
    pub context: Vec<TermHandle>,
    /// The elaborated evidence term: fully annotated, so checking never
    /// searches.
    pub term: TermHandle,
    /// The claimed type. The caller decides whether it is the obligation it
    /// reconstructed; the kernel only decides that `term` inhabits it.
    pub expected: TermHandle,
}

/// Stack reserved for one certificate verification or judgment traversal.
/// The checkers recurse over the certificate's term structure — elaborated
/// judgments from real programs nest far deeper than the default thread
/// stack admits — so the public verification entries run on a dedicated
/// stack rather than trusting the caller's, exactly as the checked
/// interpreter runs its tree-walker. The reservation is virtual: pages
/// commit only as recursion actually descends.
const VERIFICATION_STACK_SIZE: usize = 256 * 1024 * 1024;

/// Run `f` on a scoped worker thread with [`VERIFICATION_STACK_SIZE`].
/// A panic on the worker is re-thrown on the caller's stack unchanged:
/// the wide stack absorbs depth, it never decides a different answer.
pub(crate) fn run_on_verification_stack<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(VERIFICATION_STACK_SIZE)
            .spawn_scoped(scope, f)
            .expect("spawn certificate verification worker thread")
            .join()
            .unwrap_or_else(|payload| std::panic::resume_unwind(payload))
    })
}

/// Re-decide a certificate's claimed judgment `Σ; Δ; Γ ⊢ t : T`.
///
/// The signature is checked first: every declaration must hold under the
/// declarations before it, which is what rules out recursive and
/// forward-referencing declarations before any constant resolves. Then
/// every context binding must itself be a type under the bindings before
/// it (`infer_sort` under the prefix), which is the formation half of a
/// valid context that `Context::extend` deliberately does not repeat at
/// each call. Then `term` is checked against `expected` under the rebuilt
/// context. Resource exhaustion surfaces as `CoreError::StepCeiling`,
/// never as a false judgment.
pub fn verify_mathematical_certificate(
    arena: &mut TermArena,
    certificate: &MathematicalCertificate,
    budget: &mut Budget,
) -> Result<(), CoreError> {
    run_on_verification_stack(|| {
        verify_mathematical_certificate_on_current_thread(arena, certificate, budget)
    })
}

/// [`verify_mathematical_certificate`] on the caller's stack: used by the
/// bounded-certificate route, which already runs its whole elaboration and
/// verification inside [`run_on_verification_stack`].
pub(crate) fn verify_mathematical_certificate_on_current_thread(
    arena: &mut TermArena,
    certificate: &MathematicalCertificate,
    budget: &mut Budget,
) -> Result<(), CoreError> {
    let signature = check_signature(arena, &certificate.signature, budget)?;
    let mut context = Context::with_level_arity(certificate.level_arity).with_signature(signature);
    for &binding in &certificate.context {
        infer_sort(arena, &context, binding, budget)?;
        context = context.extend(binding);
    }
    check_type(
        arena,
        &context,
        certificate.term,
        certificate.expected,
        budget,
    )
}

/// The exact assumption closure of a verified certificate's judgment:
/// every `Constant` the context bindings, the evidence term and the
/// claimed type name, followed transitively through declaration
/// statements *and* bodies — computed over the stored signature, never
/// by watching which constants conversion happened to unfold.
///
/// This is the receiver's policy input: verification decides that the
/// judgment holds; the closure decides which named assumptions that
/// judgment actually commits to. See `foundation.md`'s
/// assumptions-and-calculus-identity section.
pub fn certificate_assumption_closure(
    arena: &TermArena,
    certificate: &MathematicalCertificate,
) -> BTreeSet<u32> {
    run_on_verification_stack(|| {
        certificate_assumption_closure_on_current_thread(arena, certificate)
    })
}

/// [`certificate_assumption_closure`] on the caller's stack, for callers
/// already inside [`run_on_verification_stack`].
pub(crate) fn certificate_assumption_closure_on_current_thread(
    arena: &TermArena,
    certificate: &MathematicalCertificate,
) -> BTreeSet<u32> {
    let signature = Signature::from_declarations(certificate.signature.clone());
    let roots: Vec<TermHandle> = certificate
        .context
        .iter()
        .copied()
        .chain([certificate.term, certificate.expected])
        .collect();
    judgment_assumption_closure(arena, &signature, &roots)
}

#[cfg(test)]
mod tests {
    use super::{
        Budget, CoreError, Declaration, MathematicalCertificate, TermArena, TermHandle,
        certificate_assumption_closure, verify_mathematical_certificate,
    };
    use crate::mathematical_core::{DEFAULT_CONVERSION_STEPS, Level, Sort, Term};

    fn budget() -> Budget {
        Budget::new(DEFAULT_CONVERSION_STEPS)
    }

    fn type_sort(arena: &mut TermArena, level: u32) -> TermHandle {
        arena.insert(Term::Sort(Sort::Type(Level::Constant(level))))
    }

    fn strict_sort(arena: &mut TermArena, level: u32) -> TermHandle {
        arena.insert(Term::Sort(Sort::Strict(Level::Constant(level))))
    }

    fn variable(arena: &mut TermArena, index: u32) -> TermHandle {
        arena.insert(Term::Variable(index))
    }

    fn pi(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
        arena.insert(Term::Pi { domain, codomain })
    }

    fn lambda(arena: &mut TermArena, domain: TermHandle, body: TermHandle) -> TermHandle {
        arena.insert(Term::Lambda { domain, body })
    }

    /// `λ(A : Type 0). λ(x : A). x` and its type `Π(A : Type 0). Π(x : A). A`.
    fn polymorphic_identity(arena: &mut TermArena) -> (TermHandle, TermHandle) {
        let type_zero = type_sort(arena, 0);
        let bound = variable(arena, 0);
        let inner = lambda(arena, bound, bound);
        let identity = lambda(arena, type_zero, inner);
        let codomain_domain = variable(arena, 0);
        let codomain_body = variable(arena, 1);
        let codomain = pi(arena, codomain_domain, codomain_body);
        let expected = pi(arena, type_zero, codomain);
        (identity, expected)
    }

    #[test]
    fn a_complete_certificate_verifies() {
        let mut arena = TermArena::new();
        let (identity, expected) = polymorphic_identity(&mut arena);
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: Vec::new(),
            term: identity,
            expected,
        };
        verify_mathematical_certificate(&mut arena, &certificate, &mut budget()).unwrap();
    }

    #[test]
    fn the_context_is_part_of_the_checked_judgment() {
        let mut arena = TermArena::new();
        let type_zero = type_sort(&mut arena, 0);
        let bound = variable(&mut arena, 0);
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: vec![type_zero],
            term: bound,
            expected: type_zero,
        };
        verify_mathematical_certificate(&mut arena, &certificate, &mut budget()).unwrap();

        // The same term and type without their context is a different,
        // unprovable judgment: the variable is unbound.
        let mut arena = TermArena::new();
        let type_zero = type_sort(&mut arena, 0);
        let bound = variable(&mut arena, 0);
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: Vec::new(),
            term: bound,
            expected: type_zero,
        };
        assert_eq!(
            verify_mathematical_certificate(&mut arena, &certificate, &mut budget()),
            Err(CoreError::UnboundVariable {
                index: 0,
                context_depth: 0,
            })
        );
    }

    #[test]
    fn dependent_context_bindings_check_formation_under_their_prefix() {
        let mut arena = TermArena::new();
        // Γ = A : Type 0, x : A. In the full context x's type is `Variable(1)`.
        let type_zero = type_sort(&mut arena, 0);
        let bound = variable(&mut arena, 0);
        let shifted = variable(&mut arena, 1);
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: vec![type_zero, bound],
            term: bound,
            expected: shifted,
        };
        verify_mathematical_certificate(&mut arena, &certificate, &mut budget()).unwrap();

        // A binding that is not a type under its prefix cannot stand in the
        // context: `λ(x : Type 0). x` is a term, not a type.
        let mut arena = TermArena::new();
        let type_zero = type_sort(&mut arena, 0);
        let bound = variable(&mut arena, 0);
        let not_a_type = lambda(&mut arena, type_zero, bound);
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: vec![not_a_type],
            term: bound,
            expected: type_zero,
        };
        assert!(matches!(
            verify_mathematical_certificate(&mut arena, &certificate, &mut budget()),
            Err(CoreError::NotASort { .. })
        ));
    }

    #[test]
    fn a_tampered_term_or_claimed_type_rejects() {
        let mut arena = TermArena::new();
        let (identity, expected) = polymorphic_identity(&mut arena);
        let type_zero = type_sort(&mut arena, 0);

        // Claiming a different type for the same term rejects:
        // `Π(A : Type 0). Π(x : A). Type 0` does not return the argument.
        let bound = variable(&mut arena, 0);
        let wrong_inner = pi(&mut arena, bound, type_zero);
        let wrong_expected = pi(&mut arena, type_zero, wrong_inner);
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: Vec::new(),
            term: identity,
            expected: wrong_expected,
        };
        assert!(matches!(
            verify_mathematical_certificate(&mut arena, &certificate, &mut budget()),
            Err(CoreError::TypeMismatch { .. })
        ));

        // Claiming the right type for a different term rejects: a bare
        // variable is unbound in the empty context.
        let free_variable = variable(&mut arena, 0);
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: Vec::new(),
            term: free_variable,
            expected,
        };
        assert!(matches!(
            verify_mathematical_certificate(&mut arena, &certificate, &mut budget()),
            Err(CoreError::UnboundVariable { .. })
        ));
    }

    #[test]
    fn a_certificate_carries_dependent_two_elimination() {
        let mut arena = TermArena::new();
        // Γ = A : Type 0, B : Type 0, a : A, b : B, t : Two proves
        // `caseTwo(C, a, b, t) : C t` for `C := λ(s:Two). caseTwo(M, A, B, s)`
        // with `M := λ(_:Two). Type 0`. The family lands at A on `zero`
        // and at B on `one`, so each branch is checked at a
        // definitionally different type — the certificate carries the
        // inductive profile's eliminator as data, and the kernel
        // re-decides it.
        let type_zero = type_sort(&mut arena, 0);
        let a_binding = variable(&mut arena, 1);
        let b_binding = variable(&mut arena, 1);
        let two_binding = arena.insert(Term::Two);
        // `M := λ(_:Two). Type 0` is closed.
        let motive_domain = arena.insert(Term::Two);
        let motive_body = type_sort(&mut arena, 0);
        let motive = lambda(&mut arena, motive_domain, motive_body);
        // C's body under its own binder (depth 6): A is index 5, B is
        // index 4, s is index 0.
        let a_under = variable(&mut arena, 5);
        let b_under = variable(&mut arena, 4);
        let s_under = variable(&mut arena, 0);
        let family_body = arena.insert(Term::CaseTwo {
            motive,
            zero_branch: a_under,
            one_branch: b_under,
            scrutinee: s_under,
        });
        let family_domain = arena.insert(Term::Two);
        let family = lambda(&mut arena, family_domain, family_body);
        // The elimination in the full context: a is index 2, b is index
        // 1, t is index 0.
        let a_term = variable(&mut arena, 2);
        let b_term = variable(&mut arena, 1);
        let scrutinee = variable(&mut arena, 0);
        let term = arena.insert(Term::CaseTwo {
            motive: family,
            zero_branch: a_term,
            one_branch: b_term,
            scrutinee,
        });
        let expected = arena.insert(Term::Apply {
            function: family,
            argument: scrutinee,
        });
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: vec![type_zero, type_zero, a_binding, b_binding, two_binding],
            term,
            expected,
        };
        verify_mathematical_certificate(&mut arena, &certificate, &mut budget()).unwrap();

        // The same elimination at `zero`'s landing `C zero ≡ A` is not
        // the claimed judgment: `C t` stays stuck on the neutral
        // scrutinee and never collapses to either branch's type.
        let zero = arena.insert(Term::TwoZero);
        let wrong_expected = arena.insert(Term::Apply {
            function: family,
            argument: zero,
        });
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: vec![type_zero, type_zero, a_binding, b_binding, two_binding],
            term,
            expected: wrong_expected,
        };
        assert!(matches!(
            verify_mathematical_certificate(&mut arena, &certificate, &mut budget()),
            Err(CoreError::TypeMismatch { .. })
        ));
    }

    #[test]
    fn a_certificate_carries_identity_elimination() {
        let mut arena = TermArena::new();
        // Γ = A : Type 0, x : A, P : Π(y:A). Type 0, y : A,
        // p : Id A x y, h : P x proves `J(C, λ(h:Px).h, y, p) h : P y`
        // for the transport motive `C := λ(y:A). λ(_:Id A x y).
        // Π(_:P x). P y`: the kernel re-decides the dependent
        // elimination and the outer application together. In depth 6:
        // A is index 5, x is 4, P is 3, y is 2, p is 1, h is 0.
        let type_zero = type_sort(&mut arena, 0);
        let x_binding = variable(&mut arena, 0);
        let predicate_binding = {
            let domain = variable(&mut arena, 1);
            pi(&mut arena, domain, type_zero)
        };
        let y_binding = variable(&mut arena, 2);
        let proof_binding = {
            let ty = variable(&mut arena, 3);
            let x = variable(&mut arena, 2);
            let y = variable(&mut arena, 0);
            arena.insert(Term::Id {
                ty,
                left: x,
                right: y,
            })
        };
        let hypothesis_binding = {
            let predicate = variable(&mut arena, 2);
            let x = variable(&mut arena, 3);
            arena.insert(Term::Apply {
                function: predicate,
                argument: x,
            })
        };
        // The motive under the full context: its second domain is
        // `Id A x y` under the endpoint binder (A index 6, x index 5,
        // bound y index 0), and its body `Π(_:P x). P y` under both
        // binders (P index 5, x index 6; the bound y is index 2 under
        // the inner Π binder).
        let motive = {
            let domain = variable(&mut arena, 5);
            let inner_domain = {
                let ty = variable(&mut arena, 6);
                let x = variable(&mut arena, 5);
                let bound = variable(&mut arena, 0);
                arena.insert(Term::Id {
                    ty,
                    left: x,
                    right: bound,
                })
            };
            let body = {
                let domain = {
                    let predicate = variable(&mut arena, 5);
                    let x = variable(&mut arena, 6);
                    arena.insert(Term::Apply {
                        function: predicate,
                        argument: x,
                    })
                };
                let codomain = {
                    let predicate = variable(&mut arena, 6);
                    let bound_y = variable(&mut arena, 2);
                    arena.insert(Term::Apply {
                        function: predicate,
                        argument: bound_y,
                    })
                };
                pi(&mut arena, domain, codomain)
            };
            let inner = lambda(&mut arena, inner_domain, body);
            lambda(&mut arena, domain, inner)
        };
        // The base `λ(h : P x). h` checks at `C x (refl A x) ≡
        // Π(_ : P x). P x`.
        let base = {
            let domain = {
                let predicate = variable(&mut arena, 3);
                let x = variable(&mut arena, 4);
                arena.insert(Term::Apply {
                    function: predicate,
                    argument: x,
                })
            };
            let bound = variable(&mut arena, 0);
            lambda(&mut arena, domain, bound)
        };
        let endpoint = variable(&mut arena, 2);
        let proof = variable(&mut arena, 1);
        let elimination = arena.insert(Term::IdElim {
            motive,
            base,
            endpoint,
            proof,
        });
        let hypothesis = variable(&mut arena, 0);
        let term = arena.insert(Term::Apply {
            function: elimination,
            argument: hypothesis,
        });
        let expected = {
            let predicate = variable(&mut arena, 3);
            let y = variable(&mut arena, 2);
            arena.insert(Term::Apply {
                function: predicate,
                argument: y,
            })
        };
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: vec![
                type_zero,
                x_binding,
                predicate_binding,
                y_binding,
                proof_binding,
                hypothesis_binding,
            ],
            term,
            expected,
        };
        verify_mathematical_certificate(&mut arena, &certificate, &mut budget()).unwrap();

        // Claiming `P x` instead of `P y` is a different, false
        // judgment: transport moved the subject, and the kernel
        // re-decides that.
        let wrong_expected = {
            let predicate = variable(&mut arena, 3);
            let x = variable(&mut arena, 4);
            arena.insert(Term::Apply {
                function: predicate,
                argument: x,
            })
        };
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: vec![
                type_zero,
                x_binding,
                predicate_binding,
                y_binding,
                proof_binding,
                hypothesis_binding,
            ],
            term,
            expected: wrong_expected,
        };
        assert!(matches!(
            verify_mathematical_certificate(&mut arena, &certificate, &mut budget()),
            Err(CoreError::TypeMismatch { .. })
        ));
    }

    #[test]
    fn a_certificate_carries_w_induction() {
        let mut arena = TermArena::new();
        // Γ = A : Type 0, B : Π(_:A). Type 0, a : A,
        //     k : Π(b : B a). W A B, P : Π(_ : W A B). Type 0,
        //     s : <step type>, t : W A B
        // proves `indW(P, s, sup A B a k) : P (sup A B a k)` — the
        // kernel re-decides W formation, the `sup` annotation, and the
        // dependent induction step type. At depth 7: t = 0, s = 1,
        // P = 2, k = 3, a = 4, B = 5, A = 6.
        let type_zero = type_sort(&mut arena, 0);
        let b_binding = {
            let a_in_prefix = variable(&mut arena, 0);
            pi(&mut arena, a_in_prefix, type_zero)
        };
        let a_binding = variable(&mut arena, 1);
        let k_binding = {
            let b_at = variable(&mut arena, 1);
            let a_at = variable(&mut arena, 0);
            let child_positions = arena.insert(Term::Apply {
                function: b_at,
                argument: a_at,
            });
            let carrier = variable(&mut arena, 3);
            let children = variable(&mut arena, 2);
            let w_under_b = arena.insert(Term::W { carrier, children });
            pi(&mut arena, child_positions, w_under_b)
        };
        let p_binding = {
            let carrier = variable(&mut arena, 3);
            let children = variable(&mut arena, 2);
            let w = arena.insert(Term::W { carrier, children });
            pi(&mut arena, w, type_zero)
        };
        // `Π(a' : A). Π(k' : Π(b : B a'). W A B). Π(_ : Π(b : B a').
        // P (k' b)). P (sup A B a' k')` over the five-binding prefix.
        let s_binding = {
            let a_domain = variable(&mut arena, 4);
            let function_type = {
                let b_at = variable(&mut arena, 4);
                let a_bound = variable(&mut arena, 0);
                let child_positions = arena.insert(Term::Apply {
                    function: b_at,
                    argument: a_bound,
                });
                let carrier = variable(&mut arena, 6);
                let children = variable(&mut arena, 5);
                let w_at_two = arena.insert(Term::W { carrier, children });
                pi(&mut arena, child_positions, w_at_two)
            };
            let hypothesis = {
                let b_at = variable(&mut arena, 5);
                let a_bound = variable(&mut arena, 1);
                let hypothesis_domain = arena.insert(Term::Apply {
                    function: b_at,
                    argument: a_bound,
                });
                let p_at = variable(&mut arena, 3);
                let k_bound = variable(&mut arena, 1);
                let b_bound = variable(&mut arena, 0);
                let child = arena.insert(Term::Apply {
                    function: k_bound,
                    argument: b_bound,
                });
                let hypothesis_codomain = arena.insert(Term::Apply {
                    function: p_at,
                    argument: child,
                });
                pi(&mut arena, hypothesis_domain, hypothesis_codomain)
            };
            let result = {
                let p_at = variable(&mut arena, 3);
                let carrier = variable(&mut arena, 7);
                let children = variable(&mut arena, 6);
                let label = variable(&mut arena, 2);
                let function = variable(&mut arena, 1);
                let node = arena.insert(Term::Sup {
                    carrier,
                    children,
                    label,
                    function,
                });
                arena.insert(Term::Apply {
                    function: p_at,
                    argument: node,
                })
            };
            let inner = pi(&mut arena, hypothesis, result);
            let middle = pi(&mut arena, function_type, inner);
            pi(&mut arena, a_domain, middle)
        };
        let t_binding = {
            let carrier = variable(&mut arena, 5);
            let children = variable(&mut arena, 4);
            arena.insert(Term::W { carrier, children })
        };
        let node = {
            let carrier = variable(&mut arena, 6);
            let children = variable(&mut arena, 5);
            let label = variable(&mut arena, 4);
            let function = variable(&mut arena, 3);
            arena.insert(Term::Sup {
                carrier,
                children,
                label,
                function,
            })
        };
        let term = {
            let motive = variable(&mut arena, 2);
            let step = variable(&mut arena, 1);
            arena.insert(Term::IndW {
                motive,
                step,
                tree: node,
            })
        };
        let expected = {
            let p = variable(&mut arena, 2);
            arena.insert(Term::Apply {
                function: p,
                argument: node,
            })
        };
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: vec![
                type_zero, b_binding, a_binding, k_binding, p_binding, s_binding, t_binding,
            ],
            term,
            expected,
        };
        verify_mathematical_certificate(&mut arena, &certificate, &mut budget()).unwrap();

        // Claiming `P t` instead of `P (sup …)` is a different, false
        // judgment: the neutral tree `t` is not the constructor the
        // induction ran on.
        let wrong_expected = {
            let p = variable(&mut arena, 2);
            let t = variable(&mut arena, 0);
            arena.insert(Term::Apply {
                function: p,
                argument: t,
            })
        };
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: vec![
                type_zero, b_binding, a_binding, k_binding, p_binding, s_binding, t_binding,
            ],
            term,
            expected: wrong_expected,
        };
        assert!(matches!(
            verify_mathematical_certificate(&mut arena, &certificate, &mut budget()),
            Err(CoreError::TypeMismatch { .. })
        ));
    }

    #[test]
    fn strict_sorts_flow_through_the_certificate() {
        let mut arena = TermArena::new();
        // Γ = P : Strict 0, x : P. In the full context x's type is
        // `Variable(1)`; checking `x : P` exercises the strict layer the
        // certificate carries.
        let strict_zero = strict_sort(&mut arena, 0);
        let bound = variable(&mut arena, 0);
        let shifted = variable(&mut arena, 1);
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: vec![strict_zero, bound],
            term: bound,
            expected: shifted,
        };
        verify_mathematical_certificate(&mut arena, &certificate, &mut budget()).unwrap();

        // A claimed type that is not a type at all — a lambda — rejects
        // rather than inheriting any collapse.
        let mut arena = TermArena::new();
        let type_zero = type_sort(&mut arena, 0);
        let bound = variable(&mut arena, 0);
        let identity_body = lambda(&mut arena, type_zero, bound);
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: Vec::new(),
            term: bound,
            expected: identity_body,
        };
        assert!(matches!(
            verify_mathematical_certificate(&mut arena, &certificate, &mut budget()),
            Err(CoreError::NotASort { .. })
        ));
    }

    #[test]
    fn a_certificate_is_parametric_over_its_level_arity() {
        let mut arena = TermArena::new();
        // Under one universe parameter `λ(A : Type u). λ(x : A). x` proves
        // `Π(A : Type u). Π(x : A). A`: the certificate quantifies the
        // judgment over every instantiation of `u`.
        let type_u = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
        let bound = variable(&mut arena, 0);
        let inner = lambda(&mut arena, bound, bound);
        let identity = lambda(&mut arena, type_u, inner);
        let codomain_domain = variable(&mut arena, 0);
        let codomain_body = variable(&mut arena, 1);
        let codomain = pi(&mut arena, codomain_domain, codomain_body);
        let expected = pi(&mut arena, type_u, codomain);
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 1,
            context: Vec::new(),
            term: identity,
            expected,
        };
        verify_mathematical_certificate(&mut arena, &certificate, &mut budget()).unwrap();

        // The same bytes under a closed judgment are a malformed
        // universe: `u` names nothing without the arity to bind it.
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: Vec::new(),
            term: identity,
            expected,
        };
        assert_eq!(
            verify_mathematical_certificate(&mut arena, &certificate, &mut budget()),
            Err(CoreError::UnboundLevelParameter { index: 0, arity: 0 })
        );

        // A binding whose type references a parameter outside the arity
        // rejects at context formation — a certificate cannot widen its
        // level scope by declaring a binding inside it.
        let mut arena = TermArena::new();
        let type_v = arena.insert(Term::Sort(Sort::Type(Level::Parameter(1))));
        let bound = variable(&mut arena, 0);
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 1,
            context: vec![type_v],
            term: bound,
            expected: type_v,
        };
        assert_eq!(
            verify_mathematical_certificate(&mut arena, &certificate, &mut budget()),
            Err(CoreError::UnboundLevelParameter { index: 1, arity: 1 })
        );
    }

    /// `polyId : Π(A : Type u). Π(x : A). A := λA. λx. x`, one level
    /// parameter — shared by the signature tests below.
    fn polymorphic_identity_declaration(arena: &mut TermArena) -> Declaration {
        let type_u = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
        let bound_a = variable(arena, 0);
        let inner_a = variable(arena, 1);
        let inner_pi = pi(arena, bound_a, inner_a);
        let ty = pi(arena, type_u, inner_pi);
        let type_u = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
        let bound_a = variable(arena, 0);
        let inner_x = variable(arena, 0);
        let inner = lambda(arena, bound_a, inner_x);
        let body = lambda(arena, type_u, inner);
        Declaration::definition(1, ty, body)
    }

    fn instantiated_identity_type(arena: &mut TermArena, u: Level) -> TermHandle {
        let type_u = arena.insert(Term::Sort(Sort::Type(u)));
        let bound_a = variable(arena, 0);
        let inner_a = variable(arena, 1);
        let inner_pi = pi(arena, bound_a, inner_a);
        pi(arena, type_u, inner_pi)
    }

    #[test]
    fn a_signed_certificate_verifies_its_declaration_closure() {
        let mut arena = TermArena::new();
        let declaration = polymorphic_identity_declaration(&mut arena);
        let evidence = arena.insert(Term::Constant {
            declaration: 0,
            levels: vec![Level::Constant(0)],
        });
        let expected = instantiated_identity_type(&mut arena, Level::Constant(0));
        let certificate = MathematicalCertificate {
            signature: vec![declaration],
            level_arity: 0,
            context: Vec::new(),
            term: evidence,
            expected,
        };
        verify_mathematical_certificate(&mut arena, &certificate, &mut budget()).unwrap();
    }

    #[test]
    fn a_forged_declaration_index_never_verifies() {
        let mut arena = TermArena::new();
        let declaration = polymorphic_identity_declaration(&mut arena);
        // The signature has exactly one declaration; index 1 points past it.
        let forged = arena.insert(Term::Constant {
            declaration: 1,
            levels: vec![Level::Constant(0)],
        });
        let expected = instantiated_identity_type(&mut arena, Level::Constant(0));
        let certificate = MathematicalCertificate {
            signature: vec![declaration],
            level_arity: 0,
            context: Vec::new(),
            term: forged,
            expected,
        };
        assert_eq!(
            verify_mathematical_certificate(&mut arena, &certificate, &mut budget()),
            Err(CoreError::UnknownDeclaration {
                declaration: 1,
                signature_len: 1,
            })
        );
    }

    #[test]
    fn an_unchecked_signature_entry_never_verifies() {
        let mut arena = TermArena::new();
        // The second declaration claims `Type 0` but supplies `Type 0` as
        // its body — a universe is not a `Type 0` inhabitant, so the
        // signature itself is rejected before the judgment runs.
        let type_zero = type_sort(&mut arena, 0);
        let two = arena.insert(Term::Two);
        let sound = Declaration::definition(0, type_zero, two);
        let two = arena.insert(Term::Two);
        let type_zero = type_sort(&mut arena, 0);
        let unsound = Declaration::definition(0, two, type_zero);
        let evidence = arena.insert(Term::Constant {
            declaration: 1,
            levels: Vec::new(),
        });
        let two = arena.insert(Term::Two);
        let certificate = MathematicalCertificate {
            signature: vec![sound, unsound],
            level_arity: 0,
            context: Vec::new(),
            term: evidence,
            expected: two,
        };
        assert!(matches!(
            verify_mathematical_certificate(&mut arena, &certificate, &mut budget()),
            Err(CoreError::TypeMismatch { .. })
        ));
    }

    #[test]
    fn the_certificate_records_its_exact_assumption_closure() {
        let mut arena = TermArena::new();
        // `axiom : Type 0` and `uses : Type 0 := axiom`; the evidence names
        // only `uses`, but the closure reports `axiom` through its body.
        let type_zero = type_sort(&mut arena, 0);
        let axiom = Declaration::assumption(0, type_zero);
        let axiom_constant = arena.insert(Term::Constant {
            declaration: 0,
            levels: Vec::new(),
        });
        let type_zero = type_sort(&mut arena, 0);
        let uses = Declaration::definition(0, type_zero, axiom_constant);
        let evidence = arena.insert(Term::Constant {
            declaration: 1,
            levels: Vec::new(),
        });
        let type_zero = type_sort(&mut arena, 0);
        let certificate = MathematicalCertificate {
            signature: vec![axiom, uses],
            level_arity: 0,
            context: Vec::new(),
            term: evidence,
            expected: type_zero,
        };
        verify_mathematical_certificate(&mut arena, &certificate, &mut budget()).unwrap();
        assert_eq!(
            certificate_assumption_closure(&arena, &certificate),
            [0].into_iter().collect()
        );
    }
}
