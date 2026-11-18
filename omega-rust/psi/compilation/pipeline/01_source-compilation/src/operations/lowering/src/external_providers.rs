//! Native supply keeps a declaration header, not an invented executable body.
//! Candidates are unsealed questions until the lowered checker proves them.

use arena::IdTable;
use declaration_contracts::ExternalBindingIdentity;
use semantic_vocabulary::{BoundaryMachineId, MachineId, PackageKeyIdentity};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use terminal_psi::{ExternalProviderConformance, ExternalProviderOwner, TerminalModule};

use crate::lowering::{Signature, boundaries::BoundaryNamespace, callables::CallableNamespace};

pub use terminal_check_tables::contracts::ExternalProviderCandidate;

#[cfg(any(test, feature = "test-support"))]
pub type ProviderContractHeader = crate::lowering::callable_headers::CallableHeader;

#[derive(Debug, Clone)]
pub(crate) struct ProviderOwnerSource {
    pub nominal: SymbolHandle,
    pub identity: String,
    pub package: Option<PackageKeyIdentity>,
}
#[derive(Debug, Clone)]
pub(crate) struct ExternalProviderSource {
    pub source: SymbolHandle,
    pub satisfaction: arena::Handle<crate::source_state::machine::TraitConformance>,
    pub state: SymbolHandle,
    pub requirement: BoundaryMachineId,
    pub signature: Signature,
    pub envelope: crate::lowering::reach::SourceEnvelope,
    pub declaration: crate::lowering::declare::BoundaryDeclarationSource,
    pub provider: Option<ProviderOwnerSource>,
    pub package: Option<PackageKeyIdentity>,
    pub requirement_package: Option<PackageKeyIdentity>,
    pub target: Option<String>,
    pub binding: ExternalBindingIdentity,
    pub sealed_intrinsic: Option<terminal_psi::TerminalSealedNativeIntrinsic>,
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
        outcomes: &IdTable<MachineId, crate::lowering::Outcome>,
        records: &crate::lowering::lower::places::Records<'_>,
        callables: &CallableNamespace,
        boundaries: &BoundaryNamespace,
        services: &crate::lowering::reach::ServiceMap,
        qualifications: &mut crate::lowering::qualifications::Qualifications,
        module: &mut TerminalModule,
        typed_source: Option<&crate::source_state::TypedTrees>,
    ) -> Checked<Vec<ExternalProviderCandidate>> {
        let refuse = |detail| crate::lowering::read::services::unavailable(context, detail);
        if !module.external_providers.is_empty() {
            return Err(refuse(
                "native candidates cannot overwrite checked provider publication",
            ));
        }
        qualifications.identities = std::mem::take(&mut module.identities).into_builder();
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
            let mut header = |signature: &Signature| {
                crate::lowering::lower::signature_header(
                    context,
                    signature,
                    signatures,
                    outcomes,
                    records,
                    callables,
                    boundaries,
                    qualifications,
                )
            };
            let supplied = header(&source.signature)?;
            let required_header = header(&required.signature)?;
            let required_envelope = services.envelope(context, &required.envelope)?;
            let supplied_envelope = services.envelope(context, &source.envelope)?;
            let realization = crate::lowering::declare::provider_declaration(
                &source.declaration,
                &source.signature,
                &supplied.proof,
                &supplied_envelope,
                services,
                &mut qualifications.identities,
            )
            .map_err(refuse)?;
            let provider = source
                .provider
                .as_ref()
                .map(|owner| -> Checked<ExternalProviderOwner> {
                    let typed = typed_source.ok_or_else(|| {
                        refuse("native provider owner lost its immutable source declaration table")
                    })?;
                    let sources = source_admission::source_identity::toolchain_source_identities(
                        typed.symbols.source_files(),
                    )
                    .map_err(|_| {
                        refuse("native provider owner lost its exact toolchain source identities")
                    })?;
                    let declaration = crate::lowering::declare::nominal(
                        typed,
                        &sources,
                        owner.nominal,
                        &mut qualifications.identities,
                    )
                    .map_err(refuse)?;
                    Ok(ExternalProviderOwner {
                        declaration: terminal_psi::NominalDeclarationIdentity::new(declaration),
                        identity: qualifications.identities.intern(&owner.identity),
                        package: owner.package,
                    })
                })
                .transpose()?;
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
                    sealed_intrinsic: source.sealed_intrinsic,
                },
            });
        }
        module.identities = std::mem::take(&mut qualifications.identities).build();
        Ok(candidates)
    }
}
