//! Source-owned product selections, independent of an execution engine.
//!
//! Descriptions name declarations in the admitted product context. Execution
//! engines expose only opaque issued values and retain these payloads privately.
//! A completed binding carries its lexical request directly, never an AST
//! statement handle to reinterpret later. Authored paths remain locators until
//! the product join resolves them under that source's authority.
//! These are in-process source selections, not serialized Terminal identities.

use super::BuildOperation;

/// Compiler-issued context for one ordinary Build selection call. Entry slots
/// and declaration choices share activation custody, while retaining their
/// distinct Product admission obligations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildSelectionRequest<Requirement> {
    ProviderReadiness(ProviderReadinessRequest<Requirement>),
    Entry(RootBindingRequest),
    Declaration(std::sync::Arc<ProductDeclarationRequest>),
    Exclusion(EnumExclusionRequest),
}

impl<Requirement> Default for BuildSelectionRequest<Requirement> {
    fn default() -> Self {
        Self::Entry(RootBindingRequest::default())
    }
}

impl<Requirement> BuildSelectionRequest<Requirement> {
    /// Exclusions and authority-bearing declarations retain their selecting
    /// machine for final admission. Provider and representation operands carry
    /// their own lexical scopes and require no additional machine subject.
    pub fn requires_lexical_subject(&self) -> bool {
        match self {
            Self::Exclusion(_) => true,
            Self::Declaration(request) => matches!(
                request.operation,
                super::BuildOperation::ServiceExclusion
                    | super::BuildOperation::BoundaryAcceptance
                    | super::BuildOperation::ServiceAudit
                    | super::BuildOperation::AssumedBindingContract
                    | super::BuildOperation::WireCompatibilityRequest
            ),
            Self::Entry(_) | Self::ProviderReadiness(_) => false,
        }
    }

    pub fn source_span(&self) -> source::SourceSpan {
        match self {
            Self::ProviderReadiness(request) => request.source_span,
            Self::Entry(request) => request.source_span,
            Self::Declaration(request) => request.source_span,
            Self::Exclusion(request) => request.source_span,
        }
    }

    pub fn entry(&self) -> Option<&RootBindingRequest> {
        match self {
            Self::Entry(request) => Some(request),
            Self::ProviderReadiness(_) | Self::Declaration(_) | Self::Exclusion(_) => None,
        }
    }

    /// Only these operands may remain unresolved during Build checking.
    pub fn designated_operands(&self) -> &[ProductDeclarationOperand] {
        match self {
            Self::ProviderReadiness(request) => std::slice::from_ref(&request.backing),
            Self::Declaration(request) => &request.operands,
            Self::Entry(_) | Self::Exclusion(_) => &[],
        }
    }

    /// The producing layer replaces its requirement representation without
    /// changing the authored backing or unrelated Build selections.
    pub fn map_requirement<Next, Error>(
        self,
        map: impl FnOnce(Requirement) -> Result<Next, Error>,
    ) -> Result<BuildSelectionRequest<Next>, Error> {
        Ok(match self {
            Self::ProviderReadiness(request) => {
                BuildSelectionRequest::ProviderReadiness(ProviderReadinessRequest {
                    source_span: request.source_span,
                    requirement: map(request.requirement)?,
                    backing: request.backing,
                })
            }
            Self::Entry(request) => BuildSelectionRequest::Entry(request),
            Self::Declaration(request) => BuildSelectionRequest::Declaration(request),
            Self::Exclusion(request) => BuildSelectionRequest::Exclusion(request),
        })
    }
}

/// The requirement is already resolved in this source owner. Only the named
/// dependency backing is deferred; it grants no ordinary import or call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderReadinessRequest<Requirement> {
    pub source_span: source::SourceSpan,
    pub requirement: Requirement,
    pub backing: ProductDeclarationOperand,
}

/// Evaluated enum selections use checked Omega helpers to encode the closed
/// toolchain vocabulary. Codes are protocol values, never guessed layout tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExclusionVocabulary {
    CrashCause,
    PhysicalAuthorityClass,
}

impl ExclusionVocabulary {
    pub const fn type_name(self) -> &'static str {
        match self {
            Self::CrashCause => "CrashCause",
            Self::PhysicalAuthorityClass => "PhysicalAuthorityClass",
        }
    }

    pub const fn helper(self) -> &'static str {
        match self {
            Self::CrashCause => "build_exclusion_crash_code",
            Self::PhysicalAuthorityClass => "build_exclusion_physical_code",
        }
    }

    pub const fn cases(self) -> &'static [&'static str] {
        match self {
            Self::CrashCause => &["Trap", "Abort"],
            Self::PhysicalAuthorityClass => &[
                "FilesystemContentRead",
                "FilesystemContentWrite",
                "FilesystemMetadataQuery",
                "DirectoryEnumeration",
                "FilesystemNamespaceMutation",
                "FilesystemMetadataMutation",
                "ProcessOutput",
                "ProcessTermination",
                "MachineControl",
                "PortIo",
                "InterruptControl",
                "InterruptEntry",
                "RootMemoryAccess",
                "ProcessInput",
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumExclusionRequest {
    pub source_span: source::SourceSpan,
    pub vocabulary: ExclusionVocabulary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedEnumExclusion {
    pub request: EnumExclusionRequest,
    pub code: u64,
    pub selecting_machine: source::SourceSpan,
}

/// Declaration role required by one exact compiler-owned Build operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductOperandRole {
    BoundarySubject,
    Provider,
    Requirement,
    Data,
    Representation,
    WireCompatibilityFact,
}

impl BuildOperation {
    pub const fn product_operand_role(self, position: usize) -> Option<ProductOperandRole> {
        use ProductOperandRole as Role;
        match (self, position) {
            (Self::ProviderSelection, 0)
            | (Self::ServiceExclusion | Self::ServiceAudit | Self::BoundaryAcceptance, 0) => {
                Some(Role::BoundarySubject)
            }
            (Self::ProviderSelection, 1) => Some(Role::Provider),
            (Self::AssumedBindingContract, 0 | 1) => Some(Role::Requirement),
            (Self::RepresentationSelection, 0) | (Self::WireCompatibilityRequest, 0..=3) => {
                Some(Role::Data)
            }
            (Self::RepresentationSelection, 1) => Some(Role::Representation),
            (Self::WireCompatibilityRequest, 4..) => Some(Role::WireCompatibilityFact),
            _ => None,
        }
    }

    pub const fn product_operand_count_valid(self, count: usize) -> bool {
        match self {
            Self::ProviderSelection
            | Self::RepresentationSelection
            | Self::AssumedBindingContract => count == 2,
            Self::ServiceExclusion | Self::ServiceAudit | Self::BoundaryAcceptance => count == 1,
            Self::WireCompatibilityRequest => count >= 5,
            _ => false,
        }
    }
}

/// Source-authorized locators until the Product join admits their exact roles.
/// Neither these strings nor a copied request grants declaration visibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDeclarationRequest {
    pub source_span: source::SourceSpan,
    pub operation: super::BuildOperation,
    pub operands: Box<[ProductDeclarationOperand]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDeclarationOperand {
    pub path: String,
    pub source_span: source::SourceSpan,
}

impl ProductDeclarationRequest {
    pub fn validate_shape(&self) -> Result<(), &'static str> {
        if !self
            .operation
            .product_operand_count_valid(self.operands.len())
        {
            return Err("Build product request has the wrong declaration operand count");
        }
        if self.source_span.span.start >= self.source_span.span.end {
            return Err("Build product request has no authored occurrence");
        }
        for (position, operand) in self.operands.iter().enumerate() {
            if operand.path.is_empty()
                || operand.source_span.span.start >= operand.source_span.span.end
            {
                return Err("Build product operand has no authored declaration locator");
            }
            if self.operation.product_operand_role(position)
                == Some(ProductOperandRole::WireCompatibilityFact)
            {
                let fact = crate::wire::WireCompatibilityFact::from_name(&operand.path)
                    .ok_or("Build wire demand names an unknown compatibility fact")?;
                if self.operands[..position]
                    .iter()
                    .enumerate()
                    .any(|(index, previous)| {
                        self.operation.product_operand_role(index)
                            == Some(ProductOperandRole::WireCompatibilityFact)
                            && crate::wire::WireCompatibilityFact::from_name(&previous.path)
                                == Some(fact)
                    })
                {
                    return Err("Build wire demand repeats a compatibility fact");
                }
            }
        }
        Ok(())
    }
}

/// The ordinary provider composition choice, independent of declaration or
/// target identity. Omission at the source surface means Fused.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompositionMode {
    #[default]
    Fused,
    Independent,
}

/// One reached operation authenticated against its original Build activation.
/// Capturing a potential call does not mint an execution event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutedBuildSelection {
    Entry(ExecutedRootBinding),
    Declaration(ExecutedProductSelection),
    Exclusion(ExecutedEnumExclusion),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedProductSelection {
    pub request: std::sync::Arc<ProductDeclarationRequest>,
    pub selecting_machine: Option<source::SourceSpan>,
    /// Provider choices retain the actual evaluated mode. Representation
    /// choices carry no composition argument.
    pub composition: Option<CompositionMode>,
}

/// Inert designated operands retained under their authored lexical scope.
/// They are not executable arguments or authority to publish a product.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RootBindingRequest {
    pub source_span: source::SourceSpan,
    pub slot: String,
    pub implementation: RootBindingOperand,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum RootBindingOperand {
    AuthoredPath(String),
    #[default]
    Description,
}

/// One product-entry description issued by the compiler-owned
/// `Build.product.entry` query during build evaluation. The evaluator keeps
/// this semantic payload in a private side table; the `ProductEntryRef` value
/// handed to evaluated code carries only an opaque index into it, so source
/// cannot read, convert, or fabricate the selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescribedProductEntry {
    /// Exact product machine the description selected in its query's lexical
    /// package scope. Final admission rejoins this symbol; it never reselects
    /// by spelling.
    pub machine_symbol: symbols::SymbolHandle,
    /// The selected machine's authored name, retained for diagnostics only.
    pub machine_name: String,
    /// The root slot spelling the description was selected for. A later
    /// `Build::bind` must spell the same slot path.
    pub slot: String,
}

/// One product-type-schema description issued by the compiler-owned
/// `Build.product.schema` query during build evaluation. Same marker
/// discipline as `DescribedProductEntry`: the evaluator keeps this payload
/// in a private side table and the `ProductTypeSchema` value handed to
/// evaluated code carries only an opaque index into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescribedProductSchema {
    /// Exact product data declaration the description selected in its
    /// query's lexical package scope.
    pub schema_symbol: symbols::SymbolHandle,
    /// The selected declaration's canonical path, retained for `path()`
    /// inspection and diagnostics.
    pub canonical_path: String,
}

/// One product-provider description issued by the compiler-owned
/// `Build.product.provider` query during build evaluation. Same marker
/// discipline as `DescribedProductSchema`: the evaluator keeps this payload
/// in a private side table and the `ProductProviderRef` value handed to
/// evaluated code carries only an opaque index into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescribedProductProvider {
    /// Exact provider data declaration the description selected in its
    /// query's lexical package scope. A provider declaration is the nominal
    /// data type owning at least one `satisfies` machine.
    pub provider_symbol: symbols::SymbolHandle,
    /// The selected declaration's canonical path, retained for `path()`
    /// inspection and diagnostics.
    pub canonical_path: String,
}

/// The two source-language selection channels have different authority:
/// a path resolves under the binding author's scope, while an issued description
/// keeps the exact selection made under its original query author's scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootBindingSelection {
    AuthoredPath(String),
    Described(DescribedProductEntry),
}

/// One operation actually executed against the original Build receiver.
/// The emitter retains the admitted occurrence's lexical source; possession of
/// this record alone grants no package visibility or executable entry authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedRootBinding {
    pub source_span: source::SourceSpan,
    pub slot: String,
    pub implementation: RootBindingSelection,
}
