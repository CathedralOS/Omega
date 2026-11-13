// Provider provenance records and canonical declaration projection survive the
// old source producer. No satisfies/adapter checking or source-plan selection
// is performed by this data owner.

mod provenance_replay;
pub(crate) mod provider_rows;

use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};
pub use effects::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    CompilerPrimitiveIntegerComparisonOperation,
};
pub(crate) use provenance_replay::{
    DerivedProviderPlan, ProviderPlanProvenance, SelectedTargetMachineOrigin,
};
pub use provenance_replay::{
    ProviderRowRealization, ProviderSchemaDeclaration, same_semantic_name, satisfies_plan_name,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSelectionProvenance {
    BuildOverride(Vec<crate::ProviderSelection>),
    TargetDefault(Vec<crate::ProviderSelection>),
    UniqueCoveringCandidate,
    NativeBuild,
    TargetRealization(target::CanonicalFloatRealization),
}

impl ProviderSelectionProvenance {
    pub fn composition_mode(&self) -> Result<crate::CompositionMode, String> {
        selection_declarations::project_provenance(self.clone()).composition_mode()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderReviewProvenance {
    pub plan: ProviderPlan,
    pub provider: ProviderPlanProvenance,
    pub selected_by: ProviderSelectionProvenance,
    pub row_compiler_intrinsic_executions: Vec<Option<CompilerIntrinsicExecutionIdentity>>, // Populated with one row-aligned entry per plan row before publishing CheckedCompilation.
    pub row_external_completion_leaves: Vec<Vec<SymbolHandle>>, // Exact external leaves reached on every checked completion path.
}

pub(crate) fn trait_name_text(schema: &ServiceSchema) -> &str {
    schema.identity_text(schema.declaration.trait_name)
}

pub(crate) fn type_reference_node(
    typed: &TypedTrees,
    handle: TypeReferenceHandle,
) -> &TypeReferenceNode {
    typed.type_reference_table.type_reference(handle)
}

pub(crate) fn is_unit_type(typed: &TypedTrees, handle: TypeReferenceHandle) -> bool {
    matches!(type_reference_node(typed, handle), TypeReferenceNode::Unit)
}

pub(crate) fn entry_state<'typed>(
    typed: &'typed TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Option<&'typed typed_trees::state::State> {
    typed.machine_states(machine).first()
}

pub(crate) mod selection_declarations {
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
            crate::ProviderSelectionProvenance::TargetRealization(realization) => {
                SelectionProvenance::TargetRealization(realization)
            }
        }
    }

    pub(crate) fn restore_provenance<Declaration>(
        projected: SelectionProvenance<Declaration, ProviderSelection>,
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
            SelectionProvenance::TargetRealization(realization) => {
                crate::ProviderSelectionProvenance::TargetRealization(realization)
            }
        }
    }
}
