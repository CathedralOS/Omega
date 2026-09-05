//! Exact opaque selection/use joins with all replay receipts removed afterward.

use crate::capture::representation::physical_contract::{
    project_representation_copy_disposition, project_representation_lifecycle,
    project_representation_origin, project_value_placement,
};
use crate::capture::semantics::conformances::project_checked_conformance_policy;
use crate::capture::semantics::declarations::{nominal_identity, nominal_owner};
use crate::record::{
    PackagePolicyCallingOpaqueUse, PackageReviewNominalOwner,
    PackageReviewOpaqueRepresentationMovementRole, PackageReviewOpaqueRepresentationOccurrence,
    PackageReviewOpaqueRepresentationPathElement,
};
use omega_calling_conventions::ValidatedBoundaryEntryPlan;
use omega_compiler::CheckedCompilation;
use omega_provider_planning::calling_policy_plans::{
    BoundaryCallingPlanRealization, BoundaryOpaqueRepresentationMovementRole,
    BoundaryOpaqueRepresentationPathElement, BoundaryOpaqueRepresentationUse,
};
use omega_representation_planning::OpaqueRepresentationSelection;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::name::Identifier;

pub(super) fn project(
    compilation: &CheckedCompilation,
    realization: &BoundaryCallingPlanRealization,
    validated: &ValidatedBoundaryEntryPlan,
    lifetime_binders: &[Identifier],
) -> Result<Vec<PackagePolicyCallingOpaqueUse>, Vec<Diagnostic>> {
    if validated.plan() != realization.exact_boundary_entry_plan()
        || validated.plan() != &realization.boundary_entry_plan
    {
        return Err(rejected("a different boundary entry plan"));
    }
    let signature = realization.materialized_signature();
    let uses = signature.opaque_representation_uses();
    if uses.is_empty() {
        return Ok(Vec::new());
    }
    let retained = compilation.opaque_representation_selections();
    let selections = omega_representation_planning::rederive_opaque_representation_selections(
        &compilation.typed,
        retained
            .first()
            .map(OpaqueRepresentationSelection::selecting_machine),
        retained,
    )?;
    let mut opaque_symbols = Vec::new();
    let mut rows = Vec::new();
    for use_ in uses {
        if opaque_symbols.contains(&use_.opaque()) {
            continue;
        }
        opaque_symbols.push(use_.opaque());
        let mut matches = selections
            .iter()
            .filter(|selection| selection.opaque() == use_.opaque());
        let selection = matches
            .next()
            .ok_or_else(|| rejected("a missing selected representation"))?;
        if matches.next().is_some() {
            return Err(rejected("ambiguous selected representations"));
        }
        let selection_owner = nominal_owner(compilation, selection.selecting_machine())?;
        if selection_owner == PackageReviewNominalOwner::Unresolved {
            return Err(rejected("an unresolved selection owner"));
        }
        let mut occurrences = Vec::new();
        for use_ in uses
            .iter()
            .filter(|candidate| candidate.opaque() == selection.opaque())
        {
            validate_selection_use(selection, use_)?;
            let movement = signature
                .opaque_representation_movement(use_, validated)
                .map_err(|reason| rejected(&format!("an invalid occurrence movement: {reason}")))?;
            occurrences.push(PackageReviewOpaqueRepresentationOccurrence {
                carrier_shape_root: use_.shape_root(),
                role: match movement.role() {
                    BoundaryOpaqueRepresentationMovementRole::Parameter {
                        formal_ordinal,
                        native_ordinal,
                    } => PackageReviewOpaqueRepresentationMovementRole::Parameter {
                        formal_ordinal,
                        native_ordinal,
                    },
                    BoundaryOpaqueRepresentationMovementRole::Result => {
                        PackageReviewOpaqueRepresentationMovementRole::Result
                    }
                },
                path: movement
                    .path()
                    .iter()
                    .map(|element| match element {
                        BoundaryOpaqueRepresentationPathElement::FixedArrayElement => {
                            PackageReviewOpaqueRepresentationPathElement::FixedArrayElement
                        }
                        BoundaryOpaqueRepresentationPathElement::RecordField { ordinal } => {
                            PackageReviewOpaqueRepresentationPathElement::RecordField {
                                ordinal: *ordinal,
                            }
                        }
                    })
                    .collect(),
                placement: project_value_placement(movement.placement()),
            });
        }
        occurrences.sort();
        if occurrences.is_empty() || occurrences.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(rejected("missing or repeated exact occurrences"));
        }
        rows.push(PackagePolicyCallingOpaqueUse {
            opaque: nominal_identity(compilation, selection.opaque())?,
            carrier: nominal_identity(compilation, selection.carrier())?,
            selection_owner,
            application: project_checked_conformance_policy(
                compilation,
                selection.application(),
                lifetime_binders,
            )?,
            origin: project_representation_origin(selection.origin()),
            lifecycle: project_representation_lifecycle(selection.lifecycle()),
            copy_disposition: project_representation_copy_disposition(selection.copy_disposition()),
            occurrences,
        });
    }
    rows.sort();
    Ok(rows)
}

fn validate_selection_use(
    selection: &OpaqueRepresentationSelection,
    use_: &BoundaryOpaqueRepresentationUse,
) -> Result<(), Vec<Diagnostic>> {
    if use_.opaque() != selection.opaque()
        || use_.conformance() != selection.application().declaration
        || use_.carrier() != selection.carrier()
        || use_.application_report_fingerprint() != selection.application().report_fingerprint
        || use_.conformance_application_commitment()
            != selection.application().commitment.as_bytes()
        || use_.representation_schema_version() != selection.schema_version()
        || use_.origin() != selection.origin()
        || use_.lifecycle() != selection.lifecycle()
        || use_.copy_disposition() != selection.copy_disposition()
        || use_.selected_application_commitment() != selection.selected_application_commitment()
    {
        return Err(rejected(
            "stale or mismatched selected representation custody",
        ));
    }
    Ok(())
}

fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "calling opaque policy cannot retain {reason}"
    ))]
}
