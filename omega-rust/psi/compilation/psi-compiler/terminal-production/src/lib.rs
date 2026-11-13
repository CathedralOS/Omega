/*
Terminal production retains the new checker's actual accepted closure, its frozen
typed owner and the exact artifact/evidence pair. AcceptedTerminalSource is the
production entrance; invocation identity is fixed before lowering and judgment.

Retained Build companions, external declaration records and application receipts
describe the data a producer must provide; none manufactures checking authority.
The obsolete checked-source production request interfaces have been removed.
*/

#![forbid(unsafe_code)]

mod accepted;
mod invocation_companions;
pub use accepted::AcceptedTerminalSource;
pub use structure_elaboration::Compilation as TerminalSourceCompilation;

mod build_bindings;
pub(crate) use build_bindings::{build_filesystem, build_provider, build_queries, build_requests};
mod custody;
mod external_providers;
pub(crate) mod production_selection {
    #[derive(Debug, Clone, Copy)]
    pub enum TerminalMachineSelection<'a> {
        Declarations,
        Name(&'a str),
        Symbol(symbols::SymbolHandle),
    }

    #[derive(Clone, Copy, Default)]
    pub struct TerminalProgramRoots<'a> {
        pub machines: &'a [symbols::SymbolHandle],
        pub boundaries: &'a [symbols::SymbolHandle],
    }
}
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
pub(crate) mod terminal_production {
    use diagnostics::Diagnostic;

    pub use terminal_codec::entry_receipt::ProgramEntryTerminalReceiptError;

    #[derive(Debug)]
    pub enum TerminalArtifactProductionError {
        BuildContext(Diagnostic),
        NoProductRoute,
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
                Self::NoProductRoute => Some(Diagnostic::error(
                    "no product route yet: the source has no accepted Terminal program producer",
                )),
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
pub use build_provider::ProducedBuildProvider;
pub use build_queries::{TerminalBuildQueryCall, TerminalBuildQueryOperation};
pub use build_requests::{
    TerminalBuildRequestCall, TerminalBuildRequestOperation, TerminalBuildRequests,
};
pub use composition_roots::{
    TerminalAdmissionRoot, TerminalApplicationCustody, TerminalCompositionRoots,
};
pub use custody::{
    PortableTerminalProgram, ProducedTerminalApplication, ProducedTerminalProgram,
    TerminalProductionCustody,
};
pub use external_providers::{
    ExternalProviderDeclarations, NoProductRoute, PreparedExternalProviderDeclarations,
    project_external_provider_declarations,
};
pub use observation::{PsiCompilationObserver, PsiCompilationStage, UnmeasuredProduction};
pub use production_selection::{TerminalMachineSelection, TerminalProgramRoots};
pub use terminal_production::{ProgramEntryTerminalReceiptError, TerminalArtifactProductionError};
pub use terminal_psi::PreparedTerminalRoots;

pub(crate) mod composition_roots {
    use crate::observation::{PsiCompilationObserver, PsiCompilationStage};
    use crate::{ProducedTerminalApplication, TerminalArtifactProductionError};
    use diagnostics::Diagnostic;
    use terminal_codec::entry_receipt::prepare_program_entry_receipt;
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
