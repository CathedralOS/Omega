//! Rejoin an opaque declaration's authored dependency-scope instances.
//!
//! Build and Product imports have distinct symbols for the same source.
//! The caller supplies independently rechecked representation custody; this
//! input preparation only matches its source instances, before specialization.
//! It neither selects a representation nor replaces property validation.

use validation::{OpaqueDataPropertyReceipt, OpaqueDataPropertyReceiptKind};

pub(super) fn for_source_instances(
    program: &dyn ::validation::CheckingInput,
    receipts: &[OpaqueDataPropertyReceipt],
) -> Vec<OpaqueDataPropertyReceipt> {
    // Preserve every supplied row, including duplicates and stale handles,
    // so inference cannot erase a validation failure.
    let mut instances = receipts.to_vec();
    for receipt in receipts {
        let mut definitions = program
            .data_definitions()
            .iter()
            .filter(|definition| definition.symbol == receipt.declaration());
        let Some(definition) = definitions.next() else {
            continue;
        };
        if definitions.next().is_some() {
            continue;
        }
        let Some(declaration) = program
            .symbols
            .symbol_provenance_source_span(definition.symbol)
        else {
            continue;
        };
        let Some(file) = program.symbols.source_file(declaration) else {
            continue;
        };
        for candidate in program.data_definitions() {
            let Some(span) = program
                .symbols
                .symbol_provenance_source_span(candidate.symbol)
            else {
                continue;
            };
            let Some(candidate_file) = program.symbols.source_file(span) else {
                continue;
            };
            if span.span != declaration.span
                || candidate_file.path != file.path
                || candidate_file.package_identity != file.package_identity
                || (file.package_identity.is_none()
                    && candidate_file.package_root != file.package_root)
                || candidate_file.origin != file.origin
                || candidate_file.resolution_stratum != file.resolution_stratum
                || candidate_file.source != file.source
            {
                continue;
            }
            let instance = match receipt.kind() {
                OpaqueDataPropertyReceiptKind::Copy => {
                    OpaqueDataPropertyReceipt::copy(candidate.symbol)
                }
            };
            // A caller may already supply both instances. Deduplicate only
            // inferred rows, never the original request.
            if !instances.contains(&instance) {
                instances.push(instance);
            }
        }
    }
    instances
}
