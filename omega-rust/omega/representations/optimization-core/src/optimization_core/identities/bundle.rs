use super::{
    IDENTITY_WIDTH, OptimizationDecisionLogIdentity, OptimizationRuleSetIdentity,
    OptimizationWorkloadProfileIdentity, TargetCostModelIdentity, TransformationLedgerIdentity,
};
use crate::OptimizationSelectionIdentity;
use std::fmt;

const BUNDLE_MAGIC: &[u8; 8] = b"OMGIDB\0\0";
const BUNDLE_VERSION: u32 = 1;

canonical_identity!(
    OptimizationIdentityBundleIdentity,
    b"omega.optimization-identity-bundle-identity.v1\0"
);
/// Complete identities required to replay or cache one optimization result.
///
/// Report rendering policy is intentionally absent. Optional authoritative
/// inputs carry explicit presence tags so absence cannot collide with an
/// all-zero or otherwise valid digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizationIdentityBundle {
    selections: OptimizationSelectionIdentity,
    rule_set: OptimizationRuleSetIdentity,
    target_cost_model: TargetCostModelIdentity,
    decision_log: Option<OptimizationDecisionLogIdentity>,
    workload_profile: Option<OptimizationWorkloadProfileIdentity>,
    transformation_ledger: TransformationLedgerIdentity,
}

impl OptimizationIdentityBundle {
    pub const fn new(
        selections: OptimizationSelectionIdentity,
        rule_set: OptimizationRuleSetIdentity,
        target_cost_model: TargetCostModelIdentity,
        decision_log: Option<OptimizationDecisionLogIdentity>,
        workload_profile: Option<OptimizationWorkloadProfileIdentity>,
        transformation_ledger: TransformationLedgerIdentity,
    ) -> Self {
        Self {
            selections,
            rule_set,
            target_cost_model,
            decision_log,
            workload_profile,
            transformation_ledger,
        }
    }

    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }

    pub const fn rule_set(self) -> OptimizationRuleSetIdentity {
        self.rule_set
    }

    pub const fn target_cost_model(self) -> TargetCostModelIdentity {
        self.target_cost_model
    }

    pub const fn decision_log(self) -> Option<OptimizationDecisionLogIdentity> {
        self.decision_log
    }

    pub const fn workload_profile(self) -> Option<OptimizationWorkloadProfileIdentity> {
        self.workload_profile
    }

    pub const fn transformation_ledger(self) -> TransformationLedgerIdentity {
        self.transformation_ledger
    }

    pub fn encode(self) -> Vec<u8> {
        let optional_width = |value: bool| 1 + usize::from(value) * IDENTITY_WIDTH;
        let mut encoded = Vec::with_capacity(
            12 + IDENTITY_WIDTH * 4
                + optional_width(self.decision_log.is_some())
                + optional_width(self.workload_profile.is_some()),
        );
        encoded.extend_from_slice(BUNDLE_MAGIC);
        encoded.extend_from_slice(&BUNDLE_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.selections.bytes());
        encoded.extend_from_slice(&self.rule_set.bytes());
        encoded.extend_from_slice(&self.target_cost_model.bytes());
        encode_optional(
            &mut encoded,
            self.decision_log.map(|identity| identity.bytes()),
        );
        encode_optional(
            &mut encoded,
            self.workload_profile.map(|identity| identity.bytes()),
        );
        encoded.extend_from_slice(&self.transformation_ledger.bytes());
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, IdentityBundleDecodeError> {
        let mut cursor = BundleCursor::new(encoded);
        if cursor.take(8)? != BUNDLE_MAGIC {
            return Err(IdentityBundleDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != BUNDLE_VERSION {
            return Err(IdentityBundleDecodeError::UnsupportedVersion(version));
        }
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let rule_set = OptimizationRuleSetIdentity::from_bytes(cursor.array()?);
        let target_cost_model = TargetCostModelIdentity::from_bytes(cursor.array()?);
        let decision_log = cursor
            .optional()?
            .map(OptimizationDecisionLogIdentity::from_bytes);
        let workload_profile = cursor
            .optional()?
            .map(OptimizationWorkloadProfileIdentity::from_bytes);
        let transformation_ledger = TransformationLedgerIdentity::from_bytes(cursor.array()?);
        if cursor.remaining() != 0 {
            return Err(IdentityBundleDecodeError::TrailingBytes);
        }
        Ok(Self::new(
            selections,
            rule_set,
            target_cost_model,
            decision_log,
            workload_profile,
            transformation_ledger,
        ))
    }

    pub fn identity(self) -> OptimizationIdentityBundleIdentity {
        OptimizationIdentityBundleIdentity::from_canonical_bytes(&self.encode())
    }
}

fn encode_optional(encoded: &mut Vec<u8>, identity: Option<[u8; IDENTITY_WIDTH]>) {
    match identity {
        None => encoded.push(0),
        Some(identity) => {
            encoded.push(1);
            encoded.extend_from_slice(&identity);
        }
    }
}

struct BundleCursor<'a> {
    encoded: &'a [u8],
    position: usize,
}

impl<'a> BundleCursor<'a> {
    const fn new(encoded: &'a [u8]) -> Self {
        Self {
            encoded,
            position: 0,
        }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], IdentityBundleDecodeError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(IdentityBundleDecodeError::Truncated)?;
        let bytes = self
            .encoded
            .get(self.position..end)
            .ok_or(IdentityBundleDecodeError::Truncated)?;
        self.position = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], IdentityBundleDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| IdentityBundleDecodeError::Truncated)
    }

    fn optional(&mut self) -> Result<Option<[u8; IDENTITY_WIDTH]>, IdentityBundleDecodeError> {
        match self.array::<1>()?[0] {
            0 => Ok(None),
            1 => Ok(Some(self.array()?)),
            tag => Err(IdentityBundleDecodeError::InvalidOptionalTag(tag)),
        }
    }

    fn remaining(&self) -> usize {
        self.encoded.len() - self.position
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityBundleDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidOptionalTag(u8),
    TrailingBytes,
}

impl fmt::Display for IdentityBundleDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => formatter.write_str("optimization identity bundle is truncated"),
            Self::WrongMagic => formatter.write_str("optimization identity bundle has wrong magic"),
            Self::UnsupportedVersion(version) => write!(
                formatter,
                "optimization identity bundle version {version} is unsupported"
            ),
            Self::InvalidOptionalTag(tag) => write!(
                formatter,
                "optimization identity bundle has invalid optional-presence tag {tag}"
            ),
            Self::TrailingBytes => {
                formatter.write_str("optimization identity bundle has trailing bytes")
            }
        }
    }
}

impl std::error::Error for IdentityBundleDecodeError {}
