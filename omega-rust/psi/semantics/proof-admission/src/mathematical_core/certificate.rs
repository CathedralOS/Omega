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

use super::conversion::Budget;
use super::term::{TermArena, TermHandle};
use super::typing::{Context, CoreError, check_type, infer_sort};

/// One complete mathematical judgment supplied as producer evidence.
///
/// All handles resolve in a single [`TermArena`]; the wire form in
/// `terminal-codec` is the canonical portable shape. `context` holds binder
/// types ordered outermost-first — the order [`Context::extend`] consumes —
/// so `context.last()` is the innermost binding that de Bruijn index 0 names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathematicalCertificate {
    pub context: Vec<TermHandle>,
    /// The elaborated evidence term: fully annotated, so checking never
    /// searches.
    pub term: TermHandle,
    /// The claimed type. The caller decides whether it is the obligation it
    /// reconstructed; the kernel only decides that `term` inhabits it.
    pub expected: TermHandle,
}

/// Re-decide a certificate's claimed judgment `Γ ⊢ t : T`.
///
/// Every context binding must itself be a type under the bindings before it
/// (`infer_sort` under the prefix), which is the formation half of a valid
/// context that `Context::extend` deliberately does not repeat at each call.
/// Then `term` is checked against `expected` under the rebuilt context.
/// Resource exhaustion surfaces as `CoreError::StepCeiling`, never as a
/// false judgment.
pub fn verify_mathematical_certificate(
    arena: &mut TermArena,
    certificate: &MathematicalCertificate,
    budget: &mut Budget,
) -> Result<(), CoreError> {
    let mut context = Context::empty();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mathematical_core::{DEFAULT_CONVERSION_STEPS, Level, Sort, Term};

    fn budget() -> Budget {
        Budget::new(DEFAULT_CONVERSION_STEPS)
    }

    fn type_sort(arena: &mut TermArena, level: u32) -> TermHandle {
        arena.insert(Term::Sort(Sort::Type(Level(level))))
    }

    fn strict_sort(arena: &mut TermArena, level: u32) -> TermHandle {
        arena.insert(Term::Sort(Sort::Strict(Level(level))))
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
            context: Vec::new(),
            term: bound,
            expected: identity_body,
        };
        assert!(matches!(
            verify_mathematical_certificate(&mut arena, &certificate, &mut budget()),
            Err(CoreError::NotASort { .. })
        ));
    }
}
