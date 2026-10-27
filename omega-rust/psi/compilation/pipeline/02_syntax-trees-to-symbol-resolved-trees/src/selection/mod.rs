//! Exact declaration selection that lexical lookup alone cannot settle.
//!
//! `resolution::drive` runs these in a fixed order. Operator homes are selected
//! before symbol assignment; the rest run once every declaration has a
//! symbol: the authored-selection ledger, nominal machine-parameter
//! requirements, closed conformance rows, domain establishment routes,
//! evidence forwardings, and service reaches. `signature_free_requirements` is the shared law for paths
//! that name a trait requirement without a call signature.

pub(crate) mod authored_selections;
pub(crate) mod body_calls;
pub(crate) mod conformance_blocks;
pub(crate) mod domain_establishment;
pub(crate) mod domain_operator_homes;
pub(crate) mod evidence_forwardings;
pub(crate) mod machine_parameter_requirements;
pub(crate) mod service_reaches;
pub(crate) mod signature_free_requirements;

use crate::resolution::lowerer::Lowerer;
use diagnostics::Diagnostic;
use symbols::JoinedPath;

/// The kinds a top-level source name may resolve to — the lookup list
/// selection checks share when they accept any visible declaration.
pub(crate) const ANY_DECLARATION_LOOKUP_KINDS: &[symbols::SymbolKind] = &[
    symbols::SymbolKind::Module,
    symbols::SymbolKind::BuiltinType,
    symbols::SymbolKind::BuiltinFunction,
    symbols::SymbolKind::Data,
    symbols::SymbolKind::Domain,
    symbols::SymbolKind::Machine,
    symbols::SymbolKind::Operator,
    symbols::SymbolKind::Measure,
    symbols::SymbolKind::Proposition,
    symbols::SymbolKind::MathematicalDefinition,
    symbols::SymbolKind::Trait,
    symbols::SymbolKind::Conformance,
    symbols::SymbolKind::Const,
    symbols::SymbolKind::ConformanceParameter,
    symbols::SymbolKind::WireSchema,
];

/// Move every top-level operator into its exact domain's operator family.
/// Runs before symbol assignment.
pub(crate) fn select_operator_homes(lowerer: &mut Lowerer) -> Result<(), Vec<Diagnostic>> {
    domain_operator_homes::normalize_domain_operator_homes(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.namespace_declarations,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}

/// Record the authored-selection ledger: one exact occurrence per authored
/// source token, after constants have been substituted.
pub(crate) fn finalize_authored_selections(lowerer: &mut Lowerer) -> Result<(), Vec<Diagnostic>> {
    let const_roots = lowerer
        .pending_const_declarations
        .iter()
        .map(|pending| {
            lowerer
                .symbol_resolved_trees
                .const_declarations
                .get(pending.declaration)
        })
        .flat_map(|declaration| [declaration.initializer, declaration.authored_initializer])
        .filter(|expression| expression.is_valid())
        .collect::<Vec<_>>();
    // Substitution may copy an initializer after the pre-selection bind.
    // Rebind those copies before this ledger records a late call for the token.
    crate::constant::initializer_dependencies::bind_named_implementation_const_calls(
        &mut lowerer.symbol_resolved_trees,
        &const_roots,
    )
    .map_err(|message| vec![Diagnostic::error(message)])?;
    authored_selections::normalize_case_membership_facts(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_authored_proof_memberships,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let expression_work = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(expression, _)| {
            lowerer
                .seed
                .as_ref()
                .is_none_or(|seed| {
                    !seed
                        .source
                        .tables
                        .bodies
                        .expressions
                        .expression_is_valid(expression)
                })
                .then_some(expression)
        })
        .collect::<Vec<_>>();
    authored_selections::finalize_authored_expression_selections(
        &mut lowerer.symbol_resolved_trees,
        &expression_work,
        &lowerer.pending_authored_expressions,
        &lowerer.pending_authored_proof_memberships,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}

/// Settle every remaining selection once the ledger exists: signature-free
/// compatibility, machine-parameter requirements, evidence forwardings,
/// closed conformance rows and their reference selections, establishment
/// routes and service reaches. Resolution then normalizes the selected table
/// work and routes conformance member requirement calls over those tables.
pub(crate) fn finalize(lowerer: &mut Lowerer) -> Result<(), Vec<Diagnostic>> {
    let compatibility =
        signature_free_requirements::validate_signature_free_requirement_compatibility(
            &lowerer.symbol_resolved_trees,
        );
    if !compatibility.is_empty() {
        return Err(compatibility);
    }
    machine_parameter_requirements::normalize_nominal_machine_parameter_requirements(
        &mut lowerer.symbol_resolved_trees,
    )?;
    machine_parameter_requirements::normalize_trait_machine_requirement_arguments(
        &mut lowerer.symbol_resolved_trees,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    evidence_forwardings::bind_evidence_forwarding_owners(&mut lowerer.symbol_resolved_trees);
    let (pending_machine_service_reaches, pending_signature_service_reaches) =
        lowerer.pending_service_reaches()?;
    let machine_ordinals =
        conformance_blocks::normalize_closed_conformance_blocks(&mut lowerer.symbol_resolved_trees)
            .map_err(|diagnostic| vec![diagnostic])?;
    lowerer.pending_machine_bodies.retain_mut(|pending| {
        match machine_ordinals
            .get(pending.machine_ordinal)
            .copied()
            .flatten()
        {
            Some(ordinal) => {
                pending.machine_ordinal = ordinal;
                true
            }
            None => false,
        }
    });
    authored_selections::finalize_conformance_reference_selections(
        &mut lowerer.symbol_resolved_trees,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let seed = lowerer.seed.take();
    domain_establishment::normalize_domain_establishment_routes(
        &mut lowerer.symbol_resolved_trees,
        seed.as_ref().map(|seed| seed.source.as_ref()),
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    match seed {
        None => service_reaches::normalize_service_reaches(
            &mut lowerer.symbol_resolved_trees,
            &pending_machine_service_reaches,
            &pending_signature_service_reaches,
        ),
        Some(seed) => service_reaches::normalize_service_reaches_with_retained_tables(
            &mut lowerer.symbol_resolved_trees,
            &pending_machine_service_reaches,
            &pending_signature_service_reaches,
            seed.source.service_reaches.clone(),
            seed.source.service_reach_rows.clone(),
        ),
    }
    .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}

/// The authored-nominal machine contract beneath a type parameter, when it
/// carries one, with its `::`-rendered requirement path.
pub(super) fn authored_nominal_requirement<'parameter>(
    parameter: &'parameter symbol_resolved_trees::data::TypeParameter,
) -> Option<(
    &'parameter Vec<symbol_resolved_trees::name::DiagnosticName>,
    &'parameter arena::HandleSpan<symbol_resolved_trees::types::TypeReference>,
    String,
)> {
    let symbol_resolved_trees::data::TypeParameterKind::Machine {
        contract:
            symbol_resolved_trees::data::MachineParameterContract::AuthoredNominal {
                requirement,
                trait_arguments,
            },
    } = &parameter.kind
    else {
        return None;
    };
    let rendered = requirement.joined_path();
    Some((requirement, trait_arguments, rendered))
}

/// The `TypeReference` carriers that cannot hold a member or collection —
/// the no-op tail several selection walks repeat verbatim.
macro_rules! leaf_carrier_type_reference_pattern {
    () => {
        symbol_resolved_trees::types::TypeReference::FixedArray(_)
            | symbol_resolved_trees::types::TypeReference::Slice(_)
            | symbol_resolved_trees::types::TypeReference::Generic(_)
            | symbol_resolved_trees::types::TypeReference::ConstExpression(_)
            | symbol_resolved_trees::types::TypeReference::DynamicTrait { .. }
            | symbol_resolved_trees::types::TypeReference::Named { .. }
            | symbol_resolved_trees::types::TypeReference::Unit
    };
}
pub(super) use leaf_carrier_type_reference_pattern;
