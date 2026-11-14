/*
    Common predicative mathematical core (`sMLTT` with `Two`, `Id`, and `W`):
    de Bruijn terms, universe levels, declarations, signatures, capture-avoiding
    substitution, weak-head normalization, definitional conversion, derived
    indexed and quotient schemes, and certificate verification.

    We define the shared term arena and declaration signature model and re-decide
    complete `MathematicalCertificate` judgments under explicit conversion budgets.
*/

pub(crate) use crate::bounded_denotation;
pub(crate) mod certificate {
    mod batch {
        use super::MathematicalCertificate;
        use crate::mathematical_core::term::MAX_SUBTERMS;
        use crate::mathematical_core::{
            Budget, Context, CoreError, Declaration, Signature, Term, TermArena, TermHandle,
            check_signature, check_type, infer_sort,
        };
        use std::collections::HashMap;

        #[derive(Debug)]
        pub struct MathematicalCertificateBatchError {
            pub ordinal: usize,
            pub error: CoreError,
        }

        pub fn verify_mathematical_certificates(
            certificates: &[(&TermArena, &MathematicalCertificate)],
            budget: &mut Budget,
        ) -> Result<(), MathematicalCertificateBatchError> {
            let mut imported = ImportedTerms::default();
            let mut signatures: Vec<(Vec<Declaration>, Signature)> = Vec::new();
            for (ordinal, (source, certificate)) in certificates.iter().enumerate() {
                let certificate = imported
                    .certificate(source, certificate)
                    .map_err(|error| MathematicalCertificateBatchError { ordinal, error })?;
                let result = (|| {
                    let signature = if let Some((_, signature)) = signatures
                        .iter()
                        .find(|(declarations, _)| *declarations == certificate.signature)
                    {
                        signature.clone()
                    } else {
                        let signature =
                            check_signature(&mut imported.arena, &certificate.signature, budget)?;
                        signatures.push((certificate.signature.clone(), signature.clone()));
                        signature
                    };
                    let mut context = Context::with_level_arity(certificate.level_arity)
                        .with_signature(signature);
                    for binding in certificate.context {
                        infer_sort(&mut imported.arena, &context, binding, budget)?;
                        context = context.extend(binding);
                    }
                    check_type(
                        &mut imported.arena,
                        &context,
                        certificate.term,
                        certificate.expected,
                        budget,
                    )
                })();
                result.map_err(|error| MathematicalCertificateBatchError { ordinal, error })?;
            }
            Ok(())
        }

        #[derive(Default)]
        struct ImportedTerms {
            arena: TermArena,
            by_syntax: HashMap<Term, TermHandle>,
        }
        impl ImportedTerms {
            fn certificate(
                &mut self,
                source: &TermArena,
                certificate: &MathematicalCertificate,
            ) -> Result<MathematicalCertificate, CoreError> {
                let mut local = HashMap::new();
                Ok(MathematicalCertificate {
                    signature: certificate
                        .signature
                        .iter()
                        .map(|declaration| {
                            Ok(Declaration {
                                level_arity: declaration.level_arity,
                                ty: self.term(source, declaration.ty, &mut local)?,
                                body: declaration
                                    .body
                                    .map(|body| self.term(source, body, &mut local))
                                    .transpose()?,
                            })
                        })
                        .collect::<Result<_, CoreError>>()?,
                    level_arity: certificate.level_arity,
                    context: certificate
                        .context
                        .iter()
                        .map(|&term| self.term(source, term, &mut local))
                        .collect::<Result<_, CoreError>>()?,
                    term: self.term(source, certificate.term, &mut local)?,
                    expected: self.term(source, certificate.expected, &mut local)?,
                })
            }
            fn term(
                &mut self,
                source: &TermArena,
                handle: TermHandle,
                local: &mut HashMap<TermHandle, Option<TermHandle>>,
            ) -> Result<TermHandle, CoreError> {
                enum Pending {
                    Enter(TermHandle),
                    Finish { original: TermHandle, node: Term },
                }
                let mut pending = vec![Pending::Enter(handle)];
                while let Some(work) = pending.pop() {
                    match work {
                        Pending::Enter(original) => {
                            if let Some(term) = local.get(&original) {
                                term.ok_or(CoreError::DummyTerm)?;
                                continue;
                            }
                            local.insert(original, None);
                            let node = source.get(original);
                            let slots = node.subterms();
                            pending.push(Pending::Finish { original, node });
                            for (child, _) in slots.iter().rev() {
                                pending.push(Pending::Enter(child));
                            }
                        }
                        Pending::Finish { original, node } => {
                            let slots = node.subterms();
                            let mut children = [original; MAX_SUBTERMS];
                            for (index, (child, _)) in slots.iter().enumerate() {
                                children[index] = local
                                    .get(&child)
                                    .copied()
                                    .flatten()
                                    .expect("each preceding child import completed");
                            }
                            let term = node.with_children(&children[..slots.len()]);
                            let term = *self
                                .by_syntax
                                .entry(term.clone())
                                .or_insert_with(|| self.arena.insert(term));
                            local.insert(original, Some(term));
                        }
                    }
                }
                Ok(local
                    .get(&handle)
                    .copied()
                    .flatten()
                    .expect("the finite root import completed"))
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::mathematical_core::Level;
            use crate::mathematical_core::{
                DEFAULT_CONVERSION_STEPS, Sort, verify_mathematical_certificate,
            };

            fn judgment(padding: bool) -> (TermArena, MathematicalCertificate) {
                let mut arena = TermArena::new();
                if padding {
                    arena.insert(Term::TwoOne);
                }
                let two = arena.insert(Term::Two);
                let variable = arena.insert(Term::Variable(0));
                let function = arena.insert(Term::Lambda {
                    domain: two,
                    body: variable,
                });
                let function_type = arena.insert(Term::Pi {
                    domain: two,
                    codomain: two,
                });
                let type_function = arena.insert(Term::Lambda {
                    domain: two,
                    body: function_type,
                });
                let zero = arena.insert(Term::TwoZero);
                let function_type = arena.insert(Term::Apply {
                    function: type_function,
                    argument: zero,
                });
                let constant = arena.insert(Term::Constant {
                    declaration: 0,
                    levels: Vec::new(),
                });
                let term = arena.insert(Term::Apply {
                    function: constant,
                    argument: zero,
                });
                let certificate = MathematicalCertificate::with_signature(
                    vec![Declaration::definition(0, function_type, function)],
                    Vec::new(),
                    term,
                    two,
                );
                (arena, certificate)
            }

            #[test]
            fn batch_shares_exact_signature_across_distinct_arenas_and_checks_each_judgment() {
                let (mut first, certificate) = judgment(false);
                let (second, other) = judgment(true);
                assert_ne!(certificate.signature, other.signature);
                let mut single = Budget::new(DEFAULT_CONVERSION_STEPS);
                verify_mathematical_certificate(&mut first, &certificate, &mut single).unwrap();
                let single_steps = DEFAULT_CONVERSION_STEPS - single.remaining();
                let mut batch = Budget::new(DEFAULT_CONVERSION_STEPS);
                verify_mathematical_certificates(
                    &[(&first, &certificate), (&second, &other)],
                    &mut batch,
                )
                .unwrap();
                let batch_steps = DEFAULT_CONVERSION_STEPS - batch.remaining();
                assert!(
                    batch_steps > single_steps,
                    "second judgment must still consume work"
                );
                assert!(
                    batch_steps < 2 * single_steps,
                    "identical checked signature must be shared"
                );
                verify_mathematical_certificates(
                    &[(&first, &certificate), (&second, &other)],
                    &mut Budget::new(batch_steps),
                )
                .unwrap();
                let exhausted = verify_mathematical_certificates(
                    &[(&first, &certificate), (&second, &other)],
                    &mut Budget::new(batch_steps - 1),
                )
                .unwrap_err();
                assert_eq!(exhausted.ordinal, 1);
                assert!(matches!(exhausted.error, CoreError::StepCeiling));
                let mut wrong = other.clone();
                let mut malformed = second.clone();
                wrong.term = malformed.insert(Term::Variable(0));
                let failure = verify_mathematical_certificates(
                    &[(&first, &certificate), (&malformed, &wrong)],
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .unwrap_err();
                assert_eq!(failure.ordinal, 1);
                assert!(matches!(failure.error, CoreError::UnboundVariable { .. }));
            }

            #[test]
            fn batch_rechecks_changed_signature_body_and_context_formation() {
                let (first, certificate) = judgment(false);
                let (mut second, mut changed) = judgment(true);
                changed.signature[0].body =
                    Some(second.insert(Term::Sort(Sort::Type(Level::Constant(0)))));
                let failure = verify_mathematical_certificates(
                    &[(&first, &certificate), (&second, &changed)],
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .unwrap_err();
                assert_eq!(failure.ordinal, 1);
                let (_, mut context) = judgment(true);
                context.context.push(second.insert(Term::TwoZero));
                let failure = verify_mathematical_certificates(
                    &[(&first, &certificate), (&second, &context)],
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .unwrap_err();
                assert_eq!(failure.ordinal, 1);
            }

            #[test]
            fn batch_import_preserves_indices_levels_and_raw_dependency_closure() {
                use crate::mathematical_core::{Level, certificate_assumption_closure};
                let (mut arena, mut certificate) = judgment(false);
                let two = arena.insert(Term::Two);
                let constant = arena.insert(Term::Constant {
                    declaration: 1,
                    levels: Vec::new(),
                });
                certificate.signature.push(Declaration::assumption(0, two));
                certificate.context = vec![two];
                certificate.term = arena.insert(Term::Variable(0));
                certificate.expected = two;
                let outer = arena.insert(Term::Variable(1));
                let ignored = arena.insert(Term::Lambda {
                    domain: two,
                    body: outer,
                });
                let raw = arena.insert(Term::Apply {
                    function: ignored,
                    argument: constant,
                });
                certificate.term = raw;
                let before = certificate_assumption_closure(&arena, &certificate);
                let mut imported = ImportedTerms::default();
                let copy = imported.certificate(&arena, &certificate).unwrap();
                assert_eq!(
                    before,
                    certificate_assumption_closure(&imported.arena, &copy)
                );
                verify_mathematical_certificates(
                    &[(&arena, &certificate)],
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .unwrap();
                let sort = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
                let mut polymorphic = certificate.clone();
                polymorphic.level_arity = 1;
                polymorphic.signature.push(Declaration::assumption(1, sort));
                polymorphic.context = vec![sort];
                polymorphic.term = arena.insert(Term::Variable(0));
                polymorphic.expected = sort;
                verify_mathematical_certificates(
                    &[(&arena, &polymorphic)],
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .unwrap();
                let mut wrong_scope = polymorphic.clone();
                wrong_scope.signature.last_mut().unwrap().level_arity = 0;
                let failure = verify_mathematical_certificates(
                    &[(&arena, &polymorphic), (&arena, &wrong_scope)],
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .unwrap_err();
                assert_eq!(failure.ordinal, 1);
                assert!(matches!(
                    failure.error,
                    CoreError::UnboundLevelParameter { .. }
                ));
            }
            #[test]
            fn deep_certificate_import_preserves_judgment_on_default_stack() {
                std::thread::spawn(|| {
            let mut source = TermArena::new();
            let two = source.insert(Term::Two);
            let mut term = source.insert(Term::TwoZero);
            let mut expected = two;
            for _ in 0..8192 {
                term = source.insert(Term::Lambda {
                    domain: two,
                    body: term,
                });
                expected = source.insert(Term::Pi {
                    domain: two,
                    codomain: expected,
                });
            }
            let certificate = MathematicalCertificate::closed(Vec::new(), term, expected);
            let inferred = crate::mathematical_core::infer_type(
                &mut source,
                &Context::empty(),
                term,
                &mut Budget::new(0),
            )
            .expect("the source term independently has its complete type");
            assert!(source.structurally_equal(inferred, expected));
            eprintln!(
                "independent source inference admitted the complete 8192-binder type; begin import"
            );
            let mut imported = ImportedTerms::default();
            let copied = imported
                .certificate(&source, &certificate)
                .expect("all finite source terms import");
            let inferred = crate::mathematical_core::infer_type(
                &mut imported.arena,
                &Context::empty(),
                copied.term,
                &mut Budget::new(0),
            )
            .expect("the imported term has its complete type");
            assert!(imported.arena.structurally_equal(inferred, copied.expected));
                    })
                    .join()
                    .expect("term import uses default-stack handle worklists");
            }
            #[test]
            fn public_mathematical_verification_keeps_its_callers_default_stack() {
                std::thread::spawn(|| {
                    let mut arena = TermArena::new();
                    let two = arena.insert(Term::Two);
                    let zero = arena.insert(Term::TwoZero);
                    let mut term = zero;
                    let mut expected = two;
                    for _ in 0..512 {
                        term = arena.insert(Term::Lambda {
                            domain: two,
                            body: term,
                        });
                        expected = arena.insert(Term::Pi {
                            domain: two,
                            codomain: expected,
                        });
                    }
                    let certificate = MathematicalCertificate::closed(Vec::new(), term, expected);
                    let before = crate::mathematical_core::bounded_verification_entries();
                    verify_mathematical_certificate(&mut arena, &certificate, &mut Budget::new(0))
                        .expect("single verification uses complete handle judgments");
                    let source_nodes = arena.len();
                    verify_mathematical_certificates(
                        &[(&arena, &certificate)],
                        &mut Budget::new(0),
                    )
                    .expect("batch verification imports and checks the same complete judgment");
                    assert_eq!(
                        arena.len(),
                        source_nodes,
                        "batch import owns its destination arena"
                    );
                    let invalid = MathematicalCertificate {
                        context: vec![zero],
                        ..certificate
                    };
                    assert!(matches!(
                        verify_mathematical_certificate(&mut arena, &invalid, &mut Budget::new(0)),
                        Err(CoreError::NotASort { .. })
                    ));
                    assert_eq!(
                        crate::mathematical_core::bounded_verification_entries(),
                        before,
                        "raw mathematical entries do not re-enter bounded admission or denotation"
                    );
                })
                .join()
                .expect("public mathematical verification stays on the caller's default stack");
            }
        }
    }
    pub use batch::{MathematicalCertificateBatchError, verify_mathematical_certificates};

    use std::collections::BTreeSet;

    use super::conversion::Budget;
    use super::signature::{Declaration, Signature, check_signature, judgment_assumption_closure};
    use super::term::{TermArena, TermHandle};
    use super::typing::{Context, CoreError, check_type, infer_sort};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MathematicalCertificate {
        pub signature: Vec<Declaration>,
        pub level_arity: u32,
        pub context: Vec<TermHandle>,
        pub term: TermHandle,
        pub expected: TermHandle,
    }

    impl MathematicalCertificate {
        pub fn closed(context: Vec<TermHandle>, term: TermHandle, expected: TermHandle) -> Self {
            Self {
                signature: Vec::new(),
                level_arity: 0,
                context,
                term,
                expected,
            }
        }

        pub fn with_signature(
            signature: Vec<Declaration>,
            context: Vec<TermHandle>,
            term: TermHandle,
            expected: TermHandle,
        ) -> Self {
            Self {
                signature,
                level_arity: 0,
                context,
                term,
                expected,
            }
        }
    }

    pub fn verify_mathematical_certificate(
        arena: &mut TermArena,
        certificate: &MathematicalCertificate,
        budget: &mut Budget,
    ) -> Result<(), CoreError> {
        let signature = check_signature(arena, &certificate.signature, budget)?;
        let mut context =
            Context::with_level_arity(certificate.level_arity).with_signature(signature);
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

    pub fn certificate_assumption_closure(
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
            CoreError, Declaration, MathematicalCertificate, TermArena, TermHandle,
            certificate_assumption_closure, verify_mathematical_certificate,
        };
        use crate::mathematical_core::tests::{
            default_budget, lambda, pi, polymorphic_identity_declaration, strict_sort, type_sort,
            variable,
        };
        use crate::mathematical_core::{Level, Sort, Term};

        macro_rules! term {
            (Apply => $function:expr => $argument:expr) => {
                Term::Apply {
                    function: $function,
                    argument: $argument,
                }
            };
            (Constant => $declaration:expr => $levels:expr) => {
                Term::Constant {
                    declaration: $declaration,
                    levels: $levels,
                }
            };
        }

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
            let certificate = MathematicalCertificate::closed(Vec::new(), identity, expected);
            verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget())
                .unwrap();
        }

        #[test]
        fn the_context_is_part_of_the_checked_judgment() {
            let mut arena = TermArena::new();
            let type_zero = type_sort(&mut arena, 0);
            let bound = variable(&mut arena, 0);
            let certificate = MathematicalCertificate::closed(vec![type_zero], bound, type_zero);
            verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget())
                .unwrap();

            let mut arena = TermArena::new();
            let type_zero = type_sort(&mut arena, 0);
            let bound = variable(&mut arena, 0);
            let certificate = MathematicalCertificate::closed(Vec::new(), bound, type_zero);
            assert_eq!(
                verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget()),
                Err(CoreError::UnboundVariable {
                    index: 0,
                    context_depth: 0,
                })
            );
        }

        #[test]
        fn dependent_context_bindings_check_formation_under_their_prefix() {
            let mut arena = TermArena::new();
            let type_zero = type_sort(&mut arena, 0);
            let bound = variable(&mut arena, 0);
            let shifted = variable(&mut arena, 1);
            let certificate =
                MathematicalCertificate::closed(vec![type_zero, bound], bound, shifted);
            verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget())
                .unwrap();

            let mut arena = TermArena::new();
            let type_zero = type_sort(&mut arena, 0);
            let bound = variable(&mut arena, 0);
            let not_a_type = lambda(&mut arena, type_zero, bound);
            let certificate = MathematicalCertificate::closed(vec![not_a_type], bound, type_zero);
            assert!(matches!(
                verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget()),
                Err(CoreError::NotASort { .. })
            ));
        }

        #[test]
        fn a_tampered_term_or_claimed_type_rejects() {
            let mut arena = TermArena::new();
            let (identity, expected) = polymorphic_identity(&mut arena);
            let type_zero = type_sort(&mut arena, 0);

            let bound = variable(&mut arena, 0);
            let wrong_inner = pi(&mut arena, bound, type_zero);
            let wrong_expected = pi(&mut arena, type_zero, wrong_inner);
            let certificate = MathematicalCertificate::closed(Vec::new(), identity, wrong_expected);
            assert!(matches!(
                verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget()),
                Err(CoreError::TypeMismatch { .. })
            ));

            let free_variable = variable(&mut arena, 0);
            let certificate = MathematicalCertificate::closed(Vec::new(), free_variable, expected);
            assert!(matches!(
                verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget()),
                Err(CoreError::UnboundVariable { .. })
            ));
        }

        #[test]
        fn a_certificate_carries_dependent_two_elimination() {
            let mut arena = TermArena::new();
            let type_zero = type_sort(&mut arena, 0);
            let a_binding = variable(&mut arena, 1);
            let b_binding = variable(&mut arena, 1);
            let two_binding = arena.insert(Term::Two);
            let motive_domain = arena.insert(Term::Two);
            let motive_body = type_sort(&mut arena, 0);
            let motive = lambda(&mut arena, motive_domain, motive_body);
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
            let a_term = variable(&mut arena, 2);
            let b_term = variable(&mut arena, 1);
            let scrutinee = variable(&mut arena, 0);
            let term = arena.insert(Term::CaseTwo {
                motive: family,
                zero_branch: a_term,
                one_branch: b_term,
                scrutinee,
            });
            let expected = arena.insert(term!(Apply => family => scrutinee));
            let certificate = MathematicalCertificate::closed(
                vec![type_zero, type_zero, a_binding, b_binding, two_binding],
                term,
                expected,
            );
            verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget())
                .unwrap();

            let zero = arena.insert(Term::TwoZero);
            let wrong_expected = arena.insert(term!(Apply => family => zero));
            let certificate = MathematicalCertificate::closed(
                vec![type_zero, type_zero, a_binding, b_binding, two_binding],
                term,
                wrong_expected,
            );
            assert!(matches!(
                verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget()),
                Err(CoreError::TypeMismatch { .. })
            ));
        }

        #[test]
        fn a_certificate_carries_identity_elimination() {
            let mut arena = TermArena::new();
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
                arena.insert(term!(Apply => predicate => x))
            };
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
                        arena.insert(term!(Apply => predicate => x))
                    };
                    let codomain = {
                        let predicate = variable(&mut arena, 6);
                        let bound_y = variable(&mut arena, 2);
                        arena.insert(term!(Apply => predicate => bound_y))
                    };
                    pi(&mut arena, domain, codomain)
                };
                let inner = lambda(&mut arena, inner_domain, body);
                lambda(&mut arena, domain, inner)
            };
            let base = {
                let domain = {
                    let predicate = variable(&mut arena, 3);
                    let x = variable(&mut arena, 4);
                    arena.insert(term!(Apply => predicate => x))
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
            let term = arena.insert(term!(Apply => elimination => hypothesis));
            let expected = {
                let predicate = variable(&mut arena, 3);
                let y = variable(&mut arena, 2);
                arena.insert(term!(Apply => predicate => y))
            };
            let certificate = MathematicalCertificate::closed(
                vec![
                    type_zero,
                    x_binding,
                    predicate_binding,
                    y_binding,
                    proof_binding,
                    hypothesis_binding,
                ],
                term,
                expected,
            );
            verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget())
                .unwrap();

            let wrong_expected = {
                let predicate = variable(&mut arena, 3);
                let x = variable(&mut arena, 4);
                arena.insert(term!(Apply => predicate => x))
            };
            let certificate = MathematicalCertificate::closed(
                vec![
                    type_zero,
                    x_binding,
                    predicate_binding,
                    y_binding,
                    proof_binding,
                    hypothesis_binding,
                ],
                term,
                wrong_expected,
            );
            assert!(matches!(
                verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget()),
                Err(CoreError::TypeMismatch { .. })
            ));
        }

        #[test]
        fn a_certificate_carries_w_induction() {
            let mut arena = TermArena::new();
            let type_zero = type_sort(&mut arena, 0);
            let b_binding = {
                let a_in_prefix = variable(&mut arena, 0);
                pi(&mut arena, a_in_prefix, type_zero)
            };
            let a_binding = variable(&mut arena, 1);
            let k_binding = {
                let b_at = variable(&mut arena, 1);
                let a_at = variable(&mut arena, 0);
                let child_positions = arena.insert(term!(Apply => b_at => a_at));
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
            let s_binding = {
                let a_domain = variable(&mut arena, 4);
                let function_type = {
                    let b_at = variable(&mut arena, 4);
                    let a_bound = variable(&mut arena, 0);
                    let child_positions = arena.insert(term!(Apply => b_at => a_bound));
                    let carrier = variable(&mut arena, 6);
                    let children = variable(&mut arena, 5);
                    let w_at_two = arena.insert(Term::W { carrier, children });
                    pi(&mut arena, child_positions, w_at_two)
                };
                let hypothesis = {
                    let b_at = variable(&mut arena, 5);
                    let a_bound = variable(&mut arena, 1);
                    let hypothesis_domain = arena.insert(term!(Apply => b_at => a_bound));
                    let p_at = variable(&mut arena, 3);
                    let k_bound = variable(&mut arena, 1);
                    let b_bound = variable(&mut arena, 0);
                    let child = arena.insert(term!(Apply => k_bound => b_bound));
                    let hypothesis_codomain = arena.insert(term!(Apply => p_at => child));
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
                    arena.insert(term!(Apply => p_at => node))
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
                arena.insert(term!(Apply => p => node))
            };
            let certificate = MathematicalCertificate::closed(
                vec![
                    type_zero, b_binding, a_binding, k_binding, p_binding, s_binding, t_binding,
                ],
                term,
                expected,
            );
            verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget())
                .unwrap();

            let wrong_expected = {
                let p = variable(&mut arena, 2);
                let t = variable(&mut arena, 0);
                arena.insert(term!(Apply => p => t))
            };
            let certificate = MathematicalCertificate::closed(
                vec![
                    type_zero, b_binding, a_binding, k_binding, p_binding, s_binding, t_binding,
                ],
                term,
                wrong_expected,
            );
            assert!(matches!(
                verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget()),
                Err(CoreError::TypeMismatch { .. })
            ));
        }

        #[test]
        fn strict_sorts_flow_through_the_certificate() {
            let mut arena = TermArena::new();
            let strict_zero = strict_sort(&mut arena, 0);
            let bound = variable(&mut arena, 0);
            let shifted = variable(&mut arena, 1);
            let certificate =
                MathematicalCertificate::closed(vec![strict_zero, bound], bound, shifted);
            verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget())
                .unwrap();

            let mut arena = TermArena::new();
            let type_zero = type_sort(&mut arena, 0);
            let bound = variable(&mut arena, 0);
            let identity_body = lambda(&mut arena, type_zero, bound);
            let certificate = MathematicalCertificate::closed(Vec::new(), bound, identity_body);
            assert!(matches!(
                verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget()),
                Err(CoreError::NotASort { .. })
            ));
        }

        #[test]
        fn a_certificate_is_parametric_over_its_level_arity() {
            let mut arena = TermArena::new();
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
            verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget())
                .unwrap();

            let certificate = MathematicalCertificate::closed(Vec::new(), identity, expected);
            assert_eq!(
                verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget()),
                Err(CoreError::UnboundLevelParameter { index: 0, arity: 0 })
            );

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
                verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget()),
                Err(CoreError::UnboundLevelParameter { index: 1, arity: 1 })
            );
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
            let evidence = arena.insert(term!(Constant => 0 => vec![Level::Constant(0)]));
            let expected = instantiated_identity_type(&mut arena, Level::Constant(0));
            let certificate = MathematicalCertificate::with_signature(
                vec![declaration],
                Vec::new(),
                evidence,
                expected,
            );
            verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget())
                .unwrap();
        }

        #[test]
        fn a_forged_declaration_index_never_verifies() {
            let mut arena = TermArena::new();
            let declaration = polymorphic_identity_declaration(&mut arena);
            let forged = arena.insert(term!(Constant => 1 => vec![Level::Constant(0)]));
            let expected = instantiated_identity_type(&mut arena, Level::Constant(0));
            let certificate = MathematicalCertificate::with_signature(
                vec![declaration],
                Vec::new(),
                forged,
                expected,
            );
            assert_eq!(
                verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget()),
                Err(CoreError::UnknownDeclaration {
                    declaration: 1,
                    signature_len: 1,
                })
            );
        }

        #[test]
        fn an_unchecked_signature_entry_never_verifies() {
            let mut arena = TermArena::new();
            let type_zero = type_sort(&mut arena, 0);
            let two = arena.insert(Term::Two);
            let sound = Declaration::definition(0, type_zero, two);
            let two = arena.insert(Term::Two);
            let type_zero = type_sort(&mut arena, 0);
            let unsound = Declaration::definition(0, two, type_zero);
            let evidence = arena.insert(term!(Constant => 1 => Vec::new()));
            let two = arena.insert(Term::Two);
            let certificate = MathematicalCertificate::with_signature(
                vec![sound, unsound],
                Vec::new(),
                evidence,
                two,
            );
            assert!(matches!(
                verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget()),
                Err(CoreError::TypeMismatch { .. })
            ));
        }

        #[test]
        fn the_certificate_records_its_exact_assumption_closure() {
            let mut arena = TermArena::new();
            let type_zero = type_sort(&mut arena, 0);
            let axiom = Declaration::assumption(0, type_zero);
            let axiom_constant = arena.insert(term!(Constant => 0 => Vec::new()));
            let type_zero = type_sort(&mut arena, 0);
            let uses = Declaration::definition(0, type_zero, axiom_constant);
            let evidence = arena.insert(term!(Constant => 1 => Vec::new()));
            let type_zero = type_sort(&mut arena, 0);
            let certificate = MathematicalCertificate::with_signature(
                vec![axiom, uses],
                Vec::new(),
                evidence,
                type_zero,
            );
            verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget())
                .unwrap();
            assert_eq!(
                certificate_assumption_closure(&arena, &certificate),
                [0].into_iter().collect()
            );
        }
    }
}
pub(crate) mod conversion {
    use super::signature::Signature;
    use super::substitution::{instantiate_levels, shift, substitute};
    use super::term::{Term, TermArena, TermHandle};
    use super::typing::{Context, CoreError};

    macro_rules! row {
        (Apply => $function:expr => $argument:expr) => {
            Term::Apply {
                function: $function,
                argument: $argument,
            }
        };
    }

    pub const DEFAULT_CONVERSION_STEPS: u32 = 65_536;

    pub struct Budget {
        remaining_steps: u32,
    }

    impl Budget {
        pub fn new(remaining_steps: u32) -> Self {
            Self { remaining_steps }
        }

        pub fn remaining(&self) -> u32 {
            self.remaining_steps
        }

        fn consume(&mut self) -> Result<(), CoreError> {
            self.remaining_steps = self
                .remaining_steps
                .checked_sub(1)
                .ok_or(CoreError::StepCeiling)?;
            Ok(())
        }
    }

    impl Default for Budget {
        fn default() -> Self {
            Self::new(DEFAULT_CONVERSION_STEPS)
        }
    }

    pub fn weak_head_normalize(
        arena: &mut TermArena,
        signature: &Signature,
        term: TermHandle,
        budget: &mut Budget,
    ) -> Result<TermHandle, CoreError> {
        weak_head_normalize_retaining(arena, signature, term, budget, &|_| false)
    }

    pub fn weak_head_normalize_retaining(
        arena: &mut TermArena,
        signature: &Signature,
        term: TermHandle,
        budget: &mut Budget,
        retained: &(impl Fn(u32) -> bool + ?Sized),
    ) -> Result<TermHandle, CoreError> {
        let mut current = term;
        let mut frames = Vec::new();
        'normalize: loop {
            loop {
                match arena.get(current) {
                    Term::Constant {
                        declaration,
                        levels,
                    } => {
                        let index = declaration;
                        let Some(declaration) = signature.get(index) else {
                            return Err(CoreError::UnknownDeclaration {
                                declaration: index,
                                signature_len: signature.len(),
                            });
                        };
                        if retained(index) {
                            break;
                        }
                        let Some(body) = declaration.body else {
                            break;
                        };
                        budget.consume()?;
                        current = instantiate_levels(arena, body, &levels).map_err(|index| {
                            CoreError::UnboundLevelParameter {
                                index,
                                arity: declaration.level_arity,
                            }
                        })?;
                    }
                    _ => match weak_head_focus(arena.node(current)) {
                        Some((child, slot)) => {
                            frames.push((current, slot));
                            current = child;
                        }
                        None => break,
                    },
                }
            }
            while let Some((parent, slot)) = frames.pop() {
                match (arena.get(parent), arena.get(current)) {
                    (Term::Apply { argument, .. }, Term::Lambda { body, .. }) => {
                        budget.consume()?;
                        current = substitute(arena, body, argument);
                        continue 'normalize;
                    }
                    (Term::Fst { .. }, Term::Pair { first, .. }) => {
                        budget.consume()?;
                        current = first;
                        continue 'normalize;
                    }
                    (Term::Snd { .. }, Term::Pair { second, .. }) => {
                        budget.consume()?;
                        current = second;
                        continue 'normalize;
                    }
                    (Term::CaseTwo { zero_branch, .. }, Term::TwoZero) => {
                        budget.consume()?;
                        current = zero_branch;
                        continue 'normalize;
                    }
                    (Term::CaseTwo { one_branch, .. }, Term::TwoOne) => {
                        budget.consume()?;
                        current = one_branch;
                        continue 'normalize;
                    }
                    (Term::IdElim { base, .. }, Term::Refl { .. }) => {
                        budget.consume()?;
                        current = base;
                        continue 'normalize;
                    }
                    (
                        Term::IndW { motive, step, .. },
                        Term::Sup {
                            children,
                            label,
                            function,
                            ..
                        },
                    ) => {
                        budget.consume()?;
                        let bound_domain = arena.insert(row!(Apply => children => label));
                        let shifted_motive = shift(arena, motive, 0, 1);
                        let shifted_step = shift(arena, step, 0, 1);
                        let shifted_function = shift(arena, function, 0, 1);
                        let bound = arena.insert(Term::Variable(0));
                        let child = arena.insert(row!(Apply => shifted_function => bound));
                        let body = arena.insert(Term::IndW {
                            motive: shifted_motive,
                            step: shifted_step,
                            tree: child,
                        });
                        let hypothesis = arena.insert(Term::Lambda {
                            domain: bound_domain,
                            body,
                        });
                        let applied = arena.insert(row!(Apply => step => label));
                        let applied = arena.insert(row!(Apply => applied => function));
                        current = arena.insert(row!(Apply => applied => hypothesis));
                        continue 'normalize;
                    }
                    (Term::SquashElim { function, .. }, Term::SquashIntro { value, .. }) => {
                        budget.consume()?;
                        current = arena.insert(row!(Apply => function => value));
                        continue 'normalize;
                    }
                    (Term::BoxElim { body, .. }, Term::BoxIntro { value, .. }) => {
                        budget.consume()?;
                        current = arena.insert(row!(Apply => body => value));
                        continue 'normalize;
                    }
                    _ => {}
                }
                current = replace_head_focus(arena, parent, slot, current);
            }
            return Ok(current);
        }
    }

    fn weak_head_focus(node: &Term) -> Option<(TermHandle, usize)> {
        match node {
            Term::Apply { function, .. } => Some((*function, 0)),
            Term::Fst { pair } | Term::Snd { pair } => Some((*pair, 0)),
            Term::CaseTwo { scrutinee, .. } => Some((*scrutinee, 3)),
            Term::IdElim { proof, .. } => Some((*proof, 3)),
            Term::IndW { tree, .. } => Some((*tree, 2)),
            Term::EmptyElim { scrutinee, .. } => Some((*scrutinee, 1)),
            Term::SquashElim { scrutinee, .. } | Term::BoxElim { scrutinee, .. } => {
                Some((*scrutinee, 2))
            }
            _ => None,
        }
    }

    fn replace_head_focus(
        arena: &mut TermArena,
        parent: TermHandle,
        slot: usize,
        head: TermHandle,
    ) -> TermHandle {
        let node = arena.node(parent);
        let slots = node.subterms();
        let mut children = [parent; super::term::MAX_SUBTERMS];
        for (index, (child, _)) in slots.iter().enumerate() {
            children[index] = child;
        }
        if children[slot] == head {
            return parent;
        }
        children[slot] = head;
        let node = node.with_children(&children[..slots.len()]);
        arena.insert(node)
    }

    pub fn convertible(
        arena: &mut TermArena,
        context: &Context,
        left: TermHandle,
        right: TermHandle,
        shared_type: TermHandle,
        budget: &mut Budget,
    ) -> Result<bool, CoreError> {
        convertible_retaining(arena, context, left, right, shared_type, budget, &|_| false)
    }

    pub fn convertible_retaining(
        arena: &mut TermArena,
        context: &Context,
        left: TermHandle,
        right: TermHandle,
        shared_type: TermHandle,
        budget: &mut Budget,
        retained: &impl Fn(u32) -> bool,
    ) -> Result<bool, CoreError> {
        super::typing::convert_retaining(arena, context, left, right, shared_type, budget, retained)
    }
}
pub(crate) mod indexed {
    use super::scheme_dsl::{
        Syntax, app, apps, define, fst, id, indw, lam, pair, pi, refl, scheme_at, sigma, snd, sort,
        sup, ty, v, w,
    };
    use super::signature::Declaration;
    use super::term::{Level, Term, TermArena, TermHandle};

    macro_rules! pi {
        ($name:expr => $domain:expr => $codomain:expr) => {
            pi($name, $domain, $codomain)
        };
    }

    pub const INDEXED_AT: u32 = 0;
    pub const INDEXED_W: u32 = 1;
    pub const INDEXED_PACK: u32 = 2;
    pub const INDEXED_SUP: u32 = 3;
    pub const INDEXED_IND: u32 = 4;

    fn scheme(declaration: u32) -> Syntax {
        scheme_at(
            declaration,
            vec![
                Level::Parameter(0),
                Level::Parameter(1),
                Level::Parameter(2),
            ],
        )
    }

    fn description_telescope(
        connective: fn(&'static str, Syntax, Syntax) -> Syntax,
        rest: Syntax,
    ) -> Syntax {
        connective(
            "I",
            ty(0),
            connective(
                "A",
                ty(1),
                connective(
                    "B",
                    pi("_", v("A"), ty(2)),
                    connective(
                        "out",
                        pi("_", v("A"), v("I")),
                        connective(
                            "next",
                            pi("a", v("A"), pi("_", app(v("B"), v("a")), v("I"))),
                            rest,
                        ),
                    ),
                ),
            ),
        )
    }

    fn pi_description(rest: Syntax) -> Syntax {
        description_telescope(pi, rest)
    }

    fn lam_description(rest: Syntax) -> Syntax {
        description_telescope(lam, rest)
    }

    fn at(index: Syntax, tree: Syntax) -> Syntax {
        apps(
            scheme(INDEXED_AT),
            [v("I"), v("A"), v("B"), v("out"), v("next"), index, tree],
        )
    }

    fn iw(index: Syntax) -> Syntax {
        apps(
            scheme(INDEXED_W),
            [v("I"), v("A"), v("B"), v("out"), v("next"), index],
        )
    }

    fn family_level() -> Level {
        Level::Maximum(
            Box::new(Level::Maximum(
                Box::new(Level::Parameter(0)),
                Box::new(Level::Parameter(1)),
            )),
            Box::new(Level::Parameter(2)),
        )
    }

    fn indexed_at_declaration(arena: &mut TermArena) -> Declaration {
        let level = family_level();
        let statement = pi_description(pi(
            "i",
            v("I"),
            pi("t", w(v("A"), v("B")), sort(level.clone())),
        ));
        let motive = lam("_", w(v("A"), v("B")), pi("i2", v("I"), sort(level)));
        let step = lam(
            "a",
            v("A"),
            lam(
                "k",
                pi("b", app(v("B"), v("a")), w(v("A"), v("B"))),
                lam(
                    "ih",
                    pi!("b" => app(v("B"), v("a")) => app(motive.clone(), app(v("k"), v("b")))),
                    lam(
                        "i2",
                        v("I"),
                        sigma(
                            "_",
                            id(v("I"), app(v("out"), v("a")), v("i2")),
                            pi(
                                "b",
                                app(v("B"), v("a")),
                                app(app(v("ih"), v("b")), app(app(v("next"), v("a")), v("b"))),
                            ),
                        ),
                    ),
                ),
            ),
        );
        let body = lam_description(lam(
            "i",
            v("I"),
            lam(
                "t",
                w(v("A"), v("B")),
                app(indw(motive, step, v("t")), v("i")),
            ),
        ));
        define(arena, 3, &statement, &body)
    }

    fn indexed_w_declaration(arena: &mut TermArena) -> Declaration {
        let statement = pi_description(pi("i", v("I"), sort(family_level())));
        let body = lam_description(lam(
            "i",
            v("I"),
            sigma("t", w(v("A"), v("B")), at(v("i"), v("t"))),
        ));
        define(arena, 3, &statement, &body)
    }

    fn indexed_pack_declaration(arena: &mut TermArena) -> Declaration {
        let statement = pi_description(pi(
            "i",
            v("I"),
            pi!("t" => w(v("A"), v("B")) => pi("e", at(v("i"), v("t")), iw(v("i")))),
        ));
        let body = lam_description(lam(
            "i",
            v("I"),
            lam(
                "t",
                w(v("A"), v("B")),
                lam(
                    "e",
                    at(v("i"), v("t")),
                    app(lam("p", iw(v("i")), v("p")), pair(v("t"), v("e"))),
                ),
            ),
        ));
        define(arena, 3, &statement, &body)
    }

    fn indexed_sup_declaration(arena: &mut TermArena) -> Declaration {
        let children_type = pi(
            "b",
            app(v("B"), v("a")),
            iw(app(app(v("next"), v("a")), v("b"))),
        );
        let statement = pi_description(pi(
            "a",
            v("A"),
            pi("g", children_type.clone(), iw(app(v("out"), v("a")))),
        ));
        let body = lam_description(lam(
            "a",
            v("A"),
            lam(
                "g",
                children_type,
                app(
                    lam("p", iw(app(v("out"), v("a"))), v("p")),
                    pair(
                        sup(
                            v("A"),
                            v("B"),
                            v("a"),
                            lam("b", app(v("B"), v("a")), fst(app(v("g"), v("b")))),
                        ),
                        pair(
                            refl(v("I"), app(v("out"), v("a"))),
                            lam("b", app(v("B"), v("a")), snd(app(v("g"), v("b")))),
                        ),
                    ),
                ),
            ),
        ));
        define(arena, 3, &statement, &body)
    }

    fn indexed_ind_declaration(arena: &mut TermArena) -> Declaration {
        let motive_level = Level::Parameter(3);
        let motive_type = pi("i", v("I"), pi("_", iw(v("i")), sort(motive_level.clone())));
        let step_type = pi(
            "a",
            v("A"),
            pi(
                "g",
                pi!("b" => app(v("B"), v("a")) => iw(app(app(v("next"), v("a")), v("b")))),
                pi(
                    "_",
                    pi(
                        "b",
                        app(v("B"), v("a")),
                        app(
                            app(v("Q"), app(app(v("next"), v("a")), v("b"))),
                            app(v("g"), v("b")),
                        ),
                    ),
                    app(
                        app(v("Q"), app(v("out"), v("a"))),
                        apps(
                            scheme(INDEXED_SUP),
                            [v("I"), v("A"), v("B"), v("out"), v("next"), v("a"), v("g")],
                        ),
                    ),
                ),
            ),
        );
        let statement = pi_description(pi(
            "Q",
            motive_type.clone(),
            pi(
                "s",
                step_type.clone(),
                pi!("i" => v("I") => pi("t", iw(v("i")), app(app(v("Q"), v("i")), v("t")))),
            ),
        ));

        let inner_motive = lam(
            "u",
            w(v("A"), v("B")),
            pi(
                "i2",
                v("I"),
                pi!("e2" => at(v("i2"), v("u")) => app(app(v("Q"), v("i2")), pair(v("u"), v("e2")))),
            ),
        );
        let packed_children = lam(
            "b",
            app(v("B"), v("a")),
            apps(
                scheme(INDEXED_PACK),
                [
                    v("I"),
                    v("A"),
                    v("B"),
                    v("out"),
                    v("next"),
                    app(app(v("next"), v("a")), v("b")),
                    app(v("k"), v("b")),
                    app(snd(v("e2")), v("b")),
                ],
            ),
        );
        let hypotheses = lam(
            "b",
            app(v("B"), v("a")),
            app(
                app(app(v("ih"), v("b")), app(app(v("next"), v("a")), v("b"))),
                app(snd(v("e2")), v("b")),
            ),
        );
        let base = app(app(app(v("s"), v("a")), packed_children), hypotheses);
        let transport_motive = lam(
            "y",
            v("I"),
            lam(
                "p",
                id(v("I"), app(v("out"), v("a")), v("y")),
                app(
                    app(v("Q"), v("y")),
                    pair(
                        sup(v("A"), v("B"), v("a"), v("k")),
                        pair(v("p"), snd(v("e2"))),
                    ),
                ),
            ),
        );
        let step = lam(
            "a",
            v("A"),
            lam(
                "k",
                pi("b", app(v("B"), v("a")), w(v("A"), v("B"))),
                lam(
                    "ih",
                    pi!("b" => app(v("B"), v("a")) => app(inner_motive.clone(), app(v("k"), v("b")))),
                    lam(
                        "i2",
                        v("I"),
                        lam(
                            "e2",
                            at(v("i2"), sup(v("A"), v("B"), v("a"), v("k"))),
                            Syntax::IdElim(
                                Box::new(transport_motive),
                                Box::new(base),
                                Box::new(v("i2")),
                                Box::new(fst(v("e2"))),
                            ),
                        ),
                    ),
                ),
            ),
        );
        let body = lam_description(lam(
            "Q",
            motive_type,
            lam(
                "s",
                step_type,
                lam(
                    "i",
                    v("I"),
                    lam(
                        "t",
                        iw(v("i")),
                        app(
                            app(indw(inner_motive, step, fst(v("t"))), v("i")),
                            snd(v("t")),
                        ),
                    ),
                ),
            ),
        ));
        define(arena, 4, &statement, &body)
    }

    fn indexed_correctness_declaration(arena: &mut TermArena) -> Declaration {
        let root_step = lam(
            "a",
            v("A"),
            lam(
                "k",
                pi("b", app(v("B"), v("a")), w(v("A"), v("B"))),
                lam("_", pi("_", app(v("B"), v("a")), v("A")), v("a")),
            ),
        );
        let root_of = |tree: Syntax| {
            app(
                v("out"),
                indw(
                    lam("_", w(v("A"), v("B")), v("A")),
                    root_step.clone(),
                    fst(tree),
                ),
            )
        };
        let motive = lam(
            "i2",
            v("I"),
            lam("t2", iw(v("i2")), id(v("I"), root_of(v("t2")), v("i2"))),
        );
        let step = lam(
            "a",
            v("A"),
            lam(
                "g",
                pi!("b" => app(v("B"), v("a")) => iw(app(app(v("next"), v("a")), v("b")))),
                lam(
                    "ih",
                    pi(
                        "b",
                        app(v("B"), v("a")),
                        app(
                            app(motive.clone(), app(app(v("next"), v("a")), v("b"))),
                            app(v("g"), v("b")),
                        ),
                    ),
                    refl(v("I"), app(v("out"), v("a"))),
                ),
            ),
        );
        let statement = pi_description(pi(
            "i",
            v("I"),
            pi("t", iw(v("i")), id(v("I"), root_of(v("t")), v("i"))),
        ));
        let body = lam_description(lam(
            "i",
            v("I"),
            lam(
                "t",
                iw(v("i")),
                apps(
                    scheme_at(
                        INDEXED_IND,
                        vec![
                            Level::Parameter(0),
                            Level::Parameter(1),
                            Level::Parameter(2),
                            Level::Parameter(0),
                        ],
                    ),
                    [
                        v("I"),
                        v("A"),
                        v("B"),
                        v("out"),
                        v("next"),
                        motive,
                        step,
                        v("i"),
                        v("t"),
                    ],
                ),
            ),
        ));
        define(arena, 3, &statement, &body)
    }

    pub fn indexed_correctness(arena: &mut TermArena) -> Declaration {
        indexed_correctness_declaration(arena)
    }

    pub fn indexed_scheme(arena: &mut TermArena) -> Vec<Declaration> {
        vec![
            indexed_at_declaration(arena),
            indexed_w_declaration(arena),
            indexed_pack_declaration(arena),
            indexed_sup_declaration(arena),
            indexed_ind_declaration(arena),
        ]
    }

    #[derive(Clone, Debug)]
    pub struct IndexedFamily {
        pub levels: [Level; 3],
        pub index: TermHandle,
        pub carrier: TermHandle,
        pub children: TermHandle,
        pub out: TermHandle,
        pub next: TermHandle,
    }

    impl IndexedFamily {
        fn spine(
            &self,
            arena: &mut TermArena,
            declaration: u32,
            levels: Vec<Level>,
            arguments: &[TermHandle],
        ) -> TermHandle {
            let constant = arena.insert(Term::Constant {
                declaration,
                levels,
            });
            let instantiated = arena.apply_arguments(
                constant,
                &[self.index, self.carrier, self.children, self.out, self.next],
            );
            arena.apply_arguments(instantiated, arguments)
        }

        pub fn indexed_at(
            &self,
            arena: &mut TermArena,
            index: TermHandle,
            tree: TermHandle,
        ) -> TermHandle {
            self.spine(arena, INDEXED_AT, self.levels.to_vec(), &[index, tree])
        }

        pub fn indexed_w(&self, arena: &mut TermArena, index: TermHandle) -> TermHandle {
            self.spine(arena, INDEXED_W, self.levels.to_vec(), &[index])
        }

        pub fn pack(
            &self,
            arena: &mut TermArena,
            index: TermHandle,
            tree: TermHandle,
            evidence: TermHandle,
        ) -> TermHandle {
            self.spine(
                arena,
                INDEXED_PACK,
                self.levels.to_vec(),
                &[index, tree, evidence],
            )
        }

        pub fn sup(
            &self,
            arena: &mut TermArena,
            label: TermHandle,
            children_fn: TermHandle,
        ) -> TermHandle {
            self.spine(
                arena,
                INDEXED_SUP,
                self.levels.to_vec(),
                &[label, children_fn],
            )
        }

        pub fn ind(
            &self,
            arena: &mut TermArena,
            motive_level: Level,
            motive: TermHandle,
            step: TermHandle,
            index: TermHandle,
            tree: TermHandle,
        ) -> TermHandle {
            let mut levels = self.levels.to_vec();
            levels.push(motive_level);
            self.spine(arena, INDEXED_IND, levels, &[motive, step, index, tree])
        }
    }
}
pub(crate) mod quotient {
    use super::scheme_dsl::{
        Syntax, app, apps, assume, box_elim, box_intro, boxed, define, id, jelim, lam, pair, pi,
        pis, refl, scheme_at, sigma, sort, squash, squash_intro, strict, ty, v,
    };
    use super::signature::Declaration;
    use super::term::{Level, Term, TermArena, TermHandle};

    mod declarations {
        use super::*;

        macro_rules! pi {
            ($name:expr => $domain:expr => $codomain:expr) => {
                pi($name, $domain, $codomain)
            };
        }
        macro_rules! lam {
            ($name:expr => $domain:expr => $body:expr) => {
                lam($name, $domain, $body)
            };
        }
        macro_rules! id {
            ($ty:expr => $left:expr => $right:expr) => {
                id($ty, $left, $right)
            };
        }

        fn is_set_declaration(arena: &mut TermArena) -> Declaration {
            let statement = pi("X", ty(0), ty(0));
            let body = lam(
                "X",
                ty(0),
                pis(
                    [
                        ("x", v("X")),
                        ("y", v("X")),
                        ("p", id(v("X"), v("x"), v("y"))),
                        ("q", id(v("X"), v("x"), v("y"))),
                    ],
                    id(id(v("X"), v("x"), v("y")), v("p"), v("q")),
                ),
            );
            define(arena, 1, &statement, &body)
        }

        fn quotient_declaration(arena: &mut TermArena) -> Declaration {
            let statement = pi_description(sort(carrier_level()));
            assume(arena, 2, &statement)
        }

        fn project_declaration(arena: &mut TermArena) -> Declaration {
            let statement = pi_description(pi("_", v("A"), quotient_of()));
            assume(arena, 2, &statement)
        }

        fn set_quotient_declaration(arena: &mut TermArena) -> Declaration {
            let statement = pi_description(is_set(carrier_level(), quotient_of()));
            assume(arena, 2, &statement)
        }

        fn sound_declaration(arena: &mut TermArena) -> Declaration {
            let statement = pi_description(related_pis(
                "_",
                id(quotient_of(), project_of(v("a")), project_of(v("b"))),
            ));
            assume(arena, 2, &statement)
        }

        fn effective_declaration(arena: &mut TermArena) -> Declaration {
            let statement = pi_description(pis(
                [
                    ("a", v("A")),
                    ("b", v("A")),
                    (
                        "_",
                        id(quotient_of(), project_of(v("a")), project_of(v("b"))),
                    ),
                ],
                related(v("a"), v("b")),
            ));
            assume(arena, 2, &statement)
        }

        fn transport_declaration(arena: &mut TermArena) -> Declaration {
            let statement = pi_description(pis(
                [("P", motive_type())],
                related_pis(
                    "_",
                    pi!("_" => app(v("P"), project_of(v("a"))) => app(v("P"), project_of(v("b")))),
                ),
            ));
            let motive = lam(
                "y",
                quotient_of(),
                lam!("_" => id(quotient_of(), project_of(v("a")), v("y")) => app(v("P"), v("y"))),
            );
            let body = lam_description(lam(
                "P",
                motive_type(),
                related_lams(lam(
                    "h",
                    app(v("P"), project_of(v("a"))),
                    jelim(
                        motive,
                        v("h"),
                        project_of(v("b")),
                        sound_of(v("a"), v("b"), v("r")),
                    ),
                )),
            ));
            define(arena, 3, &statement, &body)
        }

        fn pi_cases(rest: Syntax) -> Syntax {
            pis(
                [
                    ("P", motive_type()),
                    (
                        "sets",
                        pi!("z" => quotient_of() => is_set(Level::Parameter(2), app(v("P"), v("z")))),
                    ),
                    ("d", pi("a", v("A"), app(v("P"), project_of(v("a"))))),
                    (
                        "c",
                        related_pis(
                            "r",
                            id(
                                app(v("P"), project_of(v("b"))),
                                apps(
                                    motive_scheme(QUOTIENT_TRANSPORT),
                                    [
                                        v("A"),
                                        v("R"),
                                        v("P"),
                                        v("a"),
                                        v("b"),
                                        v("r"),
                                        app(v("d"), v("a")),
                                    ],
                                ),
                                app(v("d"), v("b")),
                            ),
                        ),
                    ),
                ],
                rest,
            )
        }

        fn related_pis(evidence: &'static str, rest: Syntax) -> Syntax {
            pis(
                [
                    ("a", v("A")),
                    ("b", v("A")),
                    (evidence, related(v("a"), v("b"))),
                ],
                rest,
            )
        }

        fn related_lams(rest: Syntax) -> Syntax {
            lam!("a" => v("A") => lam("b", v("A"), lam("r", related(v("a"), v("b")), rest)))
        }

        fn elim_declaration(arena: &mut TermArena) -> Declaration {
            let statement = pi_description(pi_cases(pi("z", quotient_of(), app(v("P"), v("z")))));
            assume(arena, 3, &statement)
        }

        fn beta_declaration(arena: &mut TermArena) -> Declaration {
            let statement = pi_description(pi_cases(pi(
                "a",
                v("A"),
                id(
                    app(v("P"), project_of(v("a"))),
                    apps(
                        motive_scheme(QUOTIENT_ELIM),
                        [
                            v("A"),
                            v("R"),
                            v("P"),
                            v("sets"),
                            v("d"),
                            v("c"),
                            project_of(v("a")),
                        ],
                    ),
                    app(v("d"), v("a")),
                ),
            )));
            assume(arena, 3, &statement)
        }

        fn id_trans_declaration(arena: &mut TermArena) -> Declaration {
            let statement = pi(
                "B",
                ty(0),
                pi(
                    "x",
                    v("B"),
                    pi(
                        "y",
                        v("B"),
                        pi(
                            "z",
                            v("B"),
                            pi(
                                "_",
                                id(v("B"), v("x"), v("y")),
                                pi("_", id(v("B"), v("y"), v("z")), id(v("B"), v("x"), v("z"))),
                            ),
                        ),
                    ),
                ),
            );
            let motive = lam(
                "z2",
                v("B"),
                lam!("_" => id(v("B"), v("y"), v("z2")) => id(v("B"), v("x"), v("z2"))),
            );
            let body = lam(
                "B",
                ty(0),
                lam(
                    "x",
                    v("B"),
                    lam(
                        "y",
                        v("B"),
                        lam(
                            "z",
                            v("B"),
                            lam(
                                "p",
                                id(v("B"), v("x"), v("y")),
                                lam(
                                    "q",
                                    id(v("B"), v("y"), v("z")),
                                    jelim(motive, v("p"), v("z"), v("q")),
                                ),
                            ),
                        ),
                    ),
                ),
            );
            define(arena, 1, &statement, &body)
        }

        fn transport_const_declaration(arena: &mut TermArena) -> Declaration {
            let constant_motive =
                || lam("y2", v("A"), lam("_", id(v("A"), v("x"), v("y2")), v("B")));
            let inner = jelim(constant_motive(), v("b"), v("y"), v("p"));
            let statement = pi(
                "A",
                ty(0),
                pi(
                    "B",
                    ty(1),
                    pi(
                        "x",
                        v("A"),
                        pi(
                            "y",
                            v("A"),
                            pi(
                                "p",
                                id(v("A"), v("x"), v("y")),
                                pi("b", v("B"), id(v("B"), inner, v("b"))),
                            ),
                        ),
                    ),
                ),
            );
            let outer_motive = lam(
                "y2",
                v("A"),
                lam(
                    "p2",
                    id(v("A"), v("x"), v("y2")),
                    id!(v("B") => jelim(constant_motive(), v("b"), v("y2"), v("p2")) => v("b")),
                ),
            );
            let body = lam(
                "A",
                ty(0),
                lam(
                    "B",
                    ty(1),
                    lam(
                        "x",
                        v("A"),
                        lam(
                            "y",
                            v("A"),
                            lam(
                                "p",
                                id(v("A"), v("x"), v("y")),
                                lam(
                                    "b",
                                    v("B"),
                                    jelim(outer_motive, refl(v("B"), v("b")), v("y"), v("p")),
                                ),
                            ),
                        ),
                    ),
                ),
            );
            define(arena, 2, &statement, &body)
        }

        fn lift_declaration(arena: &mut TermArena) -> Declaration {
            let statement = pi_description(pi(
                "B",
                ty(2),
                pi(
                    "_",
                    is_set(Level::Parameter(2), v("B")),
                    pi(
                        "f",
                        pi("_", v("A"), v("B")),
                        pi(
                            "respect",
                            related_pis("_", id(v("B"), app(v("f"), v("a")), app(v("f"), v("b")))),
                            pi("_", quotient_of(), v("B")),
                        ),
                    ),
                ),
            ));
            let constant_family = || lam("_", quotient_of(), v("B"));
            let soundness = sound_of(v("a"), v("b"), v("r"));
            let transported_case = apps(
                scheme_at(
                    QUOTIENT_TRANSPORT_CONST,
                    vec![carrier_level(), Level::Parameter(2)],
                ),
                [
                    quotient_of(),
                    v("B"),
                    project_of(v("a")),
                    project_of(v("b")),
                    soundness,
                    app(v("f"), v("a")),
                ],
            );
            let transported = apps(
                motive_scheme(QUOTIENT_TRANSPORT),
                [
                    v("A"),
                    v("R"),
                    constant_family(),
                    v("a"),
                    v("b"),
                    v("r"),
                    app(v("f"), v("a")),
                ],
            );
            let coherence = related_lams(apps(
                scheme_at(QUOTIENT_ID_TRANS, vec![Level::Parameter(2)]),
                [
                    v("B"),
                    transported,
                    app(v("f"), v("a")),
                    app(v("f"), v("b")),
                    transported_case,
                    apps(v("respect"), [v("a"), v("b"), v("r")]),
                ],
            ));
            let body = lam_description(lam(
                "B",
                ty(2),
                lam(
                    "setB",
                    is_set(Level::Parameter(2), v("B")),
                    lam(
                        "f",
                        pi("_", v("A"), v("B")),
                        lam(
                            "respect",
                            related_pis("_", id(v("B"), app(v("f"), v("a")), app(v("f"), v("b")))),
                            lam(
                                "z",
                                quotient_of(),
                                apps(
                                    motive_scheme(QUOTIENT_ELIM),
                                    [
                                        v("A"),
                                        v("R"),
                                        constant_family(),
                                        lam("_", quotient_of(), v("setB")),
                                        v("f"),
                                        coherence,
                                        v("z"),
                                    ],
                                ),
                            ),
                        ),
                    ),
                ),
            ));
            define(arena, 3, &statement, &body)
        }

        fn lift_precondition_declaration(arena: &mut TermArena) -> Declaration {
            let statement = pi_description(pi(
                "B",
                ty(2),
                pi(
                    "_",
                    is_set(Level::Parameter(2), v("B")),
                    pi(
                        "Pub",
                        pi("_", quotient_of(), ty(3)),
                        pi(
                            "Pre",
                            pi("_", v("A"), ty(4)),
                            pi(
                                "t",
                                pi(
                                    "a",
                                    v("A"),
                                    pi!("_" => app(v("Pub"), project_of(v("a"))) => app(v("Pre"), v("a"))),
                                ),
                                pi(
                                    "f",
                                    pi("a", v("A"), pi("_", app(v("Pre"), v("a")), v("B"))),
                                    pi(
                                        "respect",
                                        related_pis(
                                            "_",
                                            pi(
                                                "ha",
                                                app(v("Pub"), project_of(v("a"))),
                                                pi(
                                                    "hb",
                                                    app(v("Pub"), project_of(v("b"))),
                                                    id(
                                                        v("B"),
                                                        app(
                                                            app(v("f"), v("a")),
                                                            apps(v("t"), [v("a"), v("ha")]),
                                                        ),
                                                        app(
                                                            app(v("f"), v("b")),
                                                            apps(v("t"), [v("b"), v("hb")]),
                                                        ),
                                                    ),
                                                ),
                                            ),
                                        ),
                                        pi(
                                            "z",
                                            quotient_of(),
                                            pi("_", app(v("Pub"), v("z")), v("B")),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ));
            assume(arena, 5, &statement)
        }

        fn id_sym_declaration(arena: &mut TermArena) -> Declaration {
            let statement = pi(
                "X",
                ty(0),
                pi(
                    "x",
                    v("X"),
                    pi!("y" => v("X") => pi("_", id(v("X"), v("x"), v("y")), id(v("X"), v("y"), v("x")))),
                ),
            );
            let motive = lam(
                "y2",
                v("X"),
                lam!("_" => id(v("X"), v("x"), v("y2")) => id(v("X"), v("y2"), v("x"))),
            );
            let body = lam(
                "X",
                ty(0),
                lam(
                    "x",
                    v("X"),
                    lam(
                        "y",
                        v("X"),
                        lam(
                            "p",
                            id(v("X"), v("x"), v("y")),
                            jelim(motive, refl(v("X"), v("x")), v("y"), v("p")),
                        ),
                    ),
                ),
            );
            define(arena, 1, &statement, &body)
        }

        fn cancelled(endpoint: &'static str, proof: &'static str) -> Syntax {
            let level = vec![Level::Parameter(0)];
            apps(
                scheme_at(QUOTIENT_ID_TRANS, level.clone()),
                [
                    v("X"),
                    v(endpoint),
                    v("x"),
                    v(endpoint),
                    apps(
                        scheme_at(QUOTIENT_ID_SYM, level),
                        [v("X"), v("x"), v(endpoint), v(proof)],
                    ),
                    v(proof),
                ],
            )
        }

        fn id_cancel_declaration(arena: &mut TermArena) -> Declaration {
            let statement = pi(
                "X",
                ty(0),
                pi(
                    "x",
                    v("X"),
                    pi(
                        "y",
                        v("X"),
                        pi(
                            "p",
                            id(v("X"), v("x"), v("y")),
                            id!(id(v("X"), v("y"), v("y")) => cancelled("y", "p") => refl(v("X"), v("y"))),
                        ),
                    ),
                ),
            );
            let motive = lam(
                "y2",
                v("X"),
                lam(
                    "p2",
                    id(v("X"), v("x"), v("y2")),
                    id!(id(v("X"), v("y2"), v("y2")) => cancelled("y2", "p2") => refl(v("X"), v("y2"))),
                ),
            );
            let body = lam(
                "X",
                ty(0),
                lam(
                    "x",
                    v("X"),
                    lam(
                        "y",
                        v("X"),
                        lam(
                            "p",
                            id(v("X"), v("x"), v("y")),
                            jelim(
                                motive,
                                refl(id(v("X"), v("x"), v("x")), refl(v("X"), v("x"))),
                                v("y"),
                                v("p"),
                            ),
                        ),
                    ),
                ),
            );
            define(arena, 1, &statement, &body)
        }

        fn prop_is_set_declaration(arena: &mut TermArena) -> Declaration {
            let level = || vec![Level::Parameter(0)];
            let prop = |endpoint: &'static str| apps(v("f"), [v("x"), v(endpoint)]);
            let normal = |endpoint: &'static str| {
                apps(
                    scheme_at(QUOTIENT_ID_TRANS, level()),
                    [
                        v("X"),
                        v("x"),
                        v("x"),
                        v(endpoint),
                        apps(
                            scheme_at(QUOTIENT_ID_SYM, level()),
                            [v("X"), v("x"), v("x"), prop("x")],
                        ),
                        prop(endpoint),
                    ],
                )
            };
            let path_motive = lam(
                "y2",
                v("X"),
                lam(
                    "p2",
                    id(v("X"), v("x"), v("y2")),
                    id(id(v("X"), v("x"), v("y2")), v("p2"), normal("y2")),
                ),
            );
            let path_base = apps(
                scheme_at(QUOTIENT_ID_SYM, level()),
                [
                    id(v("X"), v("x"), v("x")),
                    normal("x"),
                    refl(v("X"), v("x")),
                    apps(
                        scheme_at(QUOTIENT_ID_CANCEL, level()),
                        [v("X"), v("x"), v("x"), prop("x")],
                    ),
                ],
            );
            let path = |proof: &'static str| {
                jelim(path_motive.clone(), path_base.clone(), v("y"), v(proof))
            };
            let is_prop = pi("x2", v("X"), pi("y2", v("X"), id(v("X"), v("x2"), v("y2"))));
            let statement = pi(
                "X",
                ty(0),
                pi("f", is_prop.clone(), is_set(Level::Parameter(0), v("X"))),
            );
            let identity_xy = id(v("X"), v("x"), v("y"));
            let body = lam(
                "X",
                ty(0),
                lam(
                    "f",
                    is_prop,
                    lam(
                        "x",
                        v("X"),
                        lam(
                            "y",
                            v("X"),
                            lam(
                                "p",
                                identity_xy.clone(),
                                lam(
                                    "q",
                                    identity_xy.clone(),
                                    apps(
                                        scheme_at(QUOTIENT_ID_TRANS, level()),
                                        [
                                            identity_xy.clone(),
                                            v("p"),
                                            normal("y"),
                                            v("q"),
                                            path("p"),
                                            apps(
                                                scheme_at(QUOTIENT_ID_SYM, level()),
                                                [
                                                    identity_xy.clone(),
                                                    v("q"),
                                                    normal("y"),
                                                    path("q"),
                                                ],
                                            ),
                                        ],
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            );
            define(arena, 1, &statement, &body)
        }

        fn box_prop_declaration(arena: &mut TermArena) -> Declaration {
            let boxed_s = || boxed(v("S"));
            let inner_motive = lam(
                "y3",
                boxed_s(),
                id(boxed_s(), box_intro(v("S"), v("a")), v("y3")),
            );
            let outer_motive = lam(
                "x2",
                boxed_s(),
                pi("y2", boxed_s(), id(boxed_s(), v("x2"), v("y2"))),
            );
            let case = lam(
                "a",
                v("S"),
                lam(
                    "y2",
                    boxed_s(),
                    box_elim(
                        inner_motive,
                        lam("b", v("S"), refl(boxed_s(), box_intro(v("S"), v("a")))),
                        v("y2"),
                    ),
                ),
            );
            let statement = pi(
                "S",
                strict(0),
                pi!("x" => boxed_s() => pi("y", boxed_s(), id(boxed_s(), v("x"), v("y")))),
            );
            let body = lam(
                "S",
                strict(0),
                lam(
                    "x",
                    boxed_s(),
                    lam!("y" => boxed_s() => app(box_elim(outer_motive, case, v("x")), v("y"))),
                ),
            );
            define(arena, 1, &statement, &body)
        }

        fn ind_prop_declaration(arena: &mut TermArena) -> Declaration {
            let strict_level = || vec![Level::Parameter(2)];
            let boxed_family = lam("z2", quotient_of(), boxed(app(v("P"), v("z2"))));
            let sets = lam(
                "z3",
                quotient_of(),
                apps(
                    scheme_at(QUOTIENT_PROP_IS_SET, strict_level()),
                    [
                        boxed(app(v("P"), v("z3"))),
                        apps(
                            scheme_at(QUOTIENT_BOX_PROP, strict_level()),
                            [app(v("P"), v("z3"))],
                        ),
                    ],
                ),
            );
            let case = lam(
                "a",
                v("A"),
                box_intro(app(v("P"), project_of(v("a"))), app(v("d"), v("a"))),
            );
            let coherence = related_lams(apps(
                scheme_at(QUOTIENT_BOX_PROP, strict_level()),
                [
                    app(v("P"), project_of(v("b"))),
                    apps(
                        motive_scheme(QUOTIENT_TRANSPORT),
                        [
                            v("A"),
                            v("R"),
                            boxed_family.clone(),
                            v("a"),
                            v("b"),
                            v("r"),
                            app(case.clone(), v("a")),
                        ],
                    ),
                    app(case.clone(), v("b")),
                ],
            ));
            let section = apps(
                motive_scheme(QUOTIENT_ELIM),
                [v("A"), v("R"), boxed_family, sets, case, coherence, v("z")],
            );
            let unboxed = box_elim(
                lam("_", boxed(app(v("P"), v("z"))), app(v("P"), v("z"))),
                lam("t", app(v("P"), v("z")), v("t")),
                section,
            );
            let family_type = pi("_", quotient_of(), strict(2));
            let case_type = pi("a", v("A"), app(v("P"), project_of(v("a"))));
            let statement = pi_description(pi(
                "P",
                family_type.clone(),
                pi!("d" => case_type.clone() => pi("z", quotient_of(), app(v("P"), v("z")))),
            ));
            let body = lam_description(lam(
                "P",
                family_type,
                lam("d", case_type, lam("z", quotient_of(), unboxed)),
            ));
            define(arena, 3, &statement, &body)
        }

        fn coverage_declaration(arena: &mut TermArena) -> Declaration {
            let covered = |anchor: Syntax| {
                squash(sigma(
                    "a2",
                    v("A"),
                    id(quotient_of(), project_of(v("a2")), anchor),
                ))
            };
            let statement = pi_description(pi("z", quotient_of(), covered(v("z"))));
            let case = lam(
                "a",
                v("A"),
                squash_intro(
                    sigma(
                        "a2",
                        v("A"),
                        id(quotient_of(), project_of(v("a2")), project_of(v("a"))),
                    ),
                    pair(v("a"), refl(quotient_of(), project_of(v("a")))),
                ),
            );
            let motive = lam("z2", quotient_of(), covered(v("z2")));
            let body = lam_description(lam(
                "z",
                quotient_of(),
                apps(
                    scheme_at(
                        QUOTIENT_IND_PROP,
                        vec![Level::Parameter(0), Level::Parameter(1), carrier_level()],
                    ),
                    [v("A"), v("R"), motive, case, v("z")],
                ),
            ));
            define(arena, 2, &statement, &body)
        }

        fn unique_declaration(arena: &mut TermArena) -> Declaration {
            let identity_family = lam(
                "z2",
                quotient_of(),
                id!(app(v("P"), v("z2")) => app(v("s1"), v("z2")) => app(v("s2"), v("z2"))),
            );
            let sets = lam(
                "z3",
                quotient_of(),
                apps(
                    scheme_at(QUOTIENT_PROP_IS_SET, vec![Level::Parameter(2)]),
                    [
                        id!(app(v("P"), v("z3")) => app(v("s1"), v("z3")) => app(v("s2"), v("z3"))),
                        apps(
                            v("sets"),
                            [v("z3"), app(v("s1"), v("z3")), app(v("s2"), v("z3"))],
                        ),
                    ],
                ),
            );
            let coherence = related_lams(apps(
                v("sets"),
                [
                    project_of(v("b")),
                    app(v("s1"), project_of(v("b"))),
                    app(v("s2"), project_of(v("b"))),
                    apps(
                        motive_scheme(QUOTIENT_TRANSPORT),
                        [
                            v("A"),
                            v("R"),
                            identity_family.clone(),
                            v("a"),
                            v("b"),
                            v("r"),
                            app(v("h"), v("a")),
                        ],
                    ),
                    app(v("h"), v("b")),
                ],
            ));
            let family_type = motive_type();
            let sets_type = pi(
                "z",
                quotient_of(),
                is_set(Level::Parameter(2), app(v("P"), v("z"))),
            );
            let section_type = pi("z", quotient_of(), app(v("P"), v("z")));
            let agreement_type = pi(
                "a",
                v("A"),
                id(
                    app(v("P"), project_of(v("a"))),
                    app(v("s1"), project_of(v("a"))),
                    app(v("s2"), project_of(v("a"))),
                ),
            );
            let statement = pi_description(pi(
                "P",
                family_type.clone(),
                pi(
                    "sets",
                    sets_type.clone(),
                    pi(
                        "s1",
                        section_type.clone(),
                        pi(
                            "s2",
                            section_type.clone(),
                            pi(
                                "h",
                                agreement_type.clone(),
                                pi(
                                    "z",
                                    quotient_of(),
                                    id(
                                        app(v("P"), v("z")),
                                        app(v("s1"), v("z")),
                                        app(v("s2"), v("z")),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ));
            let body = lam_description(lam(
                "P",
                family_type,
                lam(
                    "sets",
                    sets_type,
                    lam(
                        "s1",
                        section_type.clone(),
                        lam(
                            "s2",
                            section_type,
                            lam(
                                "h",
                                agreement_type,
                                lam(
                                    "z",
                                    quotient_of(),
                                    apps(
                                        motive_scheme(QUOTIENT_ELIM),
                                        [
                                            v("A"),
                                            v("R"),
                                            identity_family,
                                            sets,
                                            v("h"),
                                            coherence,
                                            v("z"),
                                        ],
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ));
            define(arena, 3, &statement, &body)
        }

        pub fn quotient_scheme(arena: &mut TermArena) -> Vec<Declaration> {
            vec![
                is_set_declaration(arena),
                quotient_declaration(arena),
                project_declaration(arena),
                set_quotient_declaration(arena),
                sound_declaration(arena),
                effective_declaration(arena),
                transport_declaration(arena),
                elim_declaration(arena),
                beta_declaration(arena),
                id_trans_declaration(arena),
                transport_const_declaration(arena),
                lift_declaration(arena),
                lift_precondition_declaration(arena),
                id_sym_declaration(arena),
                id_cancel_declaration(arena),
                prop_is_set_declaration(arena),
                box_prop_declaration(arena),
                ind_prop_declaration(arena),
                coverage_declaration(arena),
                unique_declaration(arena),
            ]
        }
    }
    pub use declarations::quotient_scheme;

    pub const QUOTIENT_IS_SET: u32 = 0;
    pub const QUOTIENT: u32 = 1;
    pub const QUOTIENT_PROJECT: u32 = 2;
    pub const QUOTIENT_SET: u32 = 3;
    pub const QUOTIENT_SOUND: u32 = 4;
    pub const QUOTIENT_EFFECTIVE: u32 = 5;
    pub const QUOTIENT_TRANSPORT: u32 = 6;
    pub const QUOTIENT_ELIM: u32 = 7;
    pub const QUOTIENT_BETA: u32 = 8;
    pub const QUOTIENT_ID_TRANS: u32 = 9;
    pub const QUOTIENT_TRANSPORT_CONST: u32 = 10;
    pub const QUOTIENT_LIFT: u32 = 11;
    pub const QUOTIENT_LIFT_PRECONDITION: u32 = 12;
    pub const QUOTIENT_ID_SYM: u32 = 13;
    pub const QUOTIENT_ID_CANCEL: u32 = 14;
    pub const QUOTIENT_PROP_IS_SET: u32 = 15;
    pub const QUOTIENT_BOX_PROP: u32 = 16;
    pub const QUOTIENT_IND_PROP: u32 = 17;
    pub const QUOTIENT_COVERAGE: u32 = 18;
    pub const QUOTIENT_UNIQUE: u32 = 19;

    fn carrier_level() -> Level {
        Level::Maximum(Box::new(Level::Parameter(0)), Box::new(Level::Parameter(1)))
    }

    fn uv() -> Vec<Level> {
        vec![Level::Parameter(0), Level::Parameter(1)]
    }

    fn uvw() -> Vec<Level> {
        let mut levels = uv();
        levels.push(Level::Parameter(2));
        levels
    }

    fn carrier_scheme(declaration: u32) -> Syntax {
        scheme_at(declaration, uv())
    }

    fn motive_scheme(declaration: u32) -> Syntax {
        scheme_at(declaration, uvw())
    }

    fn quotient_of() -> Syntax {
        apps(carrier_scheme(QUOTIENT), [v("A"), v("R")])
    }

    fn project_of(representative: Syntax) -> Syntax {
        apps(
            carrier_scheme(QUOTIENT_PROJECT),
            [v("A"), v("R"), representative],
        )
    }

    fn related(left: Syntax, right: Syntax) -> Syntax {
        app(app(v("R"), left), right)
    }

    fn is_set(level: Level, ty: Syntax) -> Syntax {
        app(scheme_at(QUOTIENT_IS_SET, vec![level]), ty)
    }

    fn sound_of(left: Syntax, right: Syntax, evidence: Syntax) -> Syntax {
        apps(
            carrier_scheme(QUOTIENT_SOUND),
            [v("A"), v("R"), left, right, evidence],
        )
    }

    fn pi_description(rest: Syntax) -> Syntax {
        pi(
            "A",
            ty(0),
            pi("R", pi("_", v("A"), pi("_", v("A"), ty(1))), rest),
        )
    }

    fn lam_description(rest: Syntax) -> Syntax {
        lam(
            "A",
            ty(0),
            lam("R", pi("_", v("A"), pi("_", v("A"), ty(1))), rest),
        )
    }

    fn motive_type() -> Syntax {
        pi("_", quotient_of(), ty(2))
    }

    #[derive(Clone, Debug)]
    pub struct QuotientFamily {
        pub levels: [Level; 2],
        pub carrier: TermHandle,
        pub relation: TermHandle,
    }

    impl QuotientFamily {
        fn unspined(
            &self,
            arena: &mut TermArena,
            declaration: u32,
            level: Level,
            arguments: &[TermHandle],
        ) -> TermHandle {
            let constant = arena.insert(Term::Constant {
                declaration,
                levels: vec![level],
            });
            arena.apply_arguments(constant, arguments)
        }

        fn spine(
            &self,
            arena: &mut TermArena,
            declaration: u32,
            mut levels: Vec<Level>,
            arguments: &[TermHandle],
        ) -> TermHandle {
            let constant = arena.insert(Term::Constant {
                declaration,
                levels: {
                    let mut instantiation = self.levels.to_vec();
                    instantiation.append(&mut levels);
                    instantiation
                },
            });
            let related = arena.apply_arguments(constant, &[self.carrier, self.relation]);
            arena.apply_arguments(related, arguments)
        }

        pub fn quotient(&self, arena: &mut TermArena) -> TermHandle {
            self.spine(arena, QUOTIENT, Vec::new(), &[])
        }

        pub fn project(&self, arena: &mut TermArena, representative: TermHandle) -> TermHandle {
            self.spine(arena, QUOTIENT_PROJECT, Vec::new(), &[representative])
        }

        pub fn sound(
            &self,
            arena: &mut TermArena,
            left: TermHandle,
            right: TermHandle,
            evidence: TermHandle,
        ) -> TermHandle {
            self.spine(arena, QUOTIENT_SOUND, Vec::new(), &[left, right, evidence])
        }

        pub fn effective(
            &self,
            arena: &mut TermArena,
            left: TermHandle,
            right: TermHandle,
            proof: TermHandle,
        ) -> TermHandle {
            self.spine(arena, QUOTIENT_EFFECTIVE, Vec::new(), &[left, right, proof])
        }

        pub fn transport(
            &self,
            arena: &mut TermArena,
            motive_level: Level,
            family: TermHandle,
            left: TermHandle,
            right: TermHandle,
            evidence: TermHandle,
            premise: TermHandle,
        ) -> TermHandle {
            self.spine(
                arena,
                QUOTIENT_TRANSPORT,
                vec![motive_level],
                &[family, left, right, evidence, premise],
            )
        }

        pub fn elim(
            &self,
            arena: &mut TermArena,
            motive_level: Level,
            family: TermHandle,
            sets: TermHandle,
            case: TermHandle,
            compatibility: TermHandle,
            element: TermHandle,
        ) -> TermHandle {
            self.spine(
                arena,
                QUOTIENT_ELIM,
                vec![motive_level],
                &[family, sets, case, compatibility, element],
            )
        }

        pub fn beta(
            &self,
            arena: &mut TermArena,
            motive_level: Level,
            family: TermHandle,
            sets: TermHandle,
            case: TermHandle,
            compatibility: TermHandle,
            representative: TermHandle,
        ) -> TermHandle {
            self.spine(
                arena,
                QUOTIENT_BETA,
                vec![motive_level],
                &[family, sets, case, compatibility, representative],
            )
        }

        pub fn id_trans(
            &self,
            arena: &mut TermArena,
            level: Level,
            ty: TermHandle,
            left: TermHandle,
            middle: TermHandle,
            right: TermHandle,
            first: TermHandle,
            second: TermHandle,
        ) -> TermHandle {
            self.unspined(
                arena,
                QUOTIENT_ID_TRANS,
                level,
                &[ty, left, middle, right, first, second],
            )
        }

        pub fn lift(
            &self,
            arena: &mut TermArena,
            motive_level: Level,
            result: TermHandle,
            set_proof: TermHandle,
            operation: TermHandle,
            respect: TermHandle,
            element: TermHandle,
        ) -> TermHandle {
            self.spine(
                arena,
                QUOTIENT_LIFT,
                vec![motive_level],
                &[result, set_proof, operation, respect, element],
            )
        }

        pub fn lift_precondition(
            &self,
            arena: &mut TermArena,
            result_level: Level,
            public_level: Level,
            representative_level: Level,
            result: TermHandle,
            set_proof: TermHandle,
            public: TermHandle,
            representative: TermHandle,
            transport: TermHandle,
            operation: TermHandle,
            respect: TermHandle,
            element: TermHandle,
            premise: TermHandle,
        ) -> TermHandle {
            self.spine(
                arena,
                QUOTIENT_LIFT_PRECONDITION,
                vec![result_level, public_level, representative_level],
                &[
                    result,
                    set_proof,
                    public,
                    representative,
                    transport,
                    operation,
                    respect,
                    element,
                    premise,
                ],
            )
        }

        pub fn id_sym(
            &self,
            arena: &mut TermArena,
            level: Level,
            ty: TermHandle,
            left: TermHandle,
            right: TermHandle,
            proof: TermHandle,
        ) -> TermHandle {
            self.unspined(arena, QUOTIENT_ID_SYM, level, &[ty, left, right, proof])
        }

        pub fn id_cancel(
            &self,
            arena: &mut TermArena,
            level: Level,
            ty: TermHandle,
            left: TermHandle,
            right: TermHandle,
            proof: TermHandle,
        ) -> TermHandle {
            self.unspined(arena, QUOTIENT_ID_CANCEL, level, &[ty, left, right, proof])
        }

        pub fn prop_is_set(
            &self,
            arena: &mut TermArena,
            level: Level,
            ty: TermHandle,
            evidence: TermHandle,
        ) -> TermHandle {
            self.unspined(arena, QUOTIENT_PROP_IS_SET, level, &[ty, evidence])
        }

        pub fn box_prop(
            &self,
            arena: &mut TermArena,
            level: Level,
            strict: TermHandle,
            left: TermHandle,
            right: TermHandle,
        ) -> TermHandle {
            self.unspined(arena, QUOTIENT_BOX_PROP, level, &[strict, left, right])
        }

        pub fn ind_prop(
            &self,
            arena: &mut TermArena,
            strict_level: Level,
            family: TermHandle,
            case: TermHandle,
            element: TermHandle,
        ) -> TermHandle {
            self.spine(
                arena,
                QUOTIENT_IND_PROP,
                vec![strict_level],
                &[family, case, element],
            )
        }

        pub fn coverage(&self, arena: &mut TermArena, element: TermHandle) -> TermHandle {
            self.spine(arena, QUOTIENT_COVERAGE, Vec::new(), &[element])
        }

        pub fn unique(
            &self,
            arena: &mut TermArena,
            motive_level: Level,
            family: TermHandle,
            sets: TermHandle,
            first: TermHandle,
            second: TermHandle,
            agreement: TermHandle,
            element: TermHandle,
        ) -> TermHandle {
            self.spine(
                arena,
                QUOTIENT_UNIQUE,
                vec![motive_level],
                &[family, sets, first, second, agreement, element],
            )
        }
    }
}
pub(crate) mod scheme_dsl {
    use super::signature::Declaration;
    use super::term::{Level, Sort, Term, TermArena, TermHandle};

    #[derive(Clone)]
    pub(crate) enum Syntax {
        Variable(&'static str),
        Sort(Sort),
        Pi(&'static str, Box<Syntax>, Box<Syntax>),
        Lambda(&'static str, Box<Syntax>, Box<Syntax>),
        Apply(Box<Syntax>, Box<Syntax>),
        Sigma(&'static str, Box<Syntax>, Box<Syntax>),
        Pair(Box<Syntax>, Box<Syntax>),
        Fst(Box<Syntax>),
        Snd(Box<Syntax>),
        Id(Box<Syntax>, Box<Syntax>, Box<Syntax>),
        Refl(Box<Syntax>, Box<Syntax>),
        IdElim(Box<Syntax>, Box<Syntax>, Box<Syntax>, Box<Syntax>),
        W(Box<Syntax>, Box<Syntax>),
        Sup(Box<Syntax>, Box<Syntax>, Box<Syntax>, Box<Syntax>),
        IndW(Box<Syntax>, Box<Syntax>, Box<Syntax>),
        Squash(Box<Syntax>),
        SquashIntro(Box<Syntax>, Box<Syntax>),
        Boxed(Box<Syntax>),
        BoxIntro(Box<Syntax>, Box<Syntax>),
        BoxElim(Box<Syntax>, Box<Syntax>, Box<Syntax>),
        Scheme(u32, Vec<Level>),
        Two,
        CaseTwo(Box<Syntax>, Box<Syntax>, Box<Syntax>, Box<Syntax>),
    }

    pub(crate) fn define(
        arena: &mut TermArena,
        level_arity: u32,
        statement: &Syntax,
        body: &Syntax,
    ) -> Declaration {
        Declaration::definition(
            level_arity,
            build(arena, &mut Vec::new(), statement),
            build(arena, &mut Vec::new(), body),
        )
    }

    pub(crate) fn assume(
        arena: &mut TermArena,
        level_arity: u32,
        statement: &Syntax,
    ) -> Declaration {
        Declaration::assumption(level_arity, build(arena, &mut Vec::new(), statement))
    }

    pub(crate) fn build(
        arena: &mut TermArena,
        scope: &mut Vec<&'static str>,
        syntax: &Syntax,
    ) -> TermHandle {
        match syntax {
            Syntax::Variable(name) => {
                let position = scope
                    .iter()
                    .rposition(|bound| bound == name)
                    .unwrap_or_else(|| panic!("unbound scheme variable {name}"));
                let index = scope.len() - 1 - position;
                arena.insert(Term::Variable(index as u32))
            }
            Syntax::Sort(sort) => arena.insert(Term::Sort(sort.clone())),
            Syntax::Pi(name, domain, codomain) | Syntax::Sigma(name, domain, codomain) => {
                let domain = build(arena, scope, domain);
                scope.push(name);
                let codomain = build(arena, scope, codomain);
                scope.pop();
                match syntax {
                    Syntax::Pi(..) => arena.insert(Term::Pi { domain, codomain }),
                    _ => arena.insert(Term::Sigma { domain, codomain }),
                }
            }
            Syntax::Lambda(name, domain, body) => {
                let domain = build(arena, scope, domain);
                scope.push(name);
                let body = build(arena, scope, body);
                scope.pop();
                arena.insert(Term::Lambda { domain, body })
            }
            Syntax::Apply(function, argument) => {
                let function = build(arena, scope, function);
                let argument = build(arena, scope, argument);
                arena.insert(Term::Apply { function, argument })
            }
            Syntax::Pair(first, second) => {
                let first = build(arena, scope, first);
                let second = build(arena, scope, second);
                arena.insert(Term::Pair { first, second })
            }
            Syntax::Fst(pair) => {
                let pair = build(arena, scope, pair);
                arena.insert(Term::Fst { pair })
            }
            Syntax::Snd(pair) => {
                let pair = build(arena, scope, pair);
                arena.insert(Term::Snd { pair })
            }
            Syntax::Id(ty, left, right) => {
                let ty = build(arena, scope, ty);
                let left = build(arena, scope, left);
                let right = build(arena, scope, right);
                arena.insert(Term::Id { ty, left, right })
            }
            Syntax::Refl(ty, value) => {
                let ty = build(arena, scope, ty);
                let value = build(arena, scope, value);
                arena.insert(Term::Refl { ty, value })
            }
            Syntax::IdElim(motive, base, endpoint, proof) => {
                let motive = build(arena, scope, motive);
                let base = build(arena, scope, base);
                let endpoint = build(arena, scope, endpoint);
                let proof = build(arena, scope, proof);
                arena.insert(Term::IdElim {
                    motive,
                    base,
                    endpoint,
                    proof,
                })
            }
            Syntax::W(carrier, children) => {
                let carrier = build(arena, scope, carrier);
                let children = build(arena, scope, children);
                arena.insert(Term::W { carrier, children })
            }
            Syntax::Sup(carrier, children, label, function) => {
                let carrier = build(arena, scope, carrier);
                let children = build(arena, scope, children);
                let label = build(arena, scope, label);
                let function = build(arena, scope, function);
                arena.insert(Term::Sup {
                    carrier,
                    children,
                    label,
                    function,
                })
            }
            Syntax::IndW(motive, step, tree) => {
                let motive = build(arena, scope, motive);
                let step = build(arena, scope, step);
                let tree = build(arena, scope, tree);
                arena.insert(Term::IndW { motive, step, tree })
            }
            Syntax::Squash(ty) => {
                let ty = build(arena, scope, ty);
                arena.insert(Term::Squash { ty })
            }
            Syntax::SquashIntro(ty, value) => {
                let ty = build(arena, scope, ty);
                let value = build(arena, scope, value);
                arena.insert(Term::SquashIntro { ty, value })
            }
            Syntax::Boxed(ty) => {
                let ty = build(arena, scope, ty);
                arena.insert(Term::Box { ty })
            }
            Syntax::BoxIntro(ty, value) => {
                let ty = build(arena, scope, ty);
                let value = build(arena, scope, value);
                arena.insert(Term::BoxIntro { ty, value })
            }
            Syntax::BoxElim(motive, body, scrutinee) => {
                let motive = build(arena, scope, motive);
                let body = build(arena, scope, body);
                let scrutinee = build(arena, scope, scrutinee);
                arena.insert(Term::BoxElim {
                    motive,
                    body,
                    scrutinee,
                })
            }
            Syntax::Scheme(declaration, levels) => arena.insert(Term::Constant {
                declaration: *declaration,
                levels: levels.clone(),
            }),
            Syntax::Two => arena.insert(Term::Two),
            Syntax::CaseTwo(motive, zero_branch, one_branch, scrutinee) => {
                let motive = build(arena, scope, motive);
                let zero_branch = build(arena, scope, zero_branch);
                let one_branch = build(arena, scope, one_branch);
                let scrutinee = build(arena, scope, scrutinee);
                arena.insert(Term::CaseTwo {
                    motive,
                    zero_branch,
                    one_branch,
                    scrutinee,
                })
            }
        }
    }

    pub(crate) fn v(name: &'static str) -> Syntax {
        Syntax::Variable(name)
    }

    pub(crate) fn sort(level: Level) -> Syntax {
        Syntax::Sort(Sort::Type(level))
    }

    pub(crate) fn ty(parameter: u32) -> Syntax {
        sort(Level::Parameter(parameter))
    }

    pub(crate) fn strict(parameter: u32) -> Syntax {
        Syntax::Sort(Sort::Strict(Level::Parameter(parameter)))
    }

    pub(crate) fn app(function: Syntax, argument: Syntax) -> Syntax {
        Syntax::Apply(Box::new(function), Box::new(argument))
    }

    pub(crate) fn apps(function: Syntax, arguments: impl IntoIterator<Item = Syntax>) -> Syntax {
        arguments.into_iter().fold(function, app)
    }

    pub(crate) fn pi(name: &'static str, domain: Syntax, codomain: Syntax) -> Syntax {
        Syntax::Pi(name, Box::new(domain), Box::new(codomain))
    }

    pub(crate) fn pis(
        binders: impl IntoIterator<Item = (&'static str, Syntax)>,
        body: Syntax,
    ) -> Syntax {
        binders
            .into_iter()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .fold(body, |inner, (name, domain)| pi(name, domain, inner))
    }

    pub(crate) fn lam(name: &'static str, domain: Syntax, body: Syntax) -> Syntax {
        Syntax::Lambda(name, Box::new(domain), Box::new(body))
    }

    pub(crate) fn sigma(name: &'static str, domain: Syntax, codomain: Syntax) -> Syntax {
        Syntax::Sigma(name, Box::new(domain), Box::new(codomain))
    }

    pub(crate) fn pair(first: Syntax, second: Syntax) -> Syntax {
        Syntax::Pair(Box::new(first), Box::new(second))
    }

    pub(crate) fn fst(pair: Syntax) -> Syntax {
        Syntax::Fst(Box::new(pair))
    }

    pub(crate) fn snd(pair: Syntax) -> Syntax {
        Syntax::Snd(Box::new(pair))
    }

    pub(crate) fn id(ty: Syntax, left: Syntax, right: Syntax) -> Syntax {
        Syntax::Id(Box::new(ty), Box::new(left), Box::new(right))
    }

    pub(crate) fn refl(ty: Syntax, value: Syntax) -> Syntax {
        Syntax::Refl(Box::new(ty), Box::new(value))
    }

    pub(crate) fn w(carrier: Syntax, children: Syntax) -> Syntax {
        Syntax::W(Box::new(carrier), Box::new(children))
    }

    pub(crate) fn sup(
        carrier: Syntax,
        children: Syntax,
        label: Syntax,
        function: Syntax,
    ) -> Syntax {
        Syntax::Sup(
            Box::new(carrier),
            Box::new(children),
            Box::new(label),
            Box::new(function),
        )
    }

    pub(crate) fn jelim(motive: Syntax, base: Syntax, endpoint: Syntax, proof: Syntax) -> Syntax {
        Syntax::IdElim(
            Box::new(motive),
            Box::new(base),
            Box::new(endpoint),
            Box::new(proof),
        )
    }

    pub(crate) fn indw(motive: Syntax, step: Syntax, tree: Syntax) -> Syntax {
        Syntax::IndW(Box::new(motive), Box::new(step), Box::new(tree))
    }

    pub(crate) fn squash(ty: Syntax) -> Syntax {
        Syntax::Squash(Box::new(ty))
    }

    pub(crate) fn squash_intro(ty: Syntax, value: Syntax) -> Syntax {
        Syntax::SquashIntro(Box::new(ty), Box::new(value))
    }

    pub(crate) fn boxed(ty: Syntax) -> Syntax {
        Syntax::Boxed(Box::new(ty))
    }

    pub(crate) fn box_intro(ty: Syntax, value: Syntax) -> Syntax {
        Syntax::BoxIntro(Box::new(ty), Box::new(value))
    }

    pub(crate) fn box_elim(motive: Syntax, body: Syntax, scrutinee: Syntax) -> Syntax {
        Syntax::BoxElim(Box::new(motive), Box::new(body), Box::new(scrutinee))
    }

    pub(crate) fn scheme_at(declaration: u32, levels: Vec<Level>) -> Syntax {
        Syntax::Scheme(declaration, levels)
    }

    pub(crate) fn two() -> Syntax {
        Syntax::Two
    }

    pub(crate) fn case_two(
        motive: Syntax,
        zero_branch: Syntax,
        one_branch: Syntax,
        scrutinee: Syntax,
    ) -> Syntax {
        Syntax::CaseTwo(
            Box::new(motive),
            Box::new(zero_branch),
            Box::new(one_branch),
            Box::new(scrutinee),
        )
    }
}
pub(crate) mod signature {
    use std::collections::{BTreeSet, HashSet};
    use std::rc::Rc;

    use super::conversion::Budget;
    use super::term::{Term, TermArena, TermHandle};
    use super::typing::{Context, CoreError, check_type, infer_sort};

    #[derive(Clone, Debug, PartialEq, Eq, compiler_storage::Record)]
    pub struct Declaration {
        pub level_arity: u32,
        pub ty: TermHandle,
        pub body: Option<TermHandle>,
    }

    impl Declaration {
        pub fn definition(level_arity: u32, ty: TermHandle, body: TermHandle) -> Self {
            Self {
                level_arity,
                ty,
                body: Some(body),
            }
        }

        pub fn assumption(level_arity: u32, ty: TermHandle) -> Self {
            Self {
                level_arity,
                ty,
                body: None,
            }
        }

        pub fn is_assumption(&self) -> bool {
            self.body.is_none()
        }
    }

    #[derive(Clone, Debug, Default)]
    pub struct Signature {
        declarations: Rc<Vec<Declaration>>,
    }

    impl Signature {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn from_declarations(declarations: Vec<Declaration>) -> Self {
            Self {
                declarations: Rc::new(declarations),
            }
        }

        pub fn get(&self, index: u32) -> Option<&Declaration> {
            self.declarations.get(usize::try_from(index).ok()?)
        }

        pub fn len(&self) -> usize {
            self.declarations.len()
        }

        pub fn is_empty(&self) -> bool {
            self.declarations.is_empty()
        }

        pub fn declarations(&self) -> &[Declaration] {
            &self.declarations
        }

        fn push(&mut self, declaration: Declaration) {
            Rc::make_mut(&mut self.declarations).push(declaration);
        }
    }

    #[derive(Debug)]
    pub struct SignatureCheckError {
        pub declaration: usize,
        pub error: CoreError,
    }

    pub fn check_signature(
        arena: &mut TermArena,
        declarations: &[Declaration],
        budget: &mut Budget,
    ) -> Result<Signature, CoreError> {
        check_signature_with_attribution(arena, declarations, budget)
            .map_err(|failure| failure.error)
    }

    pub fn check_signature_with_attribution(
        arena: &mut TermArena,
        declarations: &[Declaration],
        budget: &mut Budget,
    ) -> Result<Signature, SignatureCheckError> {
        let mut signature = Signature::new();
        for (position, declaration) in declarations.iter().enumerate() {
            let formed = {
                let context = Context::with_level_arity(declaration.level_arity)
                    .with_signature(signature.clone());
                (|| {
                    infer_sort(arena, &context, declaration.ty, budget)?;
                    if let Some(body) = declaration.body {
                        check_type(arena, &context, body, declaration.ty, budget)?;
                    }
                    Ok::<_, CoreError>(())
                })()
            };
            formed.map_err(|error| SignatureCheckError {
                declaration: position,
                error,
            })?;
            signature.push(declaration.clone());
        }
        Ok(signature)
    }

    struct TermReferenceWalk<'arena> {
        arena: &'arena TermArena,
        seen: HashSet<TermHandle>,
        pending: Vec<TermHandle>,
    }

    impl<'arena> TermReferenceWalk<'arena> {
        fn new(arena: &'arena TermArena) -> Self {
            Self {
                arena,
                seen: HashSet::new(),
                pending: Vec::new(),
            }
        }

        fn collect(&mut self, term: TermHandle, out: &mut BTreeSet<u32>) {
            self.pending.push(term);
            while let Some(term) = self.pending.pop() {
                if !self.seen.insert(term) {
                    continue;
                }
                let node = self.arena.node(term);
                if let Term::Constant { declaration, .. } = node {
                    out.insert(*declaration);
                } else {
                    self.pending
                        .extend(node.subterms().iter().rev().map(|(child, _)| child));
                }
            }
        }
    }

    fn closure_over(
        references: &mut TermReferenceWalk<'_>,
        signature: &Signature,
        seeds: BTreeSet<u32>,
    ) -> BTreeSet<u32> {
        let mut assumptions = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut worklist: Vec<u32> = seeds.into_iter().collect();
        while let Some(index) = worklist.pop() {
            if !visited.insert(index) {
                continue;
            }
            let Some(declaration) = signature.get(index) else {
                continue;
            };
            if declaration.is_assumption() {
                assumptions.insert(index);
            }
            let mut referenced = BTreeSet::new();
            references.collect(declaration.ty, &mut referenced);
            if let Some(body) = declaration.body {
                references.collect(body, &mut referenced);
            }
            worklist.extend(referenced);
        }
        assumptions
    }

    pub fn assumption_closure(
        arena: &TermArena,
        signature: &Signature,
        roots: &[u32],
    ) -> BTreeSet<u32> {
        let mut references = TermReferenceWalk::new(arena);
        closure_over(&mut references, signature, roots.iter().copied().collect())
    }

    pub fn judgment_assumption_closure(
        arena: &TermArena,
        signature: &Signature,
        terms: &[TermHandle],
    ) -> BTreeSet<u32> {
        let mut references = TermReferenceWalk::new(arena);
        let mut seeds = BTreeSet::new();
        for &term in terms {
            references.collect(term, &mut seeds);
        }
        closure_over(&mut references, signature, seeds)
    }
}
pub(crate) mod substitution {
    use std::collections::HashMap;

    use super::term::{Level, MAX_SUBTERMS, Sort, Term, TermArena, TermHandle, instantiate_level};

    pub fn shift(arena: &mut TermArena, term: TermHandle, cutoff: u32, amount: u32) -> TermHandle {
        if amount == 0 {
            return term;
        }
        remap(arena, term, cutoff, Remapping::Shift { amount })
            .expect("a term shift has no universe parameters to refuse")
    }

    pub fn substitute(arena: &mut TermArena, body: TermHandle, argument: TermHandle) -> TermHandle {
        remap(arena, body, 0, Remapping::Substitute { argument })
            .expect("a term substitution has no universe parameters to refuse")
    }

    pub fn instantiate_levels(
        arena: &mut TermArena,
        term: TermHandle,
        arguments: &[Level],
    ) -> Result<TermHandle, u32> {
        remap(arena, term, 0, Remapping::Levels { arguments })
    }

    /// The universe telescope needed by these terms, without opening declaration
    /// bodies. Constants retain their explicit level applications. This describes
    /// syntax only; the ordinary kernel judgment still checks every parameter.
    pub fn universe_parameter_arity(arena: &TermArena, roots: &[TermHandle]) -> u32 {
        let mut pending = roots.to_vec();
        let mut seen = Vec::<u64>::new();
        let mut arity = 0;
        while let Some(term) = pending.pop() {
            // Resolve the complete generational handle before using its dense
            // slot. A stale sibling cannot hide the current node behind a bit.
            // Dummy contributes no syntax; the kernel still rejects it wherever
            // this query's result is used to form a judgment.
            let node = arena.get(term);
            if matches!(node, Term::Dummy) {
                continue;
            }
            let position = term.arena_index() as usize;
            let word = position / 64;
            if word >= seen.len() {
                seen.resize(word + 1, 0);
            }
            let bit = 1u64 << (position % 64);
            if seen[word] & bit != 0 {
                continue;
            }
            seen[word] |= bit;
            match &node {
                Term::Sort(sort) => arity = arity.max(sort.level().parameter_arity()),
                Term::Constant { levels, .. } => {
                    for level in levels {
                        arity = arity.max(level.parameter_arity());
                    }
                }
                _ => {}
            }
            pending.extend(node.subterms().iter().map(|(child, _)| child));
        }
        arity
    }

    #[derive(Clone, Copy)]
    enum Remapping<'a> {
        Shift { amount: u32 },
        Substitute { argument: TermHandle },
        Levels { arguments: &'a [Level] },
    }

    impl Remapping<'_> {
        fn child_scope(self, scope: u32, binders: u32) -> u32 {
            match self {
                Self::Levels { .. } => 0,
                Self::Shift { .. } | Self::Substitute { .. } => scope
                    .checked_add(binders)
                    .expect("term binder depth overflow"),
            }
        }

        fn leaf(
            self,
            arena: &mut TermArena,
            term: TermHandle,
            scope: u32,
        ) -> Result<TermHandle, u32> {
            match (self, arena.node(term)) {
                (Self::Shift { amount }, Term::Variable(index)) if *index >= scope => {
                    let index = index.checked_add(amount).expect("variable shift overflow");
                    Ok(arena.insert(Term::Variable(index)))
                }
                (Self::Substitute { argument }, Term::Variable(index)) => match index.cmp(&scope) {
                    std::cmp::Ordering::Equal => Ok(shift(arena, argument, 0, scope)),
                    std::cmp::Ordering::Greater => {
                        let index = index - 1;
                        Ok(arena.insert(Term::Variable(index)))
                    }
                    std::cmp::Ordering::Less => Ok(term),
                },
                (Self::Levels { arguments }, Term::Sort(sort)) => {
                    let level = instantiate_level(&sort.level(), arguments)?;
                    let sort = match sort {
                        Sort::Type(_) => Sort::Type(level),
                        Sort::Strict(_) => Sort::Strict(level),
                    };
                    Ok(arena.insert(Term::Sort(sort)))
                }
                (
                    Self::Levels { arguments },
                    Term::Constant {
                        declaration,
                        levels,
                    },
                ) => {
                    let declaration = *declaration;
                    let levels = levels
                        .iter()
                        .map(|level| instantiate_level(level, arguments))
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(arena.insert(Term::Constant {
                        declaration,
                        levels,
                    }))
                }
                _ => Ok(term),
            }
        }
    }

    #[derive(Clone, Copy)]
    enum Work {
        Enter { term: TermHandle, scope: u32 },
        Finish { term: TermHandle, scope: u32 },
    }

    fn remap(
        arena: &mut TermArena,
        root: TermHandle,
        scope: u32,
        operation: Remapping<'_>,
    ) -> Result<TermHandle, u32> {
        let mut pending = vec![Work::Enter { term: root, scope }];
        let mut values = Vec::new();
        let mut mapped = HashMap::new();
        while let Some(work) = pending.pop() {
            match work {
                Work::Enter { term, scope } => {
                    if let Some(&result) = mapped.get(&(term, scope)) {
                        values.push(result);
                        continue;
                    }
                    let slots = arena.node(term).subterms();
                    if slots.is_empty() {
                        let result = operation.leaf(arena, term, scope)?;
                        mapped.insert((term, scope), result);
                        values.push(result);
                    } else {
                        pending.push(Work::Finish { term, scope });
                        pending.extend(slots.iter().rev().map(|(term, binders)| Work::Enter {
                            term,
                            scope: operation.child_scope(scope, binders),
                        }));
                    }
                }
                Work::Finish { term, scope } => {
                    let node = arena.node(term);
                    let slots = node.subterms();
                    let mut children = [term; MAX_SUBTERMS];
                    for index in (0..slots.len()).rev() {
                        children[index] = values.pop().expect("one result per child slot");
                    }
                    let changed = slots
                        .iter()
                        .zip(children)
                        .any(|((child, _), new)| child != new);
                    let result = if changed {
                        let rebuilt = node.with_children(&children[..slots.len()]);
                        arena.insert(rebuilt)
                    } else {
                        term
                    };
                    mapped.insert((term, scope), result);
                    values.push(result);
                }
            }
        }
        let result = values.pop().expect("the root has one remapped result");
        debug_assert!(values.is_empty());
        Ok(result)
    }
}
pub(crate) mod term {
    use std::collections::HashSet;

    use arena::Arena;

    #[derive(compiler_storage::Record)]
    pub enum Level {
        Constant(u32),
        Parameter(u32),
        Successor(Box<Level>),
        Maximum(Box<Level>, Box<Level>),
    }

    mod levels {
        use super::Level;
        use std::collections::BTreeMap;
        use std::fmt;
        use std::hash::{Hash, Hasher};

        impl Level {
            /// Required parameter prefix; this does not validate a universe scope.
            pub fn parameter_arity(&self) -> u32 {
                let mut pending = vec![self];
                let mut arity = 0;
                while let Some(level) = pending.pop() {
                    match level {
                        Level::Parameter(index) => arity = arity.max(index.saturating_add(1)),
                        Level::Successor(inner) => pending.push(inner),
                        Level::Maximum(left, right) => {
                            pending.push(left);
                            pending.push(right);
                        }
                        Level::Constant(_) => {}
                    }
                }
                arity
            }

            /// Substitute exact universe actuals without resizing or cumulativity.
            pub fn instantiate(&self, arguments: &[Level]) -> Result<Level, u32> {
                instantiate_level(self, arguments)
            }

            /// Closed constants fold, with the original checked overflow boundary.
            pub fn successor(self) -> Option<Level> {
                match self {
                    Level::Constant(value) => value.checked_add(1).map(Level::Constant),
                    _ => Some(Level::Successor(Box::new(self))),
                }
            }

            pub fn maximum(self, other: Level) -> Level {
                match (self, other) {
                    (Level::Constant(left), Level::Constant(right)) => {
                        Level::Constant(left.max(right))
                    }
                    (left, right) => Level::Maximum(Box::new(left), Box::new(right)),
                }
            }
        }

        fn map_parameters<E>(
            root: &Level,
            parameter: &mut impl FnMut(u32) -> Result<Level, E>,
        ) -> Result<Level, E> {
            match root {
                Level::Constant(value) => return Ok(Level::Constant(*value)),
                Level::Parameter(index) => return parameter(*index),
                Level::Successor(_) | Level::Maximum(_, _) => {}
            }
            enum Work<'a> {
                Visit(&'a Level),
                Successor,
                Maximum,
            }
            let mut pending = vec![Work::Visit(root)];
            let mut values = Vec::new();
            while let Some(work) = pending.pop() {
                match work {
                    Work::Visit(level) => match level {
                        Level::Constant(value) => values.push(Level::Constant(*value)),
                        Level::Parameter(index) => values.push(parameter(*index)?),
                        Level::Successor(inner) => {
                            pending.push(Work::Successor);
                            pending.push(Work::Visit(inner));
                        }
                        Level::Maximum(left, right) => {
                            pending.push(Work::Maximum);
                            pending.push(Work::Visit(right));
                            pending.push(Work::Visit(left));
                        }
                    },
                    Work::Successor => {
                        let inner = values.pop().expect("successor has one rebuilt child");
                        values.push(Level::Successor(Box::new(inner)));
                    }
                    Work::Maximum => {
                        let right = values.pop().expect("maximum has a right child");
                        let left = values.pop().expect("maximum has a left child");
                        values.push(Level::Maximum(Box::new(left), Box::new(right)));
                    }
                }
            }
            debug_assert_eq!(values.len(), 1);
            Ok(values.pop().expect("a level has one rebuilt root"))
        }

        impl Clone for Level {
            fn clone(&self) -> Self {
                map_parameters(self, &mut |index| {
                    Ok::<_, std::convert::Infallible>(Level::Parameter(index))
                })
                .expect("copying level syntax cannot refuse")
            }
        }

        impl PartialEq for Level {
            fn eq(&self, other: &Self) -> bool {
                let mut current = (self, other);
                let mut pending: Vec<(&Level, &Level)> = Vec::new();
                loop {
                    match current {
                        (Level::Constant(left), Level::Constant(right))
                        | (Level::Parameter(left), Level::Parameter(right)) => {
                            if left != right {
                                return false;
                            }
                        }
                        (Level::Successor(left), Level::Successor(right)) => {
                            current = (left, right);
                            continue;
                        }
                        (Level::Maximum(ll, lr), Level::Maximum(rl, rr)) => {
                            pending.push((lr, rr));
                            current = (ll, rl);
                            continue;
                        }
                        _ => return false,
                    }
                    match pending.pop() {
                        Some(next) => current = next,
                        None => return true,
                    }
                }
            }
        }
        impl Eq for Level {}

        impl Hash for Level {
            fn hash<H: Hasher>(&self, state: &mut H) {
                let mut current = self;
                let mut pending: Vec<&Level> = Vec::new();
                loop {
                    std::mem::discriminant(current).hash(state);
                    match current {
                        Level::Constant(value) | Level::Parameter(value) => value.hash(state),
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
                        None => return,
                    }
                }
            }
        }

        impl fmt::Debug for Level {
            fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Level::Constant(value) => {
                        return output.debug_tuple("Constant").field(value).finish();
                    }
                    Level::Parameter(index) => {
                        return output.debug_tuple("Parameter").field(index).finish();
                    }
                    Level::Successor(_) | Level::Maximum(_, _) => {}
                }
                enum Work<'a> {
                    Visit(&'a Level, usize),
                    Text(&'static str),
                    Indent(usize),
                }
                let pretty = output.alternate();
                let mut pending = vec![Work::Visit(self, 0)];
                while let Some(work) = pending.pop() {
                    match work {
                        Work::Text(text) => output.write_str(text)?,
                        Work::Indent(count) => {
                            for _ in 0..count {
                                output.write_str(" ")?;
                            }
                        }
                        Work::Visit(level, indent) => {
                            let name = match level {
                                Level::Constant(_) => "Constant",
                                Level::Parameter(_) => "Parameter",
                                Level::Successor(_) => "Successor",
                                Level::Maximum(_, _) => "Maximum",
                            };
                            write!(output, "{name}(")?;
                            if pretty {
                                output.write_str("\n")?;
                            }
                            pending.push(Work::Text(")"));
                            if pretty {
                                pending.push(Work::Indent(indent));
                            }
                            match level {
                                Level::Constant(value) | Level::Parameter(value) => {
                                    if pretty {
                                        for _ in 0..indent + 4 {
                                            output.write_str(" ")?;
                                        }
                                    }
                                    write!(output, "{value:?}")?;
                                    if pretty {
                                        output.write_str(",\n")?;
                                    }
                                }
                                Level::Successor(inner) => {
                                    if pretty {
                                        pending.push(Work::Text(",\n"));
                                    }
                                    pending.push(Work::Visit(inner, indent + 4));
                                    if pretty {
                                        pending.push(Work::Indent(indent + 4));
                                    }
                                }
                                Level::Maximum(left, right) => {
                                    if pretty {
                                        pending.push(Work::Text(",\n"));
                                    }
                                    pending.push(Work::Visit(right, indent + 4));
                                    if pretty {
                                        pending.push(Work::Indent(indent + 4));
                                    }
                                    pending.push(Work::Text(if pretty { ",\n" } else { ", " }));
                                    pending.push(Work::Visit(left, indent + 4));
                                    if pretty {
                                        pending.push(Work::Indent(indent + 4));
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
        }

        impl Drop for Level {
            fn drop(&mut self) {
                fn detach(level: &mut Level, pending: &mut Vec<Level>) {
                    match level {
                        Level::Successor(inner) => {
                            pending.push(*std::mem::replace(inner, Box::new(Level::Constant(0))))
                        }
                        Level::Maximum(left, right) => {
                            pending.push(*std::mem::replace(right, Box::new(Level::Constant(0))));
                            pending.push(*std::mem::replace(left, Box::new(Level::Constant(0))));
                        }
                        Level::Constant(_) | Level::Parameter(_) => {}
                    }
                }
                let mut pending = Vec::new();
                detach(self, &mut pending);
                while let Some(mut level) = pending.pop() {
                    detach(&mut level, &mut pending);
                }
            }
        }

        #[derive(PartialEq, Eq)]
        struct NormalLevel {
            constant: u64,
            offsets: BTreeMap<u32, u64>,
        }
        fn normal_form(root: &Level) -> NormalLevel {
            let mut form = NormalLevel {
                constant: 0,
                offsets: BTreeMap::new(),
            };
            let mut pending: Vec<(&Level, u64)> = Vec::new();
            let mut current = (root, 0u64);
            loop {
                let (level, offset) = current;
                match level {
                    Level::Constant(value) => {
                        form.constant = form.constant.max(u64::from(*value) + offset)
                    }
                    Level::Parameter(index) => {
                        form.constant = form.constant.max(offset);
                        form.offsets
                            .entry(*index)
                            .and_modify(|old| *old = (*old).max(offset))
                            .or_insert(offset);
                    }
                    Level::Successor(inner) => {
                        current = (inner, offset + 1);
                        continue;
                    }
                    Level::Maximum(left, right) => {
                        pending.push((right, offset));
                        current = (left, offset);
                        continue;
                    }
                }
                match pending.pop() {
                    Some(next) => current = next,
                    None => break,
                }
            }
            if form
                .offsets
                .values()
                .max()
                .is_some_and(|offset| *offset >= form.constant)
            {
                form.constant = 0;
            }
            form
        }

        pub(crate) fn levels_equal(left: &Level, right: &Level) -> bool {
            normal_form(left) == normal_form(right)
        }

        pub(crate) fn instantiate_level(level: &Level, arguments: &[Level]) -> Result<Level, u32> {
            map_parameters(level, &mut |index| {
                arguments
                    .get(usize::try_from(index).unwrap_or(usize::MAX))
                    .cloned()
                    .ok_or(index)
            })
        }
    }
    pub(crate) use levels::{instantiate_level, levels_equal};

    pub(crate) fn sorts_equal(left: &Sort, right: &Sort) -> bool {
        match (left, right) {
            (Sort::Type(left), Sort::Type(right)) | (Sort::Strict(left), Sort::Strict(right)) => {
                levels_equal(left, right)
            }
            _ => false,
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub enum Sort {
        Type(Level),
        Strict(Level),
    }

    impl Sort {
        pub fn level(&self) -> Level {
            match self {
                Sort::Type(level) | Sort::Strict(level) => level.clone(),
            }
        }

        pub fn is_strict(&self) -> bool {
            matches!(self, Sort::Strict(_))
        }
    }

    pub type TermHandle = arena::Handle<Term>;

    #[derive(Clone, Debug, PartialEq, Eq, Hash, Default, compiler_storage::Record)]
    pub enum Term {
        #[default]
        Dummy,
        Variable(u32),
        Sort(Sort),
        Pi {
            domain: TermHandle,
            codomain: TermHandle,
        },
        Lambda {
            domain: TermHandle,
            body: TermHandle,
        },
        Apply {
            function: TermHandle,
            argument: TermHandle,
        },
        Sigma {
            domain: TermHandle,
            codomain: TermHandle,
        },
        Pair {
            first: TermHandle,
            second: TermHandle,
        },
        Fst {
            pair: TermHandle,
        },
        Snd {
            pair: TermHandle,
        },
        Two,
        TwoZero,
        TwoOne,
        CaseTwo {
            motive: TermHandle,
            zero_branch: TermHandle,
            one_branch: TermHandle,
            scrutinee: TermHandle,
        },
        Id {
            ty: TermHandle,
            left: TermHandle,
            right: TermHandle,
        },
        Refl {
            ty: TermHandle,
            value: TermHandle,
        },
        IdElim {
            motive: TermHandle,
            base: TermHandle,
            endpoint: TermHandle,
            proof: TermHandle,
        },
        W {
            carrier: TermHandle,
            children: TermHandle,
        },
        Sup {
            carrier: TermHandle,
            children: TermHandle,
            label: TermHandle,
            function: TermHandle,
        },
        IndW {
            motive: TermHandle,
            step: TermHandle,
            tree: TermHandle,
        },
        Empty,
        EmptyElim {
            ty: TermHandle,
            scrutinee: TermHandle,
        },
        Squash {
            ty: TermHandle,
        },
        SquashIntro {
            ty: TermHandle,
            value: TermHandle,
        },
        SquashElim {
            proposition: TermHandle,
            function: TermHandle,
            scrutinee: TermHandle,
        },
        Box {
            ty: TermHandle,
        },
        BoxIntro {
            ty: TermHandle,
            value: TermHandle,
        },
        BoxElim {
            motive: TermHandle,
            body: TermHandle,
            scrutinee: TermHandle,
        },
        Constant {
            declaration: u32,
            levels: Vec<Level>,
        },
    }

    pub(crate) const MAX_SUBTERMS: usize = 4;

    pub(crate) struct Subterms {
        slots: [Option<(TermHandle, u32)>; MAX_SUBTERMS],
        count: usize,
    }

    impl Subterms {
        fn new() -> Self {
            Self {
                slots: [None; MAX_SUBTERMS],
                count: 0,
            }
        }

        fn extend(&mut self, children: impl IntoIterator<Item = (TermHandle, u32)>) {
            for (handle, binders) in children {
                self.slots[self.count] = Some((handle, binders));
                self.count += 1;
            }
        }

        pub(crate) fn is_empty(&self) -> bool {
            self.count == 0
        }

        pub(crate) fn len(&self) -> usize {
            self.count
        }

        pub(crate) fn iter(
            &self,
        ) -> impl DoubleEndedIterator<Item = (TermHandle, u32)> + ExactSizeIterator + '_ {
            self.slots[..self.count]
                .iter()
                .map(|slot| slot.expect("one initialized term slot"))
        }
    }

    macro_rules! term_layout {
    ($($variant:ident { $($field:ident : $binders:expr),* $(,)? })+) => {
        impl Term {
            pub(crate) fn subterms(&self) -> Subterms {
                let mut slots = Subterms::new();
                match self {
                    $(Term::$variant { $($field),* } => slots.extend([$((*$field, $binders)),*]),)+
                    Term::Dummy
                    | Term::Variable(_)
                    | Term::Sort(_)
                    | Term::Two
                    | Term::TwoZero
                    | Term::TwoOne
                    | Term::Empty
                    | Term::Constant { .. } => {}
                }
                slots
            }

            fn payloads_equal(&self, other: &Term) -> bool {
                match (self, other) {
                    $((Term::$variant { .. }, Term::$variant { .. }) => true,)+
                    (Term::Variable(left), Term::Variable(right)) => left == right,
                    (Term::Sort(left), Term::Sort(right)) => left == right,
                    (Term::Constant { declaration: left, levels: left_levels },
                     Term::Constant { declaration: right, levels: right_levels }) => {
                        left == right && left_levels == right_levels
                    }
                    (Term::Two, Term::Two)
                    | (Term::TwoZero, Term::TwoZero)
                    | (Term::TwoOne, Term::TwoOne)
                    | (Term::Empty, Term::Empty) => true,
                    _ => false,
                }
            }

            pub(crate) fn with_children(&self, children: &[TermHandle]) -> Term {
                debug_assert_eq!(
                    children.len(),
                    self.subterms().len(),
                    "every slot of a term takes exactly one child"
                );
                let mut next = children.iter().copied();
                match self {
                    $(Term::$variant { .. } => Term::$variant {
                        $($field: next.next().expect("one child per term slot")),*
                    },)+
                    Term::Dummy
                    | Term::Variable(_)
                    | Term::Sort(_)
                    | Term::Two
                    | Term::TwoZero
                    | Term::TwoOne
                    | Term::Empty
                    | Term::Constant { .. } => self.clone(),
                }
            }
        }
    };
    }

    term_layout! {
        Pi { domain: 0, codomain: 1 }
        Lambda { domain: 0, body: 1 }
        Apply { function: 0, argument: 0 }
        Sigma { domain: 0, codomain: 1 }
        Pair { first: 0, second: 0 }
        Fst { pair: 0 }
        Snd { pair: 0 }
        CaseTwo { motive: 0, zero_branch: 0, one_branch: 0, scrutinee: 0 }
        Id { ty: 0, left: 0, right: 0 }
        Refl { ty: 0, value: 0 }
        IdElim { motive: 0, base: 0, endpoint: 0, proof: 0 }
        W { carrier: 0, children: 0 }
        Sup { carrier: 0, children: 0, label: 0, function: 0 }
        IndW { motive: 0, step: 0, tree: 0 }
        EmptyElim { ty: 0, scrutinee: 0 }
        Squash { ty: 0 }
        SquashIntro { ty: 0, value: 0 }
        SquashElim { proposition: 0, function: 0, scrutinee: 0 }
        Box { ty: 0 }
        BoxIntro { ty: 0, value: 0 }
        BoxElim { motive: 0, body: 0, scrutinee: 0 }
    }
    #[derive(Clone, Debug, PartialEq, Eq, compiler_storage::Record)]
    pub struct TermArena {
        terms: Arena<Term>,
    }

    impl TermArena {
        pub fn new() -> Self {
            Self {
                terms: Arena::new(),
            }
        }

        pub fn insert(&mut self, term: Term) -> TermHandle {
            self.terms.insert(term)
        }

        pub fn apply_arguments(
            &mut self,
            function: TermHandle,
            arguments: &[TermHandle],
        ) -> TermHandle {
            let mut spine = function;
            for &argument in arguments {
                spine = self.insert(Term::Apply {
                    function: spine,
                    argument,
                });
            }
            spine
        }

        pub fn apply_binary(
            &mut self,
            relation: TermHandle,
            left: TermHandle,
            right: TermHandle,
        ) -> TermHandle {
            self.apply_arguments(relation, &[left, right])
        }

        pub fn get(&self, handle: TermHandle) -> Term {
            self.node(handle).clone()
        }

        pub(crate) fn node(&self, handle: TermHandle) -> &Term {
            self.terms.get(handle)
        }

        pub fn len(&self) -> usize {
            self.terms.len()
        }

        pub fn is_empty(&self) -> bool {
            self.terms.is_empty()
        }

        pub fn structurally_equal(&self, left: TermHandle, right: TermHandle) -> bool {
            self.structurally_equal_shifted(left, right, 0)
        }

        /// Compare a term with the same value carried under additional binders.
        /// Locally bound variables keep their indices; only free variables cross
        /// the inserted telescope. This is structural correspondence, not a new
        /// conversion or equality rule, and allocates no shifted term tree.
        pub fn structurally_equal_shifted(
            &self,
            source: TermHandle,
            shifted: TermHandle,
            inserted_binders: u32,
        ) -> bool {
            let mut pending = vec![(false, (source, shifted, 0u32))];
            let mut active = HashSet::new();
            let mut complete = HashSet::new();
            while let Some((finish, pair)) = pending.pop() {
                if finish {
                    active.remove(&(pair.0, pair.1));
                    complete.insert(pair);
                    continue;
                }
                if complete.contains(&pair) {
                    continue;
                }
                // The local depth belongs to the key: a shared node may occur
                // both under its own binder and in the surrounding telescope.
                if !active.insert((pair.0, pair.1)) {
                    return false;
                }
                let left = self.node(pair.0);
                let right = self.node(pair.1);
                let payloads_equal = match (left, right) {
                    (Term::Variable(left), Term::Variable(right)) if *left >= pair.2 => {
                        left.checked_add(inserted_binders) == Some(*right)
                    }
                    _ => left.payloads_equal(right),
                };
                if !payloads_equal {
                    return false;
                }
                pending.push((true, pair));
                for ((left, binders), (right, _)) in
                    left.subterms().iter().zip(right.subterms().iter()).rev()
                {
                    let depth = if inserted_binders == 0 {
                        0
                    } else {
                        let Some(depth) = pair.2.checked_add(binders) else {
                            return false;
                        };
                        depth
                    };
                    pending.push((false, (left, right, depth)));
                }
            }
            true
        }
    }

    impl Default for TermArena {
        fn default() -> Self {
            Self::new()
        }
    }

    #[cfg(test)]
    mod shifted_correspondence_tests {
        use super::{Term, TermArena};

        #[test]
        fn shifted_comparison_matches_capture_avoiding_shift_on_shared_scopes() {
            let mut arena = TermArena::new();
            let two = arena.insert(Term::Two);
            let zero = arena.insert(Term::Variable(0));
            let one = arena.insert(Term::Variable(1));
            let relation = arena.insert(Term::Id {
                ty: two,
                left: zero,
                right: one,
            });
            let lambda = arena.insert(Term::Lambda {
                domain: two,
                body: relation,
            });
            let source = arena.insert(Term::Sigma {
                domain: relation,
                codomain: lambda,
            });
            for amount in [0, 1, 3] {
                let shifted = crate::shift(&mut arena, source, 0, amount);
                assert!(arena.structurally_equal_shifted(source, shifted, amount));
                assert!(!arena.structurally_equal_shifted(source, shifted, amount + 1));
            }
        }

        #[test]
        fn a_binder_cycle_is_not_correspondence_at_any_offset() {
            let mut arena = TermArena::new();
            let two = arena.insert(Term::Two);
            let cyclic = arena.insert(Term::Dummy);
            *arena.terms.get_mut(cyclic) = Term::Lambda {
                domain: two,
                body: cyclic,
            };
            for amount in [0, 1] {
                assert!(!arena.structurally_equal_shifted(cyclic, cyclic, amount));
            }
        }
    }
}
pub(crate) mod theorems {
    use super::scheme_dsl::{app, define, id, jelim, lam, pi, ty, v};
    use super::signature::Declaration;
    use super::term::TermArena;

    pub fn identity_substitution(arena: &mut TermArena) -> Declaration {
        let motive = lam(
            "y2",
            v("A"),
            lam("_", id(v("A"), v("x"), v("y2")), app(v("P"), v("y2"))),
        );
        let statement = pi(
            "A",
            ty(0),
            pi(
                "P",
                pi("_", v("A"), ty(1)),
                pi(
                    "x",
                    v("A"),
                    pi(
                        "y",
                        v("A"),
                        pi(
                            "_",
                            id(v("A"), v("x"), v("y")),
                            pi("_", app(v("P"), v("x")), app(v("P"), v("y"))),
                        ),
                    ),
                ),
            ),
        );
        let body = lam(
            "A",
            ty(0),
            lam(
                "P",
                pi("_", v("A"), ty(1)),
                lam(
                    "x",
                    v("A"),
                    lam(
                        "y",
                        v("A"),
                        lam(
                            "p",
                            id(v("A"), v("x"), v("y")),
                            lam(
                                "h",
                                app(v("P"), v("x")),
                                jelim(motive, v("h"), v("y"), v("p")),
                            ),
                        ),
                    ),
                ),
            ),
        );
        define(arena, 2, &statement, &body)
    }
}
pub(crate) use crate::mathematical_typing as typing;

#[cfg(test)]
pub(crate) use bounded_denotation::test_utilities::{assert_refused, verify_and_assert_forged};
pub(crate) use bounded_denotation::verify_bounded_certificate_with_machine_parameters;
pub use bounded_denotation::{
    BoundedDenotation, BoundedDenotationError, SourceContentTheory, denote_bounded_certificate,
    verify_bounded_certificate,
};
#[cfg(test)]
pub(crate) use bounded_observations::bounded_verification_entries;
pub use certificate::{
    MathematicalCertificate, MathematicalCertificateBatchError, certificate_assumption_closure,
    verify_mathematical_certificate, verify_mathematical_certificates,
};
pub use conversion::Budget;
pub use conversion::{
    DEFAULT_CONVERSION_STEPS, convertible, convertible_retaining, weak_head_normalize,
    weak_head_normalize_retaining,
};
pub use indexed::{
    INDEXED_AT, INDEXED_IND, INDEXED_PACK, INDEXED_SUP, INDEXED_W, IndexedFamily,
    indexed_correctness, indexed_scheme,
};
pub use quotient::{
    QUOTIENT, QUOTIENT_BETA, QUOTIENT_BOX_PROP, QUOTIENT_COVERAGE, QUOTIENT_EFFECTIVE,
    QUOTIENT_ELIM, QUOTIENT_ID_CANCEL, QUOTIENT_ID_SYM, QUOTIENT_ID_TRANS, QUOTIENT_IND_PROP,
    QUOTIENT_IS_SET, QUOTIENT_LIFT, QUOTIENT_LIFT_PRECONDITION, QUOTIENT_PROJECT,
    QUOTIENT_PROP_IS_SET, QUOTIENT_SET, QUOTIENT_SOUND, QUOTIENT_TRANSPORT,
    QUOTIENT_TRANSPORT_CONST, QUOTIENT_UNIQUE, QuotientFamily, quotient_scheme,
};
pub use signature::{
    Declaration, Signature, SignatureCheckError, assumption_closure, check_signature,
    check_signature_with_attribution, judgment_assumption_closure,
};
pub use substitution::{instantiate_levels, shift, substitute, universe_parameter_arity};
pub use term::{Level, Sort, Term, TermArena, TermHandle};
pub use theorems::identity_substitution;
pub use typing::{Context, CoreError, check_type, infer_sort, infer_type};

#[cfg(test)]
mod tests {
    mod assumption_closure {
        use super::{constant, default_budget, pi, two, type_sort};
        use crate::mathematical_core::{
            Declaration, MathematicalCertificate, Term, TermArena, bounded_verification_entries,
            certificate_assumption_closure, check_signature, judgment_assumption_closure,
            verify_mathematical_certificate,
        };
        use std::collections::BTreeSet;

        #[test]
        fn certificate_assumption_closure_stays_on_callers_stack() {
            std::thread::spawn(|| {
                let mut arena = TermArena::new();
                let universe = type_sort(&mut arena, 0);
                let abstract_type = constant(&mut arena, 0, Vec::new());
                let signature = vec![
                    Declaration::assumption(0, universe),
                    Declaration::assumption(0, universe),
                    Declaration::definition(0, universe, abstract_type),
                ];
                let mut term = constant(&mut arena, 2, Vec::new());
                let domain = two(&mut arena);
                for _ in 0..131_072 {
                    term = pi(&mut arena, domain, term);
                }
                let certificate =
                    MathematicalCertificate::with_signature(signature, Vec::new(), term, universe);
                verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget())
                    .expect("deep closed telescope has its declared universe");
                let workers = bounded_verification_entries();
                assert_eq!(
                    certificate_assumption_closure(&arena, &certificate),
                    BTreeSet::from([0]),
                    "the definition's body contributes its axiom; unused declarations do not"
                );
                assert_eq!(
                    bounded_verification_entries(),
                    workers,
                    "assumption collection does not re-enter bounded admission or denotation"
                );
            })
            .join()
            .expect("default-stack assumption traversal");
        }

        #[test]
        fn shared_term_graph_retains_exact_signature_dependencies() {
            std::thread::spawn(|| {
                let mut arena = TermArena::new();
                let universe = type_sort(&mut arena, 0);
                let first = constant(&mut arena, 0, Vec::new());
                let second = constant(&mut arena, 1, Vec::new());
                let alias = constant(&mut arena, 2, Vec::new());
                let mut graph = alias;
                for _ in 0..131_072 {
                    graph = arena.insert(Term::Pi {
                        domain: graph,
                        codomain: graph,
                    });
                }
                for (body, expected) in [(first, 0), (second, 1)] {
                    let declarations = [
                        Declaration::assumption(0, universe),
                        Declaration::assumption(0, universe),
                        Declaration::definition(0, universe, body),
                    ];
                    let signature =
                        check_signature(&mut arena, &declarations, &mut default_budget())
                            .expect("each independent signature is well formed");
                    assert_eq!(
                        judgment_assumption_closure(&arena, &signature, &[graph, alias, graph]),
                        BTreeSet::from([expected]),
                        "equal term handles cannot reuse another signature's assumption closure"
                    );
                }
            })
            .join()
            .expect("default-stack shared graph traversal");
        }
    }
    mod identity_and_w_types {
        use super::{
            apply, concrete_step, default_budget, id, id_elim, ind_w, lambda, pi, refl,
            strict_sort, sup, symmetry_motive, two, two_one, two_zero, type_sort, variable,
            w_context, w_type,
        };
        use crate::mathematical_core::{
            Budget, Context, CoreError, Signature, TermArena, check_type, convertible, infer_type,
            weak_head_normalize,
        };

        macro_rules! conv {
            ($a:expr => $b:expr => $c:expr => $d:expr => $e:expr => $f:expr) => {
                convertible($a, $b, $c, $d, $e, $f)
            };
        }

        #[test]
        fn identity_forms_at_the_carrier_level_and_refl_introduces() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let type_zero = type_sort(&mut arena, 0);
            let a_binding = variable(&mut arena, 0);
            let b_binding = variable(&mut arena, 1);
            let context = Context::empty()
                .extend(type_zero)
                .extend(a_binding)
                .extend(b_binding);

            let carrier = variable(&mut arena, 2);
            let a = variable(&mut arena, 1);
            let b = variable(&mut arena, 0);
            let identity = id(&mut arena, carrier, a, b);
            let inferred = infer_type(&mut arena, &context, identity, &mut budget).unwrap();
            let expected = type_sort(&mut arena, 0);
            assert!(arena.structurally_equal(inferred, expected));

            let type_zero = type_sort(&mut arena, 0);
            let other_binding = type_sort(&mut arena, 0);
            let type_context = Context::empty().extend(type_zero).extend(other_binding);
            let type_carrier = type_sort(&mut arena, 0);
            let a_type = variable(&mut arena, 1);
            let b_type = variable(&mut arena, 0);
            let over_types = id(&mut arena, type_carrier, a_type, b_type);
            let inferred = infer_type(&mut arena, &type_context, over_types, &mut budget).unwrap();
            let expected = type_sort(&mut arena, 1);
            assert!(arena.structurally_equal(inferred, expected));

            let carrier = variable(&mut arena, 2);
            let a = variable(&mut arena, 1);
            let reflexive = refl(&mut arena, carrier, a);
            let inferred = infer_type(&mut arena, &context, reflexive, &mut budget).unwrap();
            let carrier = variable(&mut arena, 2);
            let a = variable(&mut arena, 1);
            let expected = id(&mut arena, carrier, a, a);
            assert!(arena.structurally_equal(inferred, expected));
            check_type(&mut arena, &context, reflexive, expected, &mut budget).unwrap();

            let carrier = variable(&mut arena, 2);
            let a = variable(&mut arena, 1);
            let b = variable(&mut arena, 0);
            let off_endpoint = id(&mut arena, carrier, a, b);
            let carrier = variable(&mut arena, 2);
            let a = variable(&mut arena, 1);
            let reflexive = refl(&mut arena, carrier, a);
            let error =
                check_type(&mut arena, &context, reflexive, off_endpoint, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));

            let strict_zero = strict_sort(&mut arena, 0);
            let p_binding = variable(&mut arena, 0);
            let strict_context = Context::empty().extend(strict_zero).extend(p_binding);
            let strict_carrier = variable(&mut arena, 1);
            let p = variable(&mut arena, 0);
            let strict_identity = id(&mut arena, strict_carrier, p, p);
            let error =
                infer_type(&mut arena, &strict_context, strict_identity, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::StrictIdentityDomain { .. }));
            let strict_carrier = variable(&mut arena, 1);
            let p = variable(&mut arena, 0);
            let strict_refl = refl(&mut arena, strict_carrier, p);
            let error =
                infer_type(&mut arena, &strict_context, strict_refl, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::StrictIdentityDomain { .. }));

            let a = variable(&mut arena, 1);
            let b = variable(&mut arena, 0);
            let not_a_type = id(&mut arena, a, b, b);
            let error = infer_type(&mut arena, &context, not_a_type, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::NotASort { .. }));

            let strict_zero = strict_sort(&mut arena, 0);
            let type_zero = type_sort(&mut arena, 0);
            let a_binding = variable(&mut arena, 1);
            let p_binding = variable(&mut arena, 2);
            let context = Context::empty()
                .extend(strict_zero)
                .extend(type_zero)
                .extend(a_binding)
                .extend(p_binding);
            let carrier = variable(&mut arena, 2);
            let a = variable(&mut arena, 1);
            let p = variable(&mut arena, 0);
            let bad_endpoint = id(&mut arena, carrier, a, p);
            let error = infer_type(&mut arena, &context, bad_endpoint, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));
        }

        #[test]
        fn identity_elimination_computes_on_refl_and_respects_the_ceiling() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let type_zero = type_sort(&mut arena, 0);
            let x_binding = variable(&mut arena, 0);
            let context = Context::empty().extend(type_zero).extend(x_binding);

            let motive = symmetry_motive(&mut arena, 1, 0);
            let base = {
                let ty = variable(&mut arena, 1);
                let x = variable(&mut arena, 0);
                refl(&mut arena, ty, x)
            };
            let endpoint = variable(&mut arena, 0);
            let proof = {
                let ty = variable(&mut arena, 1);
                let x = variable(&mut arena, 0);
                refl(&mut arena, ty, x)
            };
            let elimination = id_elim(&mut arena, motive, base, endpoint, proof);

            let ty = variable(&mut arena, 1);
            let x = variable(&mut arena, 0);
            let expected = id(&mut arena, ty, x, x);
            check_type(&mut arena, &context, elimination, expected, &mut budget).unwrap();

            let normalized =
                weak_head_normalize(&mut arena, &Signature::new(), elimination, &mut budget)
                    .unwrap();
            assert_eq!(normalized, base);

            let mut empty_budget = Budget::new(0);
            let error = weak_head_normalize(
                &mut arena,
                &Signature::new(),
                elimination,
                &mut empty_budget,
            )
            .unwrap_err();
            assert_eq!(error, CoreError::StepCeiling);

            let result_type = {
                let motive = symmetry_motive(&mut arena, 1, 0);
                let at_fixed_argument = variable(&mut arena, 0);
                let at_fixed = apply(&mut arena, motive, at_fixed_argument);
                let ty = variable(&mut arena, 1);
                let x = variable(&mut arena, 0);
                let refl_proof = refl(&mut arena, ty, x);
                apply(&mut arena, at_fixed, refl_proof)
            };
            let same_refl = {
                let ty = variable(&mut arena, 1);
                let x = variable(&mut arena, 0);
                refl(&mut arena, ty, x)
            };
            assert!(
        conv!(&mut arena => &context => elimination => same_refl => result_type => &mut budget)
            .unwrap()
            );
        }

        #[test]
        fn stuck_identity_eliminations_compare_componentwise() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let type_zero = type_sort(&mut arena, 0);
            let x_binding = variable(&mut arena, 0);
            let y_binding = variable(&mut arena, 1);
            let p_binding = {
                let ty = variable(&mut arena, 2);
                let x = variable(&mut arena, 1);
                let y = variable(&mut arena, 0);
                id(&mut arena, ty, x, y)
            };
            let q_binding = {
                let ty = variable(&mut arena, 3);
                let x = variable(&mut arena, 2);
                let y = variable(&mut arena, 1);
                id(&mut arena, ty, x, y)
            };
            let context = Context::empty()
                .extend(type_zero)
                .extend(x_binding)
                .extend(y_binding)
                .extend(p_binding)
                .extend(q_binding);

            let motive = {
                let domain = variable(&mut arena, 4);
                let inner_domain = {
                    let ty = variable(&mut arena, 5);
                    let x = variable(&mut arena, 4);
                    let bound = variable(&mut arena, 0);
                    id(&mut arena, ty, x, bound)
                };
                let body = type_sort(&mut arena, 0);
                let inner = lambda(&mut arena, inner_domain, body);
                lambda(&mut arena, domain, inner)
            };
            let two_base = two(&mut arena);
            let function_base = {
                let domain = two(&mut arena);
                let codomain = two(&mut arena);
                pi(&mut arena, domain, codomain)
            };

            let endpoint = variable(&mut arena, 2);
            let p = variable(&mut arena, 1);
            let left = id_elim(&mut arena, motive, two_base, endpoint, p);
            let endpoint = variable(&mut arena, 2);
            let p = variable(&mut arena, 1);
            let left_again = id_elim(&mut arena, motive, two_base, endpoint, p);
            let endpoint = variable(&mut arena, 2);
            let q = variable(&mut arena, 0);
            let other_proof = id_elim(&mut arena, motive, two_base, endpoint, q);
            let endpoint = variable(&mut arena, 2);
            let p = variable(&mut arena, 1);
            let other_base = id_elim(&mut arena, motive, function_base, endpoint, p);
            let shared_type = type_sort(&mut arena, 0);

            assert!(
                conv!(&mut arena => &context => left => left_again => shared_type => &mut budget)
                    .unwrap()
            );
            assert!(
                !conv!(&mut arena => &context => left => other_proof => shared_type => &mut budget)
                    .unwrap()
            );
            assert!(
                !conv!(&mut arena => &context => left => other_base => shared_type => &mut budget)
                    .unwrap()
            );
        }

        #[test]
        fn identity_elimination_rejects_malformed_motives_and_endpoints() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let strict_zero = strict_sort(&mut arena, 0);
            let type_zero = type_sort(&mut arena, 0);
            let x_binding = variable(&mut arena, 0);
            let y_binding = variable(&mut arena, 1);
            let p_binding = {
                let ty = variable(&mut arena, 2);
                let x = variable(&mut arena, 1);
                let y = variable(&mut arena, 0);
                id(&mut arena, ty, x, y)
            };
            let context = Context::empty()
                .extend(strict_zero)
                .extend(type_zero)
                .extend(x_binding)
                .extend(y_binding)
                .extend(p_binding);
            let y = variable(&mut arena, 1);
            let p = variable(&mut arena, 0);

            let strict_motive = {
                let domain = variable(&mut arena, 3);
                let inner_domain = {
                    let ty = variable(&mut arena, 4);
                    let x = variable(&mut arena, 3);
                    let bound = variable(&mut arena, 0);
                    id(&mut arena, ty, x, bound)
                };
                let body = variable(&mut arena, 6);
                let inner = lambda(&mut arena, inner_domain, body);
                lambda(&mut arena, domain, inner)
            };
            let base = {
                let ty = variable(&mut arena, 3);
                let x = variable(&mut arena, 2);
                refl(&mut arena, ty, x)
            };
            let elimination = id_elim(&mut arena, strict_motive, base, y, p);
            let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
            assert!(matches!(
                error,
                CoreError::StrictIdentityMotiveCodomain { .. }
            ));

            let term_motive = {
                let domain = variable(&mut arena, 3);
                let inner_domain = {
                    let ty = variable(&mut arena, 4);
                    let x = variable(&mut arena, 3);
                    let bound = variable(&mut arena, 0);
                    id(&mut arena, ty, x, bound)
                };
                let body = variable(&mut arena, 1);
                let inner = lambda(&mut arena, inner_domain, body);
                lambda(&mut arena, domain, inner)
            };
            let elimination = id_elim(&mut arena, term_motive, base, y, p);
            let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
            assert!(matches!(
                error,
                CoreError::IdentityMotiveCodomainNotAUniverse { .. }
            ));

            let short_motive = {
                let domain = variable(&mut arena, 3);
                let body = type_sort(&mut arena, 0);
                lambda(&mut arena, domain, body)
            };
            let elimination = id_elim(&mut arena, short_motive, base, y, p);
            let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
            assert!(matches!(
                error,
                CoreError::IdentityMotiveCodomainNotAFunction { .. }
            ));

            let not_a_function = two_zero(&mut arena);
            let elimination = id_elim(&mut arena, not_a_function, base, y, p);
            let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::NotAFunction { .. }));

            let wrong_domain_motive = {
                let domain = two(&mut arena);
                let inner_domain = {
                    let ty = variable(&mut arena, 4);
                    let x = variable(&mut arena, 3);
                    let bound = variable(&mut arena, 0);
                    id(&mut arena, ty, x, bound)
                };
                let body = type_sort(&mut arena, 0);
                let inner = lambda(&mut arena, inner_domain, body);
                lambda(&mut arena, domain, inner)
            };
            let elimination = id_elim(&mut arena, wrong_domain_motive, base, y, p);
            let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));

            let wrong_inner_motive = {
                let domain = variable(&mut arena, 3);
                let inner_domain = variable(&mut arena, 4);
                let body = type_sort(&mut arena, 0);
                let inner = lambda(&mut arena, inner_domain, body);
                lambda(&mut arena, domain, inner)
            };
            let elimination = id_elim(&mut arena, wrong_inner_motive, base, y, p);
            let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));

            let motive = symmetry_motive(&mut arena, 3, 2);
            let x = variable(&mut arena, 2);
            let elimination = id_elim(&mut arena, motive, base, x, p);
            let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));

            let motive = symmetry_motive(&mut arena, 3, 2);
            let x = variable(&mut arena, 2);
            let elimination = id_elim(&mut arena, motive, base, y, x);
            let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::NotAnIdentity { .. }));
        }

        #[test]
        fn w_formation_rejects_strict_and_malformed_families() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let strict_zero = strict_sort(&mut arena, 0);
            let type_zero = type_sort(&mut arena, 0);
            let type_zero_again = type_sort(&mut arena, 0);
            let a_binding = variable(&mut arena, 1);
            let context = Context::empty()
                .extend(strict_zero)
                .extend(type_zero)
                .extend(type_zero_again)
                .extend(a_binding);

            let s = variable(&mut arena, 3);
            let any_children = two(&mut arena);
            let w = w_type(&mut arena, s, any_children);
            let error = infer_type(&mut arena, &context, w, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::StrictWCarrier { .. }));

            let a = variable(&mut arena, 0);
            let carrier = variable(&mut arena, 2);
            let w = w_type(&mut arena, carrier, a);
            let error = infer_type(&mut arena, &context, w, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::NotAFunction { .. }));

            let a2 = variable(&mut arena, 1);
            let body = two(&mut arena);
            let wrong_family = lambda(&mut arena, a2, body);
            let carrier = variable(&mut arena, 2);
            let w = w_type(&mut arena, carrier, wrong_family);
            let error = infer_type(&mut arena, &context, w, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));

            let a = variable(&mut arena, 2);
            let s_body = variable(&mut arena, 4);
            let strict_family = lambda(&mut arena, a, s_body);
            let carrier = variable(&mut arena, 2);
            let w = w_type(&mut arena, carrier, strict_family);
            let error = infer_type(&mut arena, &context, w, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::StrictWChildrenCodomain { .. }));

            let a = variable(&mut arena, 2);
            let a_body = variable(&mut arena, 1);
            let term_family = lambda(&mut arena, a, a_body);
            let carrier = variable(&mut arena, 2);
            let w = w_type(&mut arena, carrier, term_family);
            let error = infer_type(&mut arena, &context, w, &mut budget).unwrap_err();
            assert!(matches!(
                error,
                CoreError::WChildrenCodomainNotAUniverse { .. }
            ));
        }

        #[test]
        fn sup_checks_its_annotation_and_components() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let context = w_context(&mut arena);

            let carrier = variable(&mut arena, 7);
            let children = variable(&mut arena, 6);
            let w = w_type(&mut arena, carrier, children);
            let label = variable(&mut arena, 5);
            let function = variable(&mut arena, 4);
            let node = sup(&mut arena, carrier, children, label, function);
            check_type(&mut arena, &context, node, w, &mut budget).unwrap();
            let inferred = infer_type(&mut arena, &context, node, &mut budget).unwrap();
            assert!(arena.structurally_equal(inferred, w));

            let x = variable(&mut arena, 0);
            let b_under = variable(&mut arena, 7);
            let applied = apply(&mut arena, b_under, x);
            let a_domain = variable(&mut arena, 7);
            let eta_family = lambda(&mut arena, a_domain, applied);
            let carrier = variable(&mut arena, 7);
            let label = variable(&mut arena, 5);
            let function = variable(&mut arena, 4);
            let node = sup(&mut arena, carrier, eta_family, label, function);
            check_type(&mut arena, &context, node, w, &mut budget).unwrap();

            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let strict_zero = strict_sort(&mut arena, 0);
            let context = Context::empty().extend(strict_zero);
            let s = variable(&mut arena, 0);
            let node = sup(&mut arena, s, s, s, s);
            let error = infer_type(&mut arena, &context, node, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::StrictWCarrier { .. }));

            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let context = w_context(&mut arena);
            let carrier = variable(&mut arena, 7);
            let children = variable(&mut arena, 6);
            let wrong_label_term = variable(&mut arena, 1);
            let function = variable(&mut arena, 4);
            let wrong_label = sup(&mut arena, carrier, children, wrong_label_term, function);
            let error = infer_type(&mut arena, &context, wrong_label, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));
            let carrier = variable(&mut arena, 7);
            let children = variable(&mut arena, 6);
            let label = variable(&mut arena, 5);
            let wrong_function_term = variable(&mut arena, 5);
            let wrong_function = sup(&mut arena, carrier, children, label, wrong_function_term);
            let error = infer_type(&mut arena, &context, wrong_function, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));
        }

        #[test]
        fn w_induction_checks_the_step_against_its_dependent_type() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let context = w_context(&mut arena);

            let p = variable(&mut arena, 3);
            let s = variable(&mut arena, 2);
            let t = variable(&mut arena, 1);
            let induction = ind_w(&mut arena, p, s, t);
            let p = variable(&mut arena, 3);
            let expected = apply(&mut arena, p, t);
            check_type(&mut arena, &context, induction, expected, &mut budget).unwrap();
            let inferred = infer_type(&mut arena, &context, induction, &mut budget).unwrap();
            assert!(arena.structurally_equal(inferred, expected));

            let carrier = variable(&mut arena, 7);
            let children = variable(&mut arena, 6);
            let label = variable(&mut arena, 5);
            let function = variable(&mut arena, 4);
            let node = sup(&mut arena, carrier, children, label, function);
            let p = variable(&mut arena, 3);
            let induction = ind_w(&mut arena, p, s, node);
            let p = variable(&mut arena, 3);
            let expected = apply(&mut arena, p, node);
            check_type(&mut arena, &context, induction, expected, &mut budget).unwrap();

            let carrier = variable(&mut arena, 7);
            let children = variable(&mut arena, 6);
            let w = w_type(&mut arena, carrier, children);
            let body = two(&mut arena);
            let constant_motive = lambda(&mut arena, w, body);
            let step = concrete_step(&mut arena);
            let induction = ind_w(&mut arena, constant_motive, step, t);
            let expected = two(&mut arena);
            check_type(&mut arena, &context, induction, expected, &mut budget).unwrap();

            let a = variable(&mut arena, 5);
            let p = variable(&mut arena, 3);
            let induction = ind_w(&mut arena, p, s, a);
            let error = infer_type(&mut arena, &context, induction, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::NotAW { .. }));

            let not_a_function = two_zero(&mut arena);
            let induction = ind_w(&mut arena, not_a_function, s, t);
            let error = infer_type(&mut arena, &context, induction, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::NotAFunction { .. }));

            let wrong_domain = variable(&mut arena, 7);
            let body = two(&mut arena);
            let wrong_motive = lambda(&mut arena, wrong_domain, body);
            let induction = ind_w(&mut arena, wrong_motive, s, t);
            let error = infer_type(&mut arena, &context, induction, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));

            let a_domain = variable(&mut arena, 7);
            let a_bound = variable(&mut arena, 0);
            let wrong_step = lambda(&mut arena, a_domain, a_bound);
            let p = variable(&mut arena, 3);
            let induction = ind_w(&mut arena, p, wrong_step, t);
            let error = infer_type(&mut arena, &context, induction, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));
        }

        #[test]
        fn w_induction_rejects_strict_and_non_universe_motives() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let strict_zero = strict_sort(&mut arena, 0);
            let type_zero = type_sort(&mut arena, 0);
            let a_in_prefix = variable(&mut arena, 0);
            let b_binding = pi(&mut arena, a_in_prefix, type_zero);
            let a_binding = variable(&mut arena, 1);
            let carrier = variable(&mut arena, 2);
            let children = variable(&mut arena, 1);
            let t_binding = w_type(&mut arena, carrier, children);
            let context = Context::empty()
                .extend(strict_zero)
                .extend(type_zero)
                .extend(b_binding)
                .extend(a_binding)
                .extend(t_binding);

            let carrier = variable(&mut arena, 3);
            let children = variable(&mut arena, 2);
            let w = w_type(&mut arena, carrier, children);
            let s_body = variable(&mut arena, 5);
            let strict_motive = lambda(&mut arena, w, s_body);
            let t = variable(&mut arena, 0);
            let step_placeholder = variable(&mut arena, 0);
            let induction = ind_w(&mut arena, strict_motive, step_placeholder, t);
            let error = infer_type(&mut arena, &context, induction, &mut budget).unwrap_err();
            assert!(matches!(
                error,
                CoreError::StrictInductionMotiveCodomain { .. }
            ));

            let carrier = variable(&mut arena, 3);
            let children = variable(&mut arena, 2);
            let w = w_type(&mut arena, carrier, children);
            let a_body = variable(&mut arena, 2);
            let term_motive = lambda(&mut arena, w, a_body);
            let induction = ind_w(&mut arena, term_motive, step_placeholder, t);
            let error = infer_type(&mut arena, &context, induction, &mut budget).unwrap_err();
            assert!(matches!(
                error,
                CoreError::InductionMotiveCodomainNotAUniverse { .. }
            ));
        }

        #[test]
        fn w_induction_computes_on_sup_with_a_neutral_child_function() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let context = w_context(&mut arena);

            let carrier = variable(&mut arena, 7);
            let children = variable(&mut arena, 6);
            let label = variable(&mut arena, 5);
            let function = variable(&mut arena, 4);
            let node = sup(&mut arena, carrier, children, label, function);
            let p = variable(&mut arena, 3);
            let s = variable(&mut arena, 2);
            let induction = ind_w(&mut arena, p, s, node);

            let before = budget.remaining();
            let normalized =
                weak_head_normalize(&mut arena, &Signature::new(), induction, &mut budget).unwrap();
            assert_eq!(before - budget.remaining(), 1);
            let ih_domain = {
                let b = variable(&mut arena, 6);
                let a = variable(&mut arena, 5);
                apply(&mut arena, b, a)
            };
            let ih_body = {
                let p = variable(&mut arena, 4);
                let s = variable(&mut arena, 3);
                let k = variable(&mut arena, 5);
                let bound = variable(&mut arena, 0);
                let child = apply(&mut arena, k, bound);
                ind_w(&mut arena, p, s, child)
            };
            let ih = lambda(&mut arena, ih_domain, ih_body);
            let expected = {
                let s = variable(&mut arena, 2);
                let a = variable(&mut arena, 5);
                let k = variable(&mut arena, 4);
                let applied = apply(&mut arena, s, a);
                let applied = apply(&mut arena, applied, k);
                apply(&mut arena, applied, ih)
            };
            assert!(arena.structurally_equal(normalized, expected));

            let inferred = infer_type(&mut arena, &context, induction, &mut budget).unwrap();
            let p = variable(&mut arena, 3);
            let expected_type = apply(&mut arena, p, node);
            assert!(arena.structurally_equal(inferred, expected_type));

            let mut empty_budget = Budget::new(0);
            let error =
                weak_head_normalize(&mut arena, &Signature::new(), induction, &mut empty_budget)
                    .unwrap_err();
            assert_eq!(error, CoreError::StepCeiling);

            let t = variable(&mut arena, 1);
            let p = variable(&mut arena, 3);
            let s = variable(&mut arena, 2);
            let stuck = ind_w(&mut arena, p, s, t);
            let normalized =
                weak_head_normalize(&mut arena, &Signature::new(), stuck, &mut budget).unwrap();
            assert_eq!(normalized, stuck);
        }

        #[test]
        fn stuck_w_inductions_compare_componentwise() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let context = w_context(&mut arena);

            let carrier = variable(&mut arena, 7);
            let children = variable(&mut arena, 6);
            let w = w_type(&mut arena, carrier, children);
            let body = two(&mut arena);
            let motive = lambda(&mut arena, w, body);
            let step = concrete_step(&mut arena);
            let t = variable(&mut arena, 1);
            let u = variable(&mut arena, 0);
            let shared = two(&mut arena);

            let left = ind_w(&mut arena, motive, step, t);
            let left_again = ind_w(&mut arena, motive, step, t);
            let right = ind_w(&mut arena, motive, step, u);
            assert!(
                convertible(&mut arena, &context, left, left_again, shared, &mut budget).unwrap()
            );
            assert!(!convertible(&mut arena, &context, left, right, shared, &mut budget).unwrap());

            let a_domain = variable(&mut arena, 7);
            let b_at = variable(&mut arena, 7);
            let a_bound = variable(&mut arena, 0);
            let child_positions = apply(&mut arena, b_at, a_bound);
            let carrier = variable(&mut arena, 9);
            let children = variable(&mut arena, 8);
            let w_at_two = w_type(&mut arena, carrier, children);
            let function_type = pi(&mut arena, child_positions, w_at_two);
            let b_at = variable(&mut arena, 8);
            let a_bound = variable(&mut arena, 1);
            let hypothesis_domain = apply(&mut arena, b_at, a_bound);
            let hypothesis_codomain = two(&mut arena);
            let hypothesis = pi(&mut arena, hypothesis_domain, hypothesis_codomain);
            let body = two_one(&mut arena);
            let inner = lambda(&mut arena, hypothesis, body);
            let middle = lambda(&mut arena, function_type, inner);
            let other_step = lambda(&mut arena, a_domain, middle);
            let other = ind_w(&mut arena, motive, other_step, t);
            assert!(!convertible(&mut arena, &context, left, other, shared, &mut budget).unwrap());
        }

        #[test]
        fn w_types_convert_componentwise_and_sup_has_no_eta() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let context = w_context(&mut arena);
            let shared_sort = type_sort(&mut arena, 0);

            let carrier = variable(&mut arena, 7);
            let children = variable(&mut arena, 6);
            let w = w_type(&mut arena, carrier, children);
            let x = variable(&mut arena, 0);
            let b_under = variable(&mut arena, 7);
            let applied = apply(&mut arena, b_under, x);
            let a_domain = variable(&mut arena, 7);
            let eta_family = lambda(&mut arena, a_domain, applied);
            let carrier = variable(&mut arena, 7);
            let w_eta = w_type(&mut arena, carrier, eta_family);
            assert!(convertible(&mut arena, &context, w, w_eta, shared_sort, &mut budget).unwrap());

            let a_domain = variable(&mut arena, 7);
            let body = two(&mut arena);
            let constant_family = lambda(&mut arena, a_domain, body);
            let carrier = variable(&mut arena, 7);
            let w_constant = w_type(&mut arena, carrier, constant_family);
            assert!(
                !conv!(&mut arena => &context => w => w_constant => shared_sort => &mut budget)
                    .unwrap()
            );

            let a_domain = variable(&mut arena, 7);
            let body = type_sort(&mut arena, 0);
            let higher_family = lambda(&mut arena, a_domain, body);
            let carrier = variable(&mut arena, 7);
            let w_higher = w_type(&mut arena, carrier, higher_family);
            let higher_sort = type_sort(&mut arena, 1);
            assert!(
                !convertible(&mut arena, &context, w, w_higher, higher_sort, &mut budget).unwrap()
            );

            let carrier = variable(&mut arena, 7);
            let children = variable(&mut arena, 6);
            let label = variable(&mut arena, 5);
            let function = variable(&mut arena, 4);
            let node = sup(&mut arena, carrier, children, label, function);
            let carrier = variable(&mut arena, 7);
            let children = variable(&mut arena, 6);
            let label = variable(&mut arena, 5);
            let function = variable(&mut arena, 4);
            let node_again = sup(&mut arena, carrier, children, label, function);
            assert!(convertible(&mut arena, &context, node, node_again, w, &mut budget).unwrap());
            let t = variable(&mut arena, 1);
            assert!(!convertible(&mut arena, &context, node, t, w, &mut budget).unwrap());
            assert!(!convertible(&mut arena, &context, t, node, w, &mut budget).unwrap());
        }
    }
    mod level_worklists {
        use super::default_budget;
        use crate::mathematical_core::{
            Context, CoreError, Level, Sort, Term, TermArena, infer_type,
        };

        const DEEP: usize = 131_072;

        fn successor_chain(mut level: Level, count: usize) -> Level {
            for _ in 0..count {
                level = Level::Successor(Box::new(level));
            }
            level
        }

        #[test]
        fn deep_universe_formation_and_scope_refusal_use_default_stack() {
            std::thread::spawn(|| {
                let mut arena = TermArena::new();
                let level = successor_chain(Level::Parameter(0), DEEP);
                let term = arena.insert(Term::Sort(Sort::Type(level)));
                eprintln!("constructed deep parametric universe; entering ordinary infer_type");
                let inferred = infer_type(
                    &mut arena,
                    &Context::with_level_arity(1),
                    term,
                    &mut default_budget(),
                )
                .expect("well-scoped universe formation");
                let Term::Sort(Sort::Type(mut level)) = arena.get(inferred) else {
                    panic!("relevant universe")
                };
                let mut depth = 0;
                let mut cursor = &level;
                while let Level::Successor(child) = cursor {
                    depth += 1;
                    cursor = child;
                }
                assert_eq!(depth, DEEP + 1);
                assert!(matches!(cursor, Level::Parameter(0)));
                assert_eq!(
                    infer_type(&mut arena, &Context::empty(), term, &mut default_budget()),
                    Err(CoreError::UnboundLevelParameter { index: 0, arity: 0 })
                );
                let maximum = arena.insert(Term::Sort(Sort::Type(Level::Constant(u32::MAX))));
                assert_eq!(
                    infer_type(
                        &mut arena,
                        &Context::empty(),
                        maximum,
                        &mut default_budget()
                    ),
                    Err(CoreError::LevelOverflow)
                );
                level = Level::Constant(0);
                assert_eq!(level, Level::Constant(0));
            })
            .join()
            .expect("universe formation and cleanup use the default stack");
        }

        #[test]
        fn level_syntax_traits_keep_derived_hash_and_diagnostic_contracts() {
            use std::hash::{Hash, Hasher};
            #[derive(Debug, Hash)]
            enum Reference {
                Constant(u32),
                Parameter(u32),
                Successor(Box<Reference>),
                Maximum(Box<Reference>, Box<Reference>),
            }
            fn level(value: &Reference) -> Level {
                match value {
                    Reference::Constant(value) => Level::Constant(*value),
                    Reference::Parameter(index) => Level::Parameter(*index),
                    Reference::Successor(inner) => Level::Successor(Box::new(level(inner))),
                    Reference::Maximum(left, right) => {
                        Level::Maximum(Box::new(level(left)), Box::new(level(right)))
                    }
                }
            }
            fn hash(value: &impl Hash) -> u64 {
                let mut state = std::hash::DefaultHasher::new();
                value.hash(&mut state);
                state.finish()
            }
            for reference in [
                Reference::Constant(0),
                Reference::Constant(u32::MAX),
                Reference::Parameter(0),
                Reference::Successor(Box::new(Reference::Parameter(3))),
                Reference::Maximum(
                    Box::new(Reference::Parameter(0)),
                    Box::new(Reference::Successor(Box::new(Reference::Constant(7)))),
                ),
            ] {
                let actual = level(&reference);
                assert_eq!(format!("{actual:?}"), format!("{reference:?}"));
                assert_eq!(format!("{actual:#?}"), format!("{reference:#?}"));
                assert_eq!(hash(&actual), hash(&reference));
                assert_eq!(actual, actual.clone());
            }
        }

        #[test]
        fn deep_level_operations_preserve_syntax_algebra_and_first_refusal() {
            std::thread::spawn(|| {
                use crate::mathematical_core::term::{instantiate_level, levels_equal};
                use std::hash::{Hash, Hasher};
                let level = successor_chain(Level::Parameter(0), DEEP);
                let clone = level.clone();
                assert_eq!(level, clone);
                let mut first = std::hash::DefaultHasher::new();
                level.hash(&mut first);
                let mut second = std::hash::DefaultHasher::new();
                clone.hash(&mut second);
                assert_eq!(first.finish(), second.finish());
                assert_eq!(
                    format!("{level:?}").len(),
                    DEEP * "Successor()".len() + "Parameter(0)".len()
                );
                let absorbed = level.clone().maximum(Level::Constant(DEEP as u32));
                assert_ne!(
                    level, absorbed,
                    "syntactic equality must not become semantic conversion"
                );
                assert!(levels_equal(&level, &absorbed));
                let different = level.clone().maximum(Level::Constant(DEEP as u32 + 1));
                assert!(!levels_equal(&level, &different));
                let closed = instantiate_level(&level, &[Level::Constant(1)]).unwrap();
                assert!(levels_equal(&closed, &Level::Constant(DEEP as u32 + 1)));
                assert!(
                    matches!(closed, Level::Successor(_)),
                    "instantiation preserves explicit syntax for wire-depth admission"
                );
                let missing =
                    Level::Maximum(Box::new(level.clone()), Box::new(Level::Parameter(1)));
                assert_eq!(instantiate_level(&missing, &[Level::Constant(0)]), Err(1));
                assert_eq!(
                    instantiate_level(&missing, &[]),
                    Err(0),
                    "left-to-right missing-parameter precedence"
                );
            })
            .join()
            .expect("all level operations and refusal cleanup use the default stack");
        }
    }
    mod levels_and_declarations {
        use super::{
            constant, default_budget, lambda, pi, polymorphic_identity_declaration, sort_level,
            type_sort, variable,
        };
        use crate::mathematical_core::{
            Budget, Context, CoreError, Declaration, Level, Term, TermArena, check_signature,
            check_type, convertible, infer_type, weak_head_normalize,
        };

        #[test]
        fn a_universe_polymorphic_identity_checks_parametrically() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let context = Context::with_level_arity(1);
            let type_u = sort_level(&mut arena, Level::Parameter(0));
            let bound = variable(&mut arena, 0);
            let inner = lambda(&mut arena, bound, bound);
            let identity = lambda(&mut arena, type_u, inner);
            let codomain_domain = variable(&mut arena, 0);
            let codomain_body = variable(&mut arena, 1);
            let codomain = pi(&mut arena, codomain_domain, codomain_body);
            let expected = pi(&mut arena, type_u, codomain);
            check_type(&mut arena, &context, identity, expected, &mut budget).unwrap();

            let inferred = infer_type(&mut arena, &context, expected, &mut budget).unwrap();
            let claimed_sort = sort_level(&mut arena, Level::Parameter(0).successor().unwrap());
            let relevant = type_sort(&mut arena, 0);
            assert!(
                convertible(
                    &mut arena,
                    &context,
                    inferred,
                    claimed_sort,
                    relevant,
                    &mut budget
                )
                .unwrap()
            );
        }

        #[test]
        fn check_signature_rejects_self_and_forward_references() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let type_zero = type_sort(&mut arena, 0);
            let self_index = constant(&mut arena, 0, Vec::new());
            let self_reference = Declaration::assumption(0, pi(&mut arena, type_zero, self_index));
            let error = check_signature(&mut arena, &[self_reference], &mut budget).unwrap_err();
            assert_eq!(
                error,
                CoreError::UnknownDeclaration {
                    declaration: 0,
                    signature_len: 0,
                }
            );

            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let type_zero = type_sort(&mut arena, 0);
            let forward_index = constant(&mut arena, 1, Vec::new());
            let forward = Declaration::assumption(0, pi(&mut arena, type_zero, forward_index));
            let type_zero = type_sort(&mut arena, 0);
            let later = Declaration::assumption(0, type_zero);
            let error = check_signature(&mut arena, &[forward, later], &mut budget).unwrap_err();
            assert_eq!(
                error,
                CoreError::UnknownDeclaration {
                    declaration: 1,
                    signature_len: 0,
                }
            );
        }

        #[test]
        fn a_declaration_whose_statement_is_not_a_type_rejects() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let two_zero = arena.insert(Term::TwoZero);
            let malformed = Declaration::assumption(0, two_zero);
            let error = check_signature(&mut arena, &[malformed], &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::NotASort { .. }));
        }

        #[test]
        fn a_definition_constant_unfolds_and_an_assumption_stays_neutral() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let type_u = sort_level(&mut arena, Level::Parameter(0));
            let axiom = Declaration::assumption(1, type_u);
            let identity = polymorphic_identity_declaration(&mut arena);
            let signature = check_signature(&mut arena, &[axiom, identity], &mut budget).unwrap();
            let context = Context::with_level_arity(1).with_signature(signature);

            let defined = constant(&mut arena, 1, vec![Level::Parameter(0)]);
            let normalized =
                weak_head_normalize(&mut arena, context.signature(), defined, &mut budget).unwrap();
            let expected_body = {
                let type_u = sort_level(&mut arena, Level::Parameter(0));
                let bound_a = variable(&mut arena, 0);
                let inner_x = variable(&mut arena, 0);
                let inner = lambda(&mut arena, bound_a, inner_x);
                lambda(&mut arena, type_u, inner)
            };
            assert!(arena.structurally_equal(normalized, expected_body));

            let mut empty_budget = Budget::new(0);
            let error =
                weak_head_normalize(&mut arena, context.signature(), defined, &mut empty_budget)
                    .unwrap_err();
            assert_eq!(error, CoreError::StepCeiling);

            let neutral = constant(&mut arena, 0, vec![Level::Parameter(0)]);
            let mut budget = default_budget();
            let normalized =
                weak_head_normalize(&mut arena, context.signature(), neutral, &mut budget).unwrap();
            assert_eq!(normalized, neutral);
        }

        #[test]
        fn stuck_constants_convert_at_semantically_equal_instantiations() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let type_u = sort_level(&mut arena, Level::Parameter(0));
            let axiom = Declaration::assumption(2, type_u);
            let signature = check_signature(&mut arena, &[axiom], &mut budget).unwrap();
            let context = Context::with_level_arity(2).with_signature(signature);
            let u = Level::Parameter(0);
            let v = Level::Parameter(1);

            let left = constant(&mut arena, 0, vec![u.clone().maximum(v.clone())]);
            let right = constant(&mut arena, 0, vec![v.clone().maximum(u.clone())]);
            let shared = sort_level(&mut arena, u.clone().maximum(v.clone()));
            assert!(
                convertible(&mut arena, &context, left, right, shared, &mut budget).unwrap(),
                "the same assumption at equal levels must convert"
            );

            let other = constant(&mut arena, 0, vec![u.clone()]);
            assert!(
                !convertible(&mut arena, &context, left, other, shared, &mut budget).unwrap(),
                "distinct level instantiations must not convert"
            );

            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let type_u = sort_level(&mut arena, Level::Parameter(0));
            let first = Declaration::assumption(1, type_u);
            let type_u = sort_level(&mut arena, Level::Parameter(0));
            let second = Declaration::assumption(1, type_u);
            let signature = check_signature(&mut arena, &[first, second], &mut budget).unwrap();
            let context = Context::with_level_arity(1).with_signature(signature);
            let left = constant(&mut arena, 0, vec![Level::Parameter(0)]);
            let right = constant(&mut arena, 1, vec![Level::Parameter(0)]);
            let shared = sort_level(&mut arena, Level::Parameter(0));
            assert!(!convertible(&mut arena, &context, left, right, shared, &mut budget).unwrap());
        }

        #[test]
        fn retained_carrier_aliases_survive_dependent_type_conversion() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let universe = type_sort(&mut arena, 0);
            let carrier = arena.insert(Term::Two);
            let narrow = Declaration {
                level_arity: 0,
                ty: universe,
                body: Some(carrier),
            };
            let wide = Declaration {
                level_arity: 0,
                ty: universe,
                body: Some(carrier),
            };
            let family_type = pi(&mut arena, universe, universe);
            let family = Declaration::assumption(0, family_type);
            let signature =
                check_signature(&mut arena, &[narrow, wide, family], &mut budget).unwrap();
            let context = Context::empty().with_signature(signature);
            let narrow = constant(&mut arena, 0, vec![]);
            let wide = constant(&mut arena, 1, vec![]);
            let family = constant(&mut arena, 2, vec![]);
            let left = arena.insert(Term::Apply {
                function: family,
                argument: narrow,
            });
            let right = arena.insert(Term::Apply {
                function: family,
                argument: wide,
            });
            let retained = |declaration| declaration < 2;

            assert!(convertible(&mut arena, &context, left, right, universe, &mut budget).unwrap());
            assert!(
                !crate::mathematical_core::convertible_retaining(
                    &mut arena,
                    &context,
                    left,
                    right,
                    universe,
                    &mut budget,
                    &retained,
                )
                .unwrap()
            );

            let left_function = pi(&mut arena, narrow, carrier);
            let right_function = pi(&mut arena, wide, carrier);
            assert!(
                convertible(
                    &mut arena,
                    &context,
                    left_function,
                    right_function,
                    universe,
                    &mut budget
                )
                .unwrap()
            );
            assert!(
                !crate::mathematical_core::convertible_retaining(
                    &mut arena,
                    &context,
                    left_function,
                    right_function,
                    universe,
                    &mut budget,
                    &retained,
                )
                .unwrap()
            );

            let bound = variable(&mut arena, 0);
            let identity = lambda(&mut arena, universe, bound);
            let application = arena.insert(Term::Apply {
                function: identity,
                argument: narrow,
            });
            let same_family = arena.insert(Term::Apply {
                function: family,
                argument: application,
            });
            assert!(
                crate::mathematical_core::convertible_retaining(
                    &mut arena,
                    &context,
                    left,
                    same_family,
                    universe,
                    &mut budget,
                    &retained,
                )
                .unwrap()
            );
        }
    }
    mod normalization_worklists {
        use super::{constant, default_budget, pi, two};
        use crate::mathematical_core::{
            Budget, Context, CoreError, Declaration, Term, TermArena, check_signature, convertible,
            weak_head_normalize,
        };

        #[test]
        fn deep_typed_conversion_preserves_each_questions_budget_on_the_default_stack() {
            std::thread::spawn(|| {
                let mut arena = TermArena::new();
                let two = two(&mut arena);
                let zero = arena.insert(Term::TwoZero);
                let type_function = arena.insert(Term::Lambda {
                    domain: two,
                    body: two,
                });
                let mut right = arena.insert(Term::Apply {
                    function: type_function,
                    argument: zero,
                });
                let mut left = two;
                for _ in 0..4096 {
                    left = pi(&mut arena, two, left);
                    right = pi(&mut arena, two, right);
                }
                let shared_type = super::type_sort(&mut arena, 0);
                let context = Context::empty();
                for (allowance, accepted) in [(1, true), (0, false)] {
                    let mut budget = Budget::new(allowance);
                    let observed =
                        convertible(&mut arena, &context, left, right, shared_type, &mut budget);
                    if accepted {
                        assert_eq!(observed, Ok(true));
                    } else {
                        assert_eq!(observed, Err(CoreError::StepCeiling));
                    }
                    assert_eq!(budget.remaining(), 0);
                }
            })
            .join()
            .expect("typed conversion uses default-stack judgment continuations");
        }

        #[test]
        fn deep_neutral_application_normalization_stays_on_default_stack() {
            std::thread::spawn(|| {
                let mut arena = TermArena::new();
                let two = two(&mut arena);
                let mut function_type = two;
                for _ in 0..131_072 {
                    function_type = pi(&mut arena, two, function_type);
                }
                let signature = check_signature(
                    &mut arena,
                    &[Declaration::assumption(0, function_type)],
                    &mut default_budget(),
                )
                .expect("the neutral function has a well-formed complete telescope");
                let mut term = constant(&mut arena, 0, vec![]);
                let zero = arena.insert(Term::TwoZero);
                for _ in 0..131_072 {
                    term = arena.insert(Term::Apply {
                        function: term,
                        argument: zero,
                    });
                }
                let before = arena.len();
                let mut budget = Budget::new(0);
                assert_eq!(
                    weak_head_normalize(&mut arena, &signature, term, &mut budget),
                    Ok(term)
                );
                assert_eq!(
                    arena.len(),
                    before,
                    "a neutral spine keeps all original handles"
                );
                assert_eq!(budget.remaining(), 0, "inspection is not a reduction step");
            })
            .join()
            .expect("normalization uses a default-stack handle worklist");
        }

        #[test]
        fn normalization_frames_preserve_reduction_budgets_and_refusals() {
            let mut arena = TermArena::new();
            let two = two(&mut arena);
            let zero = arena.insert(Term::TwoZero);
            let one = arena.insert(Term::TwoOne);
            let pair = arena.insert(Term::Pair {
                first: zero,
                second: one,
            });
            let body = arena.insert(Term::Variable(0));
            let identity = arena.insert(Term::Lambda { domain: two, body });
            let applied = arena.insert(Term::Apply {
                function: identity,
                argument: pair,
            });
            let first = arena.insert(Term::Fst { pair: applied });
            let signature = crate::mathematical_core::Signature::new();
            assert_eq!(
                weak_head_normalize(&mut arena, &signature, first, &mut Budget::new(1)),
                Err(CoreError::StepCeiling)
            );
            let mut budget = Budget::new(2);
            assert_eq!(
                weak_head_normalize(&mut arena, &signature, first, &mut budget),
                Ok(zero)
            );
            assert_eq!(budget.remaining(), 0);
            let unknown = constant(&mut arena, 9, vec![]);
            let nested = arena.insert(Term::Snd { pair: unknown });
            assert_eq!(
                weak_head_normalize(&mut arena, &signature, nested, &mut default_budget()),
                Err(CoreError::UnknownDeclaration {
                    declaration: 9,
                    signature_len: 0
                })
            );
        }

        #[test]
        fn deep_structural_comparison_keeps_every_payload_and_tail() {
            std::thread::spawn(|| {
                let mut arena = TermArena::new();
                let mut left = two(&mut arena);
                let mut equal = two(&mut arena);
                let mut different = arena.insert(Term::TwoOne);
                let domain = two(&mut arena);
                for _ in 0..131_072 {
                    left = pi(&mut arena, domain, left);
                    equal = pi(&mut arena, domain, equal);
                    different = pi(&mut arena, domain, different);
                }
                assert!(arena.structurally_equal(left, equal));
                assert!(!arena.structurally_equal(left, different));
                let dummy = arena.insert(Term::Dummy);
                assert!(
                    !arena.structurally_equal(dummy, dummy),
                    "invalid placeholders are never equality evidence"
                );
                let parameter = crate::mathematical_core::Level::Parameter(0);
                let levels_a = vec![
                    parameter
                        .clone()
                        .maximum(crate::mathematical_core::Level::Constant(0)),
                ];
                let levels_b = vec![parameter];
                let a = constant(&mut arena, 0, levels_a);
                let b = constant(&mut arena, 0, levels_b);
                assert!(
                    !arena.structurally_equal(a, b),
                    "structural comparison does not silently become semantic level conversion"
                );
            })
            .join()
            .expect("structural comparison uses a default-stack worklist");
        }
    }
    mod schemes_and_quotients {
        mod indexed_schemes {
            use crate::mathematical_core::tests::{
                apply, constant, default_budget, indexed_context, lambda, pair, pi, refl,
                strict_sort, sup, two, two_indexed_family, two_zero, variable, w_type,
            };
            use crate::mathematical_core::{
                CoreError, INDEXED_IND, INDEXED_W, Level, TermArena, check_signature, check_type,
                convertible, indexed_scheme, infer_type,
            };

            #[test]
            fn indexed_scheme_checks_as_a_parametric_signature() {
                let mut arena = TermArena::new();
                let mut budget = default_budget();
                let declarations = indexed_scheme(&mut arena);
                assert_eq!(declarations.len(), 5);

                let before = budget.remaining();
                let signature = check_signature(&mut arena, &declarations, &mut budget).unwrap();
                assert_eq!(signature.len(), 5);
                assert!(before - budget.remaining() > 0);

                assert!(!arena.is_empty());
            }

            #[test]
            fn isup_and_iindw_check_and_compute_with_a_neutral_child_function() {
                let mut arena = TermArena::new();
                let mut budget = default_budget();
                let family = two_indexed_family(&mut arena);
                let context = indexed_context(&mut arena, &family);

                let a = variable(&mut arena, 1);
                let g = variable(&mut arena, 0);
                let constructor = family.sup(&mut arena, a, g);
                let a_bound = variable(&mut arena, 1);
                let at_a = family.indexed_w(&mut arena, a_bound);
                check_type(&mut arena, &context, constructor, at_a, &mut budget).unwrap();

                let motive = variable(&mut arena, 3);
                let step = variable(&mut arena, 2);
                let index = variable(&mut arena, 1);
                let elimination = family.ind(
                    &mut arena,
                    Level::Constant(0),
                    motive,
                    step,
                    index,
                    constructor,
                );
                let shared = {
                    let q = variable(&mut arena, 3);
                    let a_bound = variable(&mut arena, 1);
                    let q_a = apply(&mut arena, q, a_bound);
                    apply(&mut arena, q_a, constructor)
                };
                check_type(&mut arena, &context, elimination, shared, &mut budget).unwrap();

                let hypothesis = {
                    let a_bound = variable(&mut arena, 1);
                    let domain = apply(&mut arena, family.children, a_bound);
                    let body = {
                        let a_bound = variable(&mut arena, 2);
                        let next_a = apply(&mut arena, family.next, a_bound);
                        let b_bound = variable(&mut arena, 0);
                        let next_ab = apply(&mut arena, next_a, b_bound);
                        let g = variable(&mut arena, 1);
                        let b_bound = variable(&mut arena, 0);
                        let g_b = apply(&mut arena, g, b_bound);
                        let motive = variable(&mut arena, 4);
                        let step = variable(&mut arena, 3);
                        family.ind(&mut arena, Level::Constant(0), motive, step, next_ab, g_b)
                    };
                    lambda(&mut arena, domain, body)
                };
                let expected = {
                    let s = variable(&mut arena, 2);
                    let a_bound = variable(&mut arena, 1);
                    let s_a = apply(&mut arena, s, a_bound);
                    let g = variable(&mut arena, 0);
                    let s_a_g = apply(&mut arena, s_a, g);
                    apply(&mut arena, s_a_g, hypothesis)
                };
                check_type(&mut arena, &context, expected, shared, &mut budget).unwrap();
                assert!(
                    convertible(
                        &mut arena,
                        &context,
                        elimination,
                        expected,
                        shared,
                        &mut budget
                    )
                    .unwrap()
                );

                let k_binding = {
                    let a_bound = variable(&mut arena, 1);
                    let b_domain = apply(&mut arena, family.children, a_bound);
                    let carrier = two(&mut arena);
                    let codomain = w_type(&mut arena, carrier, family.children);
                    pi(&mut arena, b_domain, codomain)
                };
                let f_binding = {
                    let a_bound = variable(&mut arena, 2);
                    let b_domain = apply(&mut arena, family.children, a_bound);
                    let body = {
                        let a_bound = variable(&mut arena, 3);
                        let next_a = apply(&mut arena, family.next, a_bound);
                        let b_bound = variable(&mut arena, 0);
                        let next_ab = apply(&mut arena, next_a, b_bound);
                        let k = variable(&mut arena, 1);
                        let b_bound = variable(&mut arena, 0);
                        let k_b = apply(&mut arena, k, b_bound);
                        family.indexed_at(&mut arena, next_ab, k_b)
                    };
                    pi(&mut arena, b_domain, body)
                };
                let context = context.extend(k_binding).extend(f_binding);
                let node = {
                    let carrier = two(&mut arena);
                    let label = variable(&mut arena, 3);
                    let function = variable(&mut arena, 1);
                    sup(&mut arena, carrier, family.children, label, function)
                };
                let evidence = {
                    let ty = two(&mut arena);
                    let a_bound = variable(&mut arena, 3);
                    let value = apply(&mut arena, family.out, a_bound);
                    let refl_part = refl(&mut arena, ty, value);
                    let f = variable(&mut arena, 0);
                    pair(&mut arena, refl_part, f)
                };
                let index = variable(&mut arena, 3);
                let packed = family.pack(&mut arena, index, node, evidence);
                let a_bound = variable(&mut arena, 3);
                let at_a = family.indexed_w(&mut arena, a_bound);
                check_type(&mut arena, &context, packed, at_a, &mut budget).unwrap();
            }

            #[test]
            fn indexed_applications_reject_wrong_descriptions_and_indices() {
                let mut arena = TermArena::new();
                let mut budget = default_budget();
                let family = two_indexed_family(&mut arena);
                let context = indexed_context(&mut arena, &family);

                let a = variable(&mut arena, 1);
                let g = variable(&mut arena, 0);
                let constructor = family.sup(&mut arena, a, g);
                let zero = two_zero(&mut arena);
                let wrong_index = family.indexed_w(&mut arena, zero);
                let error = check_type(&mut arena, &context, constructor, wrong_index, &mut budget)
                    .unwrap_err();
                assert!(matches!(error, CoreError::TypeMismatch { .. }));

                let motive = variable(&mut arena, 3);
                let step = variable(&mut arena, 2);
                let zero = two_zero(&mut arena);
                let elimination = family.ind(
                    &mut arena,
                    Level::Constant(0),
                    motive,
                    step,
                    zero,
                    constructor,
                );
                let expected = {
                    let q = variable(&mut arena, 3);
                    let zero = two_zero(&mut arena);
                    let q_zero = apply(&mut arena, q, zero);
                    apply(&mut arena, q_zero, constructor)
                };
                let error = check_type(&mut arena, &context, elimination, expected, &mut budget)
                    .unwrap_err();
                assert!(matches!(error, crate::argument_error_pair_pattern!()));

                let mut bad_family = family.clone();
                bad_family.index = two_zero(&mut arena);
                let zero = two_zero(&mut arena);
                let malformed = bad_family.indexed_w(&mut arena, zero);
                let error = infer_type(&mut arena, &context, malformed, &mut budget).unwrap_err();
                assert!(matches!(error, crate::argument_error_pair_pattern!()));

                let mut strict_family = family.clone();
                strict_family.index = strict_sort(&mut arena, 0);
                let zero = two_zero(&mut arena);
                let malformed = strict_family.indexed_w(&mut arena, zero);
                let error = infer_type(&mut arena, &context, malformed, &mut budget).unwrap_err();
                assert!(matches!(error, crate::argument_error_pair_pattern!()));

                let bad_constant = constant(&mut arena, INDEXED_W, vec![Level::Constant(0)]);
                let error =
                    infer_type(&mut arena, &context, bad_constant, &mut budget).unwrap_err();
                assert_eq!(
                    error,
                    CoreError::DeclarationArityMismatch {
                        declaration: INDEXED_W,
                        expected: 3,
                        supplied: 1,
                    }
                );

                let bad_ind = constant(
                    &mut arena,
                    INDEXED_IND,
                    vec![Level::Constant(0), Level::Constant(0), Level::Constant(0)],
                );
                let error = infer_type(&mut arena, &context, bad_ind, &mut budget).unwrap_err();
                assert_eq!(
                    error,
                    CoreError::DeclarationArityMismatch {
                        declaration: INDEXED_IND,
                        expected: 4,
                        supplied: 3,
                    }
                );
            }
        }
        mod quotient_derived {
            use crate::mathematical_core::tests::{
                apply, boxed, default_budget, id, is_set_zero, lambda, pi, quotient_signature,
                refl, squash, squash_intro, two, two_identity, two_quotient, variable,
            };
            use crate::mathematical_core::{
                Context, CoreError, Level, TermArena, check_type, convertible, infer_type,
            };

            #[test]
            fn quotient_identity_lemmas_check_at_two() {
                let mut arena = TermArena::new();
                let mut budget = default_budget();
                let family = two_quotient(&mut arena);
                let signature = quotient_signature(&mut arena);
                let zero = Level::Constant(0);

                let identity_ab = two_identity(&mut arena, 1, 0);
                let context = Context::empty()
                    .with_signature(signature)
                    .extend(two(&mut arena))
                    .extend(two(&mut arena))
                    .extend(identity_ab);

                let ty = two(&mut arena);
                let a = variable(&mut arena, 2);
                let b = variable(&mut arena, 1);
                let p = variable(&mut arena, 0);
                let symmetrized = family.id_sym(&mut arena, zero.clone(), ty, a, b, p);
                let expected = two_identity(&mut arena, 1, 2);
                check_type(&mut arena, &context, symmetrized, expected, &mut budget).unwrap();

                let sym_p = {
                    let ty = two(&mut arena);
                    let a = variable(&mut arena, 2);
                    let b = variable(&mut arena, 1);
                    let p = variable(&mut arena, 0);
                    family.id_sym(&mut arena, zero.clone(), ty, a, b, p)
                };
                let trans_sym_p = {
                    let ty = two(&mut arena);
                    let b = variable(&mut arena, 1);
                    let a = variable(&mut arena, 2);
                    let b2 = variable(&mut arena, 1);
                    let p = variable(&mut arena, 0);
                    family.id_trans(&mut arena, zero.clone(), ty, b, a, b2, sym_p, p)
                };
                let ty = two(&mut arena);
                let a = variable(&mut arena, 2);
                let b = variable(&mut arena, 1);
                let p = variable(&mut arena, 0);
                let cancelled = family.id_cancel(&mut arena, zero.clone(), ty, a, b, p);
                let expected = {
                    let id_bb = two_identity(&mut arena, 1, 1);
                    let ty = two(&mut arena);
                    let b = variable(&mut arena, 1);
                    let refl_b = refl(&mut arena, ty, b);
                    id(&mut arena, id_bb, trans_sym_p, refl_b)
                };
                check_type(&mut arena, &context, cancelled, expected, &mut budget).unwrap();
            }

            #[test]
            fn quotient_prop_is_set_and_box_prop_close_propositions() {
                let mut arena = TermArena::new();
                let mut budget = default_budget();
                let family = two_quotient(&mut arena);
                let signature = quotient_signature(&mut arena);
                let zero = Level::Constant(0);

                let is_prop_two = {
                    let x_domain = two(&mut arena);
                    let codomain = {
                        let y_domain = two(&mut arena);
                        let body = two_identity(&mut arena, 1, 0);
                        pi(&mut arena, y_domain, body)
                    };
                    pi(&mut arena, x_domain, codomain)
                };
                let identity_ab = two_identity(&mut arena, 1, 0);
                let identity_ab2 = two_identity(&mut arena, 2, 1);
                let context = Context::empty()
                    .with_signature(signature)
                    .extend(is_prop_two)
                    .extend(two(&mut arena))
                    .extend(two(&mut arena))
                    .extend(identity_ab)
                    .extend(identity_ab2);

                let ty = two(&mut arena);
                let f = variable(&mut arena, 4);
                let a = variable(&mut arena, 3);
                let b = variable(&mut arena, 2);
                let p = variable(&mut arena, 1);
                let q = variable(&mut arena, 0);
                let set_witness = family.prop_is_set(&mut arena, zero.clone(), ty, f);
                let set_witness = apply(&mut arena, set_witness, a);
                let set_witness = apply(&mut arena, set_witness, b);
                let set_witness = apply(&mut arena, set_witness, p);
                let set_witness = apply(&mut arena, set_witness, q);
                let expected = {
                    let id_ab = two_identity(&mut arena, 3, 2);
                    let p = variable(&mut arena, 1);
                    let q = variable(&mut arena, 0);
                    id(&mut arena, id_ab, p, q)
                };
                check_type(&mut arena, &context, set_witness, expected, &mut budget).unwrap();
            }

            #[test]
            fn quotient_box_prop_makes_boxed_strict_types_propositional() {
                let mut arena = TermArena::new();
                let mut budget = default_budget();
                let family = two_quotient(&mut arena);
                let signature = quotient_signature(&mut arena);
                let zero = Level::Constant(0);

                let box_squash_two = {
                    let carrier = two(&mut arena);
                    let inner = squash(&mut arena, carrier);
                    boxed(&mut arena, inner)
                };
                let box_squash_two2 = {
                    let carrier = two(&mut arena);
                    let inner = squash(&mut arena, carrier);
                    boxed(&mut arena, inner)
                };
                let context = Context::empty()
                    .with_signature(signature)
                    .extend(box_squash_two)
                    .extend(box_squash_two2);

                let carrier = two(&mut arena);
                let strict_ty = squash(&mut arena, carrier);
                let x = variable(&mut arena, 1);
                let y = variable(&mut arena, 0);
                let prop = family.box_prop(&mut arena, zero.clone(), strict_ty, x, y);
                let expected = {
                    let ty = {
                        let carrier = two(&mut arena);
                        let inner = squash(&mut arena, carrier);
                        boxed(&mut arena, inner)
                    };
                    let x = variable(&mut arena, 1);
                    let y = variable(&mut arena, 0);
                    id(&mut arena, ty, x, y)
                };
                check_type(&mut arena, &context, prop, expected, &mut budget).unwrap();

                let x = variable(&mut arena, 1);
                let y = variable(&mut arena, 0);
                let shared = {
                    let carrier = two(&mut arena);
                    let inner = squash(&mut arena, carrier);
                    boxed(&mut arena, inner)
                };
                assert!(
                    !convertible(&mut arena, &context, x, y, shared, &mut budget).unwrap(),
                    "boxed inhabitants are propositionally, not definitionally, equal"
                );
            }

            #[test]
            fn quotient_ind_prop_reaches_strict_targets_through_boxing() {
                let mut arena = TermArena::new();
                let mut budget = default_budget();
                let family = two_quotient(&mut arena);
                let signature = quotient_signature(&mut arena);
                let zero = Level::Constant(0);

                let strict_motive = {
                    let domain = family.quotient(&mut arena);
                    let inner = two(&mut arena);
                    let body = squash(&mut arena, inner);
                    lambda(&mut arena, domain, body)
                };
                let case = {
                    let domain = two(&mut arena);
                    let bound = variable(&mut arena, 0);
                    let inner = two(&mut arena);
                    let body = squash_intro(&mut arena, inner, bound);
                    lambda(&mut arena, domain, body)
                };
                let element = family.quotient(&mut arena);
                let context = Context::empty().with_signature(signature).extend(element);

                let z = variable(&mut arena, 0);
                let induced = family.ind_prop(&mut arena, zero.clone(), strict_motive, case, z);
                let inner = two(&mut arena);
                let expected = squash(&mut arena, inner);
                check_type(&mut arena, &context, induced, expected, &mut budget).unwrap();

                let relevant_motive = {
                    let domain = family.quotient(&mut arena);
                    let body = two(&mut arena);
                    lambda(&mut arena, domain, body)
                };
                let case = {
                    let domain = two(&mut arena);
                    let body = variable(&mut arena, 0);
                    lambda(&mut arena, domain, body)
                };
                let z = variable(&mut arena, 0);
                let wrong = family.ind_prop(&mut arena, zero, relevant_motive, case, z);
                let error = infer_type(&mut arena, &context, wrong, &mut budget).unwrap_err();
                assert!(
                    matches!(error, CoreError::ArgumentTypeMismatch { .. }),
                    "a Type-valued motive is not proposition-valued: {error:?}"
                );
            }

            #[test]
            fn quotient_unique_collapses_sections_agreeing_on_projections() {
                let mut arena = TermArena::new();
                let mut budget = default_budget();
                let family = two_quotient(&mut arena);
                let signature = quotient_signature(&mut arena);
                let zero = Level::Constant(0);

                let two_ty = two(&mut arena);
                let set_b = is_set_zero(&mut arena, two_ty);
                let section = {
                    let domain = family.quotient(&mut arena);
                    let codomain = two(&mut arena);
                    pi(&mut arena, domain, codomain)
                };
                let section2 = {
                    let domain = family.quotient(&mut arena);
                    let codomain = two(&mut arena);
                    pi(&mut arena, domain, codomain)
                };
                let agreement = {
                    let a_domain = two(&mut arena);
                    let codomain = {
                        let a = variable(&mut arena, 0);
                        let projected = family.project(&mut arena, a);
                        let s1 = variable(&mut arena, 2);
                        let left = apply(&mut arena, s1, projected);
                        let a = variable(&mut arena, 0);
                        let projected = family.project(&mut arena, a);
                        let s2 = variable(&mut arena, 1);
                        let right = apply(&mut arena, s2, projected);
                        let ty = two(&mut arena);
                        id(&mut arena, ty, left, right)
                    };
                    pi(&mut arena, a_domain, codomain)
                };
                let element = family.quotient(&mut arena);
                let context = Context::empty()
                    .with_signature(signature)
                    .extend(set_b)
                    .extend(section)
                    .extend(section2)
                    .extend(agreement)
                    .extend(element);

                let constant_family = {
                    let domain = family.quotient(&mut arena);
                    let codomain = two(&mut arena);
                    lambda(&mut arena, domain, codomain)
                };
                let sets = {
                    let domain = family.quotient(&mut arena);
                    let set_b = variable(&mut arena, 5);
                    lambda(&mut arena, domain, set_b)
                };

                let s1 = variable(&mut arena, 3);
                let s2 = variable(&mut arena, 2);
                let h = variable(&mut arena, 1);
                let z = variable(&mut arena, 0);
                let unique = family.unique(
                    &mut arena,
                    zero.clone(),
                    constant_family,
                    sets,
                    s1,
                    s2,
                    h,
                    z,
                );
                let expected = {
                    let ty = two(&mut arena);
                    let s1 = variable(&mut arena, 3);
                    let z = variable(&mut arena, 0);
                    let left = apply(&mut arena, s1, z);
                    let s2 = variable(&mut arena, 2);
                    let z = variable(&mut arena, 0);
                    let right = apply(&mut arena, s2, z);
                    id(&mut arena, ty, left, right)
                };
                check_type(&mut arena, &context, unique, expected, &mut budget).unwrap();

                let s1 = variable(&mut arena, 3);
                let s2 = variable(&mut arena, 2);
                let h = variable(&mut arena, 1);
                let z = variable(&mut arena, 0);
                let unique = family.unique(&mut arena, zero, constant_family, sets, s1, s2, h, z);
                let function_identity = {
                    let ty = section;
                    let s1 = variable(&mut arena, 3);
                    let s2 = variable(&mut arena, 2);
                    id(&mut arena, ty, s1, s2)
                };
                let error =
                    check_type(&mut arena, &context, unique, function_identity, &mut budget)
                        .unwrap_err();
                assert!(
                    matches!(error, CoreError::TypeMismatch { .. }),
                    "pointwise uniqueness is not function extensionality: {error:?}"
                );
            }
        }
        mod quotient_receiver_policies {
            use std::collections::BTreeSet;

            use crate::mathematical_core::tests::{
                box_elim, box_intro, boxed, default_budget, id, lambda, quotient_signature, sigma,
                squash, squash_elim, squash_intro, two, two_identity, two_quotient,
                two_quotient_over, variable,
            };
            use crate::mathematical_core::{
                Context, CoreError, Level, MathematicalCertificate, QUOTIENT, QUOTIENT_BETA,
                QUOTIENT_EFFECTIVE, QUOTIENT_ELIM, QUOTIENT_LIFT_PRECONDITION, QUOTIENT_PROJECT,
                QUOTIENT_SET, QUOTIENT_SOUND, QuotientFamily, Signature, TermArena,
                assumption_closure, certificate_assumption_closure, check_type, infer_type,
                quotient_scheme, verify_mathematical_certificate,
            };

            fn squashed_two_quotient(arena: &mut TermArena) -> QuotientFamily {
                two_quotient_over(arena, |arena| {
                    let identity = two_identity(arena, 1, 0);
                    let squashed = squash(arena, identity);
                    boxed(arena, squashed)
                })
            }

            #[test]
            fn a_squashed_relation_yields_only_squashed_evidence() {
                let mut arena = TermArena::new();
                let mut budget = default_budget();
                let family = squashed_two_quotient(&mut arena);
                let signature = quotient_signature(&mut arena);

                let identity_ab = two_identity(&mut arena, 1, 0);
                let squashed_evidence = {
                    let body = two_identity(&mut arena, 2, 1);
                    let body = squash(&mut arena, body);
                    boxed(&mut arena, body)
                };
                let projection_identity = {
                    let a = variable(&mut arena, 3);
                    let left = family.project(&mut arena, a);
                    let b = variable(&mut arena, 2);
                    let right = family.project(&mut arena, b);
                    let ty = family.quotient(&mut arena);
                    id(&mut arena, ty, left, right)
                };
                let context = Context::empty()
                    .with_signature(signature)
                    .extend(two(&mut arena))
                    .extend(two(&mut arena))
                    .extend(identity_ab)
                    .extend(squashed_evidence)
                    .extend(projection_identity);

                let congruence = {
                    let a = variable(&mut arena, 4);
                    let b = variable(&mut arena, 3);
                    let r = variable(&mut arena, 1);
                    family.sound(&mut arena, a, b, r)
                };
                let ambient_projection_identity = {
                    let a = variable(&mut arena, 4);
                    let left = family.project(&mut arena, a);
                    let b = variable(&mut arena, 3);
                    let right = family.project(&mut arena, b);
                    let ty = family.quotient(&mut arena);
                    id(&mut arena, ty, left, right)
                };
                check_type(
                    &mut arena,
                    &context,
                    congruence,
                    ambient_projection_identity,
                    &mut budget,
                )
                .unwrap();

                let effectivity = {
                    let a = variable(&mut arena, 4);
                    let b = variable(&mut arena, 3);
                    let p = variable(&mut arena, 0);
                    family.effective(&mut arena, a, b, p)
                };
                let ambient_squashed_identity = {
                    let body = two_identity(&mut arena, 4, 3);
                    let body = squash(&mut arena, body);
                    boxed(&mut arena, body)
                };
                check_type(
                    &mut arena,
                    &context,
                    effectivity,
                    ambient_squashed_identity,
                    &mut budget,
                )
                .unwrap();

                let unboxed = {
                    let motive = {
                        let body = {
                            let body = two_identity(&mut arena, 5, 4);
                            squash(&mut arena, body)
                        };
                        lambda(&mut arena, ambient_squashed_identity, body)
                    };
                    let body = {
                        let domain = {
                            let body = two_identity(&mut arena, 4, 3);
                            squash(&mut arena, body)
                        };
                        let bound = variable(&mut arena, 0);
                        lambda(&mut arena, domain, bound)
                    };
                    let scrutinee = {
                        let a = variable(&mut arena, 4);
                        let b = variable(&mut arena, 3);
                        let p = variable(&mut arena, 0);
                        family.effective(&mut arena, a, b, p)
                    };
                    box_elim(&mut arena, motive, body, scrutinee)
                };
                let strict_evidence = {
                    let body = two_identity(&mut arena, 4, 3);
                    squash(&mut arena, body)
                };
                check_type(&mut arena, &context, unboxed, strict_evidence, &mut budget).unwrap();

                let extraction = {
                    let proposition = two_identity(&mut arena, 4, 3);
                    let function = {
                        let domain = two_identity(&mut arena, 4, 3);
                        let bound = variable(&mut arena, 0);
                        lambda(&mut arena, domain, bound)
                    };
                    squash_elim(&mut arena, proposition, function, unboxed)
                };
                let error = infer_type(&mut arena, &context, extraction, &mut budget).unwrap_err();
                assert!(
                    matches!(error, CoreError::SquashTargetNotStrict { .. }),
                    "squashed relation evidence never opens to the relevant witness: {error:?}"
                );

                let raw_congruence = {
                    let a = variable(&mut arena, 4);
                    let b = variable(&mut arena, 3);
                    let q = variable(&mut arena, 2);
                    family.sound(&mut arena, a, b, q)
                };
                let error =
                    infer_type(&mut arena, &context, raw_congruence, &mut budget).unwrap_err();
                assert!(
                    matches!(error, CoreError::ArgumentTypeMismatch { .. }),
                    "a relevant identity witness is not evidence of the squashed relation: {error:?}"
                );
                let selected = {
                    let identity = two_identity(&mut arena, 4, 3);
                    let q = variable(&mut arena, 2);
                    let squashed = squash_intro(&mut arena, identity, q);
                    let strict = {
                        let body = two_identity(&mut arena, 4, 3);
                        squash(&mut arena, body)
                    };
                    box_intro(&mut arena, strict, squashed)
                };
                let selected_congruence = {
                    let a = variable(&mut arena, 4);
                    let b = variable(&mut arena, 3);
                    family.sound(&mut arena, a, b, selected)
                };
                check_type(
                    &mut arena,
                    &context,
                    selected_congruence,
                    ambient_projection_identity,
                    &mut budget,
                )
                .unwrap();

                let certificate = {
                    let projection_identity = {
                        let a = variable(&mut arena, 1);
                        let left = family.project(&mut arena, a);
                        let b = variable(&mut arena, 0);
                        let right = family.project(&mut arena, b);
                        let ty = family.quotient(&mut arena);
                        id(&mut arena, ty, left, right)
                    };
                    let term = {
                        let a = variable(&mut arena, 2);
                        let b = variable(&mut arena, 1);
                        let p = variable(&mut arena, 0);
                        family.effective(&mut arena, a, b, p)
                    };
                    let expected = {
                        let body = two_identity(&mut arena, 2, 1);
                        let body = squash(&mut arena, body);
                        boxed(&mut arena, body)
                    };
                    MathematicalCertificate {
                        signature: quotient_scheme(&mut arena),
                        level_arity: 0,
                        context: vec![two(&mut arena), two(&mut arena), projection_identity],
                        term,
                        expected,
                    }
                };
                verify_mathematical_certificate(&mut arena, &certificate, &mut budget).unwrap();
                let closure = certificate_assumption_closure(&arena, &certificate);
                let expected_closure: BTreeSet<u32> =
                    [QUOTIENT, QUOTIENT_PROJECT, QUOTIENT_EFFECTIVE]
                        .into_iter()
                        .collect();
                assert_eq!(
                    closure, expected_closure,
                    "the squashed effectivity judgment commits to exactly the interface it cites"
                );
            }

            #[test]
            fn a_quotient_refusing_policy_distinguishes_the_same_certificate() {
                let mut arena = TermArena::new();
                let mut budget = default_budget();
                let family = two_quotient(&mut arena);
                let declarations = quotient_scheme(&mut arena);
                let signature = Signature::from_declarations(declarations.clone());

                let element = family.quotient(&mut arena);
                let covered = {
                    let z = variable(&mut arena, 0);
                    family.coverage(&mut arena, z)
                };
                let covered_type = {
                    let a_domain = two(&mut arena);
                    let codomain = {
                        let a = variable(&mut arena, 0);
                        let projected = family.project(&mut arena, a);
                        let z = variable(&mut arena, 1);
                        let ty = family.quotient(&mut arena);
                        id(&mut arena, ty, projected, z)
                    };
                    let packed = sigma(&mut arena, a_domain, codomain);
                    squash(&mut arena, packed)
                };
                let quotient_certificate = MathematicalCertificate::with_signature(
                    declarations.clone(),
                    vec![element],
                    covered,
                    covered_type,
                );
                verify_mathematical_certificate(&mut arena, &quotient_certificate, &mut budget)
                    .unwrap();

                let identity_ab = two_identity(&mut arena, 1, 0);
                let symmetrized = {
                    let ty = two(&mut arena);
                    let a = variable(&mut arena, 2);
                    let b = variable(&mut arena, 1);
                    let p = variable(&mut arena, 0);
                    family.id_sym(&mut arena, Level::Constant(0), ty, a, b, p)
                };
                let symmetrized_type = two_identity(&mut arena, 1, 2);
                let representative_certificate = MathematicalCertificate::with_signature(
                    declarations.clone(),
                    vec![two(&mut arena), two(&mut arena), identity_ab],
                    symmetrized,
                    symmetrized_type,
                );
                verify_mathematical_certificate(
                    &mut arena,
                    &representative_certificate,
                    &mut budget,
                )
                .unwrap();

                let forged = MathematicalCertificate::with_signature(
                    declarations,
                    vec![element],
                    covered,
                    {
                        let carrier = two(&mut arena);
                        squash(&mut arena, carrier)
                    },
                );
                let error =
                    verify_mathematical_certificate(&mut arena, &forged, &mut default_budget())
                        .unwrap_err();
                assert!(
                    matches!(error, CoreError::TypeMismatch { .. }),
                    "a forged certificate is refused before any policy runs: {error:?}"
                );

                let quotient_closure =
                    certificate_assumption_closure(&arena, &quotient_certificate);
                let expected_quotient_closure: BTreeSet<u32> =
                    [QUOTIENT, QUOTIENT_PROJECT, QUOTIENT_SOUND, QUOTIENT_ELIM]
                        .into_iter()
                        .collect();
                assert_eq!(quotient_closure, expected_quotient_closure);
                let representative_closure =
                    certificate_assumption_closure(&arena, &representative_certificate);
                assert!(
                    representative_closure.is_empty(),
                    "a checked definition drags no admitted assumption into the closure"
                );

                let quotient_refusing: BTreeSet<u32> = BTreeSet::new();
                let quotient_admitting: BTreeSet<u32> = [
                    QUOTIENT,
                    QUOTIENT_PROJECT,
                    QUOTIENT_SET,
                    QUOTIENT_SOUND,
                    QUOTIENT_EFFECTIVE,
                    QUOTIENT_ELIM,
                    QUOTIENT_BETA,
                    QUOTIENT_LIFT_PRECONDITION,
                ]
                .into_iter()
                .collect();
                assert!(representative_closure.is_subset(&quotient_refusing));
                assert!(!quotient_closure.is_subset(&quotient_refusing));
                assert!(representative_closure.is_subset(&quotient_admitting));
                assert!(quotient_closure.is_subset(&quotient_admitting));

                let effectivity_closure =
                    assumption_closure(&arena, &signature, &[QUOTIENT_EFFECTIVE]);
                let expected_effectivity_closure: BTreeSet<u32> =
                    [QUOTIENT, QUOTIENT_PROJECT, QUOTIENT_EFFECTIVE]
                        .into_iter()
                        .collect();
                assert_eq!(effectivity_closure, expected_effectivity_closure);
                let elim_refusing: BTreeSet<u32> = [QUOTIENT, QUOTIENT_PROJECT, QUOTIENT_EFFECTIVE]
                    .into_iter()
                    .collect();
                assert!(effectivity_closure.is_subset(&elim_refusing));
                assert!(!quotient_closure.is_subset(&elim_refusing));
            }
        }
        mod quotient_schemes {
            use crate::mathematical_core::tests::{
                apply, constant, default_budget, id, is_set_zero, lambda, pi, quotient_signature,
                two, two_identity, two_quotient, type_sort, variable,
            };
            use crate::mathematical_core::{
                Context, CoreError, Declaration, Level, QUOTIENT_ELIM, QUOTIENT_IS_SET,
                QUOTIENT_LIFT, QUOTIENT_SOUND, TermArena, check_signature, check_type, infer_type,
                quotient_scheme,
            };

            #[test]
            fn quotient_beta_is_propositional_evidence_not_conversion() {
                let mut arena = TermArena::new();
                let mut budget = default_budget();
                let family = two_quotient(&mut arena);
                let signature = quotient_signature(&mut arena);
                let zero = Level::Constant(0);

                let family_type = {
                    let domain = family.quotient(&mut arena);
                    let codomain = type_sort(&mut arena, 0);
                    pi(&mut arena, domain, codomain)
                };
                let sets_type = {
                    let domain = family.quotient(&mut arena);
                    let bound = variable(&mut arena, 0);
                    let family_ref = variable(&mut arena, 1);
                    let applied = apply(&mut arena, family_ref, bound);
                    let codomain = {
                        let is_set =
                            constant(&mut arena, QUOTIENT_IS_SET, vec![Level::Constant(0)]);
                        apply(&mut arena, is_set, applied)
                    };
                    pi(&mut arena, domain, codomain)
                };
                let case_type = {
                    let domain = two(&mut arena);
                    let bound = variable(&mut arena, 0);
                    let projected = family.project(&mut arena, bound);
                    let family_ref = variable(&mut arena, 2);
                    let codomain = apply(&mut arena, family_ref, projected);
                    pi(&mut arena, domain, codomain)
                };
                let coherence_type = {
                    let a_domain = two(&mut arena);
                    let b_domain = two(&mut arena);
                    let r_domain = two_identity(&mut arena, 1, 0);
                    let codomain = {
                        let b = variable(&mut arena, 1);
                        let projected_b = family.project(&mut arena, b);
                        let p = variable(&mut arena, 5);
                        let p_at_b = apply(&mut arena, p, projected_b);
                        let a = variable(&mut arena, 2);
                        let b = variable(&mut arena, 1);
                        let r = variable(&mut arena, 0);
                        let d = variable(&mut arena, 3);
                        let a2 = variable(&mut arena, 2);
                        let case_at_a = apply(&mut arena, d, a2);
                        let p = variable(&mut arena, 5);
                        let transported =
                            family.transport(&mut arena, zero.clone(), p, a, b, r, case_at_a);
                        let d = variable(&mut arena, 3);
                        let b = variable(&mut arena, 1);
                        let case_at_b = apply(&mut arena, d, b);
                        id(&mut arena, p_at_b, transported, case_at_b)
                    };
                    let inner = pi(&mut arena, r_domain, codomain);
                    let middle = pi(&mut arena, b_domain, inner);
                    pi(&mut arena, a_domain, middle)
                };
                let representative_binding = two(&mut arena);
                let context = Context::empty()
                    .with_signature(signature)
                    .extend(family_type)
                    .extend(sets_type)
                    .extend(case_type)
                    .extend(coherence_type)
                    .extend(representative_binding);

                let p = variable(&mut arena, 4);
                let sets = variable(&mut arena, 3);
                let d = variable(&mut arena, 2);
                let c = variable(&mut arena, 1);
                let a = variable(&mut arena, 0);
                let point = family.beta(&mut arena, zero.clone(), p, sets, d, c, a);
                let expected = {
                    let a = variable(&mut arena, 0);
                    let projected_a = family.project(&mut arena, a);
                    let p = variable(&mut arena, 4);
                    let left_ty = apply(&mut arena, p, projected_a);
                    let p = variable(&mut arena, 4);
                    let sets = variable(&mut arena, 3);
                    let d = variable(&mut arena, 2);
                    let c = variable(&mut arena, 1);
                    let a = variable(&mut arena, 0);
                    let projected = family.project(&mut arena, a);
                    let section = family.elim(&mut arena, zero, p, sets, d, c, projected);
                    let d = variable(&mut arena, 2);
                    let a = variable(&mut arena, 0);
                    let case = apply(&mut arena, d, a);
                    id(&mut arena, left_ty, section, case)
                };
                check_type(&mut arena, &context, point, expected, &mut budget).unwrap();
            }

            #[test]
            fn quotient_lift_rejects_a_malformed_congruence() {
                let mut arena = TermArena::new();
                let mut budget = default_budget();
                let family = two_quotient(&mut arena);
                let signature = quotient_signature(&mut arena);
                let zero = Level::Constant(0);

                let two_ty = two(&mut arena);
                let set_b = is_set_zero(&mut arena, two_ty);
                let operation = {
                    let domain = two(&mut arena);
                    let codomain = two(&mut arena);
                    pi(&mut arena, domain, codomain)
                };
                let flipped = {
                    let domain = two(&mut arena);
                    let inner_domain = two(&mut arena);
                    let related = two_identity(&mut arena, 1, 0);
                    let ty = two(&mut arena);
                    let f = variable(&mut arena, 3);
                    let b = variable(&mut arena, 1);
                    let left = apply(&mut arena, f, b);
                    let f = variable(&mut arena, 3);
                    let a = variable(&mut arena, 2);
                    let right = apply(&mut arena, f, a);
                    let conclusion = id(&mut arena, ty, left, right);
                    let inner = pi(&mut arena, related, conclusion);
                    let middle = pi(&mut arena, inner_domain, inner);
                    pi(&mut arena, domain, middle)
                };
                let element = family.quotient(&mut arena);
                let context = Context::empty()
                    .with_signature(signature)
                    .extend(set_b)
                    .extend(operation)
                    .extend(flipped)
                    .extend(element);

                let set_b = variable(&mut arena, 3);
                let operation = variable(&mut arena, 2);
                let flipped = variable(&mut arena, 1);
                let element = variable(&mut arena, 0);
                let result_ty = two(&mut arena);
                let wrong = family.lift(
                    &mut arena,
                    zero.clone(),
                    result_ty,
                    set_b,
                    operation,
                    flipped,
                    element,
                );
                let error = infer_type(&mut arena, &context, wrong, &mut budget).unwrap_err();
                assert!(
                    matches!(error, CoreError::ArgumentTypeMismatch { .. }),
                    "a flipped congruence is a different theorem: {error:?}"
                );

                let lift = constant(&mut arena, QUOTIENT_LIFT, vec![zero.clone(); 3]);
                let applied = apply(&mut arena, lift, family.carrier);
                let applied = apply(&mut arena, applied, family.relation);
                let result_ty = two(&mut arena);
                let applied = apply(&mut arena, applied, result_ty);
                let set_b = variable(&mut arena, 3);
                let applied = apply(&mut arena, applied, set_b);
                let operation = variable(&mut arena, 2);
                let applied = apply(&mut arena, applied, operation);
                let element = variable(&mut arena, 0);
                let missing_theorem = apply(&mut arena, applied, element);
                let error =
                    infer_type(&mut arena, &context, missing_theorem, &mut budget).unwrap_err();
                assert!(
                    matches!(error, CoreError::ArgumentTypeMismatch { .. }),
                    "the section value is not the congruence theorem: {error:?}"
                );
            }

            #[test]
            fn quotient_lift_precondition_admits_the_transport_backed_shape() {
                let mut arena = TermArena::new();
                let mut budget = default_budget();
                let family = two_quotient(&mut arena);
                let signature = quotient_signature(&mut arena);
                let zero = Level::Constant(0);
                let one = Level::Constant(1);

                let two_ty = two(&mut arena);
                let set_b = is_set_zero(&mut arena, two_ty);
                let public = {
                    let domain = family.quotient(&mut arena);
                    let codomain = type_sort(&mut arena, 0);
                    lambda(&mut arena, domain, codomain)
                };
                let representative_premise = {
                    let domain = two(&mut arena);
                    let codomain = type_sort(&mut arena, 0);
                    lambda(&mut arena, domain, codomain)
                };
                let transport_type = {
                    let domain = two(&mut arena);
                    let premise = type_sort(&mut arena, 0);
                    let conclusion = type_sort(&mut arena, 0);
                    let inner = pi(&mut arena, premise, conclusion);
                    pi(&mut arena, domain, inner)
                };
                let operation_type = {
                    let domain = two(&mut arena);
                    let premise = type_sort(&mut arena, 0);
                    let conclusion = two(&mut arena);
                    let inner = pi(&mut arena, premise, conclusion);
                    pi(&mut arena, domain, inner)
                };
                let respect_type = {
                    let a_domain = two(&mut arena);
                    let b_domain = two(&mut arena);
                    let r_domain = two_identity(&mut arena, 1, 0);
                    let ha_domain = type_sort(&mut arena, 0);
                    let hb_domain = type_sort(&mut arena, 0);
                    let codomain = {
                        let f = variable(&mut arena, 5);
                        let a = variable(&mut arena, 4);
                        let left = apply(&mut arena, f, a);
                        let t = variable(&mut arena, 6);
                        let a = variable(&mut arena, 4);
                        let transported = apply(&mut arena, t, a);
                        let ha = variable(&mut arena, 1);
                        let transported = apply(&mut arena, transported, ha);
                        let left = apply(&mut arena, left, transported);
                        let f = variable(&mut arena, 5);
                        let b = variable(&mut arena, 3);
                        let right = apply(&mut arena, f, b);
                        let t = variable(&mut arena, 6);
                        let b = variable(&mut arena, 3);
                        let transported = apply(&mut arena, t, b);
                        let hb = variable(&mut arena, 0);
                        let transported = apply(&mut arena, transported, hb);
                        let right = apply(&mut arena, right, transported);
                        let ty = two(&mut arena);
                        id(&mut arena, ty, left, right)
                    };
                    let inner = pi(&mut arena, hb_domain, codomain);
                    let inner = pi(&mut arena, ha_domain, inner);
                    let inner = pi(&mut arena, r_domain, inner);
                    let middle = pi(&mut arena, b_domain, inner);
                    pi(&mut arena, a_domain, middle)
                };
                let element = family.quotient(&mut arena);
                let premise = type_sort(&mut arena, 0);
                let context = Context::empty()
                    .with_signature(signature)
                    .extend(set_b)
                    .extend(transport_type)
                    .extend(operation_type)
                    .extend(respect_type)
                    .extend(element)
                    .extend(premise);

                let result_ty = two(&mut arena);
                let set_b_var = variable(&mut arena, 5);
                let transport_var = variable(&mut arena, 4);
                let operation_var = variable(&mut arena, 3);
                let respect_var = variable(&mut arena, 2);
                let element_var = variable(&mut arena, 1);
                let premise_var = variable(&mut arena, 0);
                let lifted = family.lift_precondition(
                    &mut arena,
                    zero.clone(),
                    one.clone(),
                    one,
                    result_ty,
                    set_b_var,
                    public,
                    representative_premise,
                    transport_var,
                    operation_var,
                    respect_var,
                    element_var,
                    premise_var,
                );
                let result = two(&mut arena);
                check_type(&mut arena, &context, lifted, result, &mut budget).unwrap();
            }

            #[test]
            fn quotient_scheme_rejects_a_wrong_derived_body() {
                let mut arena = TermArena::new();
                let mut budget = default_budget();
                let mut declarations = quotient_scheme(&mut arena);

                let sound_arity = declarations[QUOTIENT_SOUND as usize].level_arity;
                let sound_ty = declarations[QUOTIENT_SOUND as usize].ty;
                let wrong_body = {
                    let a_ty = type_sort(&mut arena, 0);
                    let rel_ty = {
                        let a = variable(&mut arena, 0);
                        let codomain = type_sort(&mut arena, 1);
                        let a_inner = variable(&mut arena, 1);
                        let inner = pi(&mut arena, a_inner, codomain);
                        pi(&mut arena, a, inner)
                    };
                    let r_var = variable(&mut arena, 0);
                    let r_dom = {
                        let relation = variable(&mut arena, 2);
                        let a = variable(&mut arena, 1);
                        let applied = apply(&mut arena, relation, a);
                        let b = variable(&mut arena, 0);
                        apply(&mut arena, applied, b)
                    };
                    let inner = lambda(&mut arena, r_dom, r_var);
                    let b_ty = variable(&mut arena, 2);
                    let inner = lambda(&mut arena, b_ty, inner);
                    let a_ty2 = variable(&mut arena, 1);
                    let inner = lambda(&mut arena, a_ty2, inner);
                    let inner = lambda(&mut arena, rel_ty, inner);
                    lambda(&mut arena, a_ty, inner)
                };
                declarations[QUOTIENT_SOUND as usize] =
                    Declaration::definition(sound_arity, sound_ty, wrong_body);
                let error = check_signature(&mut arena, &declarations, &mut budget).unwrap_err();
                assert!(
                    matches!(error, crate::argument_error_pair_pattern!()),
                    "a wrong body never passes as the law: {error:?}"
                );
            }

            #[test]
            fn quotient_elim_rejects_missing_set_evidence() {
                let mut arena = TermArena::new();
                let mut budget = default_budget();
                let family = two_quotient(&mut arena);
                let signature = quotient_signature(&mut arena);
                let zero = Level::Constant(0);

                let context = Context::empty()
                    .with_signature(signature)
                    .extend(two(&mut arena));

                let premise_family = {
                    let domain = family.quotient(&mut arena);
                    let codomain = two(&mut arena);
                    lambda(&mut arena, domain, codomain)
                };
                let elim = constant(&mut arena, QUOTIENT_ELIM, vec![zero; 3]);
                let applied = apply(&mut arena, elim, family.carrier);
                let applied = apply(&mut arena, applied, family.relation);
                let applied = apply(&mut arena, applied, premise_family);
                let not_sets = variable(&mut arena, 0);
                let missing = apply(&mut arena, applied, not_sets);
                let error = infer_type(&mut arena, &context, missing, &mut budget).unwrap_err();
                assert!(
                    matches!(error, CoreError::ArgumentTypeMismatch { .. }),
                    "elimination without the setness obligation rejects: {error:?}"
                );
            }
        }
    }
    mod spine_chains {
        use super::{apply, default_budget, lambda, pair, pi, sigma, snd, two, two_zero, variable};
        use crate::mathematical_core::{
            Context, CoreError, Term, TermArena, TermHandle, check_type, infer_type,
        };

        const DEEP: usize = 131_072;
        const SHALLOW: usize = 4_096;

        fn two_telescope(arena: &mut TermArena, n: usize) -> TermHandle {
            let mut tail = two(arena);
            for _ in 0..n {
                let domain = two(arena);
                tail = pi(arena, domain, tail);
            }
            tail
        }

        #[test]
        fn long_lambda_spine_infers_off_the_call_stack() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let context = Context::empty();

            let mut term = two_zero(&mut arena);
            for _ in 0..DEEP {
                let domain = two(&mut arena);
                term = lambda(&mut arena, domain, term);
            }
            let inferred = infer_type(&mut arena, &context, term, &mut budget).unwrap();

            let mut cursor = inferred;
            for _ in 0..DEEP {
                match arena.get(cursor) {
                    Term::Pi { domain, codomain } => {
                        assert!(matches!(arena.get(domain), Term::Two));
                        cursor = codomain;
                    }
                    other => panic!("expected a Pi link, found {other:?}"),
                }
            }
            assert!(matches!(arena.get(cursor), Term::Two));
        }

        #[test]
        fn long_pair_witness_checks_against_a_sigma_chain() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let context = Context::empty();

            let mut witness = two_zero(&mut arena);
            for _ in 0..SHALLOW {
                let first = two_zero(&mut arena);
                witness = pair(&mut arena, first, witness);
            }
            let mut expected = two(&mut arena);
            for _ in 0..SHALLOW {
                let domain = two(&mut arena);
                expected = sigma(&mut arena, domain, expected);
            }
            check_type(&mut arena, &context, witness, expected, &mut budget).unwrap();

            let inferred = infer_type(&mut arena, &context, witness, &mut budget).unwrap();
            let mut cursor = inferred;
            for _ in 0..SHALLOW {
                match arena.get(cursor) {
                    Term::Sigma { domain, codomain } => {
                        assert!(matches!(arena.get(domain), Term::Two));
                        cursor = codomain;
                    }
                    other => panic!("expected a Sigma link, found {other:?}"),
                }
            }
            assert!(matches!(arena.get(cursor), Term::Two));
        }

        #[test]
        fn left_nested_pair_witness_checks_on_the_default_stack() {
            std::thread::spawn(|| {
                let mut arena = TermArena::new();
                let two = two(&mut arena);
                let zero = two_zero(&mut arena);
                let mut witness = zero;
                let mut expected = two;
                for _ in 0..SHALLOW {
                    witness = pair(&mut arena, witness, zero);
                    expected = sigma(&mut arena, expected, two);
                }
                check_type(
                    &mut arena,
                    &Context::empty(),
                    witness,
                    expected,
                    &mut default_budget(),
                )
                .unwrap();
                let inferred = infer_type(
                    &mut arena,
                    &Context::empty(),
                    witness,
                    &mut default_budget(),
                )
                .unwrap();
                assert!(arena.structurally_equal(inferred, expected));
                assert!(matches!(
                    check_type(
                        &mut arena,
                        &Context::empty(),
                        witness,
                        two,
                        &mut default_budget(),
                    ),
                    Err(CoreError::TypeMismatch { expected, .. }) if expected == two
                ));
            })
            .join()
            .expect("both pair components use the default stack");
        }

        #[test]
        fn pair_checking_restores_binders_and_substitutes_only_checked_components() {
            let mut arena = TermArena::new();
            let universe = super::type_sort(&mut arena, 0);
            let a_in_prefix = variable(&mut arena, 0);
            let context = Context::empty().extend(universe).extend(a_in_prefix);
            let two = two(&mut arena);
            let zero = two_zero(&mut arena);
            let bound = variable(&mut arena, 0);
            let identity = lambda(&mut arena, two, bound);
            let identity_type = pi(&mut arena, two, two);
            let ambient_a = variable(&mut arena, 0);
            let a_under_pair = variable(&mut arena, 2);
            let expected = sigma(&mut arena, identity_type, a_under_pair);
            let witness = pair(&mut arena, identity, ambient_a);
            check_type(
                &mut arena,
                &context,
                witness,
                expected,
                &mut default_budget(),
            )
            .unwrap();

            let family = variable(&mut arena, 0);
            let dependent = sigma(&mut arena, universe, family);
            let witness = pair(&mut arena, two, zero);
            check_type(
                &mut arena,
                &Context::empty(),
                witness,
                dependent,
                &mut default_budget(),
            )
            .unwrap();
            let invalid = pair(&mut arena, two, two);
            assert!(matches!(
                check_type(
                    &mut arena,
                    &Context::empty(),
                    invalid,
                    dependent,
                    &mut default_budget(),
                ),
                Err(CoreError::TypeMismatch { expected, .. }) if expected == two
            ));

            let unbound = variable(&mut arena, 0);
            let expected = sigma(&mut arena, identity_type, two);
            let invalid = pair(&mut arena, identity, unbound);
            assert_eq!(
                check_type(
                    &mut arena,
                    &Context::empty(),
                    invalid,
                    expected,
                    &mut default_budget(),
                ),
                Err(CoreError::UnboundVariable {
                    index: 0,
                    context_depth: 0
                })
            );
        }

        #[test]
        fn pair_judgments_refuse_the_first_component_before_visiting_the_second() {
            let mut arena = TermArena::new();
            let dummy = arena.insert(Term::Dummy);
            let unbound = variable(&mut arena, 0);
            let witness = pair(&mut arena, dummy, unbound);
            let two = two(&mut arena);
            let expected = sigma(&mut arena, two, two);
            assert_eq!(
                infer_type(
                    &mut arena,
                    &Context::empty(),
                    witness,
                    &mut default_budget()
                ),
                Err(CoreError::DummyTerm)
            );
            assert_eq!(
                check_type(
                    &mut arena,
                    &Context::empty(),
                    witness,
                    expected,
                    &mut default_budget(),
                ),
                Err(CoreError::DummyTerm)
            );
        }

        #[test]
        fn mixed_constructor_inference_stays_on_the_default_stack() {
            std::thread::spawn(|| {
                let mut arena = TermArena::new();
                let two = two(&mut arena);
                let zero = two_zero(&mut arena);
                let mut witness = zero;
                let mut expected = two;
                for _ in 0..SHALLOW {
                    let function = lambda(&mut arena, two, witness);
                    witness = pair(&mut arena, function, zero);
                    let function_type = pi(&mut arena, two, expected);
                    expected = sigma(&mut arena, function_type, two);
                }
                let inferred = infer_type(
                    &mut arena,
                    &Context::empty(),
                    witness,
                    &mut default_budget(),
                )
                .unwrap();
                assert!(arena.structurally_equal(inferred, expected));
            })
            .join()
            .expect("constructor boundaries use one handle judgment worklist");
        }

        #[test]
        fn mixed_constructor_children_rejoin_their_own_binder_prefix() {
            let mut arena = TermArena::new();
            let two = two(&mut arena);
            let universe = super::type_sort(&mut arena, 0);
            let bound = variable(&mut arena, 0);
            let body = lambda(&mut arena, bound, bound);
            let polymorphic = lambda(&mut arena, universe, body);
            let a_under_x = variable(&mut arena, 1);
            let body_type = pi(&mut arena, bound, a_under_x);
            let polymorphic_type = pi(&mut arena, universe, body_type);
            let expected = sigma(&mut arena, polymorphic_type, two);
            let context = Context::empty().extend(two);
            let witness = pair(&mut arena, polymorphic, bound);
            let inferred =
                infer_type(&mut arena, &context, witness, &mut default_budget()).unwrap();
            assert!(arena.structurally_equal(inferred, expected));

            let invalid_domain = pi(&mut arena, bound, two);
            let invalid = pair(&mut arena, polymorphic, invalid_domain);
            assert_eq!(
                infer_type(&mut arena, &context, invalid, &mut default_budget()),
                Err(CoreError::NotASort {
                    term: bound,
                    actual_type: two
                })
            );
            let unbound = variable(&mut arena, 1);
            let invalid = pair(&mut arena, polymorphic, unbound);
            assert_eq!(
                infer_type(&mut arena, &context, invalid, &mut default_budget()),
                Err(CoreError::UnboundVariable {
                    index: 1,
                    context_depth: 1
                })
            );
        }

        #[test]
        fn mixed_application_and_projection_preserve_order_and_reduction_budget() {
            let mut arena = TermArena::new();
            let two = two(&mut arena);
            let bound = variable(&mut arena, 0);
            let identity = lambda(&mut arena, two, bound);
            let zero = two_zero(&mut arena);
            let one = arena.insert(Term::TwoOne);
            let argument = pair(&mut arena, zero, one);
            let first = arena.insert(Term::Fst { pair: argument });
            let applied = apply(&mut arena, identity, first);
            let mut budget = crate::mathematical_core::Budget::new(0);
            assert_eq!(
                infer_type(&mut arena, &Context::empty(), applied, &mut budget),
                Ok(two)
            );
            assert_eq!(
                budget.remaining(),
                0,
                "inference does not evaluate the lambda body"
            );

            let dummy = arena.insert(Term::Dummy);
            let invalid = apply(&mut arena, dummy, bound);
            assert_eq!(
                infer_type(
                    &mut arena,
                    &Context::empty(),
                    invalid,
                    &mut default_budget()
                ),
                Err(CoreError::DummyTerm)
            );
        }

        #[test]
        fn dependent_pair_arguments_reenter_inference_on_the_default_stack() {
            std::thread::spawn(|| {
                let mut arena = TermArena::new();
                let two = two(&mut arena);
                let zero = two_zero(&mut arena);
                let universe = super::type_sort(&mut arena, 0);
                let family = variable(&mut arena, 0);
                let argument_type = sigma(&mut arena, universe, family);
                let function = lambda(&mut arena, argument_type, zero);
                for (seed, valid) in [(zero, true), (two, false)] {
                    let mut term = seed;
                    for _ in 0..SHALLOW {
                        let argument = pair(&mut arena, two, term);
                        term = apply(&mut arena, function, argument);
                    }
                    let mut budget = crate::mathematical_core::Budget::new(0);
                    let result = infer_type(&mut arena, &Context::empty(), term, &mut budget);
                    if valid {
                        let inferred =
                            result.expect("the complete dependent application tree is well typed");
                        assert!(arena.structurally_equal(inferred, two));
                    } else {
                        let CoreError::TypeMismatch { expected, actual } = result.unwrap_err()
                        else {
                            panic!("the wrong inner payload must fail its Two obligation");
                        };
                        assert_eq!(arena.get(expected), Term::Two);
                        assert_eq!(
                            arena.get(actual),
                            Term::Sort(crate::mathematical_core::Sort::Type(
                                crate::mathematical_core::Level::Constant(0)
                            ))
                        );
                    }
                    assert_eq!(budget.remaining(), 0);
                }
            })
            .join()
            .expect("inference and dependent argument checking share the default stack");
        }

        #[test]
        fn nested_case_branches_check_on_the_default_stack_including_the_unselected_branch() {
            std::thread::spawn(|| {
                let mut arena = TermArena::new();
                let two = two(&mut arena);
                let zero = two_zero(&mut arena);
                let one = arena.insert(Term::TwoOne);
                let motive = lambda(&mut arena, two, two);
                let expected = apply(&mut arena, motive, zero);
                let mut term = zero;
                for _ in 0..SHALLOW {
                    term = arena.insert(Term::CaseTwo {
                        motive,
                        zero_branch: term,
                        one_branch: one,
                        scrutinee: zero,
                    });
                }
                let inferred =
                    infer_type(&mut arena, &Context::empty(), term, &mut default_budget()).unwrap();
                assert!(arena.structurally_equal(inferred, expected));

                let mut invalid = arena.insert(Term::CaseTwo {
                    motive,
                    zero_branch: zero,
                    one_branch: two,
                    scrutinee: zero,
                });
                for _ in 0..SHALLOW {
                    invalid = arena.insert(Term::CaseTwo {
                        motive,
                        zero_branch: invalid,
                        one_branch: one,
                        scrutinee: zero,
                    });
                }
                let CoreError::TypeMismatch { expected, actual } = infer_type(
                    &mut arena,
                    &Context::empty(),
                    invalid,
                    &mut default_budget(),
                )
                .unwrap_err() else {
                    panic!("the unselected branch still owes its motive application");
                };
                let Term::Apply { function, argument } = arena.get(expected) else {
                    panic!("branch obligation keeps the original motive application");
                };
                assert_eq!(function, motive);
                assert_eq!(arena.get(argument), Term::TwoOne);
                assert_eq!(
                    arena.get(actual),
                    Term::Sort(crate::mathematical_core::Sort::Type(
                        crate::mathematical_core::Level::Constant(0)
                    ))
                );
            })
            .join()
            .expect("all case premises use default-stack judgment continuations");
        }

        #[test]
        fn long_application_spine_consumes_arguments_in_order() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let function_type = two_telescope(&mut arena, SHALLOW);
            let context = Context::empty().extend(function_type);
            let mut term = variable(&mut arena, 0);
            for _ in 0..SHALLOW {
                let argument = two_zero(&mut arena);
                term = apply(&mut arena, term, argument);
            }
            let inferred = infer_type(&mut arena, &context, term, &mut budget).unwrap();
            assert!(matches!(arena.get(inferred), Term::Two));

            let mut over_applied = term;
            for _ in 0..2 {
                let argument = two_zero(&mut arena);
                over_applied = apply(&mut arena, over_applied, argument);
            }
            let error = infer_type(&mut arena, &context, over_applied, &mut budget).unwrap_err();
            match error {
                CoreError::NotAFunction {
                    function,
                    actual_type,
                } => {
                    assert_eq!(function, term);
                    assert!(matches!(arena.get(actual_type), Term::Two));
                }
                other => panic!("expected NotAFunction, found {other:?}"),
            }
        }

        #[test]
        fn long_snd_chain_projects_off_the_call_stack() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let mut w_type = two(&mut arena);
            for _ in 0..SHALLOW {
                let domain = two(&mut arena);
                w_type = sigma(&mut arena, domain, w_type);
            }
            let context = Context::empty().extend(w_type);
            let mut term = variable(&mut arena, 0);
            for _ in 0..SHALLOW {
                term = snd(&mut arena, term);
            }
            let inferred = infer_type(&mut arena, &context, term, &mut budget).unwrap();
            assert!(matches!(arena.get(inferred), Term::Two));
        }

        #[test]
        fn a_chain_rejects_at_its_end() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let context = Context::empty();

            let out_of_scope = u32::try_from(SHALLOW).unwrap();
            let mut term = variable(&mut arena, out_of_scope);
            for _ in 0..SHALLOW {
                let domain = two(&mut arena);
                term = lambda(&mut arena, domain, term);
            }
            let error = infer_type(&mut arena, &context, term, &mut budget).unwrap_err();
            assert_eq!(
                error,
                CoreError::UnboundVariable {
                    index: out_of_scope,
                    context_depth: SHALLOW,
                }
            );
        }

        #[test]
        fn deep_binder_domains_form_on_the_default_stack() {
            std::thread::spawn(|| {
                let mut arena = TermArena::new();
                let two = two(&mut arena);
                let mut term = two;
                for _ in 0..DEEP {
                    term = pi(&mut arena, term, two);
                }
                let inferred =
                    infer_type(&mut arena, &Context::empty(), term, &mut default_budget())
                        .expect("the complete left-nested binder tree is a valid relevant type");
                assert_eq!(
                    arena.get(inferred),
                    Term::Sort(crate::mathematical_core::Sort::Type(
                        crate::mathematical_core::Level::Constant(0)
                    ))
                );
            })
            .join()
            .expect("binder-domain formation uses the default stack");
        }

        #[test]
        fn binder_formation_restores_the_exact_prefix_between_children() {
            let mut arena = TermArena::new();
            let universe = super::type_sort(&mut arena, 0);
            let a = variable(&mut arena, 0);
            let a_under_x = variable(&mut arena, 1);
            let inner = pi(&mut arena, a, a_under_x);
            let term = pi(&mut arena, universe, inner);
            let inferred =
                infer_type(&mut arena, &Context::empty(), term, &mut default_budget()).unwrap();
            assert_eq!(
                arena.get(inferred),
                Term::Sort(crate::mathematical_core::Sort::Type(
                    crate::mathematical_core::Level::Constant(1)
                ))
            );
            let proposition = arena.insert(Term::Squash { ty: a });
            let boxed = arena.insert(Term::Box { ty: proposition });
            let inner = pi(&mut arena, boxed, a_under_x);
            let composed = pi(&mut arena, universe, inner);
            let inferred = infer_type(
                &mut arena,
                &Context::empty(),
                composed,
                &mut default_budget(),
            )
            .unwrap();
            assert_eq!(
                arena.get(inferred),
                Term::Sort(crate::mathematical_core::Sort::Type(
                    crate::mathematical_core::Level::Constant(1)
                ))
            );
            let invalid = pi(&mut arena, a, universe);
            assert_eq!(
                infer_type(
                    &mut arena,
                    &Context::empty(),
                    invalid,
                    &mut default_budget()
                ),
                Err(CoreError::UnboundVariable {
                    index: 0,
                    context_depth: 0
                })
            );
            let two = super::two(&mut arena);
            let invalid_sort = pi(&mut arena, two, a);
            assert!(
                matches!(
                    infer_type(
                        &mut arena,
                        &Context::empty(),
                        invalid_sort,
                        &mut default_budget()
                    ),
                    Err(CoreError::NotASort { .. })
                ),
                "a bound value does not become a type"
            );
        }

        #[test]
        fn deep_relevant_and_strict_formations_preserve_layer_refusals() {
            std::thread::spawn(|| {
                let mut arena = TermArena::new();
                let two = two(&mut arena);
                let mut term = two;
                for _ in 0..DEEP / 2 {
                    let proposition = arena.insert(Term::Squash { ty: term });
                    term = arena.insert(Term::Box { ty: proposition });
                }
                let inferred =
                    infer_type(&mut arena, &Context::empty(), term, &mut default_budget()).unwrap();
                assert_eq!(
                    arena.get(inferred),
                    Term::Sort(crate::mathematical_core::Sort::Type(
                        crate::mathematical_core::Level::Constant(0)
                    ))
                );
                let invalid = arena.insert(Term::Box { ty: two });
                let mut nested_invalid = invalid;
                for _ in 0..DEEP / 2 {
                    let proposition = arena.insert(Term::Squash { ty: nested_invalid });
                    nested_invalid = arena.insert(Term::Box { ty: proposition });
                }
                assert_eq!(
                    infer_type(
                        &mut arena,
                        &Context::empty(),
                        nested_invalid,
                        &mut default_budget()
                    ),
                    Err(CoreError::NonStrictBoxDomain { domain: two })
                );
                let empty = arena.insert(Term::Empty);
                let invalid = arena.insert(Term::Squash { ty: empty });
                assert_eq!(
                    infer_type(
                        &mut arena,
                        &Context::empty(),
                        invalid,
                        &mut default_budget()
                    ),
                    Err(CoreError::StrictSquashDomain { domain: empty })
                );
            })
            .join()
            .expect("relevant and strict formation uses the default stack");
        }
    }
    mod strict_layer {
        use super::{
            apply, box_elim, box_intro, boxed, default_budget, empty, empty_elim, lambda, pi,
            sigma, squash, squash_elim, squash_intro, strict_sort, two, type_sort, variable,
        };
        use crate::mathematical_core::{
            Budget, Context, CoreError, Signature, TermArena, check_type, convertible, infer_type,
            weak_head_normalize,
        };

        macro_rules! conv {
            ($a:expr => $b:expr => $c:expr => $d:expr => $e:expr => $f:expr) => {
                convertible($a, $b, $c, $d, $e, $f)
            };
        }

        #[test]
        fn empty_elimination_rejects_non_type_targets_and_foreign_scrutinees() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let strict_zero = strict_sort(&mut arena, 0);
            let type_zero = type_sort(&mut arena, 0);
            let empty_binding = empty(&mut arena);
            let x_binding = variable(&mut arena, 1);
            let context = Context::empty()
                .extend(strict_zero)
                .extend(type_zero)
                .extend(empty_binding)
                .extend(x_binding);

            let scrutinee = variable(&mut arena, 1);
            let bad = empty_elim(&mut arena, scrutinee, scrutinee);
            let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::NotASort { .. }));

            let target = variable(&mut arena, 2);
            let wrong = variable(&mut arena, 0);
            let bad = empty_elim(&mut arena, target, wrong);
            let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));

            let two_target = two(&mut arena);
            let scrutinee = variable(&mut arena, 1);
            let elimination = empty_elim(&mut arena, two_target, scrutinee);
            let inferred = infer_type(&mut arena, &context, elimination, &mut budget).unwrap();
            let expected = two(&mut arena);
            assert!(arena.structurally_equal(inferred, expected));

            let target = variable(&mut arena, 2);
            let scrutinee = variable(&mut arena, 1);
            let elimination = empty_elim(&mut arena, target, scrutinee);
            let wrong_claim = variable(&mut arena, 3);
            let error = check_type(&mut arena, &context, elimination, wrong_claim, &mut budget)
                .unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));
        }

        #[test]
        fn stuck_empty_eliminations_collapse_under_scrutinee_irrelevance() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let type_zero = type_sort(&mut arena, 0);
            let other_type = type_sort(&mut arena, 0);
            let e_binding = empty(&mut arena);
            let f_binding = empty(&mut arena);
            let context = Context::empty()
                .extend(type_zero)
                .extend(other_type)
                .extend(e_binding)
                .extend(f_binding);

            let target = variable(&mut arena, 3);
            let e = variable(&mut arena, 1);
            let left = empty_elim(&mut arena, target, e);
            let target = variable(&mut arena, 3);
            let f = variable(&mut arena, 0);
            let right = empty_elim(&mut arena, target, f);
            let shared = variable(&mut arena, 3);
            assert!(convertible(&mut arena, &context, left, right, shared, &mut budget).unwrap());

            let other_target = variable(&mut arena, 2);
            let e = variable(&mut arena, 1);
            let other = empty_elim(&mut arena, other_target, e);
            let shared = variable(&mut arena, 3);
            assert!(!convertible(&mut arena, &context, left, other, shared, &mut budget).unwrap());
        }

        #[test]
        fn squash_rejects_strict_carriers_and_mismatched_annotations() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let strict_zero = strict_sort(&mut arena, 0);
            let type_zero = type_sort(&mut arena, 0);
            let a_binding = variable(&mut arena, 0);
            let context = Context::empty()
                .extend(strict_zero)
                .extend(type_zero)
                .extend(a_binding);

            let carrier = variable(&mut arena, 2);
            let bad = squash(&mut arena, carrier);
            let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::StrictSquashDomain { .. }));

            let carrier = variable(&mut arena, 2);
            let value = variable(&mut arena, 0);
            let bad = squash_intro(&mut arena, carrier, value);
            let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::StrictSquashDomain { .. }));

            let carrier = variable(&mut arena, 0);
            let bad = squash(&mut arena, carrier);
            let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::NotASort { .. }));

            let carrier = type_sort(&mut arena, 0);
            let value = variable(&mut arena, 0);
            let bad = squash_intro(&mut arena, carrier, value);
            let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));

            let carrier = variable(&mut arena, 1);
            let value = variable(&mut arena, 0);
            let witness = squash_intro(&mut arena, carrier, value);
            let carrier = type_sort(&mut arena, 0);
            let wrong = squash(&mut arena, carrier);
            let error = check_type(&mut arena, &context, witness, wrong, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));
        }

        #[test]
        fn squashed_proofs_are_irrelevant_but_never_extract_to_relevant_data() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let type_zero = type_sort(&mut arena, 0);
            let a_binding = variable(&mut arena, 0);
            let b_binding = variable(&mut arena, 1);
            let s_binding = {
                let carrier = variable(&mut arena, 2);
                squash(&mut arena, carrier)
            };
            let t_binding = {
                let carrier = variable(&mut arena, 3);
                squash(&mut arena, carrier)
            };
            let strict_zero = strict_sort(&mut arena, 0);
            let f_binding = {
                let domain = variable(&mut arena, 5);
                let codomain = variable(&mut arena, 1);
                pi(&mut arena, domain, codomain)
            };
            let g_binding = {
                let domain = variable(&mut arena, 6);
                let codomain = variable(&mut arena, 2);
                pi(&mut arena, domain, codomain)
            };
            let context = Context::empty()
                .extend(type_zero)
                .extend(a_binding)
                .extend(b_binding)
                .extend(s_binding)
                .extend(t_binding)
                .extend(strict_zero)
                .extend(f_binding)
                .extend(g_binding);

            let carrier = variable(&mut arena, 7);
            let shared = squash(&mut arena, carrier);
            let carrier = variable(&mut arena, 7);
            let value = variable(&mut arena, 6);
            let left = squash_intro(&mut arena, carrier, value);
            let carrier = variable(&mut arena, 7);
            let value = variable(&mut arena, 5);
            let right = squash_intro(&mut arena, carrier, value);
            assert!(convertible(&mut arena, &context, left, right, shared, &mut budget).unwrap());
            let neutral_left = variable(&mut arena, 4);
            let neutral_right = variable(&mut arena, 3);
            assert!(
        conv!(&mut arena => &context => neutral_left => neutral_right => shared => &mut budget)
            .unwrap()
            );
            assert!(
                conv!(&mut arena => &context => neutral_left => right => shared => &mut budget)
                    .unwrap()
            );

            let a = variable(&mut arena, 6);
            let b = variable(&mut arena, 5);
            let relevant_shared = variable(&mut arena, 7);
            assert!(
                !convertible(&mut arena, &context, a, b, relevant_shared, &mut budget).unwrap()
            );

            let proposition = variable(&mut arena, 2);
            let function = variable(&mut arena, 1);
            let scrutinee = variable(&mut arena, 4);
            let elimination = squash_elim(&mut arena, proposition, function, scrutinee);
            let expected = variable(&mut arena, 2);
            check_type(&mut arena, &context, elimination, expected, &mut budget).unwrap();

            let proposition = variable(&mut arena, 2);
            let function = variable(&mut arena, 1);
            let scrutinee = variable(&mut arena, 4);
            let left = squash_elim(&mut arena, proposition, function, scrutinee);
            let proposition = variable(&mut arena, 2);
            let function = variable(&mut arena, 0);
            let scrutinee = variable(&mut arena, 4);
            let right = squash_elim(&mut arena, proposition, function, scrutinee);
            let shared = variable(&mut arena, 2);
            assert!(convertible(&mut arena, &context, left, right, shared, &mut budget).unwrap());

            let proposition = two(&mut arena);
            let function = variable(&mut arena, 1);
            let scrutinee = variable(&mut arena, 4);
            let bad = squash_elim(&mut arena, proposition, function, scrutinee);
            let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::SquashTargetNotStrict { .. }));

            let proposition = variable(&mut arena, 2);
            let function = variable(&mut arena, 1);
            let scrutinee = variable(&mut arena, 6);
            let bad = squash_elim(&mut arena, proposition, function, scrutinee);
            let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::NotASquash { .. }));

            let proposition = variable(&mut arena, 2);
            let function = variable(&mut arena, 6);
            let scrutinee = variable(&mut arena, 4);
            let bad = squash_elim(&mut arena, proposition, function, scrutinee);
            let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));
        }

        #[test]
        fn squash_elimination_computes_on_canonical_witnesses() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let type_zero = type_sort(&mut arena, 0);
            let a_binding = variable(&mut arena, 0);
            let strict_zero = strict_sort(&mut arena, 0);
            let f_binding = {
                let domain = variable(&mut arena, 2);
                let codomain = variable(&mut arena, 1);
                pi(&mut arena, domain, codomain)
            };
            let context = Context::empty()
                .extend(type_zero)
                .extend(a_binding)
                .extend(strict_zero)
                .extend(f_binding);

            let carrier = variable(&mut arena, 3);
            let value = variable(&mut arena, 2);
            let witness = squash_intro(&mut arena, carrier, value);
            let proposition = variable(&mut arena, 1);
            let function = variable(&mut arena, 0);
            let elimination = squash_elim(&mut arena, proposition, function, witness);
            let expected = variable(&mut arena, 1);
            check_type(&mut arena, &context, elimination, expected, &mut budget).unwrap();

            let normalized =
                weak_head_normalize(&mut arena, &Signature::new(), elimination, &mut budget)
                    .unwrap();
            let function = variable(&mut arena, 0);
            let value = variable(&mut arena, 2);
            let expected = apply(&mut arena, function, value);
            assert!(arena.structurally_equal(normalized, expected));

            let mut empty_budget = Budget::new(0);
            let error = weak_head_normalize(
                &mut arena,
                &Signature::new(),
                elimination,
                &mut empty_budget,
            )
            .unwrap_err();
            assert_eq!(error, CoreError::StepCeiling);

            let carrier = variable(&mut arena, 3);
            let s_binding = squash(&mut arena, carrier);
            let context = context.extend(s_binding);
            let proposition = variable(&mut arena, 2);
            let function = variable(&mut arena, 1);
            let scrutinee = variable(&mut arena, 0);
            let stuck = squash_elim(&mut arena, proposition, function, scrutinee);
            let expected = variable(&mut arena, 2);
            check_type(&mut arena, &context, stuck, expected, &mut budget).unwrap();
            let normalized =
                weak_head_normalize(&mut arena, &Signature::new(), stuck, &mut budget).unwrap();
            assert!(arena.structurally_equal(normalized, stuck));
        }

        #[test]
        fn box_formation_and_introduction_reject_relevant_domains() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let strict_zero = strict_sort(&mut arena, 0);
            let type_zero = type_sort(&mut arena, 0);
            let a_binding = variable(&mut arena, 0);
            let context = Context::empty()
                .extend(strict_zero)
                .extend(type_zero)
                .extend(a_binding);

            let payload = variable(&mut arena, 1);
            let bad = boxed(&mut arena, payload);
            let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::NonStrictBoxDomain { .. }));

            let payload = variable(&mut arena, 1);
            let value = variable(&mut arena, 0);
            let bad = box_intro(&mut arena, payload, value);
            let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::NonStrictBoxDomain { .. }));

            let payload = variable(&mut arena, 0);
            let bad = boxed(&mut arena, payload);
            let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::NotASort { .. }));

            let payload = variable(&mut arena, 2);
            let value = variable(&mut arena, 0);
            let bad = box_intro(&mut arena, payload, value);
            let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));
        }

        #[test]
        fn unbox_computes_and_its_motive_may_land_at_either_sort() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let strict_zero = strict_sort(&mut arena, 0);
            let p_binding = variable(&mut arena, 0);
            let type_zero = type_sort(&mut arena, 0);
            let c_binding = {
                let payload = variable(&mut arena, 2);
                let domain = boxed(&mut arena, payload);
                let codomain = type_sort(&mut arena, 0);
                pi(&mut arena, domain, codomain)
            };
            let g_binding = {
                let domain = variable(&mut arena, 3);
                let codomain = {
                    let payload = variable(&mut arena, 4);
                    let bound = variable(&mut arena, 0);
                    let boxed_a = box_intro(&mut arena, payload, bound);
                    let family = variable(&mut arena, 1);
                    apply(&mut arena, family, boxed_a)
                };
                pi(&mut arena, domain, codomain)
            };
            let b_binding = {
                let payload = variable(&mut arena, 4);
                boxed(&mut arena, payload)
            };
            let context = Context::empty()
                .extend(strict_zero)
                .extend(p_binding)
                .extend(type_zero)
                .extend(c_binding)
                .extend(g_binding)
                .extend(b_binding);

            let payload = variable(&mut arena, 5);
            let value = variable(&mut arena, 4);
            let scrutinee = box_intro(&mut arena, payload, value);
            let motive = variable(&mut arena, 2);
            let body = variable(&mut arena, 1);
            let elimination = box_elim(&mut arena, motive, body, scrutinee);
            let motive = variable(&mut arena, 2);
            let payload = variable(&mut arena, 5);
            let value = variable(&mut arena, 4);
            let argument = box_intro(&mut arena, payload, value);
            let expected = apply(&mut arena, motive, argument);
            check_type(&mut arena, &context, elimination, expected, &mut budget).unwrap();

            let normalized =
                weak_head_normalize(&mut arena, &Signature::new(), elimination, &mut budget)
                    .unwrap();
            let function = variable(&mut arena, 1);
            let value = variable(&mut arena, 4);
            let expected = apply(&mut arena, function, value);
            assert!(arena.structurally_equal(normalized, expected));

            let mut empty_budget = Budget::new(0);
            let error = weak_head_normalize(
                &mut arena,
                &Signature::new(),
                elimination,
                &mut empty_budget,
            )
            .unwrap_err();
            assert_eq!(error, CoreError::StepCeiling);

            let projection_motive = {
                let payload = variable(&mut arena, 5);
                let domain = boxed(&mut arena, payload);
                let body = variable(&mut arena, 6);
                lambda(&mut arena, domain, body)
            };
            let domain = variable(&mut arena, 5);
            let bound = variable(&mut arena, 0);
            let projection_body = lambda(&mut arena, domain, bound);
            let scrutinee = variable(&mut arena, 0);
            let projection = box_elim(&mut arena, projection_motive, projection_body, scrutinee);
            let expected = variable(&mut arena, 5);
            check_type(&mut arena, &context, projection, expected, &mut budget).unwrap();

            let motive = variable(&mut arena, 2);
            let body = variable(&mut arena, 1);
            let scrutinee = variable(&mut arena, 4);
            let bad = box_elim(&mut arena, motive, body, scrutinee);
            let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::NotABox { .. }));

            let bad_motive = {
                let payload = variable(&mut arena, 5);
                let domain = boxed(&mut arena, payload);
                let body = variable(&mut arena, 5);
                lambda(&mut arena, domain, body)
            };
            let body = variable(&mut arena, 1);
            let scrutinee = variable(&mut arena, 0);
            let bad = box_elim(&mut arena, bad_motive, body, scrutinee);
            let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
            assert!(matches!(
                error,
                CoreError::BoxMotiveCodomainNotAUniverse { .. }
            ));

            let domain = two(&mut arena);
            let body = type_sort(&mut arena, 0);
            let wrong_domain = lambda(&mut arena, domain, body);
            let body = variable(&mut arena, 1);
            let scrutinee = variable(&mut arena, 0);
            let bad = box_elim(&mut arena, wrong_domain, body, scrutinee);
            let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));

            let motive = variable(&mut arena, 2);
            let body = variable(&mut arena, 4);
            let scrutinee = variable(&mut arena, 0);
            let bad = box_elim(&mut arena, motive, body, scrutinee);
            let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));
        }

        #[test]
        fn stuck_unbox_eliminations_compare_componentwise() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let strict_zero = strict_sort(&mut arena, 0);
            let c_binding = {
                let payload = variable(&mut arena, 0);
                let domain = boxed(&mut arena, payload);
                let codomain = type_sort(&mut arena, 0);
                pi(&mut arena, domain, codomain)
            };
            let d_binding = {
                let payload = variable(&mut arena, 1);
                let domain = boxed(&mut arena, payload);
                let codomain = type_sort(&mut arena, 0);
                pi(&mut arena, domain, codomain)
            };
            let g_binding = {
                let domain = variable(&mut arena, 2);
                let codomain = {
                    let payload = variable(&mut arena, 3);
                    let bound = variable(&mut arena, 0);
                    let boxed_a = box_intro(&mut arena, payload, bound);
                    let family = variable(&mut arena, 2);
                    apply(&mut arena, family, boxed_a)
                };
                pi(&mut arena, domain, codomain)
            };
            let h_binding = {
                let domain = variable(&mut arena, 3);
                let codomain = {
                    let payload = variable(&mut arena, 4);
                    let bound = variable(&mut arena, 0);
                    let boxed_a = box_intro(&mut arena, payload, bound);
                    let family = variable(&mut arena, 3);
                    apply(&mut arena, family, boxed_a)
                };
                pi(&mut arena, domain, codomain)
            };
            let g_d_binding = {
                let domain = variable(&mut arena, 4);
                let codomain = {
                    let payload = variable(&mut arena, 5);
                    let bound = variable(&mut arena, 0);
                    let boxed_a = box_intro(&mut arena, payload, bound);
                    let family = variable(&mut arena, 3);
                    apply(&mut arena, family, boxed_a)
                };
                pi(&mut arena, domain, codomain)
            };
            let b_binding = {
                let payload = variable(&mut arena, 5);
                boxed(&mut arena, payload)
            };
            let other_binding = {
                let payload = variable(&mut arena, 6);
                boxed(&mut arena, payload)
            };
            let context = Context::empty()
                .extend(strict_zero)
                .extend(c_binding)
                .extend(d_binding)
                .extend(g_binding)
                .extend(h_binding)
                .extend(g_d_binding)
                .extend(b_binding)
                .extend(other_binding);

            let motive = variable(&mut arena, 6);
            let body = variable(&mut arena, 4);
            let scrutinee = variable(&mut arena, 1);
            let left = box_elim(&mut arena, motive, body, scrutinee);
            let motive = variable(&mut arena, 6);
            let body = variable(&mut arena, 4);
            let scrutinee = variable(&mut arena, 1);
            let again = box_elim(&mut arena, motive, body, scrutinee);
            let family = variable(&mut arena, 6);
            let argument = variable(&mut arena, 1);
            let shared = apply(&mut arena, family, argument);
            assert!(convertible(&mut arena, &context, left, again, shared, &mut budget).unwrap());

            let motive = variable(&mut arena, 6);
            let body = variable(&mut arena, 3);
            let scrutinee = variable(&mut arena, 1);
            let other_body = box_elim(&mut arena, motive, body, scrutinee);
            assert!(
                !convertible(&mut arena, &context, left, other_body, shared, &mut budget).unwrap()
            );

            let motive = variable(&mut arena, 6);
            let body = variable(&mut arena, 4);
            let scrutinee = variable(&mut arena, 0);
            let other_scrutinee = box_elim(&mut arena, motive, body, scrutinee);
            assert!(
                !conv!(&mut arena => &context => left => other_scrutinee => shared => &mut budget)
                    .unwrap()
            );

            let motive = variable(&mut arena, 5);
            let body = variable(&mut arena, 2);
            let scrutinee = variable(&mut arena, 1);
            let other_motive = box_elim(&mut arena, motive, body, scrutinee);
            assert!(
                !conv!(&mut arena => &context => left => other_motive => shared => &mut budget)
                    .unwrap()
            );
        }

        #[test]
        fn empty_proposition_is_typed_only_at_strict_level_zero() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let context = Context::empty();
            let proposition = empty(&mut arena);
            let inferred = infer_type(&mut arena, &context, proposition, &mut budget).unwrap();
            let strict_zero = strict_sort(&mut arena, 0);
            assert!(arena.structurally_equal(inferred, strict_zero));

            let strict_one = strict_sort(&mut arena, 1);
            let error =
                check_type(&mut arena, &context, proposition, strict_one, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));
        }

        #[test]
        fn a_sigma_is_strict_only_when_both_components_are_strict() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let context = Context::empty();
            let cases = [
                (true, true, strict_sort(&mut arena, 0)),
                (false, true, type_sort(&mut arena, 0)),
                (true, false, type_sort(&mut arena, 0)),
                (false, false, type_sort(&mut arena, 0)),
            ];
            for (strict_domain, strict_codomain, expected) in cases {
                let domain = if strict_domain {
                    empty(&mut arena)
                } else {
                    two(&mut arena)
                };
                let codomain = if strict_codomain {
                    empty(&mut arena)
                } else {
                    two(&mut arena)
                };
                let pair_type = sigma(&mut arena, domain, codomain);
                let inferred = infer_type(&mut arena, &context, pair_type, &mut budget).unwrap();
                assert!(
                    arena.structurally_equal(inferred, expected),
                    "domain strict: {strict_domain}, codomain strict: {strict_codomain}"
                );
            }
        }
    }
    mod substitution_worklists {
        use super::{default_budget, pi, two, type_sort, variable};
        use crate::mathematical_core::substitution::{instantiate_levels, shift, substitute};
        use crate::mathematical_core::{Context, Level, Sort, Term, TermArena, check_type};

        #[test]
        fn deep_closed_telescope_remapping_stays_on_default_stack() {
            std::thread::spawn(|| {
                let mut arena = TermArena::new();
                let domain = two(&mut arena);
                let mut term = domain;
                for _ in 0..131_072 {
                    term = pi(&mut arena, domain, term);
                }
                let expected = type_sort(&mut arena, 0);
                check_type(
                    &mut arena,
                    &Context::empty(),
                    term,
                    expected,
                    &mut default_budget(),
                )
                .expect("closed telescope is a valid type before remapping");
                assert_eq!(shift(&mut arena, term, 0, 1), term);
                assert_eq!(substitute(&mut arena, term, domain), term);
                assert_eq!(instantiate_levels(&mut arena, term, &[]), Ok(term));
            })
            .join()
            .expect("term remapping uses a default-stack handle worklist");
        }

        #[test]
        fn shared_variables_keep_their_exact_binder_scope() {
            let mut arena = TermArena::new();
            let zero = variable(&mut arena, 0);
            let two = two(&mut arena);
            let lambda = arena.insert(Term::Lambda {
                domain: two,
                body: zero,
            });
            let term = arena.insert(Term::Pair {
                first: zero,
                second: lambda,
            });
            let shifted = shift(&mut arena, term, 0, 1);
            let Term::Pair { first, second } = arena.get(shifted) else {
                panic!("pair")
            };
            assert!(matches!(arena.get(first), Term::Variable(1)));
            assert_eq!(
                second, lambda,
                "bound zero is unchanged despite sharing its handle with the free zero"
            );
            let replacement = variable(&mut arena, 4);
            let substituted = substitute(&mut arena, term, replacement);
            let Term::Pair { first, second } = arena.get(substituted) else {
                panic!("pair")
            };
            assert_eq!(first, replacement);
            assert_eq!(second, lambda);
            let free_under_binder = variable(&mut arena, 1);
            let body = arena.insert(Term::Lambda {
                domain: two,
                body: free_under_binder,
            });
            let result = substitute(&mut arena, body, replacement);
            let Term::Lambda { body, .. } = arena.get(result) else {
                panic!("lambda")
            };
            assert!(
                matches!(arena.get(body), Term::Variable(5)),
                "argument stays free under the new binder"
            );
            let higher = variable(&mut arena, 2);
            let decremented = substitute(&mut arena, higher, replacement);
            assert!(matches!(arena.get(decremented), Term::Variable(1)));
        }

        #[test]
        fn repeated_subterms_are_remapped_once_per_scope_and_query() {
            std::thread::spawn(|| {
                let mut arena = TermArena::new();
                let leaf = variable(&mut arena, 0);
                let mut term = leaf;
                for _ in 0..131_072 {
                    term = arena.insert(Term::Pair {
                        first: term,
                        second: term,
                    });
                }
                let before = arena.len();
                let mut result = shift(&mut arena, term, 0, 1);
                assert_eq!(
                    arena.len() - before,
                    131_073,
                    "one remapped node per shared input, not one per path"
                );
                for _ in 0..131_072 {
                    let Term::Pair { first, second } = arena.get(result) else {
                        panic!("pair")
                    };
                    assert_eq!(first, second);
                    result = first;
                }
                assert!(matches!(arena.get(result), Term::Variable(1)));
                let result = substitute(&mut arena, leaf, result);
                assert!(
                    matches!(arena.get(result), Term::Variable(1)),
                    "query-local mappings cannot reuse another operation's result"
                );
            })
            .join()
            .expect("shared term remapping");
        }

        #[test]
        fn universe_instantiation_preserves_scope_and_refuses_missing_parameters() {
            let mut arena = TermArena::new();
            let level = Level::Maximum(
                Box::new(Level::Parameter(0)),
                Box::new(Level::Successor(Box::new(Level::Parameter(1)))),
            );
            let sort = arena.insert(Term::Sort(Sort::Type(level.clone())));
            let constant = arena.insert(Term::Constant {
                declaration: 17,
                levels: vec![level],
            });
            let term = arena.insert(Term::Pi {
                domain: sort,
                codomain: constant,
            });
            assert_eq!(
                instantiate_levels(&mut arena, term, &[Level::Constant(2)]),
                Err(1)
            );
            let result =
                instantiate_levels(&mut arena, term, &[Level::Constant(2), Level::Constant(4)])
                    .unwrap();
            let Term::Pi { domain, codomain } = arena.get(result) else {
                panic!("pi")
            };
            let expected = Level::Maximum(
                Box::new(Level::Constant(2)),
                Box::new(Level::Successor(Box::new(Level::Constant(4)))),
            );
            assert_eq!(arena.get(domain), Term::Sort(Sort::Type(expected.clone())));
            assert_eq!(
                arena.get(codomain),
                Term::Constant {
                    declaration: 17,
                    levels: vec![expected]
                }
            );
        }
    }
    mod type_checking {
        use super::{
            apply, branching_family, case_two, default_budget, fst, lambda, pair, pi, sigma, snd,
            strict_sort, two, two_one, two_zero, type_sort, universe_motive, variable,
        };
        use crate::mathematical_core::{
            Budget, Context, CoreError, Signature, TermArena, check_type, convertible, infer_type,
            weak_head_normalize,
        };

        macro_rules! conv {
            ($a:expr => $b:expr => $c:expr => $d:expr => $e:expr => $f:expr) => {
                convertible($a, $b, $c, $d, $e, $f)
            };
        }

        #[test]
        fn unbound_variable_rejects() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let type_zero = type_sort(&mut arena, 0);
            let context = Context::empty().extend(type_zero).extend(type_zero);
            let unbound = variable(&mut arena, 3);
            let error = infer_type(&mut arena, &context, unbound, &mut budget).unwrap_err();
            assert_eq!(
                error,
                CoreError::UnboundVariable {
                    index: 3,
                    context_depth: 2,
                }
            );
        }

        #[test]
        fn conversion_refuses_at_the_step_ceiling() {
            let mut arena = TermArena::new();

            let fake_domain = type_sort(&mut arena, 0);
            let self_argument = variable(&mut arena, 0);
            let self_function = variable(&mut arena, 0);
            let self_apply = apply(&mut arena, self_function, self_argument);
            let delta = lambda(&mut arena, fake_domain, self_apply);
            let omega = apply(&mut arena, delta, delta);

            let mut budget = Budget::new(8);
            let error =
                weak_head_normalize(&mut arena, &Signature::new(), omega, &mut budget).unwrap_err();
            assert_eq!(error, CoreError::StepCeiling);
            assert_eq!(budget.remaining(), 0);
        }

        #[test]
        fn dependent_pair_type_tracks_component_sorts() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let empty = Context::empty();
            let type_zero = type_sort(&mut arena, 0);

            let body = variable(&mut arena, 0);
            let pair_type = sigma(&mut arena, type_zero, body);
            let inferred = infer_type(&mut arena, &empty, pair_type, &mut budget).unwrap();
            let expected = type_sort(&mut arena, 1);
            assert!(arena.structurally_equal(inferred, expected));

            let strict_zero = strict_sort(&mut arena, 0);
            let context = Context::empty()
                .extend(type_zero)
                .extend(strict_zero)
                .extend(strict_zero);

            let domain = variable(&mut arena, 2);
            let codomain = variable(&mut arena, 1);
            let mixed = sigma(&mut arena, domain, codomain);
            let inferred = infer_type(&mut arena, &context, mixed, &mut budget).unwrap();
            let expected = type_sort(&mut arena, 0);
            assert!(arena.structurally_equal(inferred, expected));

            let strict_domain = variable(&mut arena, 1);
            let strict_codomain = variable(&mut arena, 1);
            let conjunction = sigma(&mut arena, strict_domain, strict_codomain);
            let inferred = infer_type(&mut arena, &context, conjunction, &mut budget).unwrap();
            let expected = strict_sort(&mut arena, 0);
            assert!(arena.structurally_equal(inferred, expected));
        }

        #[test]
        fn pair_eta_converts_a_neutral_with_its_projections() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let type_zero = type_sort(&mut arena, 0);
            let a_domain = variable(&mut arena, 0);
            let a_codomain = variable(&mut arena, 1);
            let sigma_type = sigma(&mut arena, a_domain, a_codomain);
            let context = Context::empty().extend(type_zero).extend(sigma_type);

            let shared_domain = variable(&mut arena, 1);
            let shared_codomain = variable(&mut arena, 2);
            let shared_type = sigma(&mut arena, shared_domain, shared_codomain);
            let w = variable(&mut arena, 0);
            let w_first = fst(&mut arena, w);
            let w_second = snd(&mut arena, w);
            let reconstructed = pair(&mut arena, w_first, w_second);
            assert!(
                conv!(&mut arena => &context => w => reconstructed => shared_type => &mut budget)
                    .unwrap()
            );
            assert!(
                conv!(&mut arena => &context => reconstructed => w => shared_type => &mut budget)
                    .unwrap()
            );

            let domain_again = variable(&mut arena, 1);
            let codomain_again = variable(&mut arena, 2);
            let sigma_again = sigma(&mut arena, domain_again, codomain_again);
            let context = context.extend(sigma_again);
            let w_other = variable(&mut arena, 0);
            let w_inner = variable(&mut arena, 1);
            let shared_domain = variable(&mut arena, 2);
            let shared_codomain = variable(&mut arena, 3);
            let shared_type = sigma(&mut arena, shared_domain, shared_codomain);
            assert!(
                !conv!(&mut arena => &context => w_inner => w_other => shared_type => &mut budget)
                    .unwrap()
            );
        }

        #[test]
        fn projections_and_pairs_reject_ill_typed_uses() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let type_zero = type_sort(&mut arena, 0);
            let a_type = variable(&mut arena, 0);
            let context = Context::empty().extend(type_zero).extend(a_type);

            let a_term = variable(&mut arena, 0);
            let bad_projection = fst(&mut arena, a_term);
            let error = infer_type(&mut arena, &context, bad_projection, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::NotAPair { .. }));

            let a_first = variable(&mut arena, 0);
            let a_second = variable(&mut arena, 0);
            let misplaced = pair(&mut arena, a_first, a_second);
            let a_expected = variable(&mut arena, 1);
            let error =
                check_type(&mut arena, &context, misplaced, a_expected, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));
        }

        #[test]
        fn function_eta_never_deletes_an_untyped_wrapper() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let type_zero = type_sort(&mut arena, 0);
            let domain_in_prefix = variable(&mut arena, 0);
            let codomain_in_prefix = variable(&mut arena, 1);
            let function_type = pi(&mut arena, domain_in_prefix, codomain_in_prefix);
            let context = Context::empty().extend(type_zero).extend(function_type);

            let lambda_domain = variable(&mut arena, 1);
            let f_under = variable(&mut arena, 1);
            let bound = variable(&mut arena, 0);
            let body = apply(&mut arena, f_under, bound);
            let wrapped = lambda(&mut arena, lambda_domain, body);
            let f = variable(&mut arena, 0);
            let neutral_type = variable(&mut arena, 1);
            assert!(
                !convertible(&mut arena, &context, wrapped, f, neutral_type, &mut budget,).unwrap()
            );

            assert!(
                !convertible(&mut arena, &context, f, wrapped, neutral_type, &mut budget,).unwrap()
            );
        }

        #[test]
        fn dependent_two_elimination_checks_at_branch_specific_types() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let two_type = two(&mut arena);
            let context = Context::empty().extend(two_type);
            let family = branching_family(&mut arena);
            let function_body = two_zero(&mut arena);
            let function_domain = two(&mut arena);
            let function_witness = lambda(&mut arena, function_domain, function_body);
            let one_witness = two_zero(&mut arena);
            let scrutinee = variable(&mut arena, 0);
            let elimination =
                case_two(&mut arena, family, function_witness, one_witness, scrutinee);

            let expected = apply(&mut arena, family, scrutinee);
            check_type(&mut arena, &context, elimination, expected, &mut budget).unwrap();

            let wrong_zero = two_zero(&mut arena);
            let wrong_one_domain = two(&mut arena);
            let wrong_one_body = two_zero(&mut arena);
            let wrong_one = lambda(&mut arena, wrong_one_domain, wrong_one_body);
            let swapped = case_two(&mut arena, family, wrong_zero, one_witness, scrutinee);
            let error = infer_type(&mut arena, &context, swapped, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));
            let swapped = case_two(&mut arena, family, function_witness, wrong_one, scrutinee);
            let error = infer_type(&mut arena, &context, swapped, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));
        }

        #[test]
        fn case_two_computes_on_each_constructor_and_stays_stuck_on_neutrals() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let two_type = two(&mut arena);
            let context = Context::empty().extend(two_type).extend(two_type);
            let family = branching_family(&mut arena);
            let function_body = two_zero(&mut arena);
            let function_domain = two(&mut arena);
            let function_witness = lambda(&mut arena, function_domain, function_body);
            let one_witness = two_zero(&mut arena);

            let zero = two_zero(&mut arena);
            let on_zero = case_two(&mut arena, family, function_witness, one_witness, zero);
            let normalized =
                weak_head_normalize(&mut arena, &Signature::new(), on_zero, &mut budget).unwrap();
            assert_eq!(normalized, function_witness);
            let one = two_one(&mut arena);
            let on_one = case_two(&mut arena, family, function_witness, one_witness, one);
            let normalized =
                weak_head_normalize(&mut arena, &Signature::new(), on_one, &mut budget).unwrap();
            assert_eq!(normalized, one_witness);

            let result_type = infer_type(&mut arena, &context, on_zero, &mut budget).unwrap();
            let normalized_type =
                weak_head_normalize(&mut arena, &Signature::new(), result_type, &mut budget)
                    .unwrap();
            let function_domain = two(&mut arena);
            let function_codomain = two(&mut arena);
            let expected_type = pi(&mut arena, function_domain, function_codomain);
            assert!(arena.structurally_equal(normalized_type, expected_type));
            let result_type = infer_type(&mut arena, &context, on_one, &mut budget).unwrap();
            let normalized_type =
                weak_head_normalize(&mut arena, &Signature::new(), result_type, &mut budget)
                    .unwrap();
            let expected_type = two(&mut arena);
            assert!(arena.structurally_equal(normalized_type, expected_type));

            let t = variable(&mut arena, 1);
            let stuck = case_two(&mut arena, family, function_witness, one_witness, t);
            let normalized =
                weak_head_normalize(&mut arena, &Signature::new(), stuck, &mut budget).unwrap();
            assert_eq!(normalized, stuck);

            let constant_domain = two(&mut arena);
            let constant_body = two(&mut arena);
            let constant = lambda(&mut arena, constant_domain, constant_body);
            let t = variable(&mut arena, 1);
            let u = variable(&mut arena, 0);
            let left = case_two(&mut arena, constant, zero, one, t);
            let left_again = case_two(&mut arena, constant, zero, one, t);
            let right = case_two(&mut arena, constant, zero, one, u);
            assert!(
                conv!(&mut arena => &context => left => left_again => two_type => &mut budget)
                    .unwrap()
            );
            assert!(
                !convertible(&mut arena, &context, left, right, two_type, &mut budget).unwrap()
            );

            let mut empty_budget = Budget::new(0);
            let error =
                weak_head_normalize(&mut arena, &Signature::new(), on_zero, &mut empty_budget)
                    .unwrap_err();
            assert_eq!(error, CoreError::StepCeiling);
        }

        #[test]
        fn case_two_rejects_motive_and_scrutinee_violations() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();

            let strict_zero = strict_sort(&mut arena, 0);
            let type_zero = type_sort(&mut arena, 0);
            let a_type = variable(&mut arena, 1);
            let two_type = two(&mut arena);
            let context = Context::empty()
                .extend(strict_zero)
                .extend(type_zero)
                .extend(a_type)
                .extend(two_type);
            let t = variable(&mut arena, 0);
            let a = variable(&mut arena, 1);

            let strict_body = variable(&mut arena, 4);
            let strict_motive_domain = two(&mut arena);
            let strict_motive = lambda(&mut arena, strict_motive_domain, strict_body);
            let elimination = case_two(&mut arena, strict_motive, a, a, t);
            let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::StrictCaseMotiveCodomain { .. }));

            let term_body = variable(&mut arena, 2);
            let term_motive_domain = two(&mut arena);
            let term_motive = lambda(&mut arena, term_motive_domain, term_body);
            let elimination = case_two(&mut arena, term_motive, a, a, t);
            let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
            assert!(matches!(
                error,
                CoreError::CaseMotiveCodomainNotAUniverse { .. }
            ));

            let wrong_domain = variable(&mut arena, 2);
            let wrong_body = type_sort(&mut arena, 0);
            let wrong_motive = lambda(&mut arena, wrong_domain, wrong_body);
            let elimination = case_two(&mut arena, wrong_motive, a, a, t);
            let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));

            let not_a_function = two_zero(&mut arena);
            let elimination = case_two(&mut arena, not_a_function, a, a, t);
            let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::NotAFunction { .. }));

            let motive = universe_motive(&mut arena);
            let branch_type = type_sort(&mut arena, 0);
            let elimination = case_two(&mut arena, motive, branch_type, branch_type, a);
            let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
            assert!(matches!(error, CoreError::TypeMismatch { .. }));
        }
    }

    use crate::mathematical_core::{
        Budget, Context, DEFAULT_CONVERSION_STEPS, Declaration, IndexedFamily, Level,
        QUOTIENT_IS_SET, QuotientFamily, Signature, Sort, Term, TermArena, TermHandle,
        check_signature, indexed_scheme, quotient_scheme,
    };

    pub(crate) fn type_sort(arena: &mut TermArena, level: u32) -> TermHandle {
        arena.insert(Term::Sort(Sort::Type(Level::Constant(level))))
    }

    pub(crate) fn strict_sort(arena: &mut TermArena, level: u32) -> TermHandle {
        arena.insert(Term::Sort(Sort::Strict(Level::Constant(level))))
    }

    pub(crate) fn variable(arena: &mut TermArena, index: u32) -> TermHandle {
        arena.insert(Term::Variable(index))
    }

    pub(crate) fn pi(
        arena: &mut TermArena,
        domain: TermHandle,
        codomain: TermHandle,
    ) -> TermHandle {
        arena.insert(Term::Pi { domain, codomain })
    }

    pub(crate) fn lambda(
        arena: &mut TermArena,
        domain: TermHandle,
        body: TermHandle,
    ) -> TermHandle {
        arena.insert(Term::Lambda { domain, body })
    }

    fn apply(arena: &mut TermArena, function: TermHandle, argument: TermHandle) -> TermHandle {
        arena.insert(Term::Apply { function, argument })
    }

    pub(crate) fn sigma(
        arena: &mut TermArena,
        domain: TermHandle,
        codomain: TermHandle,
    ) -> TermHandle {
        arena.insert(Term::Sigma { domain, codomain })
    }

    fn pair(arena: &mut TermArena, first: TermHandle, second: TermHandle) -> TermHandle {
        arena.insert(Term::Pair { first, second })
    }

    fn fst(arena: &mut TermArena, pair: TermHandle) -> TermHandle {
        arena.insert(Term::Fst { pair })
    }

    fn snd(arena: &mut TermArena, pair: TermHandle) -> TermHandle {
        arena.insert(Term::Snd { pair })
    }

    fn two(arena: &mut TermArena) -> TermHandle {
        arena.insert(Term::Two)
    }

    fn two_zero(arena: &mut TermArena) -> TermHandle {
        arena.insert(Term::TwoZero)
    }

    fn two_one(arena: &mut TermArena) -> TermHandle {
        arena.insert(Term::TwoOne)
    }

    fn two_identity(arena: &mut TermArena, a: u32, b: u32) -> TermHandle {
        let ty = two(arena);
        let left = variable(arena, a);
        let right = variable(arena, b);
        id(arena, ty, left, right)
    }

    fn case_two(
        arena: &mut TermArena,
        motive: TermHandle,
        zero_branch: TermHandle,
        one_branch: TermHandle,
        scrutinee: TermHandle,
    ) -> TermHandle {
        arena.insert(Term::CaseTwo {
            motive,
            zero_branch,
            one_branch,
            scrutinee,
        })
    }

    fn id(
        arena: &mut TermArena,
        ty: TermHandle,
        left: TermHandle,
        right: TermHandle,
    ) -> TermHandle {
        arena.insert(Term::Id { ty, left, right })
    }

    fn refl(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
        arena.insert(Term::Refl { ty, value })
    }

    fn id_elim(
        arena: &mut TermArena,
        motive: TermHandle,
        base: TermHandle,
        endpoint: TermHandle,
        proof: TermHandle,
    ) -> TermHandle {
        arena.insert(Term::IdElim {
            motive,
            base,
            endpoint,
            proof,
        })
    }

    fn w_type(arena: &mut TermArena, carrier: TermHandle, children: TermHandle) -> TermHandle {
        arena.insert(Term::W { carrier, children })
    }

    fn sup(
        arena: &mut TermArena,
        carrier: TermHandle,
        children: TermHandle,
        label: TermHandle,
        function: TermHandle,
    ) -> TermHandle {
        arena.insert(Term::Sup {
            carrier,
            children,
            label,
            function,
        })
    }

    fn ind_w(
        arena: &mut TermArena,
        motive: TermHandle,
        step: TermHandle,
        tree: TermHandle,
    ) -> TermHandle {
        arena.insert(Term::IndW { motive, step, tree })
    }

    fn empty(arena: &mut TermArena) -> TermHandle {
        arena.insert(Term::Empty)
    }

    fn empty_elim(arena: &mut TermArena, ty: TermHandle, scrutinee: TermHandle) -> TermHandle {
        arena.insert(Term::EmptyElim { ty, scrutinee })
    }

    fn squash(arena: &mut TermArena, ty: TermHandle) -> TermHandle {
        arena.insert(Term::Squash { ty })
    }

    fn squash_intro(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
        arena.insert(Term::SquashIntro { ty, value })
    }

    fn squash_elim(
        arena: &mut TermArena,
        proposition: TermHandle,
        function: TermHandle,
        scrutinee: TermHandle,
    ) -> TermHandle {
        arena.insert(Term::SquashElim {
            proposition,
            function,
            scrutinee,
        })
    }

    fn boxed(arena: &mut TermArena, ty: TermHandle) -> TermHandle {
        arena.insert(Term::Box { ty })
    }

    fn box_intro(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
        arena.insert(Term::BoxIntro { ty, value })
    }

    fn box_elim(
        arena: &mut TermArena,
        motive: TermHandle,
        body: TermHandle,
        scrutinee: TermHandle,
    ) -> TermHandle {
        arena.insert(Term::BoxElim {
            motive,
            body,
            scrutinee,
        })
    }

    pub(crate) fn default_budget() -> Budget {
        Budget::new(DEFAULT_CONVERSION_STEPS)
    }

    fn universe_motive(arena: &mut TermArena) -> TermHandle {
        let domain = two(arena);
        let body = type_sort(arena, 0);
        lambda(arena, domain, body)
    }

    fn branching_family(arena: &mut TermArena) -> TermHandle {
        let motive = universe_motive(arena);
        let domain = two(arena);
        let codomain = two(arena);
        let function_type = pi(arena, domain, codomain);
        let one_landing = two(arena);
        let bound = variable(arena, 0);
        let body = case_two(arena, motive, function_type, one_landing, bound);
        let domain = two(arena);
        lambda(arena, domain, body)
    }

    fn symmetry_motive(arena: &mut TermArena, a: u32, x: u32) -> TermHandle {
        let domain = variable(arena, a);
        let inner = {
            let ty = variable(arena, a + 1);
            let fixed = variable(arena, x + 1);
            let bound = variable(arena, 0);
            let proof_domain = id(arena, ty, fixed, bound);
            let ty = variable(arena, a + 2);
            let bound_y = variable(arena, 1);
            let fixed = variable(arena, x + 2);
            let body = id(arena, ty, bound_y, fixed);
            lambda(arena, proof_domain, body)
        };
        lambda(arena, domain, inner)
    }

    fn w_step_binding(arena: &mut TermArena) -> TermHandle {
        let a_domain = variable(arena, 4);
        let b_at = variable(arena, 4);
        let a_bound = variable(arena, 0);
        let child_positions = apply(arena, b_at, a_bound);
        let carrier = variable(arena, 6);
        let children = variable(arena, 5);
        let w_at_two = w_type(arena, carrier, children);
        let function_type = pi(arena, child_positions, w_at_two);
        let b_at = variable(arena, 5);
        let a_bound = variable(arena, 1);
        let hypothesis_domain = apply(arena, b_at, a_bound);
        let p_at = variable(arena, 3);
        let k_bound = variable(arena, 1);
        let b_bound = variable(arena, 0);
        let child = apply(arena, k_bound, b_bound);
        let hypothesis_codomain = apply(arena, p_at, child);
        let hypothesis = pi(arena, hypothesis_domain, hypothesis_codomain);
        let p_at = variable(arena, 3);
        let carrier = variable(arena, 7);
        let children = variable(arena, 6);
        let label = variable(arena, 2);
        let function = variable(arena, 1);
        let node = sup(arena, carrier, children, label, function);
        let result = apply(arena, p_at, node);
        let inner = pi(arena, hypothesis, result);
        let middle = pi(arena, function_type, inner);
        pi(arena, a_domain, middle)
    }

    fn w_context(arena: &mut TermArena) -> Context {
        let type_zero = type_sort(arena, 0);
        let a_in_prefix = variable(arena, 0);
        let b_binding = pi(arena, a_in_prefix, type_zero);
        let a_binding = variable(arena, 1);
        let b_at = variable(arena, 1);
        let a_at = variable(arena, 0);
        let child_positions = apply(arena, b_at, a_at);
        let carrier = variable(arena, 3);
        let children = variable(arena, 2);
        let w_under_b = w_type(arena, carrier, children);
        let k_binding = pi(arena, child_positions, w_under_b);
        let carrier = variable(arena, 3);
        let children = variable(arena, 2);
        let w = w_type(arena, carrier, children);
        let p_binding = pi(arena, w, type_zero);
        let s_binding = w_step_binding(arena);
        let carrier = variable(arena, 5);
        let children = variable(arena, 4);
        let t_binding = w_type(arena, carrier, children);
        let carrier = variable(arena, 6);
        let children = variable(arena, 5);
        let u_binding = w_type(arena, carrier, children);
        Context::empty()
            .extend(type_zero)
            .extend(b_binding)
            .extend(a_binding)
            .extend(k_binding)
            .extend(p_binding)
            .extend(s_binding)
            .extend(t_binding)
            .extend(u_binding)
    }

    fn concrete_step(arena: &mut TermArena) -> TermHandle {
        let a_domain = variable(arena, 7);
        let b_at = variable(arena, 7);
        let a_bound = variable(arena, 0);
        let child_positions = apply(arena, b_at, a_bound);
        let carrier = variable(arena, 9);
        let children = variable(arena, 8);
        let w_at_two = w_type(arena, carrier, children);
        let function_type = pi(arena, child_positions, w_at_two);
        let b_at = variable(arena, 8);
        let a_bound = variable(arena, 1);
        let hypothesis_domain = apply(arena, b_at, a_bound);
        let hypothesis_codomain = two(arena);
        let hypothesis = pi(arena, hypothesis_domain, hypothesis_codomain);
        let body = two_zero(arena);
        let inner = lambda(arena, hypothesis, body);
        let middle = lambda(arena, function_type, inner);
        lambda(arena, a_domain, middle)
    }

    fn sort_level(arena: &mut TermArena, level: Level) -> TermHandle {
        arena.insert(Term::Sort(Sort::Type(level)))
    }

    fn constant(arena: &mut TermArena, declaration: u32, levels: Vec<Level>) -> TermHandle {
        arena.insert(Term::Constant {
            declaration,
            levels,
        })
    }

    pub(crate) fn polymorphic_identity_declaration(arena: &mut TermArena) -> Declaration {
        let type_u = sort_level(arena, Level::Parameter(0));
        let bound_a = variable(arena, 0);
        let inner_a = variable(arena, 1);
        let inner_pi = pi(arena, bound_a, inner_a);
        let ty = pi(arena, type_u, inner_pi);
        let type_u = sort_level(arena, Level::Parameter(0));
        let bound_a = variable(arena, 0);
        let inner_x = variable(arena, 0);
        let inner = lambda(arena, bound_a, inner_x);
        let body = lambda(arena, type_u, inner);
        Declaration::definition(1, ty, body)
    }

    fn two_indexed_family(arena: &mut TermArena) -> IndexedFamily {
        let index = two(arena);
        let carrier = two(arena);
        let children = {
            let domain = two(arena);
            let ty = two(arena);
            let left = two_one(arena);
            let right = two_one(arena);
            let body = id(arena, ty, left, right);
            lambda(arena, domain, body)
        };
        let out = {
            let domain = two(arena);
            let body = variable(arena, 0);
            lambda(arena, domain, body)
        };
        let next = {
            let domain = two(arena);
            let inner_domain = {
                let ty = two(arena);
                let left = two_one(arena);
                let right = two_one(arena);
                id(arena, ty, left, right)
            };
            let body = variable(arena, 1);
            let inner = lambda(arena, inner_domain, body);
            lambda(arena, domain, inner)
        };
        IndexedFamily {
            levels: [Level::Constant(0), Level::Constant(0), Level::Constant(0)],
            index,
            carrier,
            children,
            out,
            next,
        }
    }

    fn scheme_signature(arena: &mut TermArena) -> Signature {
        let mut budget = default_budget();
        let declarations = indexed_scheme(arena);
        check_signature(arena, &declarations, &mut budget).unwrap()
    }

    fn indexed_context(arena: &mut TermArena, family: &IndexedFamily) -> Context {
        let motive_type = {
            let i_domain = two(arena);
            let i_bound = variable(arena, 0);
            let iw_at_i = family.indexed_w(arena, i_bound);
            let type_zero = type_sort(arena, 0);
            let inner = pi(arena, iw_at_i, type_zero);
            pi(arena, i_domain, inner)
        };
        let step_type = {
            let a_domain = two(arena);
            let g_domain = {
                let a_bound = variable(arena, 0);
                let b_domain = apply(arena, family.children, a_bound);
                let next_ab = {
                    let a_bound = variable(arena, 1);
                    let next_a = apply(arena, family.next, a_bound);
                    let b_bound = variable(arena, 0);
                    apply(arena, next_a, b_bound)
                };
                let codomain = family.indexed_w(arena, next_ab);
                pi(arena, b_domain, codomain)
            };
            let hypothesis_domain = {
                let a_bound = variable(arena, 1);
                let b_domain = apply(arena, family.children, a_bound);
                let codomain = {
                    let next_ab = {
                        let a_bound = variable(arena, 2);
                        let next_a = apply(arena, family.next, a_bound);
                        let b_bound = variable(arena, 0);
                        apply(arena, next_a, b_bound)
                    };
                    let q = variable(arena, 3);
                    let q_at = apply(arena, q, next_ab);
                    let g = variable(arena, 1);
                    let b_bound = variable(arena, 0);
                    let g_b = apply(arena, g, b_bound);
                    apply(arena, q_at, g_b)
                };
                pi(arena, b_domain, codomain)
            };
            let step_result = {
                let q = variable(arena, 3);
                let a_bound = variable(arena, 2);
                let out_a = apply(arena, family.out, a_bound);
                let a_bound = variable(arena, 2);
                let g = variable(arena, 1);
                let node = family.sup(arena, a_bound, g);
                let q_out = apply(arena, q, out_a);
                apply(arena, q_out, node)
            };
            let inner = pi(arena, hypothesis_domain, step_result);
            let middle = pi(arena, g_domain, inner);
            pi(arena, a_domain, middle)
        };
        let a_binding = two(arena);
        let g_binding = {
            let a_bound = variable(arena, 0);
            let b_domain = apply(arena, family.children, a_bound);
            let next_ab = {
                let a_bound = variable(arena, 1);
                let next_a = apply(arena, family.next, a_bound);
                let b_bound = variable(arena, 0);
                apply(arena, next_a, b_bound)
            };
            let codomain = family.indexed_w(arena, next_ab);
            pi(arena, b_domain, codomain)
        };
        Context::empty()
            .with_signature(scheme_signature(arena))
            .extend(motive_type)
            .extend(step_type)
            .extend(a_binding)
            .extend(g_binding)
    }

    fn two_quotient_over(
        arena: &mut TermArena,
        body: impl FnOnce(&mut TermArena) -> TermHandle,
    ) -> QuotientFamily {
        let carrier = two(arena);
        let relation = {
            let domain = two(arena);
            let inner_domain = two(arena);
            let body = body(arena);
            let inner = lambda(arena, inner_domain, body);
            lambda(arena, domain, inner)
        };
        QuotientFamily {
            levels: [Level::Constant(0), Level::Constant(0)],
            carrier,
            relation,
        }
    }

    fn two_quotient(arena: &mut TermArena) -> QuotientFamily {
        two_quotient_over(arena, |arena| two_identity(arena, 1, 0))
    }

    fn quotient_signature(arena: &mut TermArena) -> Signature {
        let mut budget = default_budget();
        let declarations = quotient_scheme(arena);
        check_signature(arena, &declarations, &mut budget).unwrap()
    }

    fn is_set_zero(arena: &mut TermArena, ty: TermHandle) -> TermHandle {
        let is_set = constant(arena, QUOTIENT_IS_SET, vec![Level::Constant(0)]);
        apply(arena, is_set, ty)
    }
}

#[cfg(test)]
pub(crate) mod bounded_observations {
    thread_local! {
        static ENTRIES: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    }

    pub(crate) fn bounded_verification_entries() -> (usize, usize) {
        ENTRIES.get()
    }

    pub(crate) fn record_admission() {
        ENTRIES.update(|(admission, denotation)| (admission + 1, denotation));
    }

    pub(crate) fn record_denotation() {
        ENTRIES.update(|(admission, denotation)| (admission, denotation + 1));
    }
}
