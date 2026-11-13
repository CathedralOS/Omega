//! Native supply keeps a declaration header, not an invented executable body.
//! Candidates are unsealed questions until the lowered checker proves them.

use language_semantics::ExternalBindingIdentity;
use semantic_vocabulary::{BoundaryMachineId, IdTable, MachineId, PackageKeyIdentity};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use terminal_psi::{
    ExternalProviderConformance, ExternalProviderOwner, TerminalCallableEnvelope, TerminalModule,
};

use crate::{Signature, boundaries::BoundaryNamespace, callables::CallableNamespace};

pub type ProviderContractHeader = crate::callable_headers::CallableHeader;

#[derive(Debug, Clone)]
pub struct ExternalProviderCandidate {
    pub source: SymbolHandle,
    pub state: SymbolHandle,
    pub requirement: BoundaryMachineId,
    pub required: ProviderContractHeader,
    pub supplied: ProviderContractHeader,
    pub required_envelope: TerminalCallableEnvelope,
    pub supplied_envelope: TerminalCallableEnvelope,
    pub conformance: ExternalProviderConformance,
    pub completion: semantic_vocabulary::BoundaryCompletion,
    pub native_boundary: Option<BoundaryMachineId>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProviderOwnerSource {
    pub nominal: String,
    pub identity: String,
    pub package: Option<PackageKeyIdentity>,
}
#[derive(Debug, Clone)]
pub(crate) struct ExternalProviderSource {
    pub source: SymbolHandle,
    pub satisfaction: arena::Handle<typed_trees::machine::TraitConformance>,
    pub state: SymbolHandle,
    pub requirement: BoundaryMachineId,
    pub signature: Signature,
    pub envelope: crate::reach::SourceEnvelope,
    pub declaration: crate::declare::BoundaryDeclarationSource,
    pub provider: Option<ProviderOwnerSource>,
    pub package: Option<PackageKeyIdentity>,
    pub requirement_package: Option<PackageKeyIdentity>,
    pub target: Option<String>,
    pub binding: ExternalBindingIdentity,
    pub boundary: Option<BoundaryMachineId>,
    pub completion: semantic_vocabulary::BoundaryCompletion,
}
#[derive(Debug, Clone, Default)]
pub(crate) struct ExternalProviderNamespace {
    pub rows: Vec<ExternalProviderSource>,
    pub selected: Vec<SymbolHandle>,
    pub prepared: bool,
}
impl ExternalProviderNamespace {
    pub(crate) fn publish(
        &self,
        context: MachineId,
        signatures: &IdTable<MachineId, Signature>,
        outcomes: &IdTable<MachineId, crate::Outcome>,
        records: &crate::lower::places::Records<'_>,
        callables: &CallableNamespace,
        boundaries: &BoundaryNamespace,
        services: &crate::reach::ServiceMap,
        module: &mut TerminalModule,
    ) -> Checked<Vec<ExternalProviderCandidate>> {
        let refuse = |detail| crate::read::services::unavailable(context, detail);
        if !module.external_providers.is_empty() {
            return Err(refuse(
                "native candidates cannot overwrite checked provider publication",
            ));
        }
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        let mut candidates = Vec::with_capacity(self.rows.len());
        for source in &self.rows {
            let required = boundaries
                .row(source.requirement)
                .ok_or_else(|| refuse("native candidate lost its exact boundary requirement"))?;
            let boundary = module
                .boundary_machine(source.requirement)
                .ok_or_else(|| refuse("native candidate requirement has no published header"))?;
            let checked_requirement = boundary.declaration.as_ref().ok_or_else(|| {
                refuse("native candidate requirement lost its source declaration")
            })?;
            let header = |signature: &Signature| {
                crate::lower::signature_header(
                    context, signature, signatures, outcomes, records, callables, boundaries,
                )
                .map(ProviderContractHeader::from)
            };
            let supplied = header(&source.signature)?;
            let required_header = header(&required.signature)?;
            let required_envelope = services.envelope(context, &required.envelope)?;
            let supplied_envelope = services.envelope(context, &source.envelope)?;
            let realization = crate::declare::provider_declaration(
                &source.declaration,
                &source.signature,
                &supplied.proof,
                &supplied_envelope,
                services,
                &mut identities,
            )
            .map_err(refuse)?;
            let provider = source.provider.as_ref().map(|owner| ExternalProviderOwner {
                declaration: terminal_psi::NominalDeclarationIdentity::new(
                    identities.intern(&owner.nominal),
                ),
                identity: identities.intern(&owner.identity),
                package: owner.package,
            });
            candidates.push(ExternalProviderCandidate {
                source: source.source,
                state: source.state,
                requirement: source.requirement,
                required: required_header,
                supplied,
                required_envelope,
                supplied_envelope,
                completion: source.completion,
                native_boundary: source.boundary,
                conformance: ExternalProviderConformance {
                    requirement: checked_requirement.source.machine.clone(),
                    requirement_identity: boundary.identity,
                    requirement_package: source.requirement_package,
                    numeric_intrinsic: boundary.numeric_intrinsic,
                    requirement_schema: None,
                    target: source.target.clone(),
                    realization,
                    realization_package: source.package,
                    provider,
                    binding: source.binding.clone(),
                },
            });
        }
        module.identities = identities.build();
        Ok(candidates)
    }
}
