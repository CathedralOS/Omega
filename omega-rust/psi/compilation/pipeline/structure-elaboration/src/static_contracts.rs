//! Static schemas retain open bounds explicitly. Their conformance questions
//! name real selected bodies, independently of runtime callable construction.

use language_semantics::ServiceReachId;
use semantic_vocabulary::{BoundaryMachineId, IdTable, MachineId, PackageKeyIdentity, ServiceId};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use terminal_psi::{TerminalCallableInvocation, TerminalCallableTermination, TerminalModule};

use crate::callable_headers::CallableHeader;
use crate::reach::{ServiceMap, SourceInvocation};
use crate::{Signature, Unserved, refuse};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticServiceCeiling {
    Open,
    Closed(Vec<ServiceId>),
}
#[derive(Debug, Clone)]
pub struct StaticCallableEnvelope {
    pub service_reach: StaticServiceCeiling,
    pub invocations: Vec<TerminalCallableInvocation>,
    pub suspends: bool,
    pub blocks: bool,
    pub termination: TerminalCallableTermination,
}
#[derive(Debug, Clone)]
pub struct StaticRequirementSchema {
    pub declaration: SymbolHandle,
    pub abstract_types: u32,
    pub signature: Signature,
    pub envelope: StaticCallableEnvelope,
}
#[derive(Debug, Clone)]
pub struct StaticSchemaQuestion {
    pub declaration: SymbolHandle,
    pub requirements: Vec<StaticRequirementSchema>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticImplementation {
    Machine(MachineId),
    Native {
        source: SymbolHandle,
        candidate: usize,
    },
}
#[derive(Debug, Clone)]
pub struct StaticConformanceQuestion {
    pub declaration: SymbolHandle,
    pub requirement: SymbolHandle,
    /// Portable identity produced by the exact AppliedTraitScope owner.
    pub application: String,
    pub implementation: StaticImplementation,
    pub required: CallableHeader,
    pub envelope: StaticCallableEnvelope,
    pub provider: Option<StaticProviderCandidate>,
}

/// Source identities of one authored boundary implementation. Admission adds
/// the realized signature only after the ordinary refinement proof succeeds.
#[derive(Debug, Clone)]
pub struct StaticProviderCandidate {
    pub boundary: BoundaryMachineId,
    pub declaration: String,
    pub source_requirement: String,
    pub source_candidate: String,
    pub package: Option<PackageKeyIdentity>,
}

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
    pub provider: Option<StaticProviderCandidate>,
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
        module: &TerminalModule,
        native: &[crate::ExternalProviderCandidate],
    ) -> Checked<(Vec<StaticSchemaQuestion>, Vec<StaticConformanceQuestion>)> {
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
                                signature: requirement.signature.clone(),
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
                provider: source.provider.clone(),
            });
        }
        Ok((schemas, conformances))
    }
}
