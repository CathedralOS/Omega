use super::{
    AnalysisInvalidationSet, AnalysisSet, CoreContractDecodeError, OptimizationSafetyClass,
};
use crate::{OptimizationPassIdentity, OptimizationRuleIdentity};
use std::fmt;

const RULE_CONTRACT_MAGIC: &[u8; 8] = b"OMGRUL\0\0";
const RULE_CONTRACT_SCHEMA_VERSION: u32 = 1;

/// Stable declaration consumed by an ordered registry and pass manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OptimizationRuleContract {
    identity: OptimizationRuleIdentity,
    pass: OptimizationPassIdentity,
    version: u32,
    required_analyses: AnalysisSet,
    invalidated_analyses: AnalysisInvalidationSet,
    safety_class: OptimizationSafetyClass,
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
        encoded.extend_from_slice(&RULE_CONTRACT_SCHEMA_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&self.pass.bytes());
        encoded.extend_from_slice(&self.version.to_le_bytes());
        encoded.extend_from_slice(&self.required_analyses.encode());
        encoded.extend_from_slice(&self.invalidated_analyses.encode());
        encoded.extend_from_slice(&self.safety_class.encode());
        encoded
    }

    pub const fn identity(self) -> OptimizationRuleIdentity {
        self.identity
    }

    pub const fn pass(self) -> OptimizationPassIdentity {
        self.pass
    }

    pub const fn version(self) -> u32 {
        self.version
    }

    pub const fn required_analyses(self) -> AnalysisSet {
        self.required_analyses
    }

    pub const fn invalidated_analyses(self) -> AnalysisInvalidationSet {
        self.invalidated_analyses
    }

    pub const fn safety_class(self) -> OptimizationSafetyClass {
        self.safety_class
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, CoreContractDecodeError> {
        if encoded.len() != 97 {
            return Err(CoreContractDecodeError::WrongLength {
                expected: 97,
                actual: encoded.len(),
            });
        }
        if &encoded[..8] != RULE_CONTRACT_MAGIC {
            return Err(CoreContractDecodeError::WrongMagic);
        }
        let schema = u32::from_le_bytes(encoded[8..12].try_into().expect("fixed schema width"));
        if schema != RULE_CONTRACT_SCHEMA_VERSION {
            return Err(CoreContractDecodeError::UnsupportedVersion(schema));
        }
        let identity = OptimizationRuleIdentity::from_bytes(
            encoded[12..44]
                .try_into()
                .expect("fixed rule identity width"),
        );
        let pass = OptimizationPassIdentity::from_bytes(
            encoded[44..76]
                .try_into()
                .expect("fixed pass identity width"),
        );
        let version = u32::from_le_bytes(encoded[76..80].try_into().expect("fixed version width"));
        let required_analyses = AnalysisSet::decode(&encoded[80..88])?;
        let invalidated_analyses = AnalysisInvalidationSet::decode(&encoded[88..96])?;
        let safety_class = OptimizationSafetyClass::decode(&encoded[96..97])?;
        Self::new(
            identity,
            pass,
            version,
            required_analyses,
            invalidated_analyses,
            safety_class,
        )
        .map_err(|error| match error {
            InvalidOptimizationRuleContract::ZeroVersion => {
                CoreContractDecodeError::ZeroRuleVersion
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidOptimizationRuleContract {
    ZeroVersion,
}

impl fmt::Display for InvalidOptimizationRuleContract {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("optimization rule version must be nonzero")
    }
}

impl std::error::Error for InvalidOptimizationRuleContract {}
