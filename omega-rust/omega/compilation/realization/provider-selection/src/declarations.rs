//! Exact correspondence is separate from candidate lookup and diagnostic spelling.

use diagnostics::Diagnostic;
use effects::provider_plan::ProviderPlan;
use language_semantics::declaration_selection::{CompositionMode, ProductTarget};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIdentity<Declaration> {
    /// Missing portable ownership is a deferred refusal, never an identity.
    pub declaration: Result<Declaration, String>,
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
    /// The complete authored family, never the subset reached by an artifact.
    pub coordinates: Vec<ProviderFamilyCoordinate<Declaration>>,
    /// Source preparation checks this exact family against its declaration owner.
    /// An unused family's refusal remains inert until a selection demands it.
    pub preparation_errors: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSubject<Declaration> {
    BoundaryTrait(ProviderIdentity<Declaration>),
    BoundaryRequirement(ProviderIdentity<Declaration>),
    BoundaryOperatorFamily(ProviderFamily<Declaration>),
}

pub(crate) fn same_declaration<Declaration: PartialEq>(
    left: &Result<Declaration, String>,
    right: &Result<Declaration, String>,
) -> bool {
    matches!((left, right), (Ok(left), Ok(right)) if left == right)
}

impl<Declaration: PartialEq> ProviderSubject<Declaration> {
    /// Demand and publication retain the same exact schema/application/family
    /// choice, without applying a target fence earlier than whole-plan selection.
    pub fn selects_plan(&self, schema: &Result<Declaration, String>, plan: &ProviderPlan) -> bool {
        plan.rows.iter().any(|row| {
            self.selects_schema(
                schema,
                plan.identity_text(row.requirement_identity),
                plan.schema.root_application_identity(),
            )
        })
    }

    pub fn authored_path(&self) -> &str {
        match self {
            Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => {
                &identity.authored_path
            }
            Self::BoundaryOperatorFamily(family) => &family.authored_path,
        }
    }

    pub fn selects_schema(
        &self,
        schema: &Result<Declaration, String>,
        requirement: &str,
        application: Option<&str>,
    ) -> bool {
        match self {
            Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => {
                same_declaration(&identity.declaration, schema)
                    && identity
                        .application_identity
                        .as_deref()
                        .is_none_or(|selected| Some(selected) == application)
            }
            Self::BoundaryOperatorFamily(family) => family.coordinates.iter().any(|coordinate| {
                same_declaration(&coordinate.declaration, schema)
                    && coordinate.requirement_identity == requirement
            }),
        }
    }

    pub(crate) fn same_declaration_as(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::BoundaryTrait(left), Self::BoundaryTrait(right))
            | (Self::BoundaryRequirement(left), Self::BoundaryRequirement(right)) => {
                same_declaration(&left.declaration, &right.declaration)
                    && left.application_identity == right.application_identity
            }
            (Self::BoundaryOperatorFamily(left), Self::BoundaryOperatorFamily(right)) => {
                left.package == right.package && left.canonical_path == right.canonical_path
            }
            _ => false,
        }
    }

    pub(crate) fn owner_errors(&self) -> Vec<Diagnostic> {
        match self {
            Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => identity
                .declaration
                .as_ref()
                .err()
                .map(|reason| Diagnostic::error(reason.clone()))
                .into_iter()
                .collect(),
            Self::BoundaryOperatorFamily(family) => family
                .coordinates
                .iter()
                .filter_map(|coordinate| coordinate.declaration.as_ref().err())
                .map(|reason| Diagnostic::error(reason.clone()))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDeclaration<Declaration, Origin = ()> {
    pub target: Option<ProductTarget>,
    pub subject: ProviderSubject<Declaration>,
    pub provider_type: ProviderIdentity<Declaration>,
    pub composition_mode: CompositionMode,
    /// Returned unchanged to the producing adapter, never used to choose a row.
    pub origin: Origin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionProvenance<Declaration, Origin = ()> {
    BuildOverride(Vec<ProviderDeclaration<Declaration, Origin>>),
    TargetDefault(Vec<ProviderDeclaration<Declaration, Origin>>),
    UniqueCoveringCandidate,
    NativeBuild,
}

impl<Declaration, Origin> SelectionProvenance<Declaration, Origin> {
    pub fn declarations(&self) -> &[ProviderDeclaration<Declaration, Origin>] {
        match self {
            Self::BuildOverride(declarations) | Self::TargetDefault(declarations) => declarations,
            Self::UniqueCoveringCandidate | Self::NativeBuild => &[],
        }
    }

    pub fn composition_mode(&self) -> Result<CompositionMode, String> {
        match self {
            Self::UniqueCoveringCandidate | Self::NativeBuild => Ok(CompositionMode::Fused),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateDeclarations<Declaration> {
    pub schema: Result<Declaration, String>,
    pub provider: Option<Result<Declaration, String>>,
}

#[derive(Debug, Clone)]
pub struct ProviderCandidate<Declaration, Owner = ()> {
    pub plan: ProviderPlan,
    pub declarations: CandidateDeclarations<Declaration>,
    pub native_build: bool,
    /// Exact derivation or canonical closure travels with its plan.
    pub owner: Owner,
}

#[derive(Debug, Clone)]
pub struct SelectedProvider<Declaration, Owner = (), Origin = ()> {
    pub candidate: ProviderCandidate<Declaration, Owner>,
    pub selected_by: SelectionProvenance<Declaration, Origin>,
}
