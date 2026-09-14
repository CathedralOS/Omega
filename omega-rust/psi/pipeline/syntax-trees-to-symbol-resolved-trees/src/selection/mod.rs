//! Exact declaration selection that lexical lookup alone cannot settle.
//!
//! `resolution::drive` runs these in a fixed order. Operator homes are selected
//! before symbol assignment; the rest run once every declaration has a
//! symbol: the authored-selection ledger, nominal machine-parameter
//! requirements, closed conformance rows, domain establishment routes,
//! evidence forwardings, and service reaches. `signature_free_requirements` is the shared law for paths
//! that name a trait requirement without a call signature.

pub(crate) mod authored_selections;
pub(crate) mod conformance_blocks;
pub(crate) mod domain_establishment;
pub(crate) mod domain_operator_homes;
pub(crate) mod evidence_forwardings;
pub(crate) mod machine_parameter_requirements;
pub(crate) mod service_reaches;
pub(crate) mod signature_free_requirements;

use crate::resolution::lowerer::Lowerer;
use diagnostics::Diagnostic;

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
    authored_selections::finalize_authored_expression_selections(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_authored_expressions,
        &lowerer.pending_authored_proof_memberships,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}

/// Settle every remaining selection once the ledger exists: signature-free
/// compatibility, machine-parameter requirements, evidence forwardings,
/// closed conformance rows and their reference selections, establishment
/// routes, service reaches, and inline member routing over rebuilt tables.
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
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    machine_parameter_requirements::normalize_trait_machine_requirement_arguments(
        &mut lowerer.symbol_resolved_trees,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    evidence_forwardings::bind_evidence_forwarding_owners(&mut lowerer.symbol_resolved_trees);
    let (pending_machine_service_reaches, pending_signature_service_reaches) =
        lowerer.pending_service_reaches();
    conformance_blocks::normalize_closed_conformance_blocks(&mut lowerer.symbol_resolved_trees)
        .map_err(|diagnostic| vec![diagnostic])?;
    authored_selections::finalize_conformance_reference_selections(
        &mut lowerer.symbol_resolved_trees,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    domain_establishment::normalize_domain_establishment_routes(&mut lowerer.symbol_resolved_trees)
        .map_err(|diagnostic| vec![diagnostic])?;
    match lowerer.seed.take() {
        None => service_reaches::normalize_service_reaches(
            &mut lowerer.symbol_resolved_trees,
            &pending_machine_service_reaches,
            &pending_signature_service_reaches,
        ),
        Some(seed) => service_reaches::normalize_service_reaches_with_retained_tables(
            &mut lowerer.symbol_resolved_trees,
            &pending_machine_service_reaches,
            &pending_signature_service_reaches,
            seed.service_reaches,
            seed.service_reach_rows,
        ),
    }
    .map_err(|diagnostic| vec![diagnostic])?;
    lowerer.symbol_resolved_trees.rebuild_tables();
    conformance_blocks::route_inline_member_calls(&mut lowerer.symbol_resolved_trees);
    Ok(())
}
