mod batch {
    use super::MathematicalCertificate;
    use crate::term::MAX_SUBTERMS;
    use crate::{
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
        let mut destination = TermArena::new();
        let mut imported = ImportedTerms::new(&mut destination);
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
                        check_signature(imported.arena, &certificate.signature, budget)?;
                    signatures.push((certificate.signature.clone(), signature.clone()));
                    signature
                };
                let mut context = Context::with_level_arity(certificate.level_arity)
                    .with_signature(signature);
                for binding in certificate.context {
                    infer_sort(imported.arena, &context, binding, budget)?;
                    context = context.extend(binding);
                }
                check_type(
                    imported.arena,
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

    pub(super) fn share_certificate_in_place(
        arena: &mut TermArena,
        certificate: &MathematicalCertificate,
    ) -> Result<MathematicalCertificate, CoreError> {
        // Authenticate the reachable source before new rows can occupy an
        // invalid forward reference. Full generational handles stay frozen.
        let mut pending = certificate
            .signature
            .iter()
            .flat_map(|declaration| std::iter::once(declaration.ty).chain(declaration.body))
            .chain(certificate.context.iter().copied())
            .chain([certificate.term, certificate.expected])
            .collect::<Vec<_>>();
        let mut source_generations = Vec::<Option<u32>>::new();
        while let Some(handle) = pending.pop() {
            let node = arena.try_node(handle).ok_or(CoreError::DummyTerm)?;
            let slot = handle.arena_index() as usize;
            if source_generations.get(slot).copied().flatten() == Some(handle.generation()) {
                continue;
            }
            if slot >= source_generations.len() {
                source_generations.resize(slot + 1, None);
            }
            source_generations[slot] = Some(handle.generation());
            pending.extend(node.subterms().iter().map(|(child, _)| child));
        }
        let mut imported = ImportedTerms::new(arena);
        imported.certificate_from(ImportSource::Destination(&source_generations), certificate)
    }

    #[derive(Clone, Copy)]
    enum ImportSource<'source> {
        AnotherArena(&'source TermArena),
        Destination(&'source [Option<u32>]),
    }

    struct ImportedTerms<'arena> {
        arena: &'arena mut TermArena,
        by_syntax: HashMap<Term, TermHandle>,
    }
    impl<'arena> ImportedTerms<'arena> {
        fn new(arena: &'arena mut TermArena) -> Self {
            Self {
                arena,
                by_syntax: HashMap::new(),
            }
        }

        fn certificate(
            &mut self,
            source: &TermArena,
            certificate: &MathematicalCertificate,
        ) -> Result<MathematicalCertificate, CoreError> {
            self.certificate_from(ImportSource::AnotherArena(source), certificate)
        }

        fn certificate_from(
            &mut self,
            source: ImportSource<'_>,
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
            source: ImportSource<'_>,
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
                        let node = match source {
                            ImportSource::AnotherArena(source) => source.try_node(original),
                            ImportSource::Destination(source_generations) => {
                                if source_generations
                                    .get(original.arena_index() as usize)
                                    .copied()
                                    .flatten()
                                    != Some(original.generation())
                                {
                                    return Err(CoreError::DummyTerm);
                                }
                                self.arena.try_node(original)
                            }
                        }
                        .cloned()
                        .ok_or(CoreError::DummyTerm)?;
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
        use crate::Level;
        use crate::{DEFAULT_CONVERSION_STEPS, Sort, verify_mathematical_certificate};

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
            use crate::{Level, certificate_assumption_closure};
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
            let mut destination = TermArena::new();
            let mut imported = ImportedTerms::new(&mut destination);
            let copy = imported.certificate(&arena, &certificate).unwrap();
            assert_eq!(
                before,
                certificate_assumption_closure(imported.arena, &copy)
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
        let inferred = crate::infer_type(
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
        let mut destination = TermArena::new();
        let mut imported = ImportedTerms::new(&mut destination);
        let copied = imported
            .certificate(&source, &certificate)
            .expect("all finite source terms import");
        let inferred = crate::infer_type(
            imported.arena,
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
        fn single_and_batch_share_repeated_closed_applications() {
            let (mut arena, mut certificate) = judgment(false);
            for _ in 1..64 {
                let function = arena.insert(Term::Constant {
                    declaration: 0,
                    levels: Vec::new(),
                });
                let argument = arena.insert(Term::TwoZero);
                let field = arena.insert(Term::Apply { function, argument });
                certificate.term = arena.insert(Term::Pair {
                    first: field,
                    second: certificate.term,
                });
                let domain = arena.insert(Term::Two);
                certificate.expected = arena.insert(Term::Sigma {
                    domain,
                    codomain: certificate.expected,
                });
            }
            let mut single_arena = arena.clone();
            let mut single = Budget::new(DEFAULT_CONVERSION_STEPS);
            verify_mathematical_certificate(&mut single_arena, &certificate, &mut single)
                .expect("every independently allocated field has its declared type");
            let mut batch = Budget::new(DEFAULT_CONVERSION_STEPS);
            verify_mathematical_certificates(&[(&arena, &certificate)], &mut batch)
                .expect("sharing raw syntax preserves the complete judgment");
            assert_eq!(
                single.remaining(),
                batch.remaining(),
                "equal syntax should not repeat inference because its nodes were allocated separately"
            );
            assert!(batch.remaining() < DEFAULT_CONVERSION_STEPS);
            assert!(matches!(
                verify_mathematical_certificate(
                    &mut arena.clone(),
                    &certificate,
                    &mut Budget::new(0),
                ),
                Err(CoreError::StepCeiling)
            ));

            let Term::Pair { second, .. } = arena.get(certificate.term) else {
                unreachable!()
            };
            let function = arena.insert(Term::Constant {
                declaration: 0,
                levels: Vec::new(),
            });
            certificate.term = arena.insert(Term::Pair {
                first: function,
                second,
            });
            assert!(matches!(
                verify_mathematical_certificate(
                    &mut arena,
                    &certificate,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                ),
                Err(CoreError::TypeMismatch { .. })
            ));
        }

        #[test]
        fn single_failure_terms_stay_in_the_callers_arena() {
            let mut arena = TermArena::new();
            arena.insert(Term::Empty);
            let two = arena.insert(Term::Two);
            let body = arena.insert(Term::Variable(0));
            let term = arena.insert(Term::Lambda { domain: two, body });
            let certificate = MathematicalCertificate::closed(Vec::new(), term, two);
            let failure = verify_mathematical_certificate(
                &mut arena,
                &certificate,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
            )
            .expect_err("a function does not have the scalar receiving type");
            let CoreError::TypeMismatch { expected, actual } = failure else {
                panic!("the result must name both sides of the type mismatch: {failure:?}")
            };
            assert_eq!(
                arena.get(expected),
                Term::Two,
                "the receiving type stays caller-owned"
            );
            let Term::Pi { domain, codomain } = arena.get(actual) else {
                panic!("the inferred function type stays caller-owned")
            };
            assert_eq!(arena.get(domain), Term::Two);
            assert_eq!(arena.get(codomain), Term::Two);
        }

        #[test]
        fn single_import_refuses_handles_born_during_its_own_copy() {
            let mut arena = TermArena::new();
            let lower = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
            let higher = arena.insert(Term::Sort(Sort::Type(Level::Constant(1))));
            let future = TermHandle::from_arena_index(u32::try_from(arena.len() + 1).unwrap());
            let stale = TermHandle::from_parts(lower.arena_index(), lower.generation() + 1);
            for invalid in [future, stale] {
                let certificate = MathematicalCertificate::closed(vec![lower], invalid, higher);
                assert!(matches!(
                    verify_mathematical_certificate(
                        &mut arena,
                        &certificate,
                        &mut Budget::new(0)
                    ),
                    Err(CoreError::DummyTerm)
                ));
            }
        }

        #[test]
        fn single_import_refuses_a_source_cycle() {
            let mut arena = TermArena::new();
            let two = arena.insert(Term::Two);
            let cycle = TermHandle::from_arena_index(u32::try_from(arena.len() + 1).unwrap());
            let term = arena.insert(Term::Lambda {
                domain: two,
                body: cycle,
            });
            assert_eq!(term, cycle);
            let certificate = MathematicalCertificate::closed(Vec::new(), term, two);
            assert!(matches!(
                verify_mathematical_certificate(&mut arena, &certificate, &mut Budget::new(0)),
                Err(CoreError::DummyTerm)
            ));
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
                verify_mathematical_certificate(&mut arena, &certificate, &mut Budget::new(0))
                    .expect("single verification uses complete handle judgments");
                let source_nodes = arena.len();
                verify_mathematical_certificates(&[(&arena, &certificate)], &mut Budget::new(0))
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
    // Keep shared syntax and inferred error terms in the caller's arena.
    let certificate = batch::share_certificate_in_place(arena, certificate)?;
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

/// A required type does not change when its proof appends private declarations.
/// Keep the whole original prefix through its last dependency, so opaque
/// declarations retain their positions and distinct types never collapse.
pub fn certificate_type_signature_prefix(
    arena: &TermArena,
    certificate: &MathematicalCertificate,
) -> Result<usize, CoreError> {
    let mut pending = certificate.context.clone();
    pending.push(certificate.expected);
    let mut visited = Vec::new();
    let mut required = 0;
    while let Some(handle) = pending.pop() {
        let node = arena.try_node(handle).ok_or(CoreError::DummyTerm)?;
        match visited.binary_search_by_key(
            &(handle.arena_index(), handle.generation()),
            |saved: &TermHandle| (saved.arena_index(), saved.generation()),
        ) {
            Ok(_) => continue,
            Err(position) => visited.insert(position, handle),
        }
        if let crate::Term::Constant { declaration, .. } = node {
            let index = *declaration as usize;
            let row = certificate
                .signature
                .get(index)
                .ok_or(CoreError::UnknownDeclaration {
                    declaration: *declaration,
                    signature_len: certificate.signature.len(),
                })?;
            required = required.max(index + 1);
            pending.push(row.ty);
            pending.extend(row.body);
        }
        pending.extend(node.children());
    }
    Ok(required)
}

#[cfg(test)]
mod tests {
    use super::{
        CoreError, Declaration, MathematicalCertificate, TermArena, TermHandle,
        certificate_assumption_closure, verify_mathematical_certificate,
    };
    use crate::tests::{
        default_budget, lambda, pi, polymorphic_identity_declaration, strict_sort, type_sort,
        variable,
    };
    use crate::{Level, Sort, Term};

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

    // Every proposition checks an argument against a computationally
    // equal type. Rechecking every shrinking pair tail repeats that work.
    fn nested_pair_certificate(arena: &mut TermArena, count: usize) -> MathematicalCertificate {
        let two = arena.insert(Term::Two);
        let zero = arena.insert(Term::TwoZero);
        let empty = arena.insert(Term::Empty);
        let strict = strict_sort(arena, 0);
        let domain_function = lambda(arena, two, two);
        let domain = arena.insert(Term::Apply {
            function: domain_function,
            argument: zero,
        });
        let truth = pi(arena, empty, empty);
        let predicate_type = pi(arena, domain, strict);
        let predicate_body = lambda(arena, domain, truth);
        let predicate = Declaration::definition(0, predicate_type, predicate_body);
        let constant = arena.insert(Term::Constant {
            declaration: 0,
            levels: Vec::new(),
        });
        let proposition = arena.insert(Term::Apply {
            function: constant,
            argument: zero,
        });
        let bound = variable(arena, 0);
        let proof = lambda(arena, empty, bound);
        let mut expected = proposition;
        let mut term = proof;
        for _ in 0..count {
            expected = arena.insert(Term::Sigma {
                domain: proposition,
                codomain: expected,
            });
            term = arena.insert(Term::Pair {
                first: proof,
                second: term,
            });
        }
        MathematicalCertificate::with_signature(vec![predicate], Vec::new(), term, expected)
    }

    #[test]
    fn nested_pair_proofs_fit_the_default_checking_budget() {
        let mut arena = TermArena::new();
        let certificate = nested_pair_certificate(&mut arena, 384);
        verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget())
            .unwrap();
    }

    #[test]
    fn a_malformed_pair_target_is_formed_before_its_components_are_used() {
        let mut arena = TermArena::new();
        let two = arena.insert(Term::Two);
        let zero = arena.insert(Term::TwoZero);
        let missing = variable(&mut arena, 1);
        let expected = arena.insert(Term::Sigma {
            domain: two,
            codomain: missing,
        });
        let term = arena.insert(Term::Pair {
            first: zero,
            second: zero,
        });
        let certificate = MathematicalCertificate::closed(Vec::new(), term, expected);
        assert!(matches!(
            verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget()),
            Err(CoreError::UnboundVariable { .. })
        ));
    }

    #[test]
    fn dependent_pair_components_keep_their_actual_target() {
        for invalid in 0..3 {
            let mut arena = TermArena::new();
            let two = arena.insert(Term::Two);
            let zero = arena.insert(Term::TwoZero);
            let one = arena.insert(Term::TwoOne);
            let type_zero = type_sort(&mut arena, 0);
            let function = pi(&mut arena, two, two);
            let motive = lambda(&mut arena, two, type_zero);
            let bound = variable(&mut arena, 0);
            let selected = arena.insert(Term::CaseTwo {
                motive,
                zero_branch: two,
                one_branch: function,
                scrutinee: bound,
            });
            let expected = arena.insert(Term::Sigma {
                domain: two,
                codomain: selected,
            });
            let identity = lambda(&mut arena, two, bound);
            let first = if invalid == 2 { type_zero } else { one };
            let second = if invalid == 1 { zero } else { identity };
            let term = arena.insert(Term::Pair { first, second });
            let certificate = MathematicalCertificate::closed(Vec::new(), term, expected);
            let checked =
                verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget());
            assert_eq!(checked.is_ok(), invalid == 0, "case {invalid}: {checked:?}");
        }
    }

    #[test]
    fn lambda_annotations_cannot_hide_an_ill_typed_argument() {
        for valid in [true, false] {
            let mut arena = TermArena::new();
            let two = arena.insert(Term::Two);
            let zero = arena.insert(Term::TwoZero);
            let empty = arena.insert(Term::Empty);
            let domain = if valid { two } else { empty };
            let function = lambda(&mut arena, domain, two);
            let annotation = arena.insert(Term::Apply {
                function,
                argument: zero,
            });
            let term = lambda(&mut arena, annotation, zero);
            let expected = pi(&mut arena, two, two);
            let certificate = MathematicalCertificate::closed(Vec::new(), term, expected);
            let checked =
                verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget());
            if valid {
                checked.expect("a well-typed computational annotation remains valid");
            } else {
                assert!(
                    matches!(
                        checked,
                        Err(CoreError::TypeMismatch { .. }
                            | CoreError::ArgumentTypeMismatch { .. })
                    ),
                    "the annotation must type-check before conversion: {checked:?}"
                );
            }
        }
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
