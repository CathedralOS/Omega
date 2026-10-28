//! Batch checking shares only successful signature formation in one owned arena.

use super::{MathematicalCertificate, run_on_verification_stack};
use crate::mathematical_core::term::MAX_SUBTERMS;
use crate::mathematical_core::{
    Budget, Context, CoreError, Declaration, Signature, Term, TermArena, TermHandle,
    check_signature, check_type, infer_sort,
};
use std::collections::HashMap;

/// The ordinal identifies the failing judgment in the supplied batch.
#[derive(Debug)]
pub struct MathematicalCertificateBatchError {
    pub ordinal: usize,
    pub error: CoreError,
}

/// Re-decide every judgment with one total budget. Original arenas remain
/// untouched. Exact syntax is imported into an owned arena, so cached signature
/// handles never cross arena ownership. Only a successfully checked complete
/// ordered declaration vector can be reused; contexts and judgments always run.
pub fn verify_mathematical_certificates(
    certificates: &[(&TermArena, &MathematicalCertificate)],
    budget: &mut Budget,
) -> Result<(), MathematicalCertificateBatchError> {
    run_on_verification_stack(|| {
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
                let mut context =
                    Context::with_level_arity(certificate.level_arity).with_signature(signature);
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
    })
}

struct ImportedTerms {
    arena: TermArena,
    by_syntax: HashMap<Term, TermHandle>,
}
impl Default for ImportedTerms {
    fn default() -> Self {
        Self {
            arena: TermArena::new(),
            by_syntax: HashMap::new(),
        }
    }
}
impl ImportedTerms {
    fn certificate(
        &mut self,
        source: &TermArena,
        certificate: &MathematicalCertificate,
    ) -> Result<MathematicalCertificate, CoreError> {
        // This map is per source judgment: equal numeric handles from another
        // arena never alias. The syntax interner compares every field exactly.
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
        if let Some(&term) = local.get(&handle) {
            // A cycle is not a finite core term. Fail before kernel checking.
            return term.ok_or(CoreError::DummyTerm);
        }
        local.insert(handle, None);
        // The node layout comes from `Term::subterms` / `Term::with_children`,
        // so importing a term graph never re-lists the variants: every slot
        // lands in the owned arena first, then the node is rebuilt from them
        // and interned by syntax.
        let node = source.get(handle);
        let slots = node.subterms();
        let mut children = [handle; MAX_SUBTERMS];
        for (index, (child, _binders)) in slots.iter().enumerate() {
            children[index] = self.term(source, child, local)?;
        }
        let term = node.with_children(&children[..slots.len()]);
        let term = *self
            .by_syntax
            .entry(term.clone())
            .or_insert_with(|| self.arena.insert(term));
        local.insert(handle, Some(term));
        Ok(term)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        // Signature formation must reduce this annotation; every judgment
        // separately reduces the applied definition's type.
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
        verify_mathematical_certificates(&[(&first, &certificate), (&second, &other)], &mut batch)
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
        changed.signature[0].body = Some(second.insert(Term::Sort(Sort::Type(
            crate::mathematical_core::Level::Constant(0),
        ))));
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
        // Retain a non-normalized, discarded reference in a separate valid proof.
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
}
