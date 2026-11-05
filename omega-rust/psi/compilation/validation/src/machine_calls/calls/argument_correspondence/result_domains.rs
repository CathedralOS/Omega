//! Initialized memory must keep its promise about the stored type even when
//! an unrelated implementation argument is generic. Apply the call's checked
//! arguments to the implementation's own result; the public requirement cannot
//! grant a promise missing from that result.

use super::{CallSite, CarrierCorrespondence, SelectedCallArgumentError, selected_call_arguments};
use symbols::SymbolHandle;
use typed_trees::data::TypeParameterKind;
use typed_trees::type_identity::{
    IndexIdentityArgument, applied_index_identity, applied_type_identity,
    applied_type_identity_in_scope,
};

/// A generic call must preserve the promise made by its implementation about
/// the returned value. Compare the retained domain operands under this exact
/// call; matching arguments does not grant authority to issue the domain.
pub fn selected_call_result_domain_matches(
    program: &dyn crate::CheckingInput,
    caller: SymbolHandle,
    site: &CallSite<'_>,
    domain_symbol: SymbolHandle,
    implementation: &[typed_trees::types::TypeReferenceHandle],
    receiving: &[typed_trees::types::TypeReferenceHandle],
) -> Result<bool, SelectedCallArgumentError> {
    // Reuse the existing admission of actuals, public formals, capture lineage,
    // and repeated supplying operands. A referenced body need not belong to
    // the selecting map, and equal repeated bindings remain valid.
    selected_call_arguments(program, caller, site)?;
    let dispatch = site
        .static_requirement_dispatch()
        .ok_or("result domain comparison has no selected application")?;
    let applications = program.selected_conformance_applications(
        caller,
        dispatch.selection_application,
        dispatch.evidence_parameter,
        dispatch.requirement,
        dispatch.application_report_fingerprint,
        dispatch.application_commitment,
    );
    let [application] = applications.as_slice() else {
        return Err("result domain comparison has no unique supplying application".into());
    };
    let target = site.target_symbol(program);
    let lineage = program
        .realization_entry_lineage(target, application)
        .ok_or("result domain comparison lost its realization lineage")?;
    let authored_application = match site {
        CallSite::Expression { call, .. } => call.authored_machine_application,
        CallSite::Statement(call) => call.authored_machine_application,
        CallSite::TransitionNamed { .. } => Default::default(),
    };
    let correspondence = CarrierCorrespondence::new(
        program,
        caller,
        dispatch.selection_application,
        application,
        &lineage,
        authored_application,
    )?;
    let lifetimes = correspondence.result_lifetimes(program)?;
    let scopes = correspondence.result_scopes(&lifetimes);
    let definition = program
        .domain_definition(domain_symbol)
        .ok_or("result domain comparison lost its domain declaration")?;
    let parameters = typed_trees::domain::index_parameters(program, definition);
    if parameters.len() != implementation.len() || parameters.len() != receiving.len() {
        return Err("result domain comparison changed its complete index roster".into());
    }
    if parameters.is_empty() {
        // Exact row equality was checked by the publisher. An atomic domain
        // has no call operands that could justify a different identity.
        return Ok(false);
    }
    for ((parameter, own), received) in parameters.iter().zip(implementation).zip(receiving) {
        let (left, right) = match parameter.kind {
            TypeParameterKind::Type => (
                applied_type_identity_in_scope(
                    program,
                    *own,
                    target,
                    correspondence.supplier,
                    &scopes,
                ),
                applied_type_identity(program, *received, caller, &[]),
            ),
            TypeParameterKind::Const { .. } => (
                applied_index_identity(
                    program,
                    IndexIdentityArgument::TypeReference(*own),
                    correspondence.supplier,
                    &scopes,
                ),
                applied_index_identity(
                    program,
                    IndexIdentityArgument::TypeReference(*received),
                    caller,
                    &[],
                ),
            ),
            _ => return Err("result domain has an ineligible index binder".into()),
        };
        let left = left.ok_or("implementation result index has no applied identity")?;
        let right = right.ok_or("receiving result index has no caller identity")?;
        if left != right {
            return Ok(false);
        }
    }
    Ok(true)
}
