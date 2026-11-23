//! Scalar proof certificates use ordinary identity types internally. Their
//! logical use must expose only Strict conclusions, without extracting a
//! relevant value from a Strict premise.

use crate::{
    Budget, CoreError, MathematicalCertificate, Term, TermArena, TermHandle,
    verify_mathematical_certificate,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrictCertificateError {
    Kernel(CoreError),
    DependentPremise(usize),
    DependentConclusion,
    ContextOverflow,
}

impl From<CoreError> for StrictCertificateError {
    fn from(error: CoreError) -> Self {
        Self::Kernel(error)
    }
}

impl std::fmt::Display for StrictCertificateError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(output, "{self:?}")
    }
}

impl std::error::Error for StrictCertificateError {}

fn context_independent(
    arena: &TermArena,
    root: TermHandle,
    free_variables: &mut [Option<u32>],
    budget: &mut Budget,
) -> Result<bool, StrictCertificateError> {
    let mut pending = vec![(root, false)];
    while let Some((handle, finish)) = pending.pop() {
        budget.consume()?;
        let node = arena.try_node(handle).ok_or(CoreError::DummyTerm)?;
        let slot = handle.arena_index() as usize;
        if free_variables[slot].is_some() {
            continue;
        }
        if !finish {
            pending.push((handle, true));
            pending.extend(node.children().map(|child| (child, false)));
            continue;
        }
        let extent = match node {
            Term::Variable(index) => index
                .checked_add(1)
                .ok_or(StrictCertificateError::ContextOverflow)?,
            _ => node
                .subterms()
                .iter()
                .map(|(child, binders)| {
                    free_variables[child.arena_index() as usize]
                        .expect("the verified type's child was visited")
                        .saturating_sub(binders)
                })
                .max()
                .unwrap_or(0),
        };
        free_variables[slot] = Some(extent);
    }
    Ok(free_variables[root.arena_index() as usize] == Some(0))
}

/// Lift `p₁: P₁, …, pₙ: Pₙ ⊢ t: Q` to the same judgment over
/// `h₁: Squash P₁, …, hₙ: Squash Pₙ ⊢ Squash Q`. Each elimination targets
/// the Strict conclusion; the relevant witnesses remain inside its function.
/// Premise and conclusion types must not depend on the proof witnesses.
/// Signature assumptions and definitions retain their original identities.
pub fn strict_certificate_context(
    arena: &mut TermArena,
    certificate: &MathematicalCertificate,
    budget: &mut Budget,
) -> Result<MathematicalCertificate, StrictCertificateError> {
    verify_mathematical_certificate(arena, certificate, budget)?;
    let mut free_variables = vec![None; arena.len() + 1];
    for (index, premise) in certificate.context.iter().copied().enumerate() {
        if !context_independent(arena, premise, &mut free_variables, budget)? {
            return Err(StrictCertificateError::DependentPremise(index));
        }
    }
    if !context_independent(arena, certificate.expected, &mut free_variables, budget)? {
        return Err(StrictCertificateError::DependentConclusion);
    }
    let count = u32::try_from(certificate.context.len())
        .map_err(|_| StrictCertificateError::ContextOverflow)?;
    let expected = arena.insert(Term::Squash {
        ty: certificate.expected,
    });
    let mut term = arena.insert(Term::SquashIntro {
        ty: certificate.expected,
        value: certificate.term,
    });
    for premise in certificate.context.iter().copied().rev() {
        let function = arena.insert(Term::Lambda {
            domain: premise,
            body: term,
        });
        // At this point the original witness prefix follows all new Strict
        // binders. That prefix offsets the selected binder to the same index.
        let scrutinee = arena.insert(Term::Variable(count - 1));
        term = arena.insert(Term::SquashElim {
            proposition: expected,
            function,
            scrutinee,
        });
    }
    let strict = MathematicalCertificate {
        signature: certificate.signature.clone(),
        level_arity: certificate.level_arity,
        context: certificate
            .context
            .iter()
            .map(|premise| arena.insert(Term::Squash { ty: *premise }))
            .collect(),
        term,
        expected,
    };
    verify_mathematical_certificate(arena, &strict, budget)?;
    Ok(strict)
}
