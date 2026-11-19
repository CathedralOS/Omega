//! Every file contributes its declarations before L3 checks bodies. The order here also determines which source error a package reports first.

use crate::contracts::guarded_calls;
use crate::contracts::{access_policies, callback_bounds, callback_invocations, cleanup, logical_hypotheses, machine_effects, trait_refinements};
use crate::declarations::constants;
use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::operator_declarations::index_operator_declarations;
use crate::declarations::resolved_sources::index_resolved_sources;
use crate::evaluation::check_static_indices::check_static_indices;
use crate::evaluation::{integer_program, specialized_indices};
use crate::expressions::check_bodies::check_bodies;
use crate::toolchain_declarations::source_package::TOOLCHAIN_PACKAGE;
use crate::type_system::width_obligations::collect_width_obligations;
use crate::{TypeError, TypedStage, TypingEnvironment};
use omega_parser::NodeKind;
use omega_resolver::{ResolvedStage, StageSource};

pub(crate) fn type_admitted_stage(
    sources: &[StageSource<'_>],
    resolved: &ResolvedStage,
    admits_toolchain_declarations: bool,
    environment: &TypingEnvironment<'_>,
) -> Result<TypedStage, TypeError> {
    let mut units = index_resolved_sources(sources, resolved, admits_toolchain_declarations)?;
    let mut catalog = DeclarationCatalog {
        assembly: environment.target,
        ranking_views: environment.ranking_views,
        ..Default::default()
    };
    catalog.types.specializations = specialized_indices::IndexSpecializations::build(&units, environment.specialized_indices)?;
    catalog.types.operator_tokens = resolved
        .operator_declarations
        .iter()
        .map(|o| o.token.as_str())
        .collect();
    catalog.types.evaluator = environment.integer_evaluator;
    if units.iter().any(|unit| {
        unit.file
            .nodes
            .iter()
            .any(|n| matches!(n.kind, NodeKind::TypeArgs | NodeKind::ExpressionArgs | NodeKind::ArrayType))
    }) {
        catalog.types.integer_program = integer_program::IntegerProgram::build(&units, environment.evaluated_integer_calls)?;
    }
    for unit in &mut units {
        unit.index_generic_parameters(&mut catalog.types)?;
    }
    for unit in &units {
        unit.index_enclosing_scopes(&mut catalog.types)?;
    }
    for unit in &units {
        unit.index_const_carriers(&mut catalog.types)?;
    }
    let mut output = TypedStage::default();
    collect_width_obligations(&units, &mut catalog, &mut output)?;
    for unit in &units {
        unit.index_aliases(&mut catalog.types)?;
    }
    catalog.types.flatten_plain_aliases();
    for unit in &units {
        unit.index_domains(&mut catalog.types)?;
    }
    for unit in &units {
        unit.index_static_equations(&mut catalog.types)?;
    }
    logical_hypotheses::index_mathematical_interfaces(&units, &mut catalog.types)?;
    for unit in &mut units {
        logical_hypotheses::index_logical_hypotheses(unit, &mut catalog, &mut output)?;
    }
    for unit in &units {
        unit.index_declarations(&mut catalog, admits_toolchain_declarations && unit.file.package == TOOLCHAIN_PACKAGE)?;
    }
    logical_hypotheses::record_assumption_dependencies(&units, &mut catalog, &mut output)?;
    callback_bounds::resolve_callback_bounds(&units, &mut catalog)?;
    index_operator_declarations(&units, resolved, &mut catalog)?;
    constants::infer(&units, &mut catalog, &mut output)?;
    check_bodies(&units, &catalog, &mut output)?;
    check_static_indices(&units, &catalog, &mut output)?;
    machine_effects::check_machine_effects(&catalog, &mut output)?;
    callback_invocations::compose_callback_invocations(&units, &catalog, environment.boundary_slots, &mut output)?;
    guarded_calls::record_guarded_calls(&units, &catalog, &mut output)?;
    cleanup::check_cleanup_selections(&units, &catalog, &mut output)?;
    trait_refinements::check_trait_refinements(&catalog, &mut output)?;
    access_policies::check_access_policies(&units, &catalog, environment, &mut output)?;
    Ok(output)
}
