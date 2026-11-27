//! A theorem's scalar inputs are arbitrary, rather than globally fixed values.
//! Bind those inputs and their carrier bounds before closing its logical
//! premises. The existing kernels check both the source and closed judgments.

use super::{
    BoundedDenotation, BoundedDenotationError, Budget, DataSubject, Denotation, Elaboration,
    ProofNode, Proposition, PropositionContext, ScalarTerm, ScalarType, Term, ValueId,
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
    /// A hypothesis's type is not closed over its own declarations.
    OpenHypothesis,
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
    data_subjects: &[DataSubject],
) -> Result<PropositionContext, BoundedTheoremError> {
    let mut ordered = subjects.to_vec();
    ordered.sort_unstable_by_key(|(value, _)| *value);
    if let Some(pair) = ordered.windows(2).find(|pair| pair[0].0 == pair[1].0) {
        return Err(BoundedTheoremError::DuplicateSubject(pair[0].0));
    }
    let places = data_subjects.iter().zip(0..).map(|(subject, position)| {
        (
            subject.place,
            semantic_vocabulary::StructuralPlaceKind::Parameter { position },
        )
    });
    PropositionContext::from_value_types_and_places(subjects.iter().copied(), places).map_err(
        |error| {
            BoundedDenotationError::Certificate(super::ProofError::MalformedProposition(error))
                .into()
        },
    )
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
            ScalarType::Boolean | ScalarType::IeeeFloat(_) => denotation.scalar_term(&subject)?,
            ScalarType::Integer(_) => denotation.fixed_scalar_term(&subject)?,
            _ => return Err(BoundedTheoremError::UnsupportedSubject(*scalar_type)),
        };
        let Term::Constant { declaration, .. } = denotation.arena.get(term) else {
            return Err(BoundedTheoremError::UnboundSubject);
        };
        selected.push(declaration);
        // Each endpoint the scalar carrier-bound judgment accepts is bound: both
        // of a fixed integer's, and an address's lower endpoint zero. An
        // address's upper endpoint is the selected target's, never assumed.
        if let ScalarType::Integer(integer) = scalar_type
            && integer.minimum_value().is_some()
            && integer.maximum_value().is_some()
        {
            let endpoints: &[bool] = match integer.carrier() {
                semantic_vocabulary::IntegerCarrier::Fixed => &[true, false],
                semantic_vocabulary::IntegerCarrier::Address => &[true],
            };
            for &lower in endpoints {
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

/// Append each closed hypothesis type, with the declarations it names, to the
/// end of `declarations`, and return its type in `arena`. Each hypothesis's
/// declarations name only each other, so they keep their meaning as a block.
/// Both the expected type and a proof of it append them at the same point,
/// right after their subjects, so both name the same declarations.
fn append_hypotheses(
    arena: &mut TermArena,
    declarations: &mut Vec<Declaration>,
    hypotheses: &[BoundedImplicationType],
) -> Result<Vec<TermHandle>, BoundedTheoremError> {
    let mut types = Vec::with_capacity(hypotheses.len());
    for hypothesis in hypotheses {
        let offset =
            u32::try_from(declarations.len()).map_err(|_| BoundedTheoremError::OpenHypothesis)?;
        // Each source slot's copy, indexed by the slot.
        let mut copied = vec![None; hypothesis.arena.len() + 1];
        for declaration in &hypothesis.signature {
            let ty = copy_term(
                &hypothesis.arena,
                declaration.ty,
                arena,
                offset,
                &mut copied,
            )?;
            let body = declaration
                .body
                .map(|body| copy_term(&hypothesis.arena, body, arena, offset, &mut copied))
                .transpose()?;
            declarations.push(Declaration {
                level_arity: declaration.level_arity,
                ty,
                body,
                kind: declaration.kind.clone(),
            });
        }
        types.push(copy_term(
            &hypothesis.arena,
            hypothesis.ty,
            arena,
            offset,
            &mut copied,
        )?);
        let end =
            u32::try_from(declarations.len()).map_err(|_| BoundedTheoremError::OpenHypothesis)?;
        // The copies may name only the block they arrived with.
        if copied.iter().flatten().any(|&handle| {
            matches!(arena.get(handle), Term::Constant { declaration, .. }
                if declaration < offset || declaration >= end)
        }) {
            return Err(BoundedTheoremError::OpenHypothesis);
        }
    }
    Ok(types)
}

/// The same term in another arena, each constant moved `offset` places later.
fn copy_term(
    source: &TermArena,
    root: TermHandle,
    target: &mut TermArena,
    offset: u32,
    copied: &mut [Option<TermHandle>],
) -> Result<TermHandle, BoundedTheoremError> {
    let slot = |handle: TermHandle| handle.arena_index() as usize;
    let mut pending = vec![(root, false)];
    while let Some((handle, finished)) = pending.pop() {
        if copied
            .get(slot(handle))
            .ok_or(BoundedTheoremError::OpenHypothesis)?
            .is_some()
        {
            continue;
        }
        let node = source.get(handle);
        if !finished {
            pending.push((handle, true));
            pending.extend(node.children().map(|child| (child, false)));
            continue;
        }
        let node = match node {
            Term::Constant {
                declaration,
                levels,
            } => Term::Constant {
                declaration: declaration
                    .checked_add(offset)
                    .ok_or(BoundedTheoremError::OpenHypothesis)?,
                levels,
            },
            Term::Dummy => return Err(BoundedTheoremError::OpenHypothesis),
            other => {
                let children = other
                    .children()
                    .map(|child| copied[slot(child)].ok_or(BoundedTheoremError::OpenHypothesis))
                    .collect::<Result<Vec<_>, _>>()?;
                other.rebuilt(&children)
            }
        };
        copied[slot(handle)] = Some(target.insert(node));
    }
    copied[slot(root)].ok_or(BoundedTheoremError::OpenHypothesis)
}

/// Form the complete expected implication independently of a supplied proof.
/// Even an uninhabited conclusion has a type; accepting its evidence remains
/// the certificate checker's separate job.
///
/// Each of `hypotheses` is a closed implication the theorem receives before
/// its premises, such as the proof `Inner` given to a contract declared as
/// `machine<Inner: NonzeroIsPositive>(n: u8)`. The type is then
/// `Π subjects. Π hypotheses. Π premises. conclusion`.
///
/// Each of `data_subjects` is a sum input, bound after the scalar subjects as
/// one value of its sum's inductive declaration. The declaration is in the
/// type's signature, so two contracts over different sums never share a type.
pub fn denote_bounded_implication_type(
    subjects: &[(ValueId, ScalarType)],
    data_subjects: &[DataSubject],
    hypotheses: &[BoundedImplicationType],
    premises: &[Proposition],
    conclusion: &Proposition,
    budget: &mut Budget,
) -> Result<BoundedImplicationType, BoundedTheoremError> {
    let context = subject_context(subjects, data_subjects)?;
    for proposition in premises.iter().chain(std::iter::once(conclusion)) {
        context.validate(proposition).map_err(|error| {
            BoundedDenotationError::Certificate(super::ProofError::MalformedProposition(error))
        })?;
    }
    let mut denotation = Denotation::new();
    // Each sum is declared before any clause reads it.
    let data_inputs = data_subjects
        .iter()
        .map(|subject| denotation.declare_data_subject(subject))
        .collect::<Result<Vec<_>, _>>()?;
    let mut bindings = Vec::with_capacity(premises.len());
    for premise in premises {
        let ty = denotation.denote(premise)?;
        bindings.push(denotation.arena.insert(Term::Squash { ty }));
    }
    let ty = denotation.denote(conclusion)?;
    let ty = denotation.arena.insert(Term::Squash { ty });
    let (mut selected, _) = prepare_subjects(&mut denotation, subjects)?;
    selected.extend(data_inputs);
    let mut context = append_hypotheses(
        &mut denotation.arena,
        &mut denotation.declarations,
        hypotheses,
    )?;
    context.extend(bindings);
    let abstracted = abstract_judgment_type(
        &mut denotation.arena,
        MathematicalJudgmentType {
            signature: &denotation.declarations,
            level_arity: 0,
            context: &context,
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
///
/// `data_subjects` and `hypotheses` are bound as in
/// `denote_bounded_implication_type`. The proof
/// does not use them: it establishes the conclusion from the premises alone.
pub fn verify_bounded_theorem(
    subjects: &[(ValueId, ScalarType)],
    data_subjects: &[DataSubject],
    hypotheses: &[BoundedImplicationType],
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    proof: &ProofNode,
    budget: &mut Budget,
) -> Result<BoundedTheorem, BoundedTheoremError> {
    let context = subject_context(subjects, data_subjects)?;
    let machine_parameters = std::collections::BTreeSet::new();
    // Each sum is declared before any clause reads it, as in the type.
    let mut denotation = Denotation::new();
    let data_inputs = data_subjects
        .iter()
        .map(|subject| denotation.declare_data_subject(subject))
        .collect::<Result<Vec<_>, _>>()?;
    let mut elaboration = Elaboration::new(
        denotation,
        &context,
        goal,
        assumptions,
        semantic_axioms,
        &machine_parameters,
    )?;
    let (mut selected, carrier_bounds) = prepare_subjects(&mut elaboration.denotation, subjects)?;
    selected.extend(data_inputs);
    let hypotheses = append_hypotheses(
        &mut elaboration.denotation.arena,
        &mut elaboration.denotation.declarations,
        hypotheses,
    )?;
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
    let mut strict =
        strict_certificate_context(&mut denotation.arena, &denotation.certificate, budget)
            .map_err(BoundedTheoremError::Strict)?;
    // The hypotheses bind outside the premises, so the proof's own references
    // to its premises keep their indices.
    strict.context.splice(0..0, hypotheses);
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
