//! Product declarations retain the exact typed owner beside the module the
//! lowered checker accepted. Source metadata cannot replace that module's
//! contracts, execution graph or proof certificates.

mod external_declarations;
mod provider_declarations;
pub use external_declarations::{
    ExternalProviderDeclarations, PreparedExternalProviderDeclarations,
};

use std::ops::Deref;
use std::sync::Arc;

use diagnostics::Diagnostic;
use semantic_vocabulary::{BoundaryMachineId, IdTable, MachineId};
use symbols::SymbolHandle;
use terminal_codec::{CanonicalTerminalArtifact, EvidenceSeals};
use typed_trees::{FrozenTypedTrees, SourceRecordCorrespondence, SourceServiceCorrespondence};

#[derive(Debug, Clone)]
pub struct PublishedProviderDeclarations {
    pub typed: Arc<FrozenTypedTrees>,
    pub machines: IdTable<MachineId, SymbolHandle>,
    pub boundaries: IdTable<BoundaryMachineId, SymbolHandle>,
    pub records: Arc<[SourceRecordCorrespondence]>,
    pub services: Arc<[SourceServiceCorrespondence]>,
}

#[derive(Debug)]
pub struct PublishedProviderSource {
    pub typed: Arc<FrozenTypedTrees>,
    artifact: Arc<CanonicalTerminalArtifact>,
    evidence: Arc<EvidenceSeals>,
    declarations: PublishedProviderDeclarations,
}

impl PublishedProviderSource {
    pub fn new(
        declarations: PublishedProviderDeclarations,
        artifact: Arc<CanonicalTerminalArtifact>,
        evidence: EvidenceSeals,
    ) -> Result<Self, Vec<Diagnostic>> {
        let refuse = |reason: &str| {
            vec![Diagnostic::error(format!(
                "published provider source: {reason}"
            ))]
        };
        let module = artifact.semantic_module();
        let identity = terminal_codec::terminal_psi_identity(module)
            .map_err(|reason| refuse(&format!("module identity: {reason:?}")))?;
        if identity != evidence.module {
            return Err(refuse("proof seals belong to another accepted module"));
        }
        if declarations.machines.len() != module.machines.len()
            || declarations.boundaries.len() != module.boundary_machines.len()
        {
            return Err(refuse(
                "source correspondence does not cover the accepted declaration census",
            ));
        }
        let typed = &declarations.typed;
        for machine in &module.machines {
            let symbol = declarations
                .machines
                .get(machine.id)
                .ok_or_else(|| refuse("machine has no exact source coordinate"))?;
            let source = typed
                .machine_by_symbol(*symbol)
                .ok_or_else(|| refuse("machine source coordinate names no typed declaration"))?;
            let declared = machine
                .declaration
                .as_ref()
                .ok_or_else(|| refuse("machine has no checked nominal declaration"))?;
            let semantic = typed
                .normalized_hermetic_symbol_identity(*symbol)
                .map_err(|reason| refuse(&reason))?;
            if module.identities.text(declared.semantic_declaration) != Some(semantic.as_str())
                || typed.machine_callable_identity_text(source).as_deref()
                    != module.identities.text(declared.callable)
            {
                return Err(refuse(
                    "machine correspondence changes its checked declaration identity",
                ));
            }
        }
        for boundary in &module.boundary_machines {
            let symbol = declarations
                .boundaries
                .get(boundary.id)
                .ok_or_else(|| refuse("boundary has no exact source coordinate"))?;
            let declared = boundary
                .declaration
                .as_ref()
                .ok_or_else(|| refuse("boundary has no checked declaration"))?;
            let semantic = typed
                .normalized_hermetic_symbol_identity(*symbol)
                .map_err(|reason| refuse(&reason))?;
            if module.identities.text(declared.source.semantic_declaration)
                != Some(semantic.as_str())
            {
                return Err(refuse(
                    "boundary correspondence changes its checked declaration identity",
                ));
            }
        }
        let mut services = declarations.services.iter().collect::<Vec<_>>();
        services.sort_unstable_by_key(|row| row.service);
        let mut catalog_services = module.services.iter().collect::<Vec<_>>();
        catalog_services.sort_unstable_by_key(|row| row.id);
        if services
            .windows(2)
            .any(|pair| pair[0].service == pair[1].service)
            || !services
                .iter()
                .map(|row| row.service)
                .eq(catalog_services.iter().map(|row| row.id))
        {
            return Err(refuse(
                "service correspondence does not uniquely cover the accepted catalog",
            ));
        }
        for (service, published) in services.into_iter().zip(catalog_services) {
            if typed
                .service_reaches
                .definition(service.source)
                .is_none_or(|row| row.symbol != service.declaration)
            {
                return Err(refuse(
                    "service correspondence changes its source declaration",
                ));
            }
            let identity = typed
                .normalized_hermetic_symbol_identity(service.declaration)
                .map_err(|reason| refuse(&reason))?;
            if module.identities.text(published.identity) != Some(identity.as_str()) {
                return Err(refuse(
                    "service correspondence changes its checked declaration identity",
                ));
            }
        }
        let mut records = declarations.records.iter().collect::<Vec<_>>();
        records.sort_unstable_by_key(|row| row.record);
        if records
            .windows(2)
            .any(|pair| pair[0].record == pair[1].record)
        {
            return Err(refuse(
                "record correspondence repeats an accepted structural type",
            ));
        }
        let mut catalog_records = module.structural_types.iter().collect::<Vec<_>>();
        catalog_records.sort_unstable_by_key(|row| row.id);
        for record in records {
            let position = catalog_records
                .binary_search_by_key(&record.record, |row| row.id)
                .map_err(|_| refuse("record correspondence names no accepted structural type"))?;
            let lowered = catalog_records[position];
            if module.identities.text(lowered.identity) != Some(record.storage_identity.as_str()) {
                return Err(refuse(
                    "record correspondence changes its accepted storage identity",
                ));
            }
        }
        Ok(Self {
            typed: Arc::clone(typed),
            artifact,
            evidence: Arc::new(evidence),
            declarations,
        })
    }

    pub fn typed(&self) -> &Arc<FrozenTypedTrees> {
        &self.typed
    }
    pub fn artifact(&self) -> &Arc<CanonicalTerminalArtifact> {
        &self.artifact
    }
    pub fn evidence(&self) -> &Arc<EvidenceSeals> {
        &self.evidence
    }
    pub fn correspondence(&self) -> &PublishedProviderDeclarations {
        &self.declarations
    }
    pub fn module(&self) -> &terminal_psi::TerminalModule {
        self.artifact.semantic_module()
    }
    pub fn accepted_machine(
        &self,
        symbol: SymbolHandle,
    ) -> Result<&terminal_psi::TerminalMachine, String> {
        let mut sources = self
            .declarations
            .machines
            .iter()
            .filter(|(_, source)| **source == symbol);
        let (Some((id, _)), None) = (sources.next(), sources.next()) else {
            return Err("provider has no unique accepted machine occurrence".into());
        };
        self.module()
            .machine(id)
            .ok_or_else(|| "provider machine disappeared from its accepted module".into())
    }
    pub fn accepted_boundary(
        &self,
        symbol: SymbolHandle,
    ) -> Result<&terminal_psi::BoundaryMachineDeclaration, String> {
        let mut sources = self
            .declarations
            .boundaries
            .iter()
            .filter(|(_, source)| **source == symbol);
        let (Some((id, _)), None) = (sources.next(), sources.next()) else {
            return Err("provider requirement has no unique accepted boundary occurrence".into());
        };
        self.module()
            .boundary_machine(id)
            .ok_or_else(|| "provider boundary disappeared from its accepted module".into())
    }
    pub fn source_machine(&self, id: MachineId) -> Result<SymbolHandle, String> {
        self.declarations
            .machines
            .get(id)
            .copied()
            .ok_or_else(|| "accepted machine lost its source declaration".into())
    }
    pub fn service_symbol(
        &self,
        id: semantic_vocabulary::ServiceId,
    ) -> Result<SymbolHandle, String> {
        let mut rows = self
            .declarations
            .services
            .iter()
            .filter(|row| row.service == id);
        match (rows.next(), rows.next()) {
            (Some(row), None) => Ok(row.declaration),
            _ => Err("accepted service has no unique source declaration".into()),
        }
    }
    pub fn qualification_issuers(
        &self,
    ) -> Result<Vec<language_semantics::BoundaryQualificationIssuer>, &'static str> {
        if self.module().boundary_machines.iter().any(|boundary| {
            !boundary.normal_qualifications.is_empty()
                || !boundary.program_local_root_introductions.is_empty()
        }) {
            return Err(
                "published boundary qualification issuers need their actual domain-establishment receipts",
            );
        }
        Ok(Vec::new())
    }
    pub fn has_proof_obligations(&self) -> bool {
        self.module()
            .scalar_applications
            .proof_scope
            .plan
            .has_obligations
            || !self.evidence.seals.is_empty()
            || !self.module().indexed_theorems.is_empty()
            || !self.module().evidence_terms.is_empty()
            || !self.module().callables.descriptors.is_empty()
    }
}
impl AsRef<typed_trees::TypedTrees> for PublishedProviderSource {
    fn as_ref(&self) -> &typed_trees::TypedTrees {
        &self.typed
    }
}

impl Deref for PublishedProviderSource {
    type Target = FrozenTypedTrees;
    fn deref(&self) -> &Self::Target {
        &self.typed
    }
}

/// Typed declarations share the accepted Product source owner; the source
/// itself retains the artifact, proof seals and source correspondence.
pub type PreparedProviderInventory =
    typed_trees::provider_declarations::PreparedProviderInventory<PublishedProviderSource>;
