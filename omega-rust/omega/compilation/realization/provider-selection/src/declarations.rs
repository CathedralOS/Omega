//! Authored provider declarations retained with selection provenance. Settlement
//! uses their composition mode when deciding how chosen plans may be joined.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{CompositionMode, ProductTarget};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIdentity<Declaration> {
    pub declaration: Result<Declaration, String>, // missing portable ownership is a deferred refusal, never an identity
    pub application_identity: Option<String>,
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub canonical_path: String,
    pub authored_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderFamilyCoordinate<Declaration> {
    pub declaration: Result<Declaration, String>,
    pub requirement_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderFamily<Declaration> {
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub canonical_path: String,
    pub authored_path: String,
    pub coordinates: Vec<ProviderFamilyCoordinate<Declaration>>, // complete authored family, never the subset reached by an artifact
    pub preparation_errors: Vec<Diagnostic>, // unused family's refusal remains inert until a selection demands it
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSubject<Declaration> {
    BoundaryTrait(ProviderIdentity<Declaration>),
    BoundaryRequirement(ProviderIdentity<Declaration>),
    BoundaryOperatorFamily(ProviderFamily<Declaration>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDeclaration<Declaration, Origin = ()> {
    pub target: Option<ProductTarget>,
    pub subject: ProviderSubject<Declaration>,
    pub provider_type: ProviderIdentity<Declaration>,
    pub composition_mode: CompositionMode,
    pub origin: Origin, // returned unchanged to the producing adapter, never used to choose a row
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionProvenance<Declaration, Origin = ()> {
    BuildOverride(Vec<ProviderDeclaration<Declaration, Origin>>),
    TargetDefault(Vec<ProviderDeclaration<Declaration, Origin>>),
    UniqueCoveringCandidate,
    NativeBuild,
    /// A canonical float operation realized by the selected target's declared
    /// realization table; no Build or package declaration chose it.
    TargetRealization(target::CanonicalFloatRealization),
}

impl<Declaration, Origin> SelectionProvenance<Declaration, Origin> {
    pub fn composition_mode(&self) -> Result<CompositionMode, String> {
        match self {
            Self::UniqueCoveringCandidate | Self::NativeBuild | Self::TargetRealization(_) => {
                Ok(CompositionMode::Fused)
            }
            Self::TargetDefault(declarations) => {
                if declarations
                    .iter()
                    .any(|declaration| declaration.composition_mode != CompositionMode::Fused)
                {
                    return Err("target-provider defaults cannot request independent composition; only the owner-controlled build may create that deployment cut".into());
                }
                Ok(CompositionMode::Fused)
            }
            Self::BuildOverride(declarations) => {
                let Some(first) = declarations.first() else {
                    return Err(
                        "selected provider plan has a build override without a declaration".into(),
                    );
                };
                if declarations
                    .iter()
                    .any(|declaration| declaration.composition_mode != first.composition_mode)
                {
                    return Err(
                        "selected provider plan has conflicting build-owned composition modes"
                            .into(),
                    );
                }
                Ok(first.composition_mode)
            }
        }
    }
}
