//! Every file contributes its declarations before L3 checks bodies. The order here also determines which source error a package reports first.

use crate::contracts::{access_policies, callback_bounds, cleanup, logical_hypotheses, machine_effects, trait_refinements};
use crate::contracts::{effect_propagation::ExecutionCalls, guarded_calls};
use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::operator_declarations::{index_operator_declarations, trait_operator_tokens};
use crate::declarations::resolved_sources::{check_domain_visibility, check_static_qualifiers, index_resolved_sources};
use crate::declarations::{constants, mathematical_signatures};
use crate::evaluation::check_static_indices::{IndexEvaluations, check_static_indices};
use crate::expressions::check_bodies::check_bodies;
use crate::toolchain_declarations::core_interface::{CheckedLibrary, configure_roles};
use crate::type_system::definition_order::index_type_definitions;
use crate::{TypeError, TypedStage, TypingEnvironment};
use omega_resolver::SymbolResolvedTrees;
use omega_toolchain::CoreInterface;

pub(crate) fn type_admitted_stage<'text>(
    resolved: &SymbolResolvedTrees<'text>,
    library: Option<&'static CheckedLibrary>,
    environment: &TypingEnvironment<'_>,
) -> Result<crate::TypedTrees<'text>, TypeError> {
    let mut output = TypedStage::default();
    let mut catalog = library.map_or_else(DeclarationCatalog::default, |lib| lib.import(&mut output));
    let first_local_obligation = output.obligations.len();
    catalog.assembly = environment.target;
    if library.is_none() {
        catalog.ranking_views = environment.ranking_views.into();
        catalog.access_plan_requirement = environment.access_plan_requirement;
    }
    check_stage(resolved, environment, &mut catalog, &mut output, None, library.map(|lib| &lib.execution))?;
    if library.is_none() && !output.collection_accesses.is_empty() {
        output.library = Some(crate::standard_library());
    }
    catalog.publish_declarations(&mut output);
    Ok(crate::TypedTrees::new(resolved.clone(), output, first_local_obligation))
}

pub(crate) fn check_stage<'a>(
    resolved: &'a SymbolResolvedTrees<'_>,
    environment: &TypingEnvironment<'a>,
    catalog: &mut DeclarationCatalog<'a>,
    output: &mut TypedStage,
    definition: Option<&'a CoreInterface>,
    library_execution: Option<&ExecutionCalls>,
) -> Result<ExecutionCalls, TypeError> {
    let mut units = index_resolved_sources(resolved);
    catalog.types.indices = IndexEvaluations::admit(&units, environment)?;
    let trait_tokens = trait_operator_tokens(resolved, &catalog.types.trait_operator_tokens);
    catalog.types.trait_operator_tokens.extend(trait_tokens);
    catalog.types.literal_constants.collect(&units);
    index_type_definitions(&mut units, &mut catalog.types)?;
    check_domain_visibility(resolved, &units, &catalog.types)?;
    for unit in &units {
        unit.index_static_equations(&mut catalog.types)?;
    }
    mathematical_signatures::index_interfaces(&units, &mut catalog.types)?;
    for unit in &mut units {
        logical_hypotheses::index_logical_hypotheses(unit, catalog, output)?;
    }
    for unit in &units {
        unit.index_declarations(catalog, definition)?;
    }
    if let Some(core) = definition {
        configure_roles(core, catalog, output);
    }
    logical_hypotheses::record_assumption_dependencies(&units, catalog, output);
    catalog.index_requirement_fields();
    callback_bounds::resolve_callback_bounds(&units, catalog)?;
    check_static_qualifiers(resolved, &units, &catalog.types)?;
    crate::contracts::conformance::index_satisfaction_edges(&units, catalog, output)?;
    index_operator_declarations(resolved, catalog, definition)?;
    constants::infer(&units, catalog, output)?;
    check_bodies(&units, catalog, output)?;
    check_static_indices(&units, catalog, environment, output)?;
    let execution = ExecutionCalls::collect(output);
    machine_effects::check_machine_effects(catalog, &execution, environment.boundary_slots, output)?;
    guarded_calls::record_guarded_calls(&execution, library_execution, catalog, output);
    cleanup::check_cleanup_selections(&units, catalog, output)?;
    trait_refinements::check_trait_refinements(catalog, output)?;
    access_policies::check_access_policies(&units, catalog, output)?;
    catalog.types.formations.check(&catalog.types, output)?;
    Ok(execution)
}
