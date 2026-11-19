//! Static schemas retain open bounds explicitly. Their conformance questions
//! name real selected bodies, independently of runtime callable construction.

use arena::IdTable;
use semantic_vocabulary::MachineId;
use semantic_vocabulary::ServiceReachId;
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use terminal_psi::{TerminalCallableInvocation, TerminalCallableTermination, TerminalModule};

use crate::lowering::reach::{ServiceMap, SourceInvocation};
use crate::lowering::{Signature, Unserved, refuse};

pub use terminal_check_tables::contracts::{
    StaticCallableEnvelope, StaticConformanceQuestion, StaticImplementation,
    StaticProviderCandidate, StaticRequirementInstance, StaticRequirementSchema,
    StaticSchemaQuestion, StaticServiceCeiling,
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
    pub(crate) fn publish(
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
    pub instance: Option<MachineId>,
    pub declaration: SymbolHandle,
    pub abstract_types: u32,
    /// `None` for a template requirement (`StaticRequirementSchema::signature`).
    pub signature: Option<Signature>,
    pub envelope: StaticSourceEnvelope,
}
#[derive(Debug, Clone)]
pub(crate) struct SourceSchema {
    pub declaration: SymbolHandle,
    pub requirements: Vec<SourceSchemaRequirement>,
    pub bindings: Vec<SourceRecordBinding>,
}
#[derive(Debug, Clone)]
pub(crate) struct SourceRecordBinding {
    pub account: terminal_check_tables::contracts::StaticRecordBinding,
    /// Applied signatures stay with the prepared schema; calls only select a member.
    pub signatures: Vec<crate::source_state::signature::StateSignature>,
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
#[derive(Debug, Clone, Copy)]
struct RecordBindingLocation {
    parameter: SymbolHandle,
    schema: SymbolHandle,
    binding: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StaticContracts {
    pub schemas: Vec<SourceSchema>,
    pub conformances: Vec<SourceConformance>,
    pub formed: Vec<FormedConformance>,
    /// Only prepared record binders are indexed; scalar formals need no rows.
    record_bindings: Vec<RecordBindingLocation>,
}

#[derive(Debug, Clone)]
pub(crate) struct FormedConformance {
    pub declaration: SymbolHandle,
    pub questions: std::ops::Range<usize>,
}

impl StaticContracts {
    pub(crate) fn conformance_questions(
        &self,
        declaration: SymbolHandle,
    ) -> Option<&[SourceConformance]> {
        let row = self
            .formed
            .binary_search_by_key(&symbol_key(declaration), |row| symbol_key(row.declaration))
            .ok()?;
        self.conformances.get(self.formed[row].questions.clone())
    }
}

fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

impl StaticContracts {
    fn record_binding_position(&self, parameter: SymbolHandle) -> Option<(usize, usize)> {
        let position = self
            .record_bindings
            .binary_search_by_key(&symbol_key(parameter), |binding| {
                symbol_key(binding.parameter)
            })
            .ok()?;
        let location = &self.record_bindings[position];
        let schema = self
            .schemas
            .binary_search_by_key(&symbol_key(location.schema), |schema| {
                symbol_key(schema.declaration)
            })
            .ok()?;
        Some((schema, location.binding))
    }

    pub(crate) fn record_binding(&self, parameter: SymbolHandle) -> Option<&SourceRecordBinding> {
        let (schema, binding) = self.record_binding_position(parameter)?;
        self.schemas[schema].bindings.get(binding)
    }

    pub(crate) fn record_binding_mut(
        &mut self,
        parameter: SymbolHandle,
    ) -> Option<&mut SourceRecordBinding> {
        let (schema, binding) = self.record_binding_position(parameter)?;
        self.schemas[schema].bindings.get_mut(binding)
    }

    pub(crate) fn insert_record_binding(
        &mut self,
        declaration: SymbolHandle,
        binding: SourceRecordBinding,
    ) -> Result<(), &'static str> {
        let schema = self
            .schemas
            .binary_search_by_key(&symbol_key(declaration), |schema| {
                symbol_key(schema.declaration)
            })
            .map_err(|_| "static record lost its formed complete schema")?;
        let parameter = binding.account.parameter;
        let position = match self
            .record_bindings
            .binary_search_by_key(&symbol_key(parameter), |binding| {
                symbol_key(binding.parameter)
            }) {
            Ok(_) => return Err("static record binder already has its prepared schema"),
            Err(position) => position,
        };
        // Schema insertion can move its row; its full handle stays stable. Binding
        // rows are append-only, so their ordinal stays valid as instances are added.
        let location = RecordBindingLocation {
            parameter,
            schema: declaration,
            binding: self.schemas[schema].bindings.len(),
        };
        self.schemas[schema].bindings.push(binding);
        self.record_bindings.insert(position, location);
        Ok(())
    }

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
        outcomes: &IdTable<MachineId, crate::lowering::Outcome>,
        records: &crate::lowering::lower::places::Records<'_>,
        callables: &crate::lowering::callables::CallableNamespace,
        boundaries: &crate::lowering::boundaries::BoundaryNamespace,
        services: &ServiceMap,
        qualifications: &mut crate::lowering::qualifications::Qualifications,
        module: &mut TerminalModule,
        source_reader: Option<&crate::lowering::read::Program<'_>>,
        native: &[crate::lowering::ExternalProviderCandidate],
    ) -> Checked<(Vec<StaticSchemaQuestion>, Vec<StaticConformanceQuestion>)> {
        qualifications.identities = std::mem::take(&mut module.identities).into_builder();
        let schemas = self
            .schemas
            .iter()
            .map(|schema| {
                Ok(StaticSchemaQuestion {
                    declaration: schema.declaration,
                    bindings: schema
                        .bindings
                        .iter()
                        .map(|binding| binding.account.clone())
                        .collect(),
                    requirements: schema
                        .requirements
                        .iter()
                        .map(|requirement| {
                            let instance = requirement
                                .instance
                                .map(|machine| {
                                    // Earlier declaration contracts can shift this contract
                                    // away from the machine ID. Read its allocated ID
                                    // from the machine rows, which are stored in ID order.
                                    let contract = usize::try_from(machine.get() - 1)
                                        .ok()
                                        .and_then(|position| module.machines.get(position))
                                        .filter(|published| published.id == machine)
                                        .ok_or_else(|| {
                                            refuse(
                                                machine,
                                                terminal_check_tables::Construct::Machine,
                                                Unserved::Declaration(
                                                    "requirement lost its published machine contract",
                                                ),
                                            )
                                        })?
                                        .contract
                                        .id;
                                    let signature = requirement.signature.as_ref().ok_or_else(|| {
                                        refuse(
                                            machine,
                                            terminal_check_tables::Construct::Machine,
                                            Unserved::Declaration(
                                                "requirement instance lost its source signature",
                                            ),
                                        )
                                    })?;
                                    let reader = source_reader.ok_or_else(|| {
                                        crate::lowering::read::services::unavailable(machine,
                                            "requirement instance lost its source identity owner")
                                    })?;
                                    let member = crate::lowering::declare::semantic_identity(
                                        reader.typed, requirement.declaration, &mut qualifications.identities,
                                    ).map_err(|reason| crate::lowering::read::services::unavailable(machine, reason))?;
                                    crate::lowering::lower::static_signature_header(
                                        machine,
                                        contract,
                                        signature,
                                        signatures,
                                        outcomes,
                                        records,
                                        callables,
                                        boundaries,
                                        qualifications,
                                    )
                                    .map(|header| StaticRequirementInstance { machine, member, header })
                                })
                                .transpose()?;
                            Ok(StaticRequirementSchema {
                                instance,
                                declaration: requirement.declaration,
                                abstract_types: requirement.abstract_types,
                                signature: requirement
                                    .signature
                                    .as_ref()
                                    .map(Signature::static_signature),
                                envelope: requirement.envelope.publish(context, services)?,
                            })
                        })
                        .collect::<Checked<Vec<_>>>()?,
                })
            })
            .collect::<Checked<Vec<_>>>()?;
        let mut conformances = Vec::with_capacity(self.conformances.len());
        for source in &self.conformances {
            // A conformance signature has no published machine contract.
            // Keep its proof tied to this local context.
            let required = crate::lowering::lower::static_signature_header(
                context,
                semantic_vocabulary::ContractId::new(context.get())
                    .expect("a machine context is nonzero"),
                &source.signature,
                signatures,
                outcomes,
                records,
                callables,
                boundaries,
                qualifications,
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
                        let reader = source_reader.ok_or_else(|| {
                            crate::lowering::read::services::unavailable(
                                context,
                                "static provider lost its immutable source declaration table",
                            )
                        })?;
                        let sources = reader.exact_toolchain_sources().map_err(|_| {
                            crate::lowering::read::services::unavailable(
                                context,
                                "static provider lost its exact toolchain source identities",
                            )
                        })?;
                        let identity = crate::lowering::declare::nominal(
                            reader.typed,
                            sources,
                            provider.declaration,
                            &mut qualifications.identities,
                        )
                        .map_err(|reason| {
                            crate::lowering::read::services::unavailable(context, reason)
                        })?;
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
        module.identities = std::mem::take(&mut qualifications.identities).build();
        Ok((schemas, conformances))
    }
}
