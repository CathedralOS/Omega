//! A theorem's scalar inputs are arbitrary, rather than globally fixed values.
//! Bind those inputs and their carrier bounds before closing its logical
//! premises. The existing kernels check both the source and closed judgments.

use super::{
    BoundedDenotation, BoundedDenotationError, Budget, Denotation, Elaboration, ProofNode,
    Proposition, PropositionContext, ScalarTerm, ScalarType, Term, ValueId,
};
use mathematical_kernel::{
    CertificateAbstractionError, Declaration, MathematicalJudgmentType, StrictCertificateError,
    TermArena, TermHandle, abstract_certificate_assumptions, abstract_judgment_type,
    strict_certificate_context,
};

pub struct BoundedImplicationType {
    pub arena: TermArena,
    pub signature: Vec<Declaration>,
    pub ty: TermHandle,
}

pub struct BoundedTheorem {
    pub denotation: BoundedDenotation,
    /// The source signature's grants keep this exact association after inputs
    /// become binders and their dependent definitions are expanded.
    pub retained_declarations: Vec<Option<u32>>,
    pub subjects: Vec<(ValueId, ScalarType)>,
}

#[derive(Debug)]
pub enum BoundedTheoremError {
    Denotation(BoundedDenotationError),
    Strict(StrictCertificateError),
    Abstraction(CertificateAbstractionError),
    DuplicateSubject(ValueId),
    UnsupportedSubject(ScalarType),
    UnboundSubject,
}

crate::debug_display!(BoundedTheoremError);
impl std::error::Error for BoundedTheoremError {}

impl From<BoundedDenotationError> for BoundedTheoremError {
    fn from(error: BoundedDenotationError) -> Self {
        Self::Denotation(error)
    }
}

fn subject_context(
    subjects: &[(ValueId, ScalarType)],
) -> Result<PropositionContext, BoundedTheoremError> {
    let mut ordered = subjects.to_vec();
    ordered.sort_unstable_by_key(|(value, _)| *value);
    if let Some(pair) = ordered.windows(2).find(|pair| pair[0].0 == pair[1].0) {
        return Err(BoundedTheoremError::DuplicateSubject(pair[0].0));
    }
    PropositionContext::from_value_types(subjects.iter().copied()).map_err(|error| {
        BoundedDenotationError::Certificate(super::ProofError::MalformedProposition(error)).into()
    })
}

fn prepare_subjects(
    denotation: &mut Denotation,
    subjects: &[(ValueId, ScalarType)],
) -> Result<(Vec<u32>, Vec<u32>), BoundedTheoremError> {
    let mut selected = Vec::with_capacity(subjects.len());
    let mut carrier_bounds = Vec::with_capacity(subjects.len());
    for (value, scalar_type) in subjects {
        let subject = ScalarTerm::value(*value, *scalar_type);
        let term = match scalar_type {
            ScalarType::Boolean => denotation.scalar_term(&subject)?,
            ScalarType::Integer(integer)
                if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                    && !integer.is_address()
                    && integer.minimum_value().is_some()
                    && integer.maximum_value().is_some() =>
            {
                denotation.fixed_scalar_term(&subject)?
            }
            _ => return Err(BoundedTheoremError::UnsupportedSubject(*scalar_type)),
        };
        let Term::Constant { declaration, .. } = denotation.arena.get(term) else {
            return Err(BoundedTheoremError::UnboundSubject);
        };
        selected.push(declaration);
        if let ScalarType::Integer(integer) = scalar_type {
            for lower in [true, false] {
                let (_, evidence) = denotation.carrier_bound(&subject, lower, integer)?;
                let Term::Constant { declaration, .. } = denotation.arena.get(evidence) else {
                    return Err(BoundedTheoremError::UnboundSubject);
                };
                selected.push(declaration);
                carrier_bounds.push(declaration);
            }
        }
    }
    Ok((selected, carrier_bounds))
}

/// Form the complete expected implication independently of a supplied proof.
/// Even an uninhabited conclusion has a type; accepting its evidence remains
/// the certificate checker's separate job.
pub fn denote_bounded_implication_type(
    subjects: &[(ValueId, ScalarType)],
    premises: &[Proposition],
    conclusion: &Proposition,
    budget: &mut Budget,
) -> Result<BoundedImplicationType, BoundedTheoremError> {
    let context = subject_context(subjects)?;
    for proposition in premises.iter().chain(std::iter::once(conclusion)) {
        context.validate(proposition).map_err(|error| {
            BoundedDenotationError::Certificate(super::ProofError::MalformedProposition(error))
        })?;
    }
    let mut denotation = Denotation::new();
    let mut bindings = Vec::with_capacity(premises.len());
    for premise in premises {
        let ty = denotation.denote(premise)?;
        bindings.push(denotation.arena.insert(Term::Squash { ty }));
    }
    let ty = denotation.denote(conclusion)?;
    let ty = denotation.arena.insert(Term::Squash { ty });
    let (selected, _) = prepare_subjects(&mut denotation, subjects)?;
    let abstracted = abstract_judgment_type(
        &mut denotation.arena,
        MathematicalJudgmentType {
            signature: &denotation.declarations,
            level_arity: 0,
            context: &bindings,
            ty,
        },
        &selected,
        budget,
    )
    .map_err(BoundedTheoremError::Abstraction)?;
    Ok(BoundedImplicationType {
        arena: denotation.arena,
        signature: abstracted.signature,
        ty: abstracted.ty,
    })
}

/// The supplied subject order is the theorem's complete input telescope.
/// Every premise remains explicit, including premises unused by this proof.
/// This operation checks evidence; it neither searches for a missing proof nor
/// grants any retained signature assumption to the eventual receiver.
pub fn verify_bounded_theorem(
    subjects: &[(ValueId, ScalarType)],
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    proof: &ProofNode,
    budget: &mut Budget,
) -> Result<BoundedTheorem, BoundedTheoremError> {
    let context = subject_context(subjects)?;
    let machine_parameters = std::collections::BTreeSet::new();
    let mut elaboration = Elaboration::new(
        &context,
        goal,
        assumptions,
        semantic_axioms,
        &machine_parameters,
    )?;
    let (selected, carrier_bounds) = prepare_subjects(&mut elaboration.denotation, subjects)?;
    let term = elaboration.node(proof)?;
    if proof.conclusion != *goal {
        return Err(BoundedDenotationError::Certificate(
            super::ProofError::CertificateConclusionMismatch,
        )
        .into());
    }
    // These exact carrier proofs become input binders below. When the
    // primitive checker already cited their type, reuse that proof as a
    // definition instead of leaving a second, unbound assumption.
    for bound in carrier_bounds {
        let denotation = &mut elaboration.denotation;
        let ty = denotation.declarations[bound as usize].ty;
        let Some(decision) = denotation.decisions.get(&ty).copied() else {
            continue;
        };
        if decision <= bound || denotation.declarations[decision as usize].body.is_some() {
            return Err(BoundedTheoremError::UnboundSubject);
        }
        let evidence = denotation.constant(bound);
        denotation.declarations[decision as usize].body = Some(evidence);
    }
    let mut denotation = elaboration.finish(term);
    let strict = strict_certificate_context(&mut denotation.arena, &denotation.certificate, budget)
        .map_err(BoundedTheoremError::Strict)?;
    let abstracted =
        abstract_certificate_assumptions(&mut denotation.arena, &strict, &selected, budget)
            .map_err(BoundedTheoremError::Abstraction)?;
    denotation.certificate = abstracted.certificate;
    // The proof's premise/rule provenance remains the original bounded
    // derivation, including the difference between ambient and semantic facts.
    Ok(BoundedTheorem {
        denotation,
        retained_declarations: abstracted.retained_declarations,
        subjects: subjects.to_vec(),
    })
}
