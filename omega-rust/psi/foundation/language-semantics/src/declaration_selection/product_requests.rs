//! Inert Product declaration requests issued by an ordinary Build activation.
//!
//! The package join supplies the declaration frontier. Before that join these
//! source paths are locators under an exact authored occurrence, not selected
//! symbols, executable imports or trust grants. A Terminal activation authenticates
//! its original receiver and records only committed calls. The Product owner then
//! resolves each operand's role, package visibility and actual declaration before
//! publishing provider choices or authority. These are source-side request records,
//! never identities reconstructed from portable Terminal bytes.

use super::{BuildOperation, ExecutedRootBinding, RootBindingRequest};
use source::SourceSpan;

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

/// One declaration-only operand, retained without importing its Product body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDeclarationLocator {
    pub source_span: SourceSpan,
    pub path: String,
}

/// A declaration locator and a closed compiler fact have different denotations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildProductOperand {
    Declaration(ProductDeclarationLocator),
    WireFact {
        source_span: SourceSpan,
        fact: crate::wire::WireCompatibilityFact,
    },
}

/// Owned source-side evidence beside an emitted, artifact-bound call. The
/// operand slice belongs to this standalone request, not an AST child arena.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDeclarationSelectionRequest {
    pub source_span: SourceSpan,
    pub operation: BuildOperation,
    pub operands: Box<[BuildProductOperand]>,
}

impl ProductDeclarationSelectionRequest {
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
            match (self.operation.product_operand_role(position), operand) {
                (
                    Some(ProductOperandRole::WireCompatibilityFact),
                    BuildProductOperand::WireFact { source_span, fact },
                ) => {
                    if source_span.span.start >= source_span.span.end {
                        return Err("Build wire fact has no authored occurrence");
                    }
                    if self.operands[..position].iter().any(|previous| matches!(previous, BuildProductOperand::WireFact { fact: previous, .. } if previous == fact)) {
                        return Err("Build wire demand repeats a compatibility fact");
                    }
                }
                (Some(role), BuildProductOperand::Declaration(locator))
                    if role != ProductOperandRole::WireCompatibilityFact =>
                {
                    if locator.path.is_empty()
                        || locator.source_span.span.start >= locator.source_span.span.end
                    {
                        return Err("Build product request has no authored declaration locator");
                    }
                }
                _ => return Err("Build product operand differs from its declared role"),
            }
        }
        Ok(())
    }
}

/// One source-issued recording operation. Queries returning issued descriptions
/// remain separate: an unresolved request cannot impersonate a successful query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildProductRequest {
    RootBinding(RootBindingRequest),
    DeclarationSelection(ProductDeclarationSelectionRequest),
    BehaviorExclusion(BuildBehaviorExclusionRequest),
}

impl Default for BuildProductRequest {
    fn default() -> Self {
        // Arena zero storage is inert; real requests still pass source/shape admission.
        Self::RootBinding(RootBindingRequest::default())
    }
}

impl BuildProductRequest {
    pub fn as_root_binding(&self) -> Option<&RootBindingRequest> {
        match self {
            Self::RootBinding(request) => Some(request),
            Self::DeclarationSelection(_) | Self::BehaviorExclusion(_) => None,
        }
    }

    pub fn source_span(&self) -> SourceSpan {
        match self {
            Self::RootBinding(request) => request.source_span,
            Self::DeclarationSelection(request) => request.source_span,
            Self::BehaviorExclusion(request) => request.source_span,
        }
    }
}

/// The source's explicit component-composition choice. Target realization owns
/// its consequences; neither provider shape nor an omitted argument selects
/// independent composition.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProviderCompositionMode {
    #[default]
    Fused,
    Independent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedProductDeclarationSelection {
    pub request: ProductDeclarationSelectionRequest,
    pub author: SourceSpan,
    pub composition: ProviderCompositionMode,
}

/// Committed selections retain their lexical request without Build arena handles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutedBuildProductRequest {
    RootBinding(ExecutedRootBinding),
    DeclarationSelection(ExecutedProductDeclarationSelection),
    BehaviorExclusion(ExecutedBuildBehaviorExclusion),
}

/// A closed toolchain value vocabulary, not a user-defined behavior classifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildBehaviorExclusionKind {
    Crash,
    PhysicalAuthority,
}

impl BuildBehaviorExclusionKind {
    pub const fn type_name(self) -> &'static str {
        match self {
            Self::Crash => "CrashCause",
            Self::PhysicalAuthority => "PhysicalAuthorityClass",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildBehaviorExclusionRequest {
    /// Source IDs are invocation-local. This exact owner/path/content row
    /// authenticates rebasing the retained byte coordinates at Product join.
    pub source: source::source_identity::ConsumedSourceUnit,
    pub source_span: SourceSpan,
    pub kind: BuildBehaviorExclusionKind,
}

/// Exact checked case occurrence, recorded only after observing its evaluated
/// discriminator with the activation's original Build value. Byte coordinates
/// rejoin through the request's source identity, never interpreter-local IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedBuildBehaviorExclusion {
    pub request: BuildBehaviorExclusionRequest,
    pub author: SourceSpan,
    pub case: SourceSpan,
}
