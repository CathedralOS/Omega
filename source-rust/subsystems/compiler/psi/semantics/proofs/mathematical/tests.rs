mod conversion_judgments {
    use super::{
        apply, default_budget, empty, lambda, pi, squash, squash_intro, two, two_one, two_zero,
        type_sort,
    };
    use crate::{
        Budget, Context, CoreError, Declaration, Level, Sort, Term, TermArena, check_signature,
        convertible, convertible_retaining,
    };

    #[test]
    fn public_conversion_forms_both_operands_before_reduction() {
        let mut arena = TermArena::new();
        let context = Context::empty();
        let two = two(&mut arena);
        let zero = two_zero(&mut arena);
        let empty = empty(&mut arena);
        let good_function = lambda(&mut arena, two, two);
        let good = apply(&mut arena, good_function, zero);
        let bad_function = lambda(&mut arena, empty, two);
        let bad = apply(&mut arena, bad_function, zero);
        let shared = type_sort(&mut arena, 0);

        for (name, left, right, valid) in [
            ("formed", good, two, true),
            ("left argument", bad, two, false),
            ("right argument", two, bad, false),
            ("reflexive malformed term", bad, bad, false),
        ] {
            let observed = convertible(
                &mut arena,
                &context,
                left,
                right,
                shared,
                &mut default_budget(),
            );
            if valid {
                assert_eq!(observed, Ok(true), "{name}");
            } else {
                assert!(
                    matches!(observed, Err(CoreError::ArgumentTypeMismatch { .. })),
                    "{name}: {observed:?}"
                );
            }
        }

        assert!(matches!(
            convertible(&mut arena, &context, bad, bad, zero, &mut default_budget()),
            Err(CoreError::NotASort { term, .. }) if term == zero
        ));
    }

    #[test]
    fn public_conversion_forms_both_strict_proofs_before_irrelevance() {
        let mut arena = TermArena::new();
        let context = Context::empty();
        let two = two(&mut arena);
        let zero = two_zero(&mut arena);
        let one = two_one(&mut arena);
        let proposition = squash(&mut arena, two);
        let left_proof = squash_intro(&mut arena, two, zero);
        let right_proof = squash_intro(&mut arena, two, one);
        for (name, left, right, valid) in [
            ("formed proofs", left_proof, right_proof, true),
            ("left is a value", zero, right_proof, false),
            ("right is a value", left_proof, zero, false),
            ("reflexive value", zero, zero, false),
        ] {
            let observed = convertible(
                &mut arena,
                &context,
                left,
                right,
                proposition,
                &mut default_budget(),
            );
            if valid {
                assert_eq!(observed, Ok(true), "{name}");
            } else {
                assert!(
                    matches!(observed, Err(CoreError::TypeMismatch { .. })),
                    "{name}: {observed:?}"
                );
            }
        }
        assert_eq!(
            convertible(&mut arena, &context, zero, one, two, &mut default_budget()),
            Ok(false)
        );
    }

    #[test]
    fn public_retaining_conversion_checks_operands_and_keeps_opaque_heads() {
        for valid in [true, false] {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let two = two(&mut arena);
            let zero = two_zero(&mut arena);
            let domain = if valid { two } else { empty(&mut arena) };
            let shared = type_sort(&mut arena, 0);
            let function_type = pi(&mut arena, domain, shared);
            let body = lambda(&mut arena, domain, two);
            let signature = check_signature(
                &mut arena,
                &[Declaration::definition(0, function_type, body)],
                &mut budget,
            )
            .expect("the function declaration itself is well-typed");
            let context = Context::empty().with_signature(signature);
            let function = arena.insert(Term::Constant {
                declaration: 0,
                levels: Vec::new(),
            });
            let application = apply(&mut arena, function, zero);
            for (name, left, right, opaque_equal) in [
                ("left application", application, two, false),
                ("right application", two, application, false),
                ("same application", application, application, true),
            ] {
                let ordinary = convertible(&mut arena, &context, left, right, shared, &mut budget);
                let retained = convertible_retaining(
                    &mut arena,
                    &context,
                    left,
                    right,
                    shared,
                    &mut budget,
                    &|declaration| declaration == 0,
                );
                if valid {
                    assert_eq!(ordinary, Ok(true), "{name}");
                    assert_eq!(retained, Ok(opaque_equal), "{name}");
                } else {
                    assert!(
                        matches!(ordinary, Err(CoreError::ArgumentTypeMismatch { .. })),
                        "{name}: {ordinary:?}"
                    );
                    assert!(
                        matches!(retained, Err(CoreError::ArgumentTypeMismatch { .. })),
                        "{name}: {retained:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn public_conversion_checks_level_arity_before_reflexivity() {
        let mut arena = TermArena::new();
        let mut budget = default_budget();
        let shared = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
        let signature = check_signature(
            &mut arena,
            &[Declaration::assumption(2, shared)],
            &mut budget,
        )
        .unwrap();
        let context = Context::with_level_arity(2).with_signature(signature);
        let malformed = arena.insert(Term::Constant {
            declaration: 0,
            levels: vec![Level::Parameter(0)],
        });
        assert!(matches!(
            convertible(
                &mut arena,
                &context,
                malformed,
                malformed,
                shared,
                &mut budget,
            ),
            Err(CoreError::DeclarationArityMismatch {
                declaration: 0,
                expected: 2,
                supplied: 1,
            })
        ));
        assert!(matches!(
            convertible_retaining(
                &mut arena,
                &context,
                malformed,
                malformed,
                shared,
                &mut budget,
                &|_| true,
            ),
            Err(CoreError::DeclarationArityMismatch {
                declaration: 0,
                expected: 2,
                supplied: 1,
            })
        ));
    }

    #[test]
    fn public_conversion_reuses_the_shared_types_formed_sort() {
        let mut arena = TermArena::new();
        let two = two(&mut arena);
        let zero = two_zero(&mut arena);
        let universe = type_sort(&mut arena, 0);
        let sort_function = lambda(&mut arena, two, universe);
        let bound = arena.insert(Term::Variable(0));
        let computed_sort = apply(&mut arena, sort_function, bound);
        let function_type = pi(&mut arena, two, computed_sort);
        let function = arena.insert(Term::Constant {
            declaration: 0,
            levels: Vec::new(),
        });
        let shared = apply(&mut arena, function, zero);
        let signature = check_signature(
            &mut arena,
            &[
                Declaration::assumption(0, function_type),
                Declaration::assumption(0, shared),
            ],
            &mut default_budget(),
        )
        .unwrap();
        let context = Context::empty().with_signature(signature);
        let value = arena.insert(Term::Constant {
            declaration: 1,
            levels: Vec::new(),
        });
        // The shared type's inferred sort takes one beta step. Neither
        // checking its two inhabitants nor comparison owes that step again.
        for (allowance, expected) in [(1, Ok(true)), (0, Err(CoreError::StepCeiling))] {
            let mut budget = Budget::new(allowance);
            assert_eq!(
                convertible(&mut arena, &context, value, value, shared, &mut budget),
                expected,
            );
            assert_eq!(budget.remaining(), 0);
        }
    }
}

mod assumption_closure {
    use super::{constant, default_budget, pi, two, type_sort};
    use crate::{
        Declaration, MathematicalCertificate, Term, TermArena, certificate_assumption_closure,
        check_signature, judgment_assumption_closure, verify_mathematical_certificate,
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
            assert_eq!(
                certificate_assumption_closure(&arena, &certificate),
                BTreeSet::from([0]),
                "the definition's body contributes its axiom; unused declarations do not"
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
                let signature = check_signature(&mut arena, &declarations, &mut default_budget())
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
        apply, default_budget, id, id_elim, lambda, pi, refl, strict_sort, symmetry_motive, two,
        two_zero, type_sort, variable,
    };
    use crate::{
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
        let error = infer_type(&mut arena, &strict_context, strict_refl, &mut budget).unwrap_err();
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
            weak_head_normalize(&mut arena, &Signature::new(), elimination, &mut budget).unwrap();
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
}
mod level_worklists {
    use super::default_budget;
    use crate::{Context, CoreError, Level, Sort, Term, TermArena, infer_type};

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
            use crate::term::{instantiate_level, levels_equal};
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
            let missing = Level::Maximum(Box::new(level.clone()), Box::new(Level::Parameter(1)));
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
    use crate::{
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
        let wrong_shared = type_sort(&mut arena, 0);
        assert!(matches!(
            convertible(
                &mut arena,
                &context,
                inferred,
                claimed_sort,
                wrong_shared,
                &mut budget,
            ),
            Err(CoreError::TypeMismatch { .. })
        ));
        // Both compared Type(u+1) universes inhabit Type(u+2).
        let relevant = sort_level(
            &mut arena,
            Level::Parameter(0)
                .successor()
                .unwrap()
                .successor()
                .unwrap(),
        );
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
        // One level argument combines the context's two independent levels.
        let axiom = Declaration::assumption(1, type_u);
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
        assert!(matches!(
            convertible(&mut arena, &context, left, other, shared, &mut budget),
            Err(CoreError::TypeMismatch { .. })
        ));

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
        let narrow = Declaration::definition(0, universe, carrier);
        let wide = Declaration::definition(0, universe, carrier);
        let family_type = pi(&mut arena, universe, universe);
        let family = Declaration::assumption(0, family_type);
        let signature = check_signature(&mut arena, &[narrow, wide, family], &mut budget).unwrap();
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
            !crate::convertible_retaining(
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
            !crate::convertible_retaining(
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
            crate::convertible_retaining(
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
    use crate::{
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
        let signature = crate::Signature::new();
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
            let parameter = crate::Level::Parameter(0);
            let levels_a = vec![parameter.clone().maximum(crate::Level::Constant(0))];
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
    mod quotient_derived {
        use crate::tests::{
            apply, boxed, default_budget, id, is_set_zero, lambda, pi, quotient_signature, refl,
            squash, squash_intro, two, two_identity, two_quotient, variable,
        };
        use crate::{Context, CoreError, Level, TermArena, check_type, convertible, infer_type};

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
            let error = check_type(&mut arena, &context, unique, function_identity, &mut budget)
                .unwrap_err();
            assert!(
                matches!(error, CoreError::TypeMismatch { .. }),
                "pointwise uniqueness is not function extensionality: {error:?}"
            );
        }
    }
    mod quotient_receiver_policies {
        use std::collections::BTreeSet;

        use crate::tests::{
            apply, box_elim, box_intro, boxed, constant, default_budget, equivalence_hypotheses,
            equivalence_proofs, id, lambda, quotient_signature, refl, sigma, squash, squash_elim,
            squash_intro, two, two_identity, two_one, two_quotient, two_quotient_over, two_zero,
            variable,
        };
        use crate::{
            Context, CoreError, Level, MathematicalCertificate, QUOTIENT, QUOTIENT_BETA,
            QUOTIENT_EFFECTIVE, QUOTIENT_ELIM, QUOTIENT_PROJECT, QUOTIENT_SET, QUOTIENT_SOUND,
            QuotientFamily, Signature, TermArena, assumption_closure,
            certificate_assumption_closure, check_type, infer_type, quotient_scheme,
            verify_mathematical_certificate,
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
            let [propositional, reflexive, symmetric, transitive] =
                equivalence_hypotheses(&mut arena, &family);
            let context = Context::empty()
                .with_signature(signature)
                .extend(propositional)
                .extend(reflexive)
                .extend(symmetric)
                .extend(transitive)
                .extend(two(&mut arena))
                .extend(two(&mut arena))
                .extend(identity_ab)
                .extend(squashed_evidence)
                .extend(projection_identity);
            let laws = equivalence_proofs(&mut arena, 5);

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
                family.effective(&mut arena, &laws, a, b, p)
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
                    family.effective(&mut arena, &laws, a, b, p)
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
            let error = infer_type(&mut arena, &context, raw_congruence, &mut budget).unwrap_err();
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
                    let laws = equivalence_proofs(&mut arena, 3);
                    let a = variable(&mut arena, 2);
                    let b = variable(&mut arena, 1);
                    let p = variable(&mut arena, 0);
                    family.effective(&mut arena, &laws, a, b, p)
                };
                let expected = {
                    let body = two_identity(&mut arena, 2, 1);
                    let body = squash(&mut arena, body);
                    boxed(&mut arena, body)
                };
                MathematicalCertificate {
                    signature: quotient_scheme(&mut arena),
                    level_arity: 0,
                    context: vec![
                        propositional,
                        reflexive,
                        symmetric,
                        transitive,
                        two(&mut arena),
                        two(&mut arena),
                        projection_identity,
                    ],
                    term,
                    expected,
                }
            };
            verify_mathematical_certificate(&mut arena, &certificate, &mut budget).unwrap();
            let closure = certificate_assumption_closure(&arena, &certificate);
            let expected_closure: BTreeSet<u32> = [QUOTIENT, QUOTIENT_PROJECT, QUOTIENT_EFFECTIVE]
                .into_iter()
                .collect();
            assert_eq!(
                closure, expected_closure,
                "the squashed effectivity judgment commits to exactly the interface it cites"
            );
        }

        /// A relation that is not an equivalence must not reach `Id Two zero one`
        /// through effectivity. With `R := λ_ _. Id Two zero one`, the bare
        /// `effective(x, x, refl)` the kernel once admitted is refused, and the
        /// same conclusion checks only under a context that assumes `R` is an
        /// equivalence, so the certificate commits to that unprovable premise.
        #[test]
        fn effectivity_requires_the_relation_to_be_a_propositional_equivalence() {
            let mut arena = TermArena::new();
            let mut budget = default_budget();
            let family = two_quotient_over(&mut arena, |arena| {
                let carrier = two(arena);
                let zero = two_zero(arena);
                let one = two_one(arena);
                id(arena, carrier, zero, one)
            });
            let absurd = {
                let carrier = two(&mut arena);
                let zero = two_zero(&mut arena);
                let one = two_one(&mut arena);
                id(&mut arena, carrier, zero, one)
            };
            let self_identity = |arena: &mut TermArena, x: u32| {
                let element = variable(arena, x);
                let projected = family.project(arena, element);
                let ty = family.quotient(arena);
                refl(arena, ty, projected)
            };

            let unhypothesized = {
                let x = variable(&mut arena, 0);
                let p = self_identity(&mut arena, 0);
                let effective = constant(
                    &mut arena,
                    QUOTIENT_EFFECTIVE,
                    vec![Level::Constant(0), Level::Constant(0)],
                );
                let unchecked = [family.carrier, family.relation, x, x, p]
                    .into_iter()
                    .fold(effective, |function, argument| {
                        apply(&mut arena, function, argument)
                    });
                MathematicalCertificate {
                    signature: quotient_scheme(&mut arena),
                    level_arity: 0,
                    context: vec![two(&mut arena)],
                    term: unchecked,
                    expected: absurd,
                }
            };
            let error = verify_mathematical_certificate(&mut arena, &unhypothesized, &mut budget)
                .unwrap_err();
            assert!(
                matches!(error, CoreError::ArgumentTypeMismatch { .. }),
                "effectivity without equivalence proofs proves nothing: {error:?}"
            );

            let [propositional, reflexive, symmetric, transitive] =
                equivalence_hypotheses(&mut arena, &family);
            let hypothesized = {
                let laws = equivalence_proofs(&mut arena, 1);
                let x = variable(&mut arena, 0);
                let p = self_identity(&mut arena, 0);
                MathematicalCertificate {
                    signature: quotient_scheme(&mut arena),
                    level_arity: 0,
                    context: vec![
                        propositional,
                        reflexive,
                        symmetric,
                        transitive,
                        two(&mut arena),
                    ],
                    term: family.effective(&mut arena, &laws, x, x, p),
                    expected: absurd,
                }
            };
            verify_mathematical_certificate(&mut arena, &hypothesized, &mut budget).unwrap();
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
            verify_mathematical_certificate(&mut arena, &representative_certificate, &mut budget)
                .unwrap();

            let forged =
                MathematicalCertificate::with_signature(declarations, vec![element], covered, {
                    let carrier = two(&mut arena);
                    squash(&mut arena, carrier)
                });
            let error = verify_mathematical_certificate(&mut arena, &forged, &mut default_budget())
                .unwrap_err();
            assert!(
                matches!(error, CoreError::TypeMismatch { .. }),
                "a forged certificate is refused before any policy runs: {error:?}"
            );

            let quotient_closure = certificate_assumption_closure(&arena, &quotient_certificate);
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
            ]
            .into_iter()
            .collect();
            assert!(representative_closure.is_subset(&quotient_refusing));
            assert!(!quotient_closure.is_subset(&quotient_refusing));
            assert!(representative_closure.is_subset(&quotient_admitting));
            assert!(quotient_closure.is_subset(&quotient_admitting));

            let effectivity_closure = assumption_closure(&arena, &signature, &[QUOTIENT_EFFECTIVE]);
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
        use crate::tests::{
            apply, constant, default_budget, id, is_set_zero, lambda, pi, quotient_signature, two,
            two_identity, two_quotient, type_sort, variable,
        };
        use crate::{
            Context, CoreError, Declaration, Level, QUOTIENT, QUOTIENT_BETA, QUOTIENT_EFFECTIVE,
            QUOTIENT_ELIM, QUOTIENT_IS_SET, QUOTIENT_LIFT, QUOTIENT_PROJECT, QUOTIENT_SET,
            QUOTIENT_SOUND, QUOTIENT_UNIQUE, TermArena, check_signature, check_type, infer_type,
            quotient_scheme,
        };
        use std::collections::BTreeSet;

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
                    let is_set = constant(&mut arena, QUOTIENT_IS_SET, vec![Level::Constant(0)]);
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
            let error = infer_type(&mut arena, &context, missing_theorem, &mut budget).unwrap_err();
            assert!(
                matches!(error, CoreError::ArgumentTypeMismatch { .. }),
                "the section value is not the congruence theorem: {error:?}"
            );
        }

        #[test]
        fn quotient_assumption_closure_records_the_admitted_interface() {
            // quotients.md and classicality.md admit exactly seven named
            // assumptions; every other quotient declaration must carry a
            // checked body, so the kernel cannot grow an unlisted axiom.
            let mut arena = TermArena::new();
            let declarations = quotient_scheme(&mut arena);
            let assumed: BTreeSet<u32> = declarations
                .iter()
                .enumerate()
                .filter(|(_, declaration)| declaration.is_assumption())
                .map(|(index, _)| u32::try_from(index).unwrap())
                .collect();
            let admitted: BTreeSet<u32> = [
                QUOTIENT,
                QUOTIENT_PROJECT,
                QUOTIENT_SET,
                QUOTIENT_SOUND,
                QUOTIENT_EFFECTIVE,
                QUOTIENT_ELIM,
                QUOTIENT_BETA,
            ]
            .into_iter()
            .collect();
            assert_eq!(assumed, admitted);
            assert_eq!(declarations.len(), QUOTIENT_UNIQUE as usize + 1);
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
    use crate::{Context, CoreError, Term, TermArena, TermHandle, check_type, infer_type};

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
        let inferred = infer_type(&mut arena, &context, witness, &mut default_budget()).unwrap();
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
        let mut budget = crate::Budget::new(0);
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
                let mut budget = crate::Budget::new(0);
                let result = infer_type(&mut arena, &Context::empty(), term, &mut budget);
                if valid {
                    let inferred =
                        result.expect("the complete dependent application tree is well typed");
                    assert!(arena.structurally_equal(inferred, two));
                } else {
                    let CoreError::TypeMismatch { expected, actual } = result.unwrap_err() else {
                        panic!("the wrong inner payload must fail its Two obligation");
                    };
                    assert_eq!(arena.get(expected), Term::Two);
                    assert_eq!(
                        arena.get(actual),
                        Term::Sort(crate::Sort::Type(crate::Level::Constant(0)))
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
                Term::Sort(crate::Sort::Type(crate::Level::Constant(0)))
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
            let inferred = infer_type(&mut arena, &Context::empty(), term, &mut default_budget())
                .expect("the complete left-nested binder tree is a valid relevant type");
            assert_eq!(
                arena.get(inferred),
                Term::Sort(crate::Sort::Type(crate::Level::Constant(0)))
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
            Term::Sort(crate::Sort::Type(crate::Level::Constant(1)))
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
            Term::Sort(crate::Sort::Type(crate::Level::Constant(1)))
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
                Term::Sort(crate::Sort::Type(crate::Level::Constant(0)))
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
        apply, box_elim, box_intro, boxed, default_budget, empty, empty_elim, lambda, pi, sigma,
        squash, squash_elim, squash_intro, strict_sort, two, type_sort, variable,
    };
    use crate::{
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
        let error =
            check_type(&mut arena, &context, elimination, wrong_claim, &mut budget).unwrap_err();
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
        assert!(matches!(
            convertible(&mut arena, &context, left, other, shared, &mut budget),
            Err(CoreError::TypeMismatch { .. })
        ));
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
        assert!(!convertible(&mut arena, &context, a, b, relevant_shared, &mut budget).unwrap());

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
            weak_head_normalize(&mut arena, &Signature::new(), elimination, &mut budget).unwrap();
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
            weak_head_normalize(&mut arena, &Signature::new(), elimination, &mut budget).unwrap();
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
        assert!(!convertible(&mut arena, &context, left, other_body, shared, &mut budget).unwrap());

        let motive = variable(&mut arena, 6);
        let body = variable(&mut arena, 4);
        let scrutinee = variable(&mut arena, 0);
        let other_scrutinee = box_elim(&mut arena, motive, body, scrutinee);
        assert!(matches!(
            conv!(&mut arena => &context => left => other_scrutinee => shared => &mut budget),
            Err(CoreError::TypeMismatch { .. })
        ));

        let motive = variable(&mut arena, 5);
        let body = variable(&mut arena, 2);
        let scrutinee = variable(&mut arena, 1);
        let other_motive = box_elim(&mut arena, motive, body, scrutinee);
        assert!(matches!(
            conv!(&mut arena => &context => left => other_motive => shared => &mut budget),
            Err(CoreError::TypeMismatch { .. })
        ));
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
    use crate::substitution::{instantiate_levels, shift, substitute};
    use crate::{Context, Level, Sort, Term, TermArena, TermHandle, check_type};

    #[test]
    fn binding_summaries_authenticate_handles_and_preserve_open_scopes() {
        let mut arena = TermArena::new();
        let two = two(&mut arena);
        let zero = variable(&mut arena, 0);
        let one = variable(&mut arena, 1);
        let closed = arena.insert(Term::Lambda {
            domain: two,
            body: zero,
        });
        let open = arena.insert(Term::Lambda {
            domain: two,
            body: one,
        });
        assert_eq!(arena.required_bindings(zero), Some(1));
        assert_eq!(arena.required_bindings(closed), Some(0));
        assert_eq!(arena.required_bindings(open), Some(1));
        assert_eq!(shift(&mut arena, open, 1, 3), open);
        let shifted = shift(&mut arena, open, 0, 1);
        let Term::Lambda { body, .. } = arena.get(shifted) else {
            panic!("lambda")
        };
        assert_eq!(arena.get(body), Term::Variable(2));

        let stale = TermHandle::from_parts(closed.arena_index(), closed.generation() + 1);
        assert_eq!(arena.required_bindings(stale), None);
        let invalid = arena.insert(Term::Lambda {
            domain: two,
            body: stale,
        });
        assert_eq!(arena.required_bindings(invalid), None);
        let future = TermHandle::from_arena_index(u32::try_from(arena.len() + 1).unwrap());
        let cycle = arena.insert(Term::Pair {
            first: future,
            second: two,
        });
        assert_eq!(cycle, future);
        assert_eq!(arena.required_bindings(cycle), None);
        assert_eq!(
            arena.required_bindings(TermHandle::from_arena_index(u32::MAX)),
            None
        );

        let maximum = variable(&mut arena, u32::MAX);
        assert_eq!(arena.required_bindings(maximum), None);
        let lowered = substitute(&mut arena, maximum, two);
        assert_eq!(arena.get(lowered), Term::Variable(u32::MAX - 1));

        let universe = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
        assert_eq!(arena.required_bindings(universe), Some(0));
        assert_eq!(instantiate_levels(&mut arena, universe, &[]), Err(0));
        let instance = instantiate_levels(&mut arena, universe, &[Level::Constant(2)]).unwrap();
        assert_eq!(
            arena.get(instance),
            Term::Sort(Sort::Type(Level::Constant(2)))
        );
    }

    #[test]
    fn binding_summaries_are_not_stored_or_part_of_arena_equality() {
        let mut arena = TermArena::new();
        let two = two(&mut arena);
        let zero = variable(&mut arena, 0);
        let closed = arena.insert(Term::Lambda {
            domain: two,
            body: zero,
        });
        let limits = record_storage::RecordLimits::default();
        let bytes = record_storage::encode(&arena, limits).unwrap();
        let mut restored: TermArena = record_storage::decode(&bytes, limits).unwrap();
        assert_eq!(arena, restored);
        assert_eq!(restored.required_bindings(closed), None);
        assert!(arena.contains_free_variable(zero, 0));
        assert!(!arena.contains_free_variable(closed, 0));
        assert!(restored.contains_free_variable(zero, 0));
        assert!(!restored.contains_free_variable(closed, 0));
        let before = crate::substitution::remapped_nodes();
        assert_eq!(shift(&mut restored, closed, 0, 1), closed);
        assert!(crate::substitution::remapped_nodes() > before);
        assert_eq!(record_storage::encode(&restored, limits).unwrap(), bytes);
    }

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
            let before = crate::substitution::remapped_nodes();
            assert_eq!(shift(&mut arena, term, 0, 1), term);
            assert_eq!(substitute(&mut arena, term, domain), term);
            assert_eq!(
                crate::substitution::remapped_nodes(),
                before,
                "closed syntax needs no variable-remapping traversal"
            );
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
    use crate::{
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
        assert!(matches!(
            convertible(&mut arena, &context, wrapped, f, neutral_type, &mut budget),
            Err(CoreError::TypeMismatch { .. })
        ));

        assert!(matches!(
            convertible(&mut arena, &context, f, wrapped, neutral_type, &mut budget),
            Err(CoreError::TypeMismatch { .. })
        ));
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
        let elimination = case_two(&mut arena, family, function_witness, one_witness, scrutinee);

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
            weak_head_normalize(&mut arena, &Signature::new(), result_type, &mut budget).unwrap();
        let function_domain = two(&mut arena);
        let function_codomain = two(&mut arena);
        let expected_type = pi(&mut arena, function_domain, function_codomain);
        assert!(arena.structurally_equal(normalized_type, expected_type));
        let result_type = infer_type(&mut arena, &context, on_one, &mut budget).unwrap();
        let normalized_type =
            weak_head_normalize(&mut arena, &Signature::new(), result_type, &mut budget).unwrap();
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
            conv!(&mut arena => &context => left => left_again => two_type => &mut budget).unwrap()
        );
        assert!(!convertible(&mut arena, &context, left, right, two_type, &mut budget).unwrap());

        let mut empty_budget = Budget::new(0);
        let error = weak_head_normalize(&mut arena, &Signature::new(), on_zero, &mut empty_budget)
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

use crate::{
    Budget, DEFAULT_CONVERSION_STEPS, Declaration, EquivalenceProofs, Level, QUOTIENT_IS_SET,
    QuotientFamily, Signature, Sort, Term, TermArena, TermHandle, check_signature, quotient_scheme,
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

pub(crate) fn pi(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
    arena.insert(Term::Pi { domain, codomain })
}

pub(crate) fn lambda(arena: &mut TermArena, domain: TermHandle, body: TermHandle) -> TermHandle {
    arena.insert(Term::Lambda { domain, body })
}

fn apply(arena: &mut TermArena, function: TermHandle, argument: TermHandle) -> TermHandle {
    arena.insert(Term::Apply { function, argument })
}

pub(crate) fn sigma(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
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

fn id(arena: &mut TermArena, ty: TermHandle, left: TermHandle, right: TermHandle) -> TermHandle {
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

/// The four hypothesis types `effective` requires of `family`'s relation,
/// in binding order: propositional, reflexive, symmetric, transitive.
/// Each is closed, so a test can place them anywhere in a context.
fn equivalence_hypotheses(arena: &mut TermArena, family: &QuotientFamily) -> [TermHandle; 4] {
    let carrier = family.carrier;
    let relation = family.relation;
    let related = |arena: &mut TermArena, left: u32, right: u32| {
        let left = variable(arena, left);
        let right = variable(arena, right);
        let partial = apply(arena, relation, left);
        apply(arena, partial, right)
    };
    let propositional = {
        let ty = related(arena, 3, 2);
        let r = variable(arena, 1);
        let s = variable(arena, 0);
        let mut body = id(arena, ty, r, s);
        for domain in [related(arena, 2, 1), related(arena, 1, 0), carrier, carrier] {
            body = pi(arena, domain, body);
        }
        body
    };
    let reflexive = {
        let body = related(arena, 0, 0);
        pi(arena, carrier, body)
    };
    let symmetric = {
        let mut body = related(arena, 1, 2);
        for domain in [related(arena, 1, 0), carrier, carrier] {
            body = pi(arena, domain, body);
        }
        body
    };
    let transitive = {
        let mut body = related(arena, 4, 2);
        for domain in [
            related(arena, 2, 1),
            related(arena, 2, 1),
            carrier,
            carrier,
            carrier,
        ] {
            body = pi(arena, domain, body);
        }
        body
    };
    [propositional, reflexive, symmetric, transitive]
}

/// References to the hypotheses from `equivalence_hypotheses` when they
/// sit at the start of a context and `transitive` is de Bruijn index
/// `transitive_index` from the term being checked.
fn equivalence_proofs(arena: &mut TermArena, transitive_index: u32) -> EquivalenceProofs {
    EquivalenceProofs {
        propositional: variable(arena, transitive_index + 3),
        reflexive: variable(arena, transitive_index + 2),
        symmetric: variable(arena, transitive_index + 1),
        transitive: variable(arena, transitive_index),
    }
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
