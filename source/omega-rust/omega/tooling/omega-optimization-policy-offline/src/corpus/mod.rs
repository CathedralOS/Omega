//! Optimizer module role: executable entrance. Offline policy-corpus admission.
//!
//! `capture` canonicalizes strict V2 logs. `validate` independently replays the
//! complete corpus contract before this entrance returns opaque custody.

mod capture;
mod codec;
mod identity;
mod model;
mod split;
mod validate;

#[cfg(test)]
mod tests;

pub use identity::{
    DecisionSurfaceIdentity, OfflinePolicyCorpusIdentity, decision_surface_identity,
};
pub use model::{
    OfflinePolicyCorpusError, OfflinePolicyCorpusReceipt, OfflinePolicyDecisionExample,
    OfflinePolicySplit, ValidatedOfflinePolicyCorpus,
};
pub use split::split_for_source;

pub fn admit_external_decision_logs<I, B>(
    encoded_logs: I,
) -> Result<ValidatedOfflinePolicyCorpus, OfflinePolicyCorpusError>
where
    I: IntoIterator<Item = B>,
    B: AsRef<[u8]>,
{
    let candidate = capture::capture(encoded_logs)?;
    validate::validate(candidate)
}

pub fn decode_offline_policy_corpus(
    encoded: &[u8],
) -> Result<ValidatedOfflinePolicyCorpus, OfflinePolicyCorpusError> {
    let candidate = codec::decode(encoded)?;
    validate::validate(candidate)
}
