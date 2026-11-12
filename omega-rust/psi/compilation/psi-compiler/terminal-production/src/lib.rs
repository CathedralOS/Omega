/*
    Shared checked-program-to-Terminal Psi (`Psi 06 -> 07 -> 08`) production,
    calling-signature capture, observation stages, and post-publication
    application attachment.

    We sequence `checked-trees-to-lowered-psi`, `lowered-psi-to-lowered-psi`,
    and `lowered-psi-to-terminal-psi` through `TerminalProductionRequest::produce`,
    capture boundary calling signatures and source-issued Build companions before
    publication erases source joins, and attach application roots and `ProgramEntry`
    receipts to the sealed artifact.
*/

/*
    Keeping checked-to-Terminal production below `psi-compiler` lets early
    build-time evaluation jobs and final product compilation share the exact
    same lowering, optimization, and canonical publication sequence without
    recursively importing the source compilation coordinator or duplicating stage
    pipelines.
*/

/*
    Start at `terminal_production` (`TerminalProductionRequest::produce` and
    `TerminalArtifactProductionError`), `calling_signatures`, `observation`, and
    `composition_roots` (`TerminalApplicationCustody::attach`) in this file;
    `build_bindings` owns source-issued Build binding, request, query,
    filesystem, and provider companions; and `custody` owns
    `ProducedTerminalProgram` and `PortableTerminalProgram`.
*/

#![forbid(unsafe_code)]

mod build_bindings;
pub(crate) use build_bindings::{
    build_filesystem, build_protocol, build_provider, build_queries, build_requests,
};
pub(crate) mod calling_signatures {
    use checked_trees::{CheckedTrees, PreparedProviderInventory};
    use diagnostics::Diagnostic;
    use semantic_vocabulary::{BoundaryMachineId, MachineId};
    use symbols::SymbolHandle;
    use terminal_psi::{
        CallingPolicyInstance, CallingPolicyRequest, CallingSignatureDeclaration,
        TargetMachineCallable, TerminalModule,
    };

    pub(crate) fn capture(
        checked: &CheckedTrees,
        inventory: &PreparedProviderInventory,
        module: &mut TerminalModule,
        boundaries: &[(SymbolHandle, BoundaryMachineId)],
        machines: &[(SymbolHandle, MachineId)],
        proof_declarations: &[(SymbolHandle, terminal_psi::ProofDeclarationId)],
    ) -> Result<(), Diagnostic> {
        let services = inventory.services_for(checked)?;
        let signatures = services.signatures();
        for &(source, boundary) in boundaries {
            let Some(application) = boundary_application(checked, signatures, source) else {
                continue;
            };
            module.calling_signatures.push(CallingSignatureDeclaration {
                boundary,
                application,
            });
        }
        module.calling_signatures.sort_by_key(|row| row.boundary);
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        let calling = signatures.calling();
        for request in calling.requests() {
            let declaration = checked
                .typed
                .normalized_hermetic_symbol_identity(request.boundary)
                .map(|identity| identities.intern(&identity));
            let instances = (|| {
                let source_instances = calling.instances(request).map_err(|errors| {
                    errors
                        .into_iter()
                        .map(|error| error.message)
                        .collect::<Vec<_>>()
                })?;
                let mut instances = Vec::with_capacity(source_instances.len());
                let owner = checked
                    .typed
                    .trait_definition_by_symbol(request.boundary)
                    .ok_or_else(|| {
                        vec!["calling policy request lost its exact trait".to_owned()]
                    })?;
                for instance in source_instances {
                    let scope = typed_trees::trait_definition::AppliedTraitScope::new(
                        &checked.typed,
                        owner,
                        typed_trees::trait_definition::TraitApplicationArguments {
                            lexical_owner: instance.argument_owner,
                            enclosing_data_applications: &[],
                            arguments: calling.arguments(instance),
                            lifetimes: &[],
                        },
                    )
                    .ok_or_else(|| {
                        vec!["calling policy instance lost its complete application".to_owned()]
                    })?;
                    let outer = scope
                        .portable_application_identity(&checked.typed)
                        .and_then(|identity| checked.typed.type_identity_text(identity))
                        .filter(|identity| !identity.is_empty())
                        .ok_or_else(|| {
                            vec!["calling policy instance lost its portable application".to_owned()]
                        })?;
                    let policy =
                        instance
                            .policy
                            .as_ref()
                            .map_err(Clone::clone)
                            .and_then(|source| {
                                let mut found = machines
                                    .iter()
                                    .filter(|(owner, _)| owner == source)
                                    .map(|(_, identity)| TargetMachineCallable::Machine(*identity))
                                    .chain(
                                        boundaries.iter().filter(|(owner, _)| owner == source).map(
                                            |(_, identity)| {
                                                TargetMachineCallable::Boundary(*identity)
                                            },
                                        ),
                                    );
                                let callable = found
                                    .next()
                                    .ok_or("calling policy has no exact published callable")?;
                                if found.next().is_some() {
                                    return Err(
                                        "calling policy has multiple published owners".into()
                                    );
                                }
                                Ok(callable)
                            });
                    let policy_declaration = instance
                        .policy
                        .as_ref()
                        .map_err(Clone::clone)
                        .and_then(|source| {
                            let mut matching = proof_declarations
                                .iter()
                                .filter(|(owner, _)| owner == source);
                            let (_, declaration) = matching
                                .next()
                                .ok_or("calling policy has no exact checked declaration")?;
                            if matching.next().is_some() {
                                return Err(
                                    "calling policy has competing checked declaration owners"
                                        .into(),
                                );
                            }
                            Ok(*declaration)
                        });
                    let applications = (|| {
                        let mut applications = Vec::new();
                        for application in calling.signatures(instance) {
                            if let Some(reason) = &application.abi_refusal {
                                return Err(reason.clone());
                            }
                            applications.extend(application.abi_signatures.iter().copied());
                        }
                        Ok(applications)
                    })();
                    instances.push(CallingPolicyInstance {
                        declaring_application: identities.intern(&outer),
                        policy_declaration,
                        policy,
                        signatures: applications,
                    });
                }
                Ok(instances)
            })();
            module.calling_policy_requests.push(CallingPolicyRequest {
                declaration,
                display_name: request.name.clone(),
                instances,
            });
        }
        module.identities = identities.build();
        Ok(())
    }

    fn boundary_application(
        checked: &CheckedTrees,
        signatures: &typed_trees::service_declarations::signatures::PreparedBoundarySignatures,
        source: SymbolHandle,
    ) -> Option<Result<semantic_vocabulary::CallingSignatureId, String>> {
        if let Some(application) = signatures.abi_for_boundary(source) {
            return Some(application.clone());
        }
        let typed = &checked.typed;
        let machine = typed.machine_by_symbol(source)?;
        if !matches!(
            machine.supply_mode,
            language_semantics::MachineSupplyMode::ExternalRealization { .. }
        ) {
            return None;
        }
        let mut applications = Vec::new();
        for edge in typed.machine_trait_conformances(machine) {
            let mut edge_applications = Vec::new();
            let scope = typed
                .trait_machine_signature(edge.requirement_symbol)
                .map(|(owner, _)| {
                    typed_trees::trait_definition::AppliedTraitScope::from_machine_conformance(
                        typed, machine, edge,
                    )
                    .and_then(|scope| scope.declaring_scope(typed, owner.symbol))
                    .ok_or_else(|| {
                        "external satisfier lost its complete declaring application".to_owned()
                    })
                })
                .transpose();
            let scope = match scope {
                Ok(scope) => scope,
                Err(reason) => return Some(Err(reason)),
            };
            for association in
                checked
                    .facts
                    .provider_closed_requirements
                    .iter()
                    .filter(|association| {
                        association.provider == source
                            && association.selecting_declaration == source
                            && association.template == edge.requirement_symbol
                    })
            {
                let receiving_matches = match &scope {
                    Some(scope) => typed
                        .closed_requirement_application(association.closed_requirement)
                        .is_some_and(|application| {
                            application.template == association.template
                                && application.trait_application_identity
                                    == association.receiving_application
                                && typed.requirement_application_matches_scope(
                                    association.closed_requirement,
                                    scope,
                                )
                        }),
                    None => association.receiving_application == Default::default(),
                };
                if !receiving_matches {
                    return Some(Err(
                        "external satisfier changed its checked receiving application".into(),
                    ));
                }
                edge_applications.push(
                    signatures
                        .abi_for_boundary(association.closed_requirement)
                        .cloned()
                        .unwrap_or_else(|| {
                            Err("external satisfier lost its checked closed declaration ABI".into())
                        }),
                );
            }
            match scope {
                Some(scope) => {
                    let Some(outer) = scope
                        .portable_application_identity(typed)
                        .and_then(|identity| typed.type_identity_text(identity))
                    else {
                        return Some(Err(
                            "external satisfier lost its portable declaring application".into(),
                        ));
                    };
                    edge_applications.extend(
                        signatures
                            .abi_signatures()
                            .iter()
                            .filter(|declaration| {
                                declaration
                                    .source_templates
                                    .contains(&edge.requirement_symbol)
                                    && declaration.declaring_application == outer
                                    && declaration.operands.is_empty()
                                    && declaration
                                        .formals
                                        .iter()
                                        .all(|formal| formal.callback.is_some())
                            })
                            .map(|declaration| Ok(declaration.id)),
                    );
                }
                None => {
                    if let Some(application) = signatures.abi_for_boundary(edge.requirement_symbol)
                    {
                        edge_applications.push(application.clone());
                    }
                }
            }
            if edge_applications.is_empty() {
                return Some(Err(
                    "external satisfaction edge has no exact checked declaration ABI application"
                        .into(),
                ));
            }
            for application in edge_applications {
                if let Err(reason) = &application {
                    return Some(Err(reason.clone()));
                }
                if !applications.contains(&application) {
                    applications.push(application);
                }
            }
        }
        Some(match applications.as_slice() {
            [application] => application.clone(),
            [] => Err("external satisfier has no exact checked declaration ABI application".into()),
            _ => {
                Err("external satisfier has competing checked declaration ABI applications".into())
            }
        })
    }
}
mod custody;
pub(crate) mod observation {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(usize)]
    pub enum PsiCompilationStage {
        MachineSelection,
        Lowering,
        Optimization,
        EntryReceipt,
        Publication,
        Resolution,
        Typing,
        Lexing,
        Parsing,
        SourceParsing,
    }

    const OBSERVATION_LABELS: [(&str, &str, &str); 10] = [
        (
            "terminal-production/machine-selection",
            "CheckedTrees",
            "TerminalMachine",
        ),
        ("terminal-production/lowering", "CheckedTrees", "LoweredPsi"),
        (
            "terminal-production/optimization",
            "LoweredPsi",
            "OptimizedLoweredPsi",
        ),
        (
            "terminal-production/entry-receipt",
            "OptimizedLoweredPsi",
            "ProgramEntryReceipt",
        ),
        (
            "terminal-production/publication",
            "OptimizedLoweredPsi",
            "CanonicalTerminalArtifact",
        ),
        ("Psi 02", "SyntaxTrees", "SymbolResolvedTrees"),
        ("Psi 03", "SymbolResolvedTrees", "TypedTrees"),
        ("Psi 00", "SourceFiles", "TokenStreams"),
        ("Psi 01", "TokenStreams", "SyntaxTrees"),
        ("Source ingress", "SourceFiles", "SyntaxTrees"),
    ];

    impl PsiCompilationStage {
        pub const fn observation_labels(self) -> (&'static str, &'static str, &'static str) {
            OBSERVATION_LABELS[self as usize]
        }
    }

    pub trait PsiCompilationObserver {
        fn record_result<Value, Error>(
            &mut self,
            stage: PsiCompilationStage,
            operation: impl FnOnce() -> Result<Value, Error>,
        ) -> Result<Value, Error>;
    }

    pub struct UnmeasuredProduction;

    impl PsiCompilationObserver for UnmeasuredProduction {
        fn record_result<Value, Error>(
            &mut self,
            _stage: PsiCompilationStage,
            operation: impl FnOnce() -> Result<Value, Error>,
        ) -> Result<Value, Error> {
            operation()
        }
    }
}
pub use checked_trees::PreparedProviderInventory;
pub(crate) mod terminal_production {
    use checked_trees::CheckedTrees;
    use checked_trees_to_lowered_psi::{LoweringError, TerminalMachineSelection, lower_machine};
    use diagnostics::Diagnostic;
    use lowered_psi_to_lowered_psi::{PsiOptimizationStageError, run_psi_optimization};

    use crate::custody::{ProducedTerminalProgram, TerminalProductionCustody};
    use crate::observation::{PsiCompilationObserver, PsiCompilationStage};

    pub use lowered_psi_to_terminal_psi::ProgramEntryTerminalReceiptError;

    use crate::build_bindings::capture_native_formations;

    pub struct TerminalProductionRequest<'a> {
        pub publication: terminal_codec::TerminalPublication,
        pub checked: &'a CheckedTrees,
        pub exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
        pub external_declarations: Option<&'a crate::PreparedExternalProviderDeclarations>,
        pub provider_inventory: Option<&'a checked_trees::PreparedProviderInventory>,
        pub machine: TerminalMachineSelection<'a>,
        pub roots: crate::TerminalProgramRoots<'a>,
        pub optimization_selections: optimization::PsiOptimizationSelections,
    }

    impl<'a> TerminalProductionRequest<'a> {
        pub fn new(
            checked: &'a CheckedTrees,
            machine: TerminalMachineSelection<'a>,
            exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
        ) -> Self {
            Self {
                publication: terminal_codec::TerminalPublication::Portable,
                checked,
                exact_toolchain_sources,
                external_declarations: None,
                provider_inventory: None,
                machine,
                roots: crate::TerminalProgramRoots::default(),
                optimization_selections: optimization::PsiOptimizationSelections::default(),
            }
        }

        pub fn produce(
            self,
            custody: TerminalProductionCustody<'_>,
            observer: &mut impl PsiCompilationObserver,
        ) -> Result<ProducedTerminalProgram, TerminalArtifactProductionError> {
            let TerminalProductionCustody { applications } = custody;
            let checked = self.checked;
            let declaration_requests = crate::build_bindings::capture_declarations(checked);
            let native_formations =
                capture_native_formations(checked, self.machine, &declaration_requests)
                    .map_err(TerminalArtifactProductionError::BuildContext)?;
            crate::__mem_probe2("tp-before-lowering");
            let mut lowered = observer
                .record_result(PsiCompilationStage::Lowering, || {
                    lower_machine(
                        self.checked,
                        self.machine,
                        self.exact_toolchain_sources,
                        self.roots,
                        self.publication,
                        self.external_declarations,
                    )
                })
                .map_err(TerminalArtifactProductionError::Lowering)?;
            if let Some(inventory) = self.provider_inventory {
                crate::calling_signatures::capture(
                    checked,
                    inventory,
                    &mut lowered.semantic_module,
                    &lowered.source_boundary_declarations,
                    &lowered.source_machine_declarations,
                    &lowered.source_proof_declarations,
                )
                .map_err(TerminalArtifactProductionError::BuildContext)?;
            }
            lowered.semantic_module.unoptimized_behavior = lowered
                .semantic_module
                .machines
                .iter()
                .map(terminal_psi::UnoptimizedMachineBehavior::capture)
                .collect();
            crate::__mem_probe2("tp-after-lowering");
            let provenance =
                checked_trees_to_lowered_psi::lowered_module_provenance(checked, &lowered);
            let source_records = std::mem::take(&mut lowered.source_records);
            let optimized = observer
                .record_result(PsiCompilationStage::Optimization, || {
                    run_psi_optimization(lowered, self.optimization_selections)
                })
                .map_err(TerminalArtifactProductionError::Optimization)?;
            crate::__mem_probe2("tp-after-optimization");
            let binding_calls = crate::build_bindings::capture(checked, optimized.lowered())
                .map_err(TerminalArtifactProductionError::BuildContext)?;
            let request_calls = crate::build_requests::capture(checked, optimized.lowered())
                .map_err(TerminalArtifactProductionError::BuildContext)?;
            let filesystem_calls = crate::build_filesystem::capture(checked, optimized.lowered())
                .map_err(TerminalArtifactProductionError::BuildContext)?;
            let query_calls = crate::build_queries::capture(checked, optimized.lowered())
                .map_err(TerminalArtifactProductionError::BuildContext)?;
            let program = observer
                .record_result(PsiCompilationStage::Publication, || {
                    lowered_psi_to_terminal_psi::finalize_terminal_program(
                        optimized,
                        self.publication,
                    )
                })
                .map_err(TerminalArtifactProductionError::Artifact)?;
            crate::__mem_probe2("tp-after-publication");
            let applications = applications
                .iter()
                .map(|application| {
                    application.attach(program.view(), program.portable(), &provenance, observer)
                })
                .collect();
            let build_bindings = crate::TerminalBuildBindings::new(
                program.view(),
                binding_calls,
                declaration_requests,
                query_calls,
                native_formations,
            );
            let build_requests = crate::TerminalBuildRequests::new(program.view(), request_calls);
            let build_filesystem =
                crate::TerminalBuildFilesystem::new(program.view(), filesystem_calls);
            Ok(ProducedTerminalProgram {
                program,
                build_bindings,
                build_requests,
                build_filesystem,
                applications,
                provenance,
                source_records,
            })
        }
    }

    #[derive(Debug)]
    pub enum TerminalArtifactProductionError {
        BuildContext(Diagnostic),
        Lowering(LoweringError),
        Optimization(PsiOptimizationStageError),
        Artifact(terminal_codec::CanonicalTerminalArtifactError),
        EntryReceipt(ProgramEntryTerminalReceiptError),
    }

    impl TerminalArtifactProductionError {
        pub fn diagnostics(&self) -> Vec<Diagnostic> {
            vec![self.diagnostic()]
        }

        pub fn diagnostic(&self) -> Diagnostic {
            match self {
                Self::BuildContext(error) => Some(error.clone()),
                Self::Lowering(error) => error.plan_gap().cloned(),
                _ => None,
            }
            .unwrap_or_else(|| {
                Diagnostic::error(format!("terminal-artifact production failed: {self}"))
            })
        }
    }

    diagnostics::debug_display!(TerminalArtifactProductionError);

    impl std::error::Error for TerminalArtifactProductionError {}
}

pub use build_bindings::{
    TerminalBuildBindingCall, TerminalBuildBindingOperation, TerminalBuildBindings,
    TerminalBuildNativeLocator, TerminalBuildProviderRequirement, TerminalBuildSelectionRequest,
    TerminalBuildValueCase, TerminalBuildValueField, TerminalBuildValueShape,
};
pub use build_filesystem::{
    TerminalBuildFilesystem, TerminalBuildFilesystemCall, TerminalBuildFilesystemOperation,
};
pub use build_provider::{BuildProviderProductionRequest, ProducedBuildProvider};
pub use build_queries::{TerminalBuildQueryCall, TerminalBuildQueryOperation};
pub use build_requests::{
    TerminalBuildRequestCall, TerminalBuildRequestOperation, TerminalBuildRequests,
};
pub use checked_trees_to_lowered_psi::lower_task_activation_demands;
pub use checked_trees_to_lowered_psi::{
    ExternalProviderDeclarations, PreparedExternalProviderDeclarations,
    project_external_provider_declarations,
};
pub use checked_trees_to_lowered_psi::{TerminalMachineSelection, TerminalProgramRoots};
pub use composition_roots::{
    TerminalAdmissionRoot, TerminalApplicationCustody, TerminalCompositionRoots,
};
pub use custody::{
    PortableTerminalProgram, ProducedTerminalApplication, ProducedTerminalProgram,
    TerminalProductionCustody,
};
pub use observation::{PsiCompilationObserver, PsiCompilationStage, UnmeasuredProduction};
pub use terminal_production::{
    ProgramEntryTerminalReceiptError, TerminalArtifactProductionError, TerminalProductionRequest,
};
pub use terminal_psi::PreparedTerminalRoots;

pub(crate) mod composition_roots {
    use crate::observation::{PsiCompilationObserver, PsiCompilationStage};
    use crate::{ProducedTerminalApplication, TerminalArtifactProductionError};
    use diagnostics::Diagnostic;
    use lowered_psi_to_terminal_psi::prepare_program_entry_receipt;
    use terminal_psi::{PreparedNominalRoot, TerminalMachineProvenance};

    #[derive(Debug, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalAdmissionRoot {
        pub machine: semantic_vocabulary::MachineId,
        pub provenance: TerminalMachineProvenance,
    }

    #[derive(Clone, Copy, Default)]
    pub struct TerminalCompositionRoots<'a> {
        pub callbacks: Option<&'a crate::PreparedTerminalRoots>,
        pub physical_adapter: Option<&'a PreparedNominalRoot>,
    }

    struct PreparedComposition<'a> {
        callbacks: Option<&'a crate::PreparedTerminalRoots>,
        adapter: Option<&'a PreparedNominalRoot>,
    }

    pub struct TerminalApplicationCustody<'a> {
        entry: Option<Result<PreparedNominalRoot, Vec<Diagnostic>>>,
        entry_identity: Option<[u8; 32]>,
        composition: Option<PreparedComposition<'a>>,
    }

    impl<'a> TerminalApplicationCustody<'a> {
        pub fn prepare(
            entry: Result<PreparedNominalRoot, Vec<Diagnostic>>,
            entry_identity: Option<[u8; 32]>,
            composition: Option<TerminalCompositionRoots<'a>>,
        ) -> Self {
            if entry_identity.is_none() && composition.is_none() {
                return Self {
                    entry: None,
                    entry_identity,
                    composition: None,
                };
            }
            let composition = composition.map(|composition| PreparedComposition {
                callbacks: composition.callbacks,
                adapter: composition.physical_adapter,
            });
            Self {
                entry: Some(entry),
                entry_identity,
                composition,
            }
        }

        pub(crate) fn attach(
            &self,
            view: terminal_codec::TerminalExecutionView<'_>,
            artifact: Result<
                &terminal_codec::CanonicalTerminalArtifact,
                terminal_codec::CanonicalTerminalArtifactError,
            >,
            provenance: &TerminalMachineProvenance,
            observer: &mut impl PsiCompilationObserver,
        ) -> Result<ProducedTerminalApplication, Vec<Diagnostic>> {
            let Some(entry) = &self.entry else {
                return Ok(ProducedTerminalApplication {
                    receipt: Ok(None),
                    roots: Vec::new(),
                });
            };
            let entry = entry.as_ref().map_err(Clone::clone)?;
            let module = view.module();
            let mut roots = Vec::new();
            if let Some(composition) = &self.composition {
                roots.push(project_root(module, provenance, entry));
                if let Some(callbacks) = composition.callbacks {
                    roots.extend(callbacks.retained_roots(module).map(|root| {
                        let root = root.map_err(|reason| {
                            vec![Diagnostic::error(format!(
                                "callback composition root: {reason}"
                            ))]
                        })?;
                        Ok(TerminalAdmissionRoot {
                            machine: root.machine,
                            provenance: provenance.for_root(
                                module,
                                root.machine,
                                root.name.map(|name| (name, root.span)),
                            ),
                        })
                    }));
                }
                if let Some(adapter) = &composition.adapter {
                    roots.push(project_root(module, provenance, adapter));
                }
            }
            let receipt = match self.entry_identity {
                Some(identity) => observer.record_result(PsiCompilationStage::EntryReceipt, || {
                    let artifact =
                        artifact.map_err(|error| vec![Diagnostic::error(error.to_string())])?;
                    prepare_program_entry_receipt(entry, identity, artifact)
                        .map(Some)
                        .map_err(|error| {
                            TerminalArtifactProductionError::EntryReceipt(error).diagnostics()
                        })
                }),
                None => Ok(None),
            };
            Ok(ProducedTerminalApplication { receipt, roots })
        }
    }

    fn project_root(
        module: &terminal_psi::TerminalModule,
        provenance: &TerminalMachineProvenance,
        declaration: &PreparedNominalRoot,
    ) -> Result<TerminalAdmissionRoot, Vec<Diagnostic>> {
        let entry = declaration.resolve(module).map_err(|reason| {
            TerminalArtifactProductionError::EntryReceipt(reason.into()).diagnostics()
        })?;
        Ok(TerminalAdmissionRoot {
            machine: entry.id,
            provenance: provenance.for_root(
                module,
                entry.id,
                declaration
                    .diagnostic_name()
                    .map(|(name, span)| (name, span.or_else(|| provenance.source_span(entry.id)))),
            ),
        })
    }
}

#[doc(hidden)]
pub fn __mem_probe2(label: &str) {
    if std::env::var_os("OMEGA_MEM_PROBE").is_none() {
        return;
    }
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        status
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|kb| kb.parse::<u64>().ok())
            .unwrap_or(0)
            / 1024
    };
    eprintln!("MEMPROBE {label}: rss={}MB hwm={}MB", field("VmRSS:"), field("VmHWM:"));
}
