/*
    Source-issued Build companions retained beside a Terminal Psi program:
    Build bindings, native locators and formations, Build requests, product
    queries, filesystem protocol calls, and standalone Build providers.

    Each companion is bound to the exact custody identity of its Terminal
    program. The captures that filled them read checked trees beside the
    removed checked-tree lowering; these are the retained record shapes.
*/

mod native_locator {
    use semantic_vocabulary::{ScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId};
    use std::sync::Arc;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalBuildNativeLocator {
        pub structural_type: StructuralTypeId,
        pub widths: [u64; 3],
        pub shape: Arc<TerminalBuildValueShape>,
    }
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum TerminalBuildValueShape {
        Scalar(ScalarType),
        Array { length: u64, element: Arc<Self> },
        Sum { cases: Vec<TerminalBuildValueCase> },
    }
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalBuildValueCase {
        pub id: StructuralCaseId,
        pub name: String,
        pub fields: Vec<TerminalBuildValueField>,
    }
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalBuildValueField {
        pub id: StructuralFieldId,
        pub name: String,
        pub shape: Arc<TerminalBuildValueShape>,
    }
}
pub use native_locator::{
    TerminalBuildNativeLocator, TerminalBuildValueCase, TerminalBuildValueField,
    TerminalBuildValueShape,
};

use crate::TerminalBuildQueryCall;
use language_semantics::declaration_selection::product_operands::ProductStaticEnvironment;
use language_semantics::declaration_selection::{
    BuildSelectionRequest, NativeBindingFormation, ProductDeclarationRequest,
};
use semantic_vocabulary::{BoundaryMachineId, OperationId};
use std::sync::Arc;
use terminal_codec::TerminalExecutionView;

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct TerminalBuildProviderRequirement {
    pub declaration: terminal_psi::NominalDeclarationIdentity,
    pub application: terminal_psi::TerminalIdentityIndex,
    pub boundaries: Vec<BoundaryMachineId>,
}

pub type TerminalBuildSelectionRequest = BuildSelectionRequest<TerminalBuildProviderRequirement>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum TerminalBuildBindingOperation {
    AwaitProvider,
    Prepare,
    PrepareTests, // Test-group registration, prepared on the Build's `tests` facet.
    Bind,
    BindNative,
    Select,
    SelectProvider,
    Exclude,
}

#[derive(Debug, PartialEq, Eq, compiler_storage::Record)]
pub struct TerminalBuildBindingCall {
    pub operation: OperationId,
    pub boundary: BoundaryMachineId,
    pub action: TerminalBuildBindingOperation,
    pub request: TerminalBuildSelectionRequest,
    pub selecting_machine: Option<source::SourceSpan>,
    pub static_environment: Option<Arc<ProductStaticEnvironment>>,
    pub native_locator: Option<TerminalBuildNativeLocator>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildBindings {
    identity: terminal_codec::TerminalExecutionIdentity,
    calls: Arc<[TerminalBuildBindingCall]>,
    queries: Arc<[TerminalBuildQueryCall]>,
    declarations: Arc<[Arc<ProductDeclarationRequest>]>,
    native_formations: Arc<[NativeBindingFormation]>,
}

impl TerminalBuildBindings {
    pub fn queries_for(
        &self,
        artifact: TerminalExecutionView<'_>,
    ) -> Option<&Arc<[TerminalBuildQueryCall]>> {
        (self.identity == artifact.custody_identity()).then_some(&self.queries)
    }
    pub fn calls_for(
        &self,
        artifact: TerminalExecutionView<'_>,
    ) -> Option<&Arc<[TerminalBuildBindingCall]>> {
        (self.identity == artifact.custody_identity()).then_some(&self.calls)
    }

    pub fn declarations_for(
        &self,
        artifact: TerminalExecutionView<'_>,
    ) -> Option<&Arc<[Arc<ProductDeclarationRequest>]>> {
        (self.identity == artifact.custody_identity()).then_some(&self.declarations)
    }

    pub fn native_formations_for(
        &self,
        artifact: TerminalExecutionView<'_>,
    ) -> Option<&Arc<[NativeBindingFormation]>> {
        (self.identity == artifact.custody_identity()).then_some(&self.native_formations)
    }

    pub(crate) fn new(
        artifact: TerminalExecutionView<'_>,
        calls: impl Into<Arc<[TerminalBuildBindingCall]>>,
        declarations: impl Into<Arc<[Arc<ProductDeclarationRequest>]>>,
        queries: impl Into<Arc<[TerminalBuildQueryCall]>>,
        native_formations: impl Into<Arc<[NativeBindingFormation]>>,
    ) -> Self {
        Self {
            identity: artifact.custody_identity(),
            calls: calls.into(),
            queries: queries.into(),
            declarations: declarations.into(),
            native_formations: native_formations.into(),
        }
    }
}

pub(crate) mod build_filesystem {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum TerminalBuildFilesystemOperation {
        ResolveSource,
        ResolveOutput,
        NamedInput,
        Open,
        Read,
        CloseSource,
        Create,
        Write,
        CloseOutput,
        IncludeSource,
        Require,
        CanComplete,
        Complete,
        Fail,
        RequiredPath,
    }

    #[derive(Debug, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalBuildFilesystemCall {
        pub operation: semantic_vocabulary::OperationId,
        pub boundary: semantic_vocabulary::BoundaryMachineId,
        pub action: TerminalBuildFilesystemOperation,
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct TerminalBuildFilesystem {
        identity: terminal_codec::TerminalExecutionIdentity,
        calls: std::sync::Arc<[TerminalBuildFilesystemCall]>,
    }

    impl TerminalBuildFilesystem {
        pub fn calls_for(
            &self,
            artifact: terminal_codec::TerminalExecutionView<'_>,
        ) -> Option<&std::sync::Arc<[TerminalBuildFilesystemCall]>> {
            (self.identity == artifact.custody_identity()).then_some(&self.calls)
        }

        pub(crate) fn new(
            artifact: terminal_codec::TerminalExecutionView<'_>,
            calls: impl Into<std::sync::Arc<[TerminalBuildFilesystemCall]>>,
        ) -> Self {
            Self {
                identity: artifact.custody_identity(),
                calls: calls.into(),
            }
        }
    }
}

pub(crate) mod build_provider {
    use checked_trees::CheckedTrees;
    use diagnostics::Diagnostic;

    pub struct ProducedBuildProvider {
        code: Option<std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>>,
        path: String,
        package: semantic_vocabulary::PackageKeyIdentity,
        declaration: String,
        requirement: String,
        application: String,
    }

    impl ProducedBuildProvider {
        pub fn artifact(
            &self,
        ) -> Option<&std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>> {
            self.code.as_ref()
        }
        pub fn path(&self) -> &str {
            &self.path
        }
        pub fn package(&self) -> semantic_vocabulary::PackageKeyIdentity {
            self.package
        }
        pub fn declaration(&self) -> &str {
            &self.declaration
        }
        pub fn requirement(&self) -> &str {
            &self.requirement
        }
        pub fn application(&self) -> &str {
            &self.application
        }
    }

    pub struct BuildProviderProductionRequest<'checked> {
        pub checked: &'checked CheckedTrees,
        pub exact_toolchain_sources: &'checked [(source::SourceId, [u8; 32])],
        pub package: semantic_vocabulary::PackageKeyIdentity,
        pub declaration_path: &'checked str,
    }

    impl BuildProviderProductionRequest<'_> {
        pub fn produce(self) -> Result<ProducedBuildProvider, Vec<Diagnostic>> {
            // The provider's Terminal program was lowered from checked trees by the
            // removed checked-tree lowering stage. Nothing produces it from the
            // lowered route's published module yet, so the Build backing is refused.
            Err(vec![Diagnostic::error(format!(
                "no product route yet: Build backing `{}` has no Terminal program producer",
                self.declaration_path
            ))])
        }
    }
}

pub(crate) mod build_queries {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum TerminalBuildQueryOperation {
        Entry,
        Schema,
        Provider,
        SchemaPath,
        ProviderPath,
    }

    #[derive(Debug, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalBuildQueryCall {
        pub operation: semantic_vocabulary::OperationId,
        pub boundary: semantic_vocabulary::BoundaryMachineId,
        pub action: TerminalBuildQueryOperation,
        pub occurrence: source::SourceSpan,
    }
}

pub(crate) mod build_requests {

    pub use language_semantics::build_requests::{
        TerminalBuildRequestCall, TerminalBuildRequestOperation,
    };

    #[derive(Debug, PartialEq, Eq)]
    pub struct TerminalBuildRequests {
        identity: terminal_codec::TerminalExecutionIdentity,
        calls: std::sync::Arc<[TerminalBuildRequestCall]>,
    }

    impl TerminalBuildRequests {
        pub fn calls_for(
            &self,
            artifact: terminal_codec::TerminalExecutionView<'_>,
        ) -> Option<&std::sync::Arc<[TerminalBuildRequestCall]>> {
            (self.identity == artifact.custody_identity()).then_some(&self.calls)
        }

        pub(crate) fn new(
            artifact: terminal_codec::TerminalExecutionView<'_>,
            calls: impl Into<std::sync::Arc<[TerminalBuildRequestCall]>>,
        ) -> Self {
            Self {
                identity: artifact.custody_identity(),
                calls: calls.into(),
            }
        }
    }
}
