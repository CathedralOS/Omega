/*
We derive candidate provider plans from explicit satisfaction declarations and
prepare their exact source associations for the shared source-free
`provider-selection` owner.

Whole-plan selection and complete binding belong to `provider-selection`: one
fully covering provider type is chosen per applicable boundary slot (an
explicit build-root declaration wins over the selected target package's
ordinary default declaration; without either, a unique covering candidate
supplies the declaration-era default). Rows are never selected individually
and partial candidates never combine.
*/

/*
We organize provider-planning into focused modules:
- `provenance_replay`: candidate derivation (`derive_satisfies_plans`,
  `named_provider_plans`, `plan_derivation`) and checked-adapter conformance
  replay (`adapter_conformance`).
- `selected_declarations`: source-free declaration projection for selected
  plans (`capture_plan_declarations`, `prepare_selected_plan_declarations`).
- `source_provider_selections`: early/reference source adapters for build-time
  provider bodies and checked boundary operators.
- `provider_rows`: row-level direct-boundary and intrinsic/exit-process
  declaration capture and deferred error settlement.
- `external_binding_rows`: normalized external binding extraction (`Import`,
  `Syscall`, `VtableField`, `TableFunction`) and foreign table-field offset
  resolution.
- `selection_declarations`: source-to-canonical projection and provenance
  restoration between `ProviderSelection` and `provider_selection`.
*/

pub(crate) mod external_binding_rows;
mod provenance_replay;
pub(crate) mod provider_rows;
pub(crate) mod selected_declarations;
mod source_provider_selections;

pub use selected_declarations::prepare_selected_plan_declarations;

pub use effects::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    CompilerPrimitiveIntegerComparisonOperation,
};
pub use external_binding_rows::extract_native_external_binding_rows;
pub(crate) use provenance_replay::{
    DerivedProviderPlan, ProviderPlanProvenance, SelectedTargetMachineOrigin,
};
pub use provenance_replay::{
    ProviderPlanDerivation, ProviderRowRealization, ProviderSchemaDeclaration,
    derive_satisfies_plans, exact_checked_adapter, same_semantic_name, satisfies_plan_name,
    selected_requirement_checked_adapter,
};
pub use source_provider_selections::{
    prepare_selected_build_time_provider_body, selected_checked_operator_body,
    selected_requirement_plan, selected_use_plan,
};

use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use component_description::VerifiedComponent;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};
use symbols::SymbolHandle;

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
pub struct SelectedProviderPlanWithProvenance {
    pub derived: DerivedProviderPlan,
    pub selected_by: ProviderSelectionProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderReviewProvenance {
    pub plan: ProviderPlan,
    pub provider: ProviderPlanProvenance,
    pub selected_by: ProviderSelectionProvenance,
    pub row_compiler_intrinsic_executions: Vec<Option<CompilerIntrinsicExecutionIdentity>>, // Populated with one row-aligned entry per plan row before publishing CheckedCompilation.
    pub row_external_completion_leaves: Vec<Vec<SymbolHandle>>, // Exact external leaves reached on every checked completion path.
}

pub fn selected_provider_plan_facts(
    context: crate::ProviderPlanningContext<'_>,
    selected: Vec<SelectedProviderPlanWithProvenance>,
) -> Result<
    (
        effects::SelectedProviderPlanFacts,
        Vec<SelectedProviderReviewProvenance>,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    selected_provider_plan_facts_with_independent_components(context, selected, &[], &[])
}

pub(crate) fn selected_provider_plan_facts_with_independent_components(
    context: crate::ProviderPlanningContext<'_>,
    selected: Vec<SelectedProviderPlanWithProvenance>,
    independent_components: &[VerifiedComponent],
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<
    (
        effects::SelectedProviderPlanFacts,
        Vec<SelectedProviderReviewProvenance>,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    let mut choices = Vec::with_capacity(selected.len());
    for selected in selected {
        let mut provenance = selection_declarations::project_provenance(selected.selected_by);
        match &mut provenance {
            provider_selection::SelectionProvenance::BuildOverride(declarations)
            | provider_selection::SelectionProvenance::TargetDefault(declarations) => {
                for declaration in declarations {
                    let source = declaration.origin.clone();
                    selection_declarations::retain_family_replay(context, &source, declaration);
                }
            }
            provider_selection::SelectionProvenance::UniqueCoveringCandidate
            | provider_selection::SelectionProvenance::NativeBuild
            | provider_selection::SelectionProvenance::TargetRealization(_) => {}
        }
        let selecting_declaration = crate::provider_declarations::close_selecting_declaration(
            context,
            exact_toolchain_sources,
            &selected.derived.provenance,
        );
        let mut candidate =
            selection_declarations::source_candidate(context.typed, selected.derived);
        candidate.declarations.selecting_declaration = selecting_declaration;
        choices.push(provider_selection::SelectedProvider {
            candidate,
            selected_by: provenance,
        });
    }
    let (facts, selected) = provider_selection::admit(choices, independent_components)?;
    let provenance = selected
        .into_iter()
        .map(|selected| SelectedProviderReviewProvenance {
            plan: selected.candidate.plan,
            provider: selected.candidate.owner,
            selected_by: selection_declarations::restore_provenance(selected.selected_by),
            row_compiler_intrinsic_executions: Vec::new(),
            row_external_completion_leaves: Vec::new(),
        })
        .collect();
    Ok((facts, provenance))
}

pub fn select_derived_provider_plans(
    typed: &TypedTrees,
    derived: &[DerivedProviderPlan],
    selected_target: target::TargetProfile,
    defaults: &[crate::ProviderSelection],
    requested: &[crate::ProviderSelection],
) -> Result<Vec<SelectedProviderPlanWithProvenance>, Vec<diagnostics::Diagnostic>> {
    let candidates = derived
        .iter()
        .cloned()
        .map(|derived| selection_declarations::source_candidate(typed, derived))
        .collect();
    let defaults = defaults
        .iter()
        .map(selection_declarations::project_source_declaration)
        .collect::<Vec<_>>();
    let requested = requested
        .iter()
        .map(selection_declarations::project_source_declaration)
        .collect::<Vec<_>>();
    let selected = provider_selection::choose(candidates, selected_target, &defaults, &requested)?;
    Ok(selected
        .into_iter()
        .map(|selected| SelectedProviderPlanWithProvenance {
            derived: DerivedProviderPlan {
                plan: selected.candidate.plan,
                provenance: selected.candidate.owner,
            },
            selected_by: selection_declarations::restore_provenance(selected.selected_by),
        })
        .collect())
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
    machine: &checked_trees::typed_trees::machine::Machine,
) -> Option<&'typed checked_trees::state::State> {
    typed.machine_states(machine).first()
}

pub(crate) mod selection_declarations {
    use crate::{ProviderSelection, ProviderSelectionSubject};
    use checked_trees::typed_trees::TypedTrees;
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

    pub(crate) fn source_candidate(
        typed: &TypedTrees,
        source: crate::DerivedProviderPlan,
    ) -> provider_selection::ProviderCandidate<SymbolHandle, crate::ProviderPlanProvenance> {
        let canonical_float_map = canonical_float_map(typed, &source.provenance);
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
                selecting_declaration: Err(vec![diagnostics::Diagnostic::error(
                    "canonical selecting declaration has not been prepared",
                )]),
            },
            native_build,
            canonical_float_map,
            owner: source.provenance,
        }
    }

    pub(crate) fn canonical_float_map(
        typed: &TypedTrees,
        provenance: &crate::ProviderPlanProvenance,
    ) -> bool {
        provenance.provider_type.is_some_and(|symbol| {
            checked_trees::typed_trees::float_providers::is_sealed_catalog_map(typed, symbol)
        })
    }
}
