use super::super::semantics::declarations::nominal_identity;
use crate::record::PackageReviewCapabilityFlow;
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;

pub(crate) fn project_capability_flow(
    compilation: &CheckedCompilation,
    flow: &flow_effects::CapabilityFlowFact,
) -> Result<PackageReviewCapabilityFlow, Vec<Diagnostic>> {
    Ok(PackageReviewCapabilityFlow {
        capability: nominal_identity(compilation, flow.capability_symbol)?,
        kind: flow.kind,
        state: nominal_identity(compilation, flow.state_symbol)?,
        statement_index: flow.statement_index,
        call_ordinal: flow.call_ordinal,
        via_state: flow
            .via_state_symbol
            .is_valid()
            .then(|| nominal_identity(compilation, flow.via_state_symbol))
            .transpose()?,
    })
}
