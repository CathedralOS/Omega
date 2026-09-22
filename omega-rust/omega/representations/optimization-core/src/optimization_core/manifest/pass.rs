//! Pass-v1 construction, ordered-rule validation, and wire codec.

use std::collections::BTreeSet;

use crate::{
    OptimizationPassIdentity, OptimizationRuleIdentity, OptimizationRuleSetIdentity,
    OptimizationUnitIdentity,
};

use super::{
    DECISION_FIXED_WIDTH, InvalidOptimizationManifestRecord, OptimizationDecisionRecord,
    OptimizationManifestDecodeError, OptimizationWorkUsage, PASS_WIRE_FORMAT,
    codec::ManifestCursor,
};

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
            if !rules.contains(&decision.rule()) {
                return Err(InvalidOptimizationManifestRecord::DecisionNamesUnscheduledRule);
            }
            if !decision_ids.insert(decision.identity()) {
                return Err(InvalidOptimizationManifestRecord::DuplicateDecisionIdentity);
            }
            if !candidates.insert(decision.candidate()) {
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
        encoded.extend_from_slice(PASS_WIRE_FORMAT.magic);
        encoded.extend_from_slice(&PASS_WIRE_FORMAT.version.to_le_bytes());
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
        if cursor.take(8)? != PASS_WIRE_FORMAT.magic {
            return Err(OptimizationManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != PASS_WIRE_FORMAT.version {
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
