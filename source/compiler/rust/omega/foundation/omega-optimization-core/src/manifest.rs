use crate::{
    AnalysisSet, CoreContractDecodeError, OptimizationCandidateIdentity,
    OptimizationCandidateVerdict, OptimizationDecisionIdentity, OptimizationPassIdentity,
    OptimizationRuleIdentity, OptimizationRuleSetIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity, OptimizationWorkBudget,
};
use std::collections::BTreeSet;
use std::fmt;

const DECISION_MAGIC: &[u8; 8] = b"OMGDEC\0\0";
const DECISION_VERSION: u32 = 1;
const PASS_RECORD_MAGIC: &[u8; 8] = b"OMGPAR\0\0";
const PASS_RECORD_VERSION: u32 = 1;
const DECISION_FIXED_WIDTH: usize = 119;

/// Actual work consumed by one pass. Zero is valid; publication separately
/// proves that every axis stayed within the selected nonzero budget.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct OptimizationWorkUsage {
    pub rule_evaluations: u64,
    pub candidates: u64,
    pub validation_steps: u64,
    pub commits: u64,
    pub iterations: u64,
}

impl OptimizationWorkUsage {
    pub const fn within(self, budget: OptimizationWorkBudget) -> bool {
        self.rule_evaluations <= budget.rule_evaluations()
            && self.candidates <= budget.candidates()
            && self.validation_steps <= budget.validation_steps()
            && self.commits <= budget.commits()
            && self.iterations <= budget.iterations()
    }

    pub fn encode(self) -> [u8; 40] {
        let mut encoded = [0; 40];
        for (index, value) in [
            self.rule_evaluations,
            self.candidates,
            self.validation_steps,
            self.commits,
            self.iterations,
        ]
        .into_iter()
        .enumerate()
        {
            encoded[index * 8..index * 8 + 8].copy_from_slice(&value.to_le_bytes());
        }
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, OptimizationManifestDecodeError> {
        if encoded.len() != 40 {
            return Err(OptimizationManifestDecodeError::WrongLength {
                expected: 40,
                actual: encoded.len(),
            });
        }
        let value = |index: usize| {
            u64::from_le_bytes(
                encoded[index * 8..index * 8 + 8]
                    .try_into()
                    .expect("checked work-usage width"),
            )
        };
        Ok(Self {
            rule_evaluations: value(0),
            candidates: value(1),
            validation_steps: value(2),
            commits: value(3),
            iterations: value(4),
        })
    }
}

/// Canonical machine record for one policy/validation decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OptimizationDecisionRecord {
    identity: OptimizationDecisionIdentity,
    candidate: OptimizationCandidateIdentity,
    rule: OptimizationRuleIdentity,
    verdict: OptimizationCandidateVerdict,
    consumed_analyses: AnalysisSet,
    validator: Option<OptimizationValidatorIdentity>,
}

impl OptimizationDecisionRecord {
    pub fn new(
        identity: OptimizationDecisionIdentity,
        candidate: OptimizationCandidateIdentity,
        rule: OptimizationRuleIdentity,
        verdict: OptimizationCandidateVerdict,
        consumed_analyses: AnalysisSet,
        validator: Option<OptimizationValidatorIdentity>,
    ) -> Result<Self, InvalidOptimizationManifestRecord> {
        if verdict == OptimizationCandidateVerdict::Applied && validator.is_none() {
            return Err(InvalidOptimizationManifestRecord::AppliedWithoutValidator);
        }
        Ok(Self {
            identity,
            candidate,
            rule,
            verdict,
            consumed_analyses,
            validator,
        })
    }

    pub fn encode(self) -> Vec<u8> {
        let mut encoded =
            Vec::with_capacity(DECISION_FIXED_WIDTH + usize::from(self.validator.is_some()) * 32);
        encoded.extend_from_slice(DECISION_MAGIC);
        encoded.extend_from_slice(&DECISION_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&self.candidate.bytes());
        encoded.extend_from_slice(&self.rule.bytes());
        encoded.extend_from_slice(&self.verdict.encode());
        encoded.extend_from_slice(&self.consumed_analyses.encode());
        match self.validator {
            None => encoded.push(0),
            Some(validator) => {
                encoded.push(1);
                encoded.extend_from_slice(&validator.bytes());
            }
        }
        encoded
    }

    pub const fn identity(self) -> OptimizationDecisionIdentity {
        self.identity
    }

    pub const fn candidate(self) -> OptimizationCandidateIdentity {
        self.candidate
    }

    pub const fn rule(self) -> OptimizationRuleIdentity {
        self.rule
    }

    pub const fn verdict(self) -> OptimizationCandidateVerdict {
        self.verdict
    }

    pub const fn consumed_analyses(self) -> AnalysisSet {
        self.consumed_analyses
    }

    pub const fn validator(self) -> Option<OptimizationValidatorIdentity> {
        self.validator
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, OptimizationManifestDecodeError> {
        if encoded.len() < DECISION_FIXED_WIDTH {
            return Err(OptimizationManifestDecodeError::Truncated);
        }
        if &encoded[..8] != DECISION_MAGIC {
            return Err(OptimizationManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(encoded[8..12].try_into().expect("fixed version width"));
        if version != DECISION_VERSION {
            return Err(OptimizationManifestDecodeError::UnsupportedVersion(version));
        }
        let identity = OptimizationDecisionIdentity::from_bytes(
            encoded[12..44]
                .try_into()
                .expect("fixed decision identity width"),
        );
        let candidate = OptimizationCandidateIdentity::from_bytes(
            encoded[44..76]
                .try_into()
                .expect("fixed candidate identity width"),
        );
        let rule = OptimizationRuleIdentity::from_bytes(
            encoded[76..108]
                .try_into()
                .expect("fixed rule identity width"),
        );
        let verdict = OptimizationCandidateVerdict::decode(&encoded[108..110])
            .map_err(OptimizationManifestDecodeError::CoreContract)?;
        let consumed_analyses = AnalysisSet::decode(&encoded[110..118])
            .map_err(OptimizationManifestDecodeError::CoreContract)?;
        let (validator, expected) = match encoded[118] {
            0 => (None, DECISION_FIXED_WIDTH),
            1 => {
                if encoded.len() < DECISION_FIXED_WIDTH + 32 {
                    return Err(OptimizationManifestDecodeError::Truncated);
                }
                (
                    Some(OptimizationValidatorIdentity::from_bytes(
                        encoded[119..151]
                            .try_into()
                            .expect("fixed validator identity width"),
                    )),
                    DECISION_FIXED_WIDTH + 32,
                )
            }
            tag => return Err(OptimizationManifestDecodeError::InvalidOptionalTag(tag)),
        };
        if encoded.len() != expected {
            return Err(OptimizationManifestDecodeError::TrailingBytes);
        }
        Self::new(
            identity,
            candidate,
            rule,
            verdict,
            consumed_analyses,
            validator,
        )
        .map_err(OptimizationManifestDecodeError::InvalidRecord)
    }
}

/// Canonical pass-level manifest row. Rule and decision order is execution
/// order. Duplicate identities or a decision naming an unscheduled rule reject
/// before the row can be published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationPassManifestRecord {
    pass: OptimizationPassIdentity,
    input: OptimizationUnitIdentity,
    output: OptimizationUnitIdentity,
    ordered_rule_set: OptimizationRuleSetIdentity,
    ordered_rules: Vec<OptimizationRuleIdentity>,
    decisions: Vec<OptimizationDecisionRecord>,
    work_usage: OptimizationWorkUsage,
}

impl OptimizationPassManifestRecord {
    pub fn new(
        pass: OptimizationPassIdentity,
        input: OptimizationUnitIdentity,
        output: OptimizationUnitIdentity,
        ordered_rule_set: OptimizationRuleSetIdentity,
        ordered_rules: Vec<OptimizationRuleIdentity>,
        decisions: Vec<OptimizationDecisionRecord>,
        work_usage: OptimizationWorkUsage,
    ) -> Result<Self, InvalidOptimizationManifestRecord> {
        let expected_rule_set = OptimizationRuleSetIdentity::from_ordered_rules(&ordered_rules)
            .map_err(|_| InvalidOptimizationManifestRecord::DuplicateRuleIdentity)?;
        if ordered_rule_set != expected_rule_set {
            return Err(InvalidOptimizationManifestRecord::RuleSetIdentityMismatch);
        }
        let rules = ordered_rules.iter().copied().collect::<BTreeSet<_>>();
        let mut decision_ids = BTreeSet::new();
        let mut candidates = BTreeSet::new();
        for decision in &decisions {
            if !rules.contains(&decision.rule) {
                return Err(InvalidOptimizationManifestRecord::DecisionNamesUnscheduledRule);
            }
            if !decision_ids.insert(decision.identity) {
                return Err(InvalidOptimizationManifestRecord::DuplicateDecisionIdentity);
            }
            if !candidates.insert(decision.candidate) {
                return Err(InvalidOptimizationManifestRecord::DuplicateCandidateIdentity);
            }
        }
        Ok(Self {
            pass,
            input,
            output,
            ordered_rule_set,
            ordered_rules,
            decisions,
            work_usage,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let decisions = self
            .decisions
            .iter()
            .map(|decision| decision.encode())
            .collect::<Vec<_>>();
        let decision_bytes = decisions
            .iter()
            .map(|decision| 4 + decision.len())
            .sum::<usize>();
        let mut encoded = Vec::with_capacity(
            12 + 32 * 4 + 4 + self.ordered_rules.len() * 32 + 4 + decision_bytes + 40,
        );
        encoded.extend_from_slice(PASS_RECORD_MAGIC);
        encoded.extend_from_slice(&PASS_RECORD_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.pass.bytes());
        encoded.extend_from_slice(&self.input.bytes());
        encoded.extend_from_slice(&self.output.bytes());
        encoded.extend_from_slice(&self.ordered_rule_set.bytes());
        encoded.extend_from_slice(
            &u32::try_from(self.ordered_rules.len())
                .expect("pass manifest rule count fits u32")
                .to_le_bytes(),
        );
        for rule in &self.ordered_rules {
            encoded.extend_from_slice(&rule.bytes());
        }
        encoded.extend_from_slice(
            &u32::try_from(decisions.len())
                .expect("pass manifest decision count fits u32")
                .to_le_bytes(),
        );
        for decision in decisions {
            encoded.extend_from_slice(
                &u32::try_from(decision.len())
                    .expect("decision record length fits u32")
                    .to_le_bytes(),
            );
            encoded.extend_from_slice(&decision);
        }
        encoded.extend_from_slice(&self.work_usage.encode());
        encoded
    }

    pub const fn pass(&self) -> OptimizationPassIdentity {
        self.pass
    }

    pub const fn input(&self) -> OptimizationUnitIdentity {
        self.input
    }

    pub const fn output(&self) -> OptimizationUnitIdentity {
        self.output
    }

    pub const fn ordered_rule_set(&self) -> OptimizationRuleSetIdentity {
        self.ordered_rule_set
    }

    pub fn ordered_rules(&self) -> &[OptimizationRuleIdentity] {
        &self.ordered_rules
    }

    pub fn decisions(&self) -> &[OptimizationDecisionRecord] {
        &self.decisions
    }

    pub const fn work_usage(&self) -> OptimizationWorkUsage {
        self.work_usage
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, OptimizationManifestDecodeError> {
        let mut cursor = ManifestCursor::new(encoded);
        if cursor.take(8)? != PASS_RECORD_MAGIC {
            return Err(OptimizationManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != PASS_RECORD_VERSION {
            return Err(OptimizationManifestDecodeError::UnsupportedVersion(version));
        }
        let pass = OptimizationPassIdentity::from_bytes(cursor.array()?);
        let input = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let output = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let ordered_rule_set = OptimizationRuleSetIdentity::from_bytes(cursor.array()?);
        let rule_count = u32::from_le_bytes(cursor.array()?) as usize;
        if rule_count > cursor.remaining() / 32 {
            return Err(OptimizationManifestDecodeError::Truncated);
        }
        let mut ordered_rules = Vec::with_capacity(rule_count);
        for _ in 0..rule_count {
            ordered_rules.push(OptimizationRuleIdentity::from_bytes(cursor.array()?));
        }
        let decision_count = u32::from_le_bytes(cursor.array()?) as usize;
        if decision_count > cursor.remaining() / (4 + DECISION_FIXED_WIDTH) {
            return Err(OptimizationManifestDecodeError::Truncated);
        }
        let mut decisions = Vec::with_capacity(decision_count);
        for _ in 0..decision_count {
            let length = u32::from_le_bytes(cursor.array()?) as usize;
            decisions.push(OptimizationDecisionRecord::decode(cursor.take(length)?)?);
        }
        let work_usage = OptimizationWorkUsage::decode(cursor.take(40)?)?;
        if cursor.remaining() != 0 {
            return Err(OptimizationManifestDecodeError::TrailingBytes);
        }
        Self::new(
            pass,
            input,
            output,
            ordered_rule_set,
            ordered_rules,
            decisions,
            work_usage,
        )
        .map_err(OptimizationManifestDecodeError::InvalidRecord)
    }
}

struct ManifestCursor<'a> {
    encoded: &'a [u8],
    position: usize,
}

impl<'a> ManifestCursor<'a> {
    const fn new(encoded: &'a [u8]) -> Self {
        Self {
            encoded,
            position: 0,
        }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], OptimizationManifestDecodeError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(OptimizationManifestDecodeError::Truncated)?;
        let value = self
            .encoded
            .get(self.position..end)
            .ok_or(OptimizationManifestDecodeError::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], OptimizationManifestDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| OptimizationManifestDecodeError::Truncated)
    }

    fn remaining(&self) -> usize {
        self.encoded.len() - self.position
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidOptimizationManifestRecord {
    AppliedWithoutValidator,
    DuplicateRuleIdentity,
    RuleSetIdentityMismatch,
    DecisionNamesUnscheduledRule,
    DuplicateDecisionIdentity,
    DuplicateCandidateIdentity,
}

impl fmt::Display for InvalidOptimizationManifestRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid optimization manifest record: {self:?}")
    }
}

impl std::error::Error for InvalidOptimizationManifestRecord {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationManifestDecodeError {
    Truncated,
    WrongLength { expected: usize, actual: usize },
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidOptionalTag(u8),
    TrailingBytes,
    CoreContract(CoreContractDecodeError),
    InvalidRecord(InvalidOptimizationManifestRecord),
}

impl fmt::Display for OptimizationManifestDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid optimization manifest encoding: {self:?}"
        )
    }
}

impl std::error::Error for OptimizationManifestDecodeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AnalysisKind, OptimizationReasonCode};

    fn rule(name: &[u8]) -> OptimizationRuleIdentity {
        OptimizationRuleIdentity::from_canonical_bytes(name)
    }

    fn decision(rule: OptimizationRuleIdentity) -> OptimizationDecisionRecord {
        OptimizationDecisionRecord::new(
            OptimizationDecisionIdentity::from_canonical_bytes(b"decision"),
            OptimizationCandidateIdentity::from_canonical_bytes(b"candidate"),
            rule,
            OptimizationCandidateVerdict::Applied,
            AnalysisSet::new([AnalysisKind::ControlFlowGraph]),
            Some(OptimizationValidatorIdentity::from_canonical_bytes(
                b"validator",
            )),
        )
        .unwrap()
    }

    #[test]
    fn applied_decision_requires_independent_validator_and_round_trips() {
        let rule = rule(b"rule");
        assert_eq!(
            OptimizationDecisionRecord::new(
                OptimizationDecisionIdentity::from_canonical_bytes(b"decision"),
                OptimizationCandidateIdentity::from_canonical_bytes(b"candidate"),
                rule,
                OptimizationCandidateVerdict::Applied,
                AnalysisSet::default(),
                None,
            ),
            Err(InvalidOptimizationManifestRecord::AppliedWithoutValidator)
        );
        let decision = decision(rule);
        assert_eq!(
            OptimizationDecisionRecord::decode(&decision.encode()),
            Ok(decision)
        );
    }

    #[test]
    fn pass_record_binds_rule_order_decisions_and_usage() {
        let rules = vec![rule(b"first"), rule(b"second")];
        let rule_set = OptimizationRuleSetIdentity::from_ordered_rules(&rules).unwrap();
        let record = OptimizationPassManifestRecord::new(
            OptimizationPassIdentity::from_canonical_bytes(b"pass"),
            OptimizationUnitIdentity::from_canonical_bytes(b"input"),
            OptimizationUnitIdentity::from_canonical_bytes(b"output"),
            rule_set,
            rules.clone(),
            vec![decision(rules[0])],
            OptimizationWorkUsage {
                rule_evaluations: 2,
                candidates: 1,
                validation_steps: 8,
                commits: 1,
                iterations: 1,
            },
        )
        .unwrap();
        assert_eq!(
            OptimizationPassManifestRecord::decode(&record.encode()),
            Ok(record.clone())
        );

        let reversed = vec![rules[1], rules[0]];
        assert_eq!(
            OptimizationPassManifestRecord::new(
                record.pass,
                record.input,
                record.output,
                rule_set,
                reversed,
                Vec::new(),
                OptimizationWorkUsage::default(),
            ),
            Err(InvalidOptimizationManifestRecord::RuleSetIdentityMismatch)
        );
    }

    #[test]
    fn duplicate_and_unscheduled_manifest_identities_reject() {
        let first = rule(b"first");
        assert_eq!(
            OptimizationPassManifestRecord::new(
                OptimizationPassIdentity::from_canonical_bytes(b"pass"),
                OptimizationUnitIdentity::from_canonical_bytes(b"input"),
                OptimizationUnitIdentity::from_canonical_bytes(b"output"),
                OptimizationRuleSetIdentity::from_canonical_bytes(b"not-the-rules"),
                vec![first, first],
                Vec::new(),
                OptimizationWorkUsage::default(),
            ),
            Err(InvalidOptimizationManifestRecord::DuplicateRuleIdentity)
        );

        let listed = vec![first];
        let unscheduled = rule(b"unscheduled");
        assert_eq!(
            OptimizationPassManifestRecord::new(
                OptimizationPassIdentity::from_canonical_bytes(b"pass"),
                OptimizationUnitIdentity::from_canonical_bytes(b"input"),
                OptimizationUnitIdentity::from_canonical_bytes(b"output"),
                OptimizationRuleSetIdentity::from_ordered_rules(&listed).unwrap(),
                listed,
                vec![OptimizationDecisionRecord::new(
                    OptimizationDecisionIdentity::from_canonical_bytes(b"skip"),
                    OptimizationCandidateIdentity::from_canonical_bytes(b"candidate"),
                    unscheduled,
                    OptimizationCandidateVerdict::Skipped(
                        OptimizationReasonCode::Inapplicable,
                    ),
                    AnalysisSet::default(),
                    None,
                )
                .unwrap()],
                OptimizationWorkUsage::default(),
            ),
            Err(InvalidOptimizationManifestRecord::DecisionNamesUnscheduledRule)
        );
    }

    #[test]
    fn usage_checks_every_budget_axis() {
        let budget = OptimizationWorkBudget::new(1, 2, 3, 4, 5).unwrap();
        assert!(OptimizationWorkUsage::default().within(budget));
        assert!(
            !OptimizationWorkUsage {
                rule_evaluations: 2,
                ..OptimizationWorkUsage::default()
            }
            .within(budget)
        );
    }
}
