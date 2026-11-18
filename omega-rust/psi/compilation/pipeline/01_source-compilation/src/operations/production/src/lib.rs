/*
Terminal production retains the new checker's actual accepted closure, its frozen
typed owner and the exact artifact/evidence pair. AcceptedTerminalSource is the
production entrance; invocation identity is fixed before lowering and judgment.

Retained Build companions and application receipts
describe the data a producer must provide; none manufactures checking authority.
The obsolete checked-source production request interfaces have been removed.
*/

#![forbid(unsafe_code)]

mod accepted;
pub use crate::lowering::Compilation as TerminalSourceCompilation;
pub use crate::lowering::SuppliedRequirement as TerminalSuppliedRequirement;
pub use crate::lowering::Supply as TerminalSourceSupply;
pub use crate::lowering::provider_selections::{
    ProviderSelectionInput as TerminalProviderSelectionInput,
    ProviderSelections as TerminalProviderSelections,
};
pub use crate::lowering::{
    CallingPolicySelections as TerminalCallingPolicySelections,
    ProgramEntryContractSelection as TerminalProgramEntryContractSelection,
};
pub use accepted::{AcceptedTerminalSource, ConstantRead, DeclaredBoundaryRequirement};
pub use terminal_semantics::callable_values::CallableBoundaryImplementation;

mod build_bindings;
mod build_companions;
pub(crate) use build_bindings::{build_filesystem, build_provider, build_queries, build_requests};
mod custody;
mod source_selection_publication;
mod source_selections;
pub use source_selections::TerminalSourceSelections;
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
    pub enum PsiCompilationStage {
        EntryReceipt,
        Publication,
        Resolution,
        Typing,
        SourceParsing,
        SourcePreparation,
    }

    impl PsiCompilationStage {
        /// Names the value the recorded call reads and the value it returns.
        ///
        /// Publication records `AcceptedClosure::publish` and
        /// `publish_invocation`: accepted closure in, `TerminalProgramProduct`
        /// out (portable canonical artifact or invocation code), with the
        /// evidence seals beside that product. Entry receipt records
        /// `prepare_program_entry_receipt`: that canonical artifact in,
        /// `CheckedProgramEntryTerminalReceipt` out. Elaboration and discharge
        /// remain the judgment in `AcceptedTerminalSource::judge`.
        pub const fn observation_labels(self) -> (&'static str, &'static str, &'static str) {
            match self {
                Self::EntryReceipt => (
                    "terminal-production/entry-receipt",
                    "CanonicalTerminalArtifact",
                    "CheckedProgramEntryTerminalReceipt",
                ),
                Self::Publication => (
                    "terminal-production/publication",
                    "AcceptedClosure",
                    "TerminalProgramProduct",
                ),
                Self::Resolution => ("Psi 02", "SyntaxTrees", "SymbolResolvedTrees"),
                Self::Typing => ("Psi 03", "SymbolResolvedTrees", "TypedTrees"),
                Self::SourceParsing => ("Source ingress", "SourceFiles", "SyntaxTrees"),
                Self::SourcePreparation => {
                    ("typed-source-preparation", "TypedTrees", "FrozenTypedTrees")
                }
            }
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

    #[cfg(test)]
    mod tests {
        use super::PsiCompilationStage;

        #[test]
        fn live_rows_name_the_representations_their_producers_return() {
            assert_eq!(
                PsiCompilationStage::EntryReceipt.observation_labels(),
                (
                    "terminal-production/entry-receipt",
                    "CanonicalTerminalArtifact",
                    "CheckedProgramEntryTerminalReceipt",
                )
            );
            assert_eq!(
                PsiCompilationStage::Publication.observation_labels(),
                (
                    "terminal-production/publication",
                    "AcceptedClosure",
                    "TerminalProgramProduct",
                )
            );
            assert_eq!(
                PsiCompilationStage::Resolution.observation_labels(),
                ("Psi 02", "SyntaxTrees", "SymbolResolvedTrees")
            );
            assert_eq!(
                PsiCompilationStage::Typing.observation_labels(),
                ("Psi 03", "SymbolResolvedTrees", "TypedTrees")
            );
            assert_eq!(
                PsiCompilationStage::SourceParsing.observation_labels(),
                ("Source ingress", "SourceFiles", "SyntaxTrees")
            );
            for stage in [
                PsiCompilationStage::EntryReceipt,
                PsiCompilationStage::Publication,
                PsiCompilationStage::Resolution,
                PsiCompilationStage::Typing,
                PsiCompilationStage::SourceParsing,
            ] {
                let (name, input, output) = stage.observation_labels();
                for label in [name, input, output] {
                    assert!(
                        !matches!(label, "CheckedTrees" | "LoweredPsi" | "OptimizedLoweredPsi"),
                        "{stage:?} still names a deleted representation: {label}"
                    );
                }
            }
        }
    }
}
pub(crate) mod terminal_production {
    use diagnostics::Diagnostic;

    pub use terminal_codec::entry_receipt::ProgramEntryTerminalReceiptError;

    #[derive(Debug)]
    pub enum TerminalArtifactProductionError {
        BuildContext(Diagnostic),
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

pub use self::terminal_production::{
    ProgramEntryTerminalReceiptError, TerminalArtifactProductionError,
};
pub use build_bindings::{
    TerminalBuildBindingCall, TerminalBuildBindingOperation, TerminalBuildBindings,
    TerminalBuildNativeLocator, TerminalBuildProviderRequirement, TerminalBuildSelectionRequest,
    TerminalBuildValueCase, TerminalBuildValueField, TerminalBuildValueShape,
};
pub use build_companions::BuildProtocolUse;
pub use build_filesystem::{
    TerminalBuildFilesystem, TerminalBuildFilesystemCall, TerminalBuildFilesystemOperation,
};
pub use build_provider::ProducedBuildProvider;
pub use build_queries::{TerminalBuildQueryCall, TerminalBuildQueryOperation};
pub use build_requests::{
    BuildCallSite, TerminalBuildRequestCall, TerminalBuildRequestOperation, TerminalBuildRequests,
};
pub use custody::{PortableTerminalProgram, ProducedTerminalProgram, TerminalProductionCustody};
pub use observation::{PsiCompilationObserver, PsiCompilationStage, UnmeasuredProduction};
pub use production_selection::{TerminalMachineSelection, TerminalProgramRoots};
pub use terminal_installation::application::{
    ProducedTerminalApplication, TerminalAdmissionRoot, TerminalApplicationCustody,
    TerminalCompositionRoots,
};
pub use terminal_psi::PreparedTerminalRoots;

// Source publication records the same entry-receipt span while the independent
// application checker remains callable with current artifact data alone.
pub(crate) struct ApplicationReceiptObserver<'a, O>(pub &'a mut O);
impl<O: PsiCompilationObserver> terminal_installation::application::EntryReceiptObserver
    for ApplicationReceiptObserver<'_, O>
{
    fn record_result<Value, Error>(
        &mut self,
        operation: impl FnOnce() -> Result<Value, Error>,
    ) -> Result<Value, Error> {
        self.0
            .record_result(PsiCompilationStage::EntryReceipt, operation)
    }
}

#[cfg(test)]
#[path = "../../../../../../../../../tests/support/frontend.rs"]
mod frontend_fixture;
