//! Publish original public content beside the source judgment that owns it.
//!
//! The checking context already produced the conservation roster. This reads
//! that roster once and shares the signature grammar with source admission;
//! downstream content hashing performs no checking or plan construction.

pub(crate) fn record(
    program: &dyn validation::CheckingInput,
) -> checked_trees::CheckedPublicContractSources {
    let conservation = validation::content_conservation_plans(program);
    let mut sources = checked_trees::CheckedPublicContractSources::default();
    for source in conservation.iter() {
        sources.retain_conservation(source.source_expression, source.plan.clone());
    }
    for (_, signature) in program.trait_machine_signatures.iter() {
        sources.retain_signature(
            signature.symbol,
            validation::contract_identities::canonical_state_signature_bytes_with_conservation(
                program,
                signature,
                &conservation,
            ),
        );
    }
    sources
}
