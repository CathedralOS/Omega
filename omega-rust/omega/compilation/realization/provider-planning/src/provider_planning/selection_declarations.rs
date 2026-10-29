//! Project source choices once; shared selection never inspects their source owner.

use crate::{ProviderSelection, ProviderSelectionSubject};
use provider_selection::{
    ProviderDeclaration, ProviderFamily, ProviderFamilyCoordinate, ProviderIdentity,
    ProviderSubject, SelectionProvenance,
};
use symbols::SymbolHandle;

pub(crate) fn project_declaration<Declaration, Origin>(
    source: &ProviderSelection,
    nominal: &impl Fn(SymbolHandle) -> Result<Declaration, String>,
    origin: Origin,
) -> ProviderDeclaration<Declaration, Origin> {
    let identity =
        |source: &build_time_evaluation::provider_declarations::ProviderSelectionIdentity| {
            ProviderIdentity {
                declaration: nominal(source.symbol),
                application_identity: source.application_identity.clone(),
                package: source.package,
                canonical_path: source.canonical_path.clone(),
                authored_path: source.authored_path.clone(),
            }
        };
    let subject = match &source.subject {
        ProviderSelectionSubject::BoundaryTrait(source) => {
            ProviderSubject::BoundaryTrait(identity(source))
        }
        ProviderSelectionSubject::BoundaryRequirement(source) => {
            ProviderSubject::BoundaryRequirement(identity(source))
        }
        ProviderSelectionSubject::BoundaryOperatorFamily(family) => {
            ProviderSubject::BoundaryOperatorFamily(ProviderFamily {
                package: family.package,
                canonical_path: family.canonical_path.clone(),
                authored_path: family.authored_path.clone(),
                coordinates: family
                    .coordinates()
                    .iter()
                    .map(|coordinate| ProviderFamilyCoordinate {
                        declaration: nominal(coordinate.symbol),
                        requirement_identity: coordinate.requirement_identity.clone(),
                    })
                    .collect(),
                preparation_errors: Vec::new(),
            })
        }
    };
    ProviderDeclaration {
        target: source.target,
        subject,
        provider_type: identity(&source.provider_type),
        composition_mode: source.composition_mode,
        origin,
    }
}

/// Family replay needs source only during preparation, before artifact demand.
pub(crate) fn retain_family_replay<Declaration, Origin>(
    context: crate::ProviderPlanningContext<'_>,
    source: &ProviderSelection,
    projected: &mut ProviderDeclaration<Declaration, Origin>,
) {
    if let ProviderSelectionSubject::BoundaryOperatorFamily(family) = &source.subject
        && let ProviderSubject::BoundaryOperatorFamily(projected) = &mut projected.subject
    {
        projected.preparation_errors = family
            .replay_against_typed(context)
            .into_iter()
            .map(diagnostics::Diagnostic::error)
            .collect();
    }
}

pub(crate) fn project_source_declaration(
    source: &ProviderSelection,
) -> ProviderDeclaration<SymbolHandle, ProviderSelection> {
    project_declaration(source, &Ok, source.clone())
}

pub(crate) fn project_provenance(
    source: crate::ProviderSelectionProvenance,
) -> SelectionProvenance<SymbolHandle, ProviderSelection> {
    match source {
        crate::ProviderSelectionProvenance::BuildOverride(declarations) => {
            SelectionProvenance::BuildOverride(
                declarations
                    .iter()
                    .map(project_source_declaration)
                    .collect(),
            )
        }
        crate::ProviderSelectionProvenance::TargetDefault(declarations) => {
            SelectionProvenance::TargetDefault(
                declarations
                    .iter()
                    .map(project_source_declaration)
                    .collect(),
            )
        }
        crate::ProviderSelectionProvenance::UniqueCoveringCandidate => {
            SelectionProvenance::UniqueCoveringCandidate
        }
        crate::ProviderSelectionProvenance::NativeBuild => SelectionProvenance::NativeBuild,
    }
}

pub(crate) fn restore_provenance(
    projected: SelectionProvenance<SymbolHandle, ProviderSelection>,
) -> crate::ProviderSelectionProvenance {
    match projected {
        SelectionProvenance::BuildOverride(declarations) => {
            crate::ProviderSelectionProvenance::BuildOverride(
                declarations
                    .into_iter()
                    .map(|declaration| declaration.origin)
                    .collect(),
            )
        }
        SelectionProvenance::TargetDefault(declarations) => {
            crate::ProviderSelectionProvenance::TargetDefault(
                declarations
                    .into_iter()
                    .map(|declaration| declaration.origin)
                    .collect(),
            )
        }
        SelectionProvenance::UniqueCoveringCandidate => {
            crate::ProviderSelectionProvenance::UniqueCoveringCandidate
        }
        SelectionProvenance::NativeBuild => crate::ProviderSelectionProvenance::NativeBuild,
    }
}

pub(crate) fn source_candidate(
    source: crate::DerivedProviderPlan,
) -> provider_selection::ProviderCandidate<SymbolHandle, crate::ProviderPlanProvenance> {
    let native_build = source
        .provenance
        .row_realizations
        .iter()
        .all(|row| row.native().is_some());
    provider_selection::ProviderCandidate {
        plan: source.plan,
        declarations: provider_selection::CandidateDeclarations {
            schema: Ok(source.provenance.schema.symbol()),
            provider: source.provenance.provider_type.map(Ok),
        },
        native_build,
        owner: source.provenance,
    }
}
