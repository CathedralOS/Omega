// `OptimizationRuleContract`: 97-byte wire declaration (`OMGRUL\0\0` v1)
// binding a rule identity, its owning pass identity, nonzero rule version,
// required and invalidated analysis bitsets, and required validation safety
// class.

use super::{AnalysisInvalidationSet, AnalysisSet, OptimizationSafetyClass};
use crate::{OptimizationPassIdentity, OptimizationRuleIdentity};

const RULE_CONTRACT_MAGIC: &[u8; 8] = b"OMGRUL\0\0";
const RULE_CONTRACT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OptimizationRuleContract {
    identity: OptimizationRuleIdentity, // Canonical identity of the rewrite rule.
    pass: OptimizationPassIdentity,     // Pass group that schedules this rule.
    version: u32,                       // Nonzero rule implementation version.
    required_analyses: AnalysisSet,     // Analyses that must be valid before evaluation.
    invalidated_analyses: AnalysisInvalidationSet, // Analyses invalidated when the rule commits.
    safety_class: OptimizationSafetyClass, // Validation strength required before commit.
}

::wire::record_views! {
    record OptimizationRuleContract;
    copied {
        identity: OptimizationRuleIdentity,
        pass: OptimizationPassIdentity,
        version: u32,
        required_analyses: AnalysisSet,
        invalidated_analyses: AnalysisInvalidationSet,
        safety_class: OptimizationSafetyClass,
    }
}

impl OptimizationRuleContract {
    pub fn new(
        identity: OptimizationRuleIdentity,
        pass: OptimizationPassIdentity,
        version: u32,
        required_analyses: AnalysisSet,
        invalidated_analyses: AnalysisInvalidationSet,
        safety_class: OptimizationSafetyClass,
    ) -> Result<Self, InvalidOptimizationRuleContract> {
        if version == 0 {
            return Err(InvalidOptimizationRuleContract::ZeroVersion);
        }
        Ok(Self {
            identity,
            pass,
            version,
            required_analyses,
            invalidated_analyses,
            safety_class,
        })
    }

    pub fn encode(self) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(97);
        encoded.extend_from_slice(RULE_CONTRACT_MAGIC);
        ::wire::codec_bytes::push_encoded(&mut encoded, RULE_CONTRACT_SCHEMA_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&self.pass.bytes());
        ::wire::codec_bytes::push_encoded(&mut encoded, self.version.to_le_bytes());
        encoded.extend_from_slice(&self.required_analyses.encode());
        encoded.extend_from_slice(&self.invalidated_analyses.encode());
        encoded.extend_from_slice(&self.safety_class.encode());
        encoded
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidOptimizationRuleContract {
    ZeroVersion,
}

::wire::fixed_error_display!(
    InvalidOptimizationRuleContract,
    "optimization rule version must be nonzero"
);
