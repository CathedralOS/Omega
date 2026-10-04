//! Serialize a component description and its producer-observed subject from one
//! decoded Terminal module. This produces bytes, not verification or installation.

use crate::component_description::describe_decoded_component_facts;
use crate::{ComponentDescriptionFacts, DescribeError, encode_component_description};
use terminal_psi::TerminalPsiIdentity;

/// Produce canonical description bytes beside the semantic subject observed on
/// the supplied artifact. Package custody binds these coordinates to a package;
/// a later consumer must independently select its expected subject and profile
/// and verify the description. Neither result grants authority.
pub fn publish_component_description(
    facts: ComponentDescriptionFacts<'_>,
) -> Result<(TerminalPsiIdentity, Vec<u8>), DescribeError> {
    let module = terminal_codec::decode_module(facts.artifact.semantic_bytes())
        .map_err(DescribeError::ArtifactDecode)?;
    let description = describe_decoded_component_facts(facts, &module)?;
    let subject =
        terminal_codec::terminal_psi_identity(&module).map_err(DescribeError::ArtifactIdentity)?;
    Ok((subject, encode_component_description(&description)))
}
