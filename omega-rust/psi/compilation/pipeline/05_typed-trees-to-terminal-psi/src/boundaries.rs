//! Bodyless calls keep their own declaration and completion contract.
//! Provider selection settles that requirement later; it does not replace this
//! identity with a named body or an implicit service receiver.

use semantic_vocabulary::{BoundaryCompletion, BoundaryMachineId, IdTable, MachineId};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use terminal_psi::{
    BoundaryDeclarationRole, BoundaryMachineDeclaration, BoundaryMachineResult,
    BoundaryStructuralResultDeclaration, CheckedBoundaryCallable, CheckedBoundaryExecution,
    StructuralDomainRequirement, TerminalCallableTermination, TerminalMachineResult,
    TerminalModule,
};

use crate::{Outcome, Signature};

#[derive(Debug, Clone)]
pub(crate) struct BoundarySource {
    pub source: SymbolHandle,
    pub kind: semantic_vocabulary::BoundaryRequirementKind,
    pub owner: SymbolHandle,
    pub signature: Signature,
    pub envelope: crate::reach::SourceEnvelope,
    pub completion: BoundaryCompletion,
    pub owning_service: Option<language_semantics::ServiceReachId>,
    pub declaration: crate::declare::BoundaryDeclarationSource,
    pub run: Option<RunBoundarySource>,
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct RunBoundarySource {
    pub application: typed_trees::static_applications::StaticMachineApplicationHandle,
    pub element: crate::body::ValueType,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct BoundaryNamespace {
    pub rows: IdTable<BoundaryMachineId, BoundarySource>,
    // Derived links into the sole row owner, prepared from actual frozen handles.
    applications: Vec<(
        typed_trees::static_applications::StaticMachineApplicationHandle,
        Option<BoundaryMachineId>,
    )>,
}
fn application_key(
    application: typed_trees::static_applications::StaticMachineApplicationHandle,
) -> (u32, u32) {
    (application.arena_index(), application.generation())
}

impl BoundaryNamespace {
    pub fn new(typed: &typed_trees::TypedTrees) -> Self {
        let mut applications = typed
            .expression_table
            .static_machine_applications()
            .map(|(application, _)| (application, None))
            .collect::<Vec<_>>();
        applications.sort_unstable_by_key(|(application, _)| application_key(*application));
        Self {
            rows: IdTable::default(),
            applications,
        }
    }

    fn application_index(
        &self,
        typed: &typed_trees::TypedTrees,
        application: typed_trees::static_applications::StaticMachineApplicationHandle,
    ) -> Result<usize, &'static str> {
        if !typed
            .expression_table
            .static_machine_application_is_valid(application)
        {
            return Err("run custody application has no live source occurrence");
        }
        self.applications
            .binary_search_by_key(&application_key(application), |(known, _)| {
                application_key(*known)
            })
            .map_err(|_| "run custody application is outside its prepared source owner")
    }

    pub fn application(
        &self,
        typed: &typed_trees::TypedTrees,
        application: typed_trees::static_applications::StaticMachineApplicationHandle,
        symbol: SymbolHandle,
    ) -> Result<Option<BoundaryMachineId>, &'static str> {
        let index = self.application_index(typed, application)?;
        let Some(id) = self.applications[index].1 else {
            return Ok(None);
        };
        if !self.rows.get(id).is_some_and(|owner| {
            owner.source == symbol && owner.run.is_some_and(|run| run.application == application)
        }) {
            return Err("run custody cached boundary belongs to another source application");
        }
        Ok(Some(id))
    }

    pub fn insert_application(
        &mut self,
        typed: &typed_trees::TypedTrees,
        source: BoundarySource,
    ) -> Result<BoundaryMachineId, &'static str> {
        let run = source
            .run
            .ok_or("run custody boundary has no source application")?;
        let index = self.application_index(typed, run.application)?;
        if self.applications[index].1.is_some() {
            return Err("run custody source application already has a published boundary");
        }
        let id = self.rows.push(source);
        self.applications[index].1 = Some(id);
        Ok(id)
    }

    pub fn row(&self, boundary: BoundaryMachineId) -> Option<&BoundarySource> {
        self.rows.get(boundary)
    }
    pub fn signature(&self, boundary: BoundaryMachineId) -> Option<&Signature> {
        Some(&self.row(boundary)?.signature)
    }
    pub fn completion(&self, boundary: BoundaryMachineId) -> Option<BoundaryCompletion> {
        Some(self.row(boundary)?.completion)
    }
    pub fn outcome(&self, boundary: BoundaryMachineId) -> Option<Outcome> {
        let source = self.row(boundary)?;
        let crash_routes = match &source.signature.crashes {
            crate::body::CrashCeiling::Authored(routes) => routes.clone(),
            crate::body::CrashCeiling::DeclaredNone => Vec::new(),
            crate::body::CrashCeiling::Inferred => return None,
        };
        Some(Outcome {
            operational: [source.envelope.suspends, source.envelope.blocks],
            completes_normally: matches!(source.completion, BoundaryCompletion::NormalReturn),
            crash_routes,
            service_reach: match &source.signature.reaches {
                crate::Ceiling::Authored(row) => row.clone(),
                crate::Ceiling::Inferred => return None,
            },
            fixed_service_reach: None,
            synchronous_entries: match &source.signature.invokes {
                crate::Ceiling::Authored(row) => row.clone(),
                crate::Ceiling::Inferred => return None,
            },
        })
    }
    pub fn publish(
        &self,
        context: MachineId,
        signatures: &IdTable<MachineId, Signature>,
        outcomes: &IdTable<MachineId, Outcome>,
        records: &crate::lower::places::Records<'_>,
        callables: &crate::callables::CallableNamespace,
        services: &crate::reach::ServiceMap,
        qualifications: &mut crate::qualifications::Qualifications,
        module: &mut TerminalModule,
        declarations: Option<&crate::read::Program<'_>>,
    ) -> Checked<()> {
        let refusal = |detail: &str| Refusal {
            machine: context,
            concerns: Construct::Machine,
            reason: RefusalReason::Unsupported,
            source_span: None,
            detail: detail.to_owned(),
        };
        if !module.boundary_machines.is_empty() {
            return Err(refusal("boundary declarations were already published"));
        }
        qualifications.identities = std::mem::take(&mut module.identities).into_builder();
        for (id, source) in self.rows.iter() {
            let envelope = services.envelope(context, &source.envelope)?;
            let header = crate::lower::signature_header(
                context,
                &source.signature,
                signatures,
                outcomes,
                records,
                callables,
                self,
                qualifications,
            )?;
            if !header.proof.outcome_specific_ensures.is_empty() {
                return Err(refusal("boundary has an unsupported guarded guarantee"));
            }
            if !header.proof.erased_scalar_formals.is_empty()
                || !header.proof.erased_proof_formals.is_empty()
            {
                return Err(refusal("boundary needs retained erased actual lanes"));
            }
            let result_lifetimes = declarations
                .ok_or_else(|| refusal("boundary lost its prepared source lifetime context"))?
                .declared_result_lifetimes(context, &source.signature, &module.structural_types)
                .map_err(refusal)?
                .ok_or_else(|| {
                    refusal("boundary result has an unresolved declared lifetime frontier")
                })?;
            let result = match header.result {
                TerminalMachineResult::Unit => BoundaryMachineResult::Unit,
                TerminalMachineResult::Scalar(result) => {
                    BoundaryMachineResult::Scalar(result.value_type())
                }
                TerminalMachineResult::Structural(result) => {
                    if !result.reference_sources.is_empty() {
                        return Err(refusal(
                            "boundary reference results need retained public region declarations",
                        ));
                    }
                    BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
                        structural_type: result.structural_type,
                        multiplicity: result.multiplicity,
                        qualifications: result.qualifications,
                        projected_qualifications: result.projected_qualifications,
                    })
                }
            };
            let declaration = crate::declare::provider_declaration(
                &source.declaration,
                &source.signature,
                &header.proof,
                &envelope,
                services,
                &mut qualifications.identities,
            )
            .map_err(refusal)?;
            let mut scalar_domain_requirements = Vec::new();
            for (index, parameter) in header.scalar_parameters.iter().enumerate() {
                for domain in qualifications.members(context, parameter.qualifications)? {
                    scalar_domain_requirements.push(terminal_psi::ScalarDomainRequirement {
                        argument_index: u32::try_from(index)
                            .map_err(|_| refusal("boundary scalar formal ordinal overflow"))?,
                        domain: *domain,
                    });
                }
            }
            let requires = header
                .structural_parameters
                .iter()
                .flat_map(|parameter| {
                    parameter
                        .qualifications
                        .iter()
                        .map(move |domain| StructuralDomainRequirement {
                            argument_index: parameter.position,
                            path: Vec::new(),
                            domain: *domain,
                        })
                })
                .collect();
            let reader = declarations.ok_or_else(|| refusal("boundary lost its source owner"))?;
            let execution = if let Some(run) = source.run {
                let (operation, _) = reader
                    .typed
                    .compiler_primitive_callable_signature(run.application, source.source)
                    .map_err(refusal)?
                    .ok_or_else(|| {
                        refusal("compiler boundary lost its exact prepared source contract")
                    })?;
                let language_core::sealed_intrinsics::CompilerPrimitive::RunCustody(operation) =
                    operation
                else {
                    return Err(refusal("compiler boundary has another operation family"));
                };
                let bindings = reader
                    .typed
                    .expression_table
                    .static_machine_application_bindings(run.application);
                let Some(typed_trees::static_applications::StaticMachineBinding {
                    value: typed_trees::static_applications::StaticMachineBindingValue::Type(policy),
                    ..
                }) = bindings.first()
                else {
                    return Err(refusal(
                        "run custody publisher lost its actual policy operand",
                    ));
                };
                let policy = reader
                    .typed
                    .type_identity_text(reader.typed.package_qualified_type_identity(*policy))
                    .filter(|text| !text.is_empty())
                    .ok_or_else(|| refusal("run custody policy has no actual type identity"))?;
                let element = match run.element {
                    crate::body::ValueType::Scalar(scalar, _) => {
                        terminal_psi::StructuralFieldType::Scalar(scalar)
                    }
                    crate::body::ValueType::Record(record, _, _) => {
                        terminal_psi::StructuralFieldType::Structural(record)
                    }
                    _ => {
                        return Err(refusal(
                            "run custody publisher lost its actual element type coordinate",
                        ));
                    }
                };
                CheckedBoundaryExecution::RunCustody {
                    operation,
                    policy: qualifications.identities.intern(&policy),
                    element,
                }
            } else {
                match reader.typed.symbols.sealed_intrinsic(source.source) {
                Some(
                    language_core::sealed_intrinsics::SealedIntrinsic::FloatOperationRequirement,
                ) => CheckedBoundaryExecution::NumericPrimitive,
                Some(language_core::sealed_intrinsics::SealedIntrinsic::HostedPrimitive(
                    operation,
                )) => {
                    let machine = reader
                        .typed
                        .machine_by_symbol(source.source)
                        .ok_or_else(|| refusal("hosted primitive lost its declaration"))?;
                    let requirement = reader
                        .typed
                        .machine_trait_conformances(machine)
                        .iter()
                        .find_map(|edge| {
                            typed_trees::machine::resolve_satisfied_declaration(
                                reader.typed,
                                machine,
                                edge,
                            )
                        })
                        .ok_or_else(|| refusal("hosted primitive lost its requirement"))?
                        .symbol();
                    let requirement = self
                        .rows
                        .iter()
                        .find_map(|(id, row)| (row.source == requirement).then_some(id))
                        .ok_or_else(|| refusal("hosted primitive requirement was not published"))?;
                    CheckedBoundaryExecution::HostedPrimitive {
                        operation,
                        requirement,
                    }
                }
                _ => CheckedBoundaryExecution::Provider,
                }
            };
            let checked =
                if execution.compiler_owned() {
                    let mut reach = Vec::new();
                    for service in &envelope.service_reach {
                        reach.push(
                            services
                                .identity(*service)
                                .ok_or_else(|| refusal("boundary reach has no exact identity"))?,
                        );
                    }
                    let mut entries = Vec::new();
                    for entry in &envelope.invocations {
                        if let terminal_psi::TerminalCallableInvocation::Service(service) = entry {
                            entries.push(services.identity(*service).ok_or_else(|| {
                                refusal("boundary invocation has no exact identity")
                            })?);
                        }
                    }
                    crate::declare::boundary(
                        reader.typed,
                        source.source,
                        source.run.map(|run| run.application),
                        execution.clone(),
                        &source.signature,
                        crate::declare::Published {
                            operational: [envelope.suspends, envelope.blocks],
                            reach: &reach,
                            entries: &entries,
                        },
                        &mut qualifications.identities,
                    )
                    .map_err(|error| refusal(&error.message))?
                    .1
                } else {
                    let required = if reader
                        .typed
                        .trait_machine_signature(source.source)
                        .is_some()
                    {
                        source.source
                    } else {
                        let mut declarations =
                            reader.external_providers.rows.iter().filter(|row| {
                                row.source == source.source && row.boundary == Some(id)
                            });
                        let (Some(declared), None) = (declarations.next(), declarations.next())
                        else {
                            return Err(refusal(
                                "native boundary has no unique retained requirement association",
                            ));
                        };
                        self.row(declared.requirement)
                            .ok_or_else(|| refusal("native boundary lost its required source row"))?
                            .source
                    };
                    let (owner, _) = reader
                        .typed
                        .trait_machine_signature(required)
                        .ok_or_else(|| refusal("boundary lost its exact service declaration"))?;
                    let service_declaration =
                        crate::declare::service_declaration_commitment(reader.typed, owner)
                            .map_err(|diagnostic| refusal(&diagnostic.message))?;
                    CheckedBoundaryCallable {
                        service_declaration,
                        source: declaration,
                        execution: execution.clone(),
                        method_arguments: source
                            .declaration
                            .method_arguments(&mut qualifications.identities),
                    }
                };
            let reference_outputs = reader
                .declared_reference_outputs(
                    context,
                    &source.signature,
                    &module.structural_types,
                    &header.structural_parameters,
                )
                .map_err(refusal)?;
            let installation_bound = reader
                .typed
                .trait_machine_signature(source.source)
                .is_some_and(|(owner, signature)| {
                    owner.is_boundary && signature.service_reach_is_installation_bound
                });
            let callable_identity = if source.run.is_some() {
                checked.source.callable
            } else {
                qualifications
                    .identities
                    .intern(&source.declaration.callable)
            };
            module.boundary_machines.push(BoundaryMachineDeclaration {
                id,
                declaration: Some(checked),
                numeric_intrinsic: if execution == CheckedBoundaryExecution::NumericPrimitive { reader.typed.numeric_intrinsic_execution(source.source) } else { None },
                role: BoundaryDeclarationRole::Callable,
                kind: source.kind,
                completion: source.completion,
                unconditional_termination: matches!(source.envelope.termination, TerminalCallableTermination::Terminates { ref premises } if premises.is_empty()),
                identity: callable_identity,
                owner: terminal_psi::NominalDeclarationIdentity::new(qualifications.identities.intern(&source.declaration.owner)),
                attachment: None,
                parameter_order: header.parameter_order,
                scalar_parameters: header.scalar_parameters.iter().map(|parameter| parameter.scalar_type).collect(),
                crash_routes: header.proof.crash_routes,
                structural_parameters: header.structural_parameters,
                result,
                result_reference_regions: Vec::new(),
                result_lifetimes: Some(result_lifetimes),
                requires,
                normal_qualifications: Vec::new(),
                scalar_domain_requirements,
                scalar_requires: header.proof.requires,
                scalar_ensures: header.proof.ensures.into_iter().map(|clause| clause.proposition).collect(),
                reference_outputs,
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                service_reach_is_installation_bound: installation_bound,
                fixed_service_reach: if installation_bound { Vec::new() } else { envelope.service_reach.clone() },
                owning_service: source.owning_service.map(|service| services.service(context, service)).transpose()?,
                published_service_ceiling: envelope.service_reach,
                published_synchronous_entries: envelope.invocations.iter().filter_map(|entry| match entry { terminal_psi::TerminalCallableInvocation::Service(service) => Some(*service), terminal_psi::TerminalCallableInvocation::Parameter(_) => None }).collect(),
            });
        }
        module.identities = std::mem::take(&mut qualifications.identities).build();
        Ok(())
    }
}
