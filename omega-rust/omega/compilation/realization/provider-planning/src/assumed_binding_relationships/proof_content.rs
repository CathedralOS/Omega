//! Canonical content of an original accepted mathematical judgment.
//!
//! Supplier dependency capture already selects each checked judgment in its
//! original Product owner. Successful certificate admission retains its exact
//! immutable term arena, certificate and used-assumption receipt. Encoding that
//! certificate consumes those original roots; it does not denote, normalize or
//! decide the judgment again. The wire includes the complete certificate-local
//! signature and ordered context, so detached term handles and premise indices
//! are never hashed as identities.
//!
//! This component commits only to mathematical certificate content. The accepted
//! fact still owns the original proposition, premise and semantic-axiom receipts;
//! their source correspondence and the authored mathematical declaration cohort
//! remain separate dependencies. Neither this component nor missing certificate
//! content supplies complete semantic closure, root authorization or consent.

use checked_trees::CheckedProofJudgment;
use diagnostics::Diagnostic;
use proof_admission::{AcceptedFactRoute, MathematicalCoreDecision};

use crate::digest_encoder::DomainDigestEncoder;

/// Content produced solely from the canonical wire of an original accepted
/// certificate. Private construction prevents an arbitrary digest from naming
/// accepted proof content through this API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedBindingMathematicalJudgmentContent {
    digest: [u8; 32],
}

impl AssumedBindingMathematicalJudgmentContent {
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }
}

pub(super) fn capture(
    judgment: &CheckedProofJudgment,
) -> Result<Option<AssumedBindingMathematicalJudgmentContent>, Diagnostic> {
    let accepted = judgment.accepted.as_ref().ok_or_else(|| {
        Diagnostic::error("assumed binding proof dependency has no original accepted fact")
    })?;
    let AcceptedFactRoute::CertificateDerived { acceptance, .. } = &accepted.route else {
        return Ok(None);
    };
    let MathematicalCoreDecision::Judged(mathematical) = &acceptance.mathematical_core else {
        return Ok(None);
    };
    let encoded = terminal_codec::encode_mathematical_certificate(
        mathematical.arena(),
        mathematical.certificate(),
    )
    .map_err(|error| {
        Diagnostic::error(format!(
            "assumed binding accepted mathematical dependency cannot encode its original certificate: {error}"
        ))
    })?;
    Ok(Some(AssumedBindingMathematicalJudgmentContent {
        digest: DomainDigestEncoder::content(
            b"omega.assumed-binding-mathematical-judgment.sha256.v1\0",
            &encoded,
        ),
    }))
}
