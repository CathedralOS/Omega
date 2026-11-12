/*
We define the canonical `OMGCMPD\0` component description carrier, its
wire codec, its producer (`publish_component_description`), and its
source-free independent verifier (`verify_component`). A component description
embeds one canonical Terminal artifact alongside sorted rosters of import slots,
export surfaces, entry kinds, outgoing authority classes, installation service
bounds, custody constraints, retained provider plans, installation obligations,
and inseparable assumptions.
*/
/*
We keep component descriptions completely free of native image emission and
installation authority so build planning, replacement checks, and topology
verification can inspect verified component boundaries without importing
backend emitters or trusting producer-authored summary rows. Every
module-evident claim is re-derived from the embedded Terminal artifact during
`verify_component`, leaving installation-dependent resource and profile
decisions as explicit undischarged obligations.
*/
/*
Our producer (`component_description`) combines a sealed
`CanonicalTerminalArtifact` from `terminal_codec` with `SelectedProviderPlanFacts`
from `effects` and optional emitter stack/realization facts. Our verifier
(`component_verification`) re-decodes that embedded artifact, checks subject
identity and admission profile constraints, and produces an inert
`VerifiedComponent` whose `realizes_selected_plan` joins build-selected
`ProviderPlan` rows to exported checked realizations.
*/
#![forbid(unsafe_code)]

use crate::component_description::{check_publication_bounds, describe_decoded_component_facts};
use terminal_psi::TerminalPsiIdentity;

pub(crate) use optimization_core::digest_encoder;

pub mod component_description;
pub(crate) mod component_verification;

pub use component_description::{
    COMPONENT_DESCRIPTION_SCHEMA_V3, ComponentDescriptionFacts, DescribeError,
    decode_component_description, decode_component_description_with_artifact, description_subject,
    encode_component_description,
};
pub use component_verification::{
    AcceptedComponentAssumptions, ComponentVerificationRejection, ComponentVerificationRequest,
    IndependentRealizationMismatch, VerifiedComponent, verify_component,
};

pub fn publish_component_description(
    facts: ComponentDescriptionFacts<'_>,
) -> Result<(TerminalPsiIdentity, Vec<u8>), DescribeError> {
    let module = facts.artifact.semantic_module();
    let description = describe_decoded_component_facts(facts, module)?;
    let subject =
        terminal_codec::terminal_psi_identity(module).map_err(DescribeError::ArtifactIdentity)?;
    let encoded = encode_component_description(&description);
    check_publication_bounds(&description, &encoded)?;
    Ok((subject, encoded))
}
