//! Static schemas retain open bounds explicitly. Their conformance questions
//! name real selected bodies, independently of runtime callable construction.

use language_semantics::ServiceReachId;
use semantic_vocabulary::{IdTable, MachineId};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use terminal_psi::{TerminalCallableInvocation, TerminalCallableTermination, TerminalModule};

use crate::reach::{ServiceMap, SourceInvocation};
use crate::{Signature, Unserved, refuse};

pub use terminal_check_tables::contracts::{
    StaticCallableEnvelope, StaticConformanceQuestion, StaticImplementation,
    StaticProviderCandidate, StaticRequirementSchema, StaticSchemaQuestion, StaticServiceCeiling,
};

#[derive(Debug, Clone)]
pub(crate) enum SourceServiceCeiling {
    Open,
    Closed(Vec<ServiceReachId>),
}
#[derive(Debug, Clone)]
pub(crate) struct StaticSourceEnvelope {
    pub service_reach: SourceServiceCeiling,
    pub invocations: Vec<SourceInvocation>,
    pub suspends: bool,
    pub blocks: bool,
    pub termination: TerminalCallableTermination,
}
impl StaticSourceEnvelope {
    fn publish(
        &self,
        context: MachineId,
        services: &ServiceMap,
    ) -> Checked<StaticCallableEnvelope> {
        let service_reach = match &self.service_reach {
            SourceServiceCeiling::Open => StaticServiceCeiling::Open,
            SourceServiceCeiling::Closed(row) => {
                StaticServiceCeiling::Closed(services.row(context, row)?)
            }
        };
        let mut invocations = self
            .invocations
            .iter()
            .map(|invocation| match invocation {
                SourceInvocation::Parameter(position) => {
                    Ok(TerminalCallableInvocation::Parameter(*position))
                }
                SourceInvocation::Service(service) => services
                    .service(context, *service)
                    .map(TerminalCallableInvocation::Service),
            })
            .collect::<Checked<Vec<_>>>()?;
        invocations.sort();
        invocations.dedup();
        Ok(StaticCallableEnvelope {
            service_reach,
            invocations,
            suspends: self.suspends,
            blocks: self.blocks,
            termination: self.termination.clone(),
        })
    }
    fn service_roots(&self, roots: &mut Vec<ServiceReachId>) {
        if let SourceServiceCeiling::Closed(row) = &self.service_reach {
            roots.extend_from_slice(row);
        }
        roots.extend(
            self.invocations
                .iter()
                .filter_map(|invocation| match invocation {
                    SourceInvocation::Service(service) => Some(*service),
                    SourceInvocation::Parameter(_) => None,
                }),
        );
    }
}
#[derive(Debug, Clone)]
pub(crate) struct SourceSchemaRequirement {
    pub declaration: SymbolHandle,
    pub abstract_types: u32,
    pub signature: Signature,
    pub envelope: StaticSourceEnvelope,
}
#[derive(Debug, Clone)]
pub(crate) struct SourceSchema {
    pub declaration: SymbolHandle,
    pub requirements: Vec<SourceSchemaRequirement>,
}
#[derive(Debug, Clone, Copy)]
pub(crate) enum SourceImplementation {
    Machine(MachineId),
    Native(SymbolHandle),
}
#[derive(Debug, Clone)]
pub(crate) struct SourceConformance {
    pub declaration: SymbolHandle,
    pub requirement: SymbolHandle,
    pub application: String,
    pub implementation: SourceImplementation,
    pub signature: Signature,
    pub envelope: StaticSourceEnvelope,
    pub provider: Option<StaticProviderCandidate<SymbolHandle>>,
}
#[derive(Debug, Clone, Default)]
pub(crate) struct StaticContracts {
    pub schemas: Vec<SourceSchema>,
    pub conformances: Vec<SourceConformance>,
    pub formed: Vec<SymbolHandle>,
}
impl StaticContracts {
    pub(crate) fn service_roots(&self, roots: &mut Vec<ServiceReachId>) {
        for schema in &self.schemas {
            for requirement in &schema.requirements {
                requirement.envelope.service_roots(roots);
            }
        }
        for conformance in &self.conformances {
            conformance.envelope.service_roots(roots);
        }
    }
    pub(crate) fn publish(
        &self,
        context: MachineId,
        signatures: &IdTable<MachineId, Signature>,
        outcomes: &IdTable<MachineId, crate::Outcome>,
        records: &crate::lower::places::Records<'_>,
        callables: &crate::callables::CallableNamespace,
        boundaries: &crate::boundaries::BoundaryNamespace,
        services: &ServiceMap,
        module: &mut TerminalModule,
        typed_source: Option<&typed_trees::TypedTrees>,
        native: &[crate::ExternalProviderCandidate],
    ) -> Checked<(Vec<StaticSchemaQuestion>, Vec<StaticConformanceQuestion>)> {
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        let schemas = self
            .schemas
            .iter()
            .map(|schema| {
                Ok(StaticSchemaQuestion {
                    declaration: schema.declaration,
                    requirements: schema
                        .requirements
                        .iter()
                        .map(|requirement| {
                            Ok(StaticRequirementSchema {
                                declaration: requirement.declaration,
                                abstract_types: requirement.abstract_types,
                                signature: requirement.signature.static_signature(),
                                envelope: requirement.envelope.publish(context, services)?,
                            })
                        })
                        .collect::<Checked<Vec<_>>>()?,
                })
            })
            .collect::<Checked<Vec<_>>>()?;
        let mut conformances = Vec::with_capacity(self.conformances.len());
        for source in &self.conformances {
            let required = crate::lower::static_signature_header(
                context,
                &source.signature,
                signatures,
                outcomes,
                records,
                callables,
                boundaries,
            )?;
            let implementation = match source.implementation {
                SourceImplementation::Machine(machine) => {
                    if module.machine(machine).is_none() {
                        return Err(refuse(
                            context,
                            terminal_check_tables::Construct::Machine,
                            Unserved::Declaration("conformance lost its exact selected body"),
                        ));
                    }
                    StaticImplementation::Machine(machine)
                }
                SourceImplementation::Native(source) => {
                    let candidate = native
                        .iter()
                        .position(|candidate| candidate.source == source)
                        .ok_or_else(|| {
                            refuse(
                                context,
                                terminal_check_tables::Construct::Machine,
                                Unserved::Declaration(
                                    "conformance lost its exact selected native header",
                                ),
                            )
                        })?;
                    StaticImplementation::Native { source, candidate }
                }
            };
            conformances.push(StaticConformanceQuestion {
                declaration: source.declaration,
                requirement: source.requirement,
                application: source.application.clone(),
                implementation,
                required,
                envelope: source.envelope.publish(context, services)?,
                provider: source
                    .provider
                    .as_ref()
                    .map(|provider| -> Checked<StaticProviderCandidate> {
                        let typed = typed_source.ok_or_else(|| {
                            crate::read::services::unavailable(
                                context,
                                "static provider lost its immutable source declaration table",
                            )
                        })?;
                        let sources = source::source_identity::toolchain_source_identities(
                            typed.symbols.source_files(),
                        )
                        .map_err(|_| {
                            crate::read::services::unavailable(
                                context,
                                "static provider lost its exact toolchain source identities",
                            )
                        })?;
                        let identity = crate::declare::nominal(
                            typed,
                            &sources,
                            provider.declaration,
                            &mut identities,
                        )
                        .map_err(|reason| crate::read::services::unavailable(context, reason))?;
                        Ok(StaticProviderCandidate {
                            boundary: provider.boundary,
                            declaration: terminal_psi::NominalDeclarationIdentity::new(identity),
                            provider_identity: provider.provider_identity.clone(),
                            source_requirement: provider.source_requirement.clone(),
                            source_candidate: provider.source_candidate.clone(),
                            package: provider.package,
                        })
                    })
                    .transpose()?,
            });
        }
        module.identities = identities.build();
        Ok((schemas, conformances))
    }
}
