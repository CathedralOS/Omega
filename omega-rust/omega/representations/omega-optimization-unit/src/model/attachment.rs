//! One-time verifier-catalog attachment and aggregate identity resealing.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnershipFrontierFactIndexError {
    AlreadyAttached,
    TerminalIdentityMismatch,
    InvalidFactIdentity,
    NonCanonicalOrder,
    NonCanonicalSnapshot,
}

impl std::fmt::Display for OwnershipFrontierFactIndexError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid ownership frontier fact index: {self:?}")
    }
}

impl std::error::Error for OwnershipFrontierFactIndexError {}

/// Attach the complete verifier projection exactly once and bind it into unit
/// identity. Construction is intentionally separate from the bare seed API.
pub fn attach_ownership_frontier_facts(
    mut unit: PsiOptimizationUnit,
    facts: Vec<OwnershipFrontierFact>,
) -> Result<PsiOptimizationUnit, OwnershipFrontierFactIndexError> {
    if !unit.ownership_frontier_facts.is_empty() {
        return Err(OwnershipFrontierFactIndexError::AlreadyAttached);
    }
    if facts.iter().any(|fact| fact.psi != unit.psi) {
        return Err(OwnershipFrontierFactIndexError::TerminalIdentityMismatch);
    }
    if facts.iter().any(|fact| !fact.has_canonical_identity()) {
        return Err(OwnershipFrontierFactIndexError::InvalidFactIdentity);
    }
    if facts
        .windows(2)
        .any(|pair| (pair[0].machine, pair[0].site) >= (pair[1].machine, pair[1].site))
    {
        return Err(OwnershipFrontierFactIndexError::NonCanonicalOrder);
    }
    if facts
        .iter()
        .any(|fact| !canonical_ownership_frontier_snapshot(&fact.snapshot))
    {
        return Err(OwnershipFrontierFactIndexError::NonCanonicalSnapshot);
    }
    unit.ownership_frontier_facts = facts;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    Ok(unit)
}

pub fn canonical_ownership_frontier_snapshot(snapshot: &OwnershipFrontierSnapshot) -> bool {
    strictly_ordered_by(&snapshot.claims, |claim| claim.claim)
        && strictly_ordered_by(&snapshot.owned_places, |place| place.place)
        && strictly_ordered_by(&snapshot.partial_custody, |partial| partial.place)
        && snapshot
            .partial_custody
            .iter()
            .all(|partial| partial.moved_paths.windows(2).all(|pair| pair[0] < pair[1]))
}

fn strictly_ordered_by<T, K: Ord>(values: &[T], key: impl Fn(&T) -> K) -> bool {
    values.windows(2).all(|pair| key(&pair[0]) < key(&pair[1]))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptedObligationFactIndexError {
    AlreadyAttached,
    TerminalIdentityMismatch,
    InvalidFactIdentity,
    DuplicateOwner,
}

impl std::fmt::Display for AcceptedObligationFactIndexError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid accepted obligation fact index: {self:?}"
        )
    }
}

impl std::error::Error for AcceptedObligationFactIndexError {}

/// Attach the canonical verifier projection exactly once and bind it into the
/// optimization-unit identity. Bare units intentionally retain an empty index.
pub fn attach_accepted_obligation_facts(
    mut unit: PsiOptimizationUnit,
    mut facts: Vec<AcceptedObligationFact>,
) -> Result<PsiOptimizationUnit, AcceptedObligationFactIndexError> {
    if !unit.accepted_obligation_facts.is_empty() {
        return Err(AcceptedObligationFactIndexError::AlreadyAttached);
    }
    if facts.iter().any(|fact| fact.psi != unit.psi) {
        return Err(AcceptedObligationFactIndexError::TerminalIdentityMismatch);
    }
    if facts.iter().any(|fact| !fact.has_canonical_identity()) {
        return Err(AcceptedObligationFactIndexError::InvalidFactIdentity);
    }
    facts.sort_by_key(|fact| (fact.machine, fact.operation, fact.obligation));
    if facts.windows(2).any(|pair| {
        (pair[0].machine, pair[0].operation, pair[0].obligation)
            == (pair[1].machine, pair[1].operation, pair[1].obligation)
    }) {
        return Err(AcceptedObligationFactIndexError::DuplicateOwner);
    }
    unit.accepted_obligation_facts = facts;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    Ok(unit)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofQuestionIndexError {
    AlreadyAttached,
    TerminalIdentityMismatch,
    InvalidQuestionIdentity,
    DuplicateQuestion,
}

impl std::fmt::Display for ProofQuestionIndexError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid proof-question index: {self:?}")
    }
}

impl std::error::Error for ProofQuestionIndexError {}

/// Attach the verifier's complete ordered proof-question projection exactly
/// once. The input order is retained rather than reconstructed or sorted.
pub fn attach_proof_questions(
    mut unit: PsiOptimizationUnit,
    questions: Vec<ProofQuestion>,
) -> Result<PsiOptimizationUnit, ProofQuestionIndexError> {
    if !unit.proof_questions.is_empty() {
        return Err(ProofQuestionIndexError::AlreadyAttached);
    }
    if questions
        .iter()
        .any(|question| question.terminal_psi != unit.psi)
    {
        return Err(ProofQuestionIndexError::TerminalIdentityMismatch);
    }
    if questions
        .iter()
        .any(|question| !question.has_canonical_identity())
    {
        return Err(ProofQuestionIndexError::InvalidQuestionIdentity);
    }
    let mut identities = BTreeSet::new();
    let mut owners = BTreeSet::new();
    if questions.iter().any(|question| {
        !identities.insert(question.identity)
            || !owners.insert((question.owner, question.obligation))
    }) {
        return Err(ProofQuestionIndexError::DuplicateQuestion);
    }
    unit.proof_questions = questions;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    Ok(unit)
}
