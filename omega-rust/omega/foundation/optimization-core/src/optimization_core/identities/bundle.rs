use super::{
    IDENTITY_WIDTH, OptimizationDecisionLogIdentity, OptimizationRuleSetIdentity,
    OptimizationWorkloadProfileIdentity, TargetCostModelIdentity, TransformationLedgerIdentity,
};
use crate::OptimizationSelectionIdentity;
use crate::cursor::Cursor;
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

crate::record_views! {
    record OptimizationIdentityBundle; new;
    copied {
        selections: OptimizationSelectionIdentity,
        rule_set: OptimizationRuleSetIdentity,
        target_cost_model: TargetCostModelIdentity,
        decision_log: Option<OptimizationDecisionLogIdentity>,
        workload_profile: Option<OptimizationWorkloadProfileIdentity>,
        transformation_ledger: TransformationLedgerIdentity,
    }
}

impl OptimizationIdentityBundle {
    pub fn encode(self) -> Vec<u8> {
        let optional_width = |value: bool| 1 + usize::from(value) * IDENTITY_WIDTH;
        let mut encoded = Vec::with_capacity(
            12 + IDENTITY_WIDTH * 4
                + optional_width(self.decision_log.is_some())
                + optional_width(self.workload_profile.is_some()),
        );
        encoded.extend_from_slice(BUNDLE_MAGIC);
        crate::codec_bytes::push_encoded(&mut encoded, BUNDLE_VERSION.to_le_bytes());
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
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != BUNDLE_MAGIC {
            return Err(IdentityBundleDecodeError::WrongMagic);
        }
        let version = cursor.u32()?;
        if version != BUNDLE_VERSION {
            return Err(IdentityBundleDecodeError::UnsupportedVersion(version));
        }
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let rule_set = OptimizationRuleSetIdentity::from_bytes(cursor.array()?);
        let target_cost_model = TargetCostModelIdentity::from_bytes(cursor.array()?);
        let decision_log =
            optional_identity(&mut cursor)?.map(OptimizationDecisionLogIdentity::from_bytes);
        let workload_profile =
            optional_identity(&mut cursor)?.map(OptimizationWorkloadProfileIdentity::from_bytes);
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

crate::truncated_from_adapter!(IdentityBundleDecodeError);

fn optional_identity(
    cursor: &mut Cursor,
) -> Result<Option<[u8; IDENTITY_WIDTH]>, IdentityBundleDecodeError> {
    match cursor.array::<1>()?[0] {
        0 => Ok(None),
        1 => Ok(Some(cursor.array()?)),
        tag => Err(IdentityBundleDecodeError::InvalidOptionalTag(tag)),
    }
}
