//! Decision-v5 construction, canonical identity, and wire codec.

use crate::{
    AnalysisSet, OptimizationCandidateIdentity, OptimizationCandidateVerdict,
    OptimizationDecisionIdentity, OptimizationRuleIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity,
};

use super::{
    codec::ManifestCursor, InvalidOptimizationManifestRecord, OptimizationFactReference,
    OptimizationFactReferenceDecodeError, OptimizationManifestDecodeError, DECISION_FIXED_WIDTH,
    DECISION_WIRE_FORMAT,
};

/// Canonical machine record for one policy/validation decision.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OptimizationDecisionRecord {
    identity: OptimizationDecisionIdentity,
    input: OptimizationUnitIdentity,
    candidate: OptimizationCandidateIdentity,
    rule: OptimizationRuleIdentity,
    verdict: OptimizationCandidateVerdict,
    consumed_analyses: AnalysisSet,
    consumed_facts: Vec<OptimizationFactReference>,
    validator: Option<OptimizationValidatorIdentity>,
}

impl OptimizationDecisionRecord {
    pub fn new(
        input: OptimizationUnitIdentity,
        candidate: OptimizationCandidateIdentity,
        rule: OptimizationRuleIdentity,
        verdict: OptimizationCandidateVerdict,
        consumed_analyses: AnalysisSet,
        consumed_facts: Vec<OptimizationFactReference>,
        validator: Option<OptimizationValidatorIdentity>,
    ) -> Result<Self, InvalidOptimizationManifestRecord> {
        if verdict == OptimizationCandidateVerdict::Applied && validator.is_none() {
            return Err(InvalidOptimizationManifestRecord::AppliedWithoutValidator);
        }
        if consumed_facts.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(InvalidOptimizationManifestRecord::NonCanonicalConsumedFacts);
        }
        let identity = decision_identity(
            input,
            candidate,
            rule,
            verdict,
            consumed_analyses,
            &consumed_facts,
            validator,
        );
        Ok(Self {
            identity,
            input,
            candidate,
            rule,
            verdict,
            consumed_analyses,
            consumed_facts,
            validator,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(
            DECISION_FIXED_WIDTH
                + self.consumed_facts.len() * 33
                + usize::from(self.validator.is_some()) * 32,
        );
        encoded.extend_from_slice(DECISION_WIRE_FORMAT.magic);
        encoded.extend_from_slice(&DECISION_WIRE_FORMAT.version.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&self.input.bytes());
        encoded.extend_from_slice(&self.candidate.bytes());
        encoded.extend_from_slice(&self.rule.bytes());
        encoded.extend_from_slice(&self.verdict.encode());
        encoded.extend_from_slice(&self.consumed_analyses.encode());
        encoded.extend_from_slice(
            &u32::try_from(self.consumed_facts.len())
                .expect("consumed fact count fits u32")
                .to_le_bytes(),
        );
        for fact in &self.consumed_facts {
            encoded.extend_from_slice(&fact.encode());
        }
        match self.validator {
            None => encoded.push(0),
            Some(validator) => {
                encoded.push(1);
                encoded.extend_from_slice(&validator.bytes());
            }
        }
        encoded
    }

    pub const fn identity(&self) -> OptimizationDecisionIdentity {
        self.identity
    }

    pub const fn input(&self) -> OptimizationUnitIdentity {
        self.input
    }

    pub const fn candidate(&self) -> OptimizationCandidateIdentity {
        self.candidate
    }

    pub const fn rule(&self) -> OptimizationRuleIdentity {
        self.rule
    }

    pub const fn verdict(&self) -> OptimizationCandidateVerdict {
        self.verdict
    }

    pub const fn consumed_analyses(&self) -> AnalysisSet {
        self.consumed_analyses
    }

    pub fn consumed_facts(&self) -> &[OptimizationFactReference] {
        &self.consumed_facts
    }

    pub const fn validator(&self) -> Option<OptimizationValidatorIdentity> {
        self.validator
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, OptimizationManifestDecodeError> {
        let mut cursor = ManifestCursor::new(encoded);
        if cursor.take(8)? != DECISION_WIRE_FORMAT.magic {
            return Err(OptimizationManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != DECISION_WIRE_FORMAT.version {
            return Err(OptimizationManifestDecodeError::UnsupportedVersion(version));
        }
        let encoded_identity = OptimizationDecisionIdentity::from_bytes(cursor.array()?);
        let input = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let candidate = OptimizationCandidateIdentity::from_bytes(cursor.array()?);
        let rule = OptimizationRuleIdentity::from_bytes(cursor.array()?);
        let verdict = OptimizationCandidateVerdict::decode(cursor.take(2)?)
            .map_err(OptimizationManifestDecodeError::CoreContract)?;
        let consumed_analyses = AnalysisSet::decode(cursor.take(8)?)
            .map_err(OptimizationManifestDecodeError::CoreContract)?;
        let fact_count = u32::from_le_bytes(cursor.array()?) as usize;
        if fact_count > cursor.remaining().saturating_sub(1) / 33 {
            return Err(OptimizationManifestDecodeError::Truncated);
        }
        let mut consumed_facts = Vec::with_capacity(fact_count);
        for _ in 0..fact_count {
            consumed_facts.push(
                OptimizationFactReference::decode(
                    cursor.take(OptimizationFactReference::ENCODED_LENGTH)?,
                )
                .map_err(|error| match error {
                    OptimizationFactReferenceDecodeError::WrongLength { expected, actual } => {
                        OptimizationManifestDecodeError::WrongLength { expected, actual }
                    }
                    OptimizationFactReferenceDecodeError::UnknownTag(tag) => {
                        OptimizationManifestDecodeError::UnknownFactReference(tag)
                    }
                })?,
            );
        }
        let validator = match cursor.take(1)?[0] {
            0 => None,
            1 => Some(OptimizationValidatorIdentity::from_bytes(cursor.array()?)),
            tag => return Err(OptimizationManifestDecodeError::InvalidOptionalTag(tag)),
        };
        if cursor.remaining() != 0 {
            return Err(OptimizationManifestDecodeError::TrailingBytes);
        }
        let record = Self::new(
            input,
            candidate,
            rule,
            verdict,
            consumed_analyses,
            consumed_facts,
            validator,
        )
        .map_err(OptimizationManifestDecodeError::InvalidRecord)?;
        if record.identity != encoded_identity {
            return Err(OptimizationManifestDecodeError::DecisionIdentityMismatch);
        }
        Ok(record)
    }
}

fn decision_identity(
    input: OptimizationUnitIdentity,
    candidate: OptimizationCandidateIdentity,
    rule: OptimizationRuleIdentity,
    verdict: OptimizationCandidateVerdict,
    consumed_analyses: AnalysisSet,
    consumed_facts: &[OptimizationFactReference],
    validator: Option<OptimizationValidatorIdentity>,
) -> OptimizationDecisionIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.optimization-manifest-decision.v5\0");
    canonical.extend_from_slice(&input.bytes());
    canonical.extend_from_slice(&candidate.bytes());
    canonical.extend_from_slice(&rule.bytes());
    canonical.extend_from_slice(&verdict.encode());
    canonical.extend_from_slice(&consumed_analyses.encode());
    canonical.extend_from_slice(
        &u64::try_from(consumed_facts.len())
            .expect("consumed fact count fits u64")
            .to_le_bytes(),
    );
    for fact in consumed_facts {
        canonical.extend_from_slice(&fact.encode());
    }
    match validator {
        None => canonical.push(0),
        Some(validator) => {
            canonical.push(1);
            canonical.extend_from_slice(&validator.bytes());
        }
    }
    OptimizationDecisionIdentity::from_canonical_bytes(&canonical)
}
