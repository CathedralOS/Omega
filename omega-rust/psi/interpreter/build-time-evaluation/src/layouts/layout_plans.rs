//! Source-authored layout policy evaluation and validation.
//!
//! Reflect the schema, admit and execute its policy, then validate the returned
//! layout and private callback demands. Typed-value byte production belongs to
//! the sibling `materialization` module.

use checked_interpreter::BuildMachineEvaluationRequest;
use layout_plans::LayoutPlanReport;
use typed_trees::TypedTrees;

use super::schema_reflection::{build_schema_value, schema_fields};
use crate::BuildTimeAdmissionPlan;

mod plan_validation;
pub(crate) use plan_validation::validate_plan;

/// Validated native layout with exact typed custody for each selected slot.
pub type NativeLayoutPlanReport =
    layout_plans::NativeLayoutPlanReport<typed_trees::typed_trees::ClosedConformanceApplication>;
pub type PrivateCallbackLayoutDemandReport = layout_plans::PrivateCallbackLayoutDemandReport<
    typed_trees::typed_trees::ClosedConformanceApplication,
>;

/// Compute the layout plan a policy machine produces for a schema; a plan
/// that also produced private native-layout demands is refused here and
/// must be consumed through [`compute_native_layout_plan`].
pub fn compute_layout_plan(
    typed: &TypedTrees,
    policy_machine: &str,
    schema_data: &str,
    selection: Option<crate::AuthorizedSelection>,
) -> Result<LayoutPlanReport, String> {
    let report = compute_native_layout_plan(typed, policy_machine, schema_data, selection)?;
    if !report.private_callback_demands.is_empty() {
        return Err(format!(
            "policy `{policy_machine}` produced private native-layout demands; consume it through the native layout-plan path so those demands cannot be discarded"
        ));
    }
    Ok(report.layout)
}

/// Compute the native layout plan a policy machine produces for a schema,
/// under an authorized selection when the caller holds one.
pub fn compute_native_layout_plan(
    typed: &TypedTrees,
    policy_machine: &str,
    schema_data: &str,
    selection: Option<crate::AuthorizedSelection>,
) -> Result<NativeLayoutPlanReport, String> {
    let (selection_authority, custody) = match selection {
        Some(selection) => (Some(selection.authority), Some(selection.custody)),
        None => (None, None),
    };
    let (schema_fields, schema_report_fingerprint) = schema_fields(typed, schema_data)?;
    let schema_value = build_schema_value(typed, schema_data, &schema_fields)?;

    let machine = typed
        .realized_machine_named(policy_machine)
        .ok_or_else(|| format!("no machine named `{policy_machine}` exists"))?;
    let admission = BuildTimeAdmissionPlan::infer(typed, selection_authority);
    match custody {
        Some(custody) => admission.require_common_floor_for_invocation(typed, machine, custody)?,
        None => admission.require_common_floor(typed, machine)?,
    }

    let evaluation = checked_interpreter::evaluate_build_time_machine(
        typed,
        BuildMachineEvaluationRequest::named(policy_machine, vec![schema_value]),
    )
    .map_err(|reason| format!("build-time evaluation of `{policy_machine}` failed: {reason}"))?;

    let layout = validate_plan(
        evaluation.value(),
        &schema_fields,
        schema_report_fingerprint,
        policy_machine,
    )?;
    let private_callback_demands = normalize_private_callback_demands(
        typed,
        machine,
        evaluation.private_layout_placements(),
        policy_machine,
    )?;
    Ok(NativeLayoutPlanReport {
        layout,
        private_callback_demands,
    })
}

fn normalize_private_callback_demands(
    typed: &TypedTrees,
    policy_machine: &typed_trees::machine::Machine,
    receipts: &[checked_interpreter::PrivateLayoutPlacementReceipt],
    policy_name: &str,
) -> Result<Vec<PrivateCallbackLayoutDemandReport>, String> {
    let mut normalized = Vec::with_capacity(receipts.len());
    for receipt in receipts {
        let selected = &receipt.selected_slot;
        if selected.const_literal.is_some()
            || selected.evidence_projection.is_some()
            || !selected.symbol.is_valid()
            || typed.symbols.get(selected.symbol).kind != symbols::SymbolKind::Conformance
        {
            return Err(format!(
                "policy `{policy_name}` selected a private layout slot that is not one exact named conformance"
            ));
        }
        let conformance = typed
            .conformances()
            .iter()
            .find(|candidate| candidate.symbol == selected.symbol)
            .ok_or_else(|| {
                format!(
                    "policy `{policy_name}` selected private layout conformance `{}` whose declaration was not retained",
                    selected.display_name()
                )
            })?;
        if conformance.carrier_symbol != policy_machine.attached_data_symbol {
            return Err(format!(
                "policy `{policy_name}` selected private slot `{}` for layout `{}`, but the active layout producer is attached to `{}`",
                selected.display_name(),
                conformance
                    .carrier_name()
                    .map_or("<subjectless>", |name| name.as_str()),
                typed.symbols.name(policy_machine.attached_data_symbol),
            ));
        }
        let trait_definition = typed
            .traits()
            .iter()
            .find(|candidate| candidate.symbol == conformance.trait_symbol)
            .ok_or_else(|| {
                format!(
                    "private layout conformance `{}` lost its exact trait declaration",
                    selected.display_name()
                )
            })?;
        if trait_definition.name.as_str() != "PrivateCallbackSlot"
            || typed.symbols.symbol_source_origin(trait_definition.symbol)
                != Some(source::SourceOrigin::Toolchain)
        {
            return Err(format!(
                "private layout conformance `{}` must implement core `PrivateCallbackSlot<Requirement>`",
                selected.display_name()
            ));
        }
        let arguments = typed
            .type_reference_table
            .type_reference_handles(conformance.arguments);
        let [requirement_argument] = arguments else {
            return Err(format!(
                "private layout conformance `{}` must carry exactly one callback-requirement argument",
                selected.display_name()
            ));
        };
        let typed_trees::types::TypeReferenceNode::Named {
            symbol: requirement,
            ..
        } = typed
            .type_reference_table
            .type_reference(*requirement_argument)
        else {
            return Err(format!(
                "private layout conformance `{}` callback argument is not one exact requirement declaration",
                selected.display_name()
            ));
        };
        if !requirement.is_valid()
            || typed.symbols.get(*requirement).kind != symbols::SymbolKind::State
        {
            return Err(format!(
                "private layout conformance `{}` callback argument is not one exact trait requirement",
                selected.display_name()
            ));
        }
        let requirement_parent = typed.symbols.get(*requirement).parent;
        if !requirement_parent.is_valid()
            || typed.symbols.get(requirement_parent).kind != symbols::SymbolKind::Trait
            || !typed
                .traits()
                .iter()
                .any(|owner| owner.symbol == requirement_parent && owner.is_boundary)
        {
            return Err(format!(
                "private layout conformance `{}` must name one exact boundary-trait callback requirement",
                selected.display_name()
            ));
        }
        let requirement_trait = typed
            .traits()
            .iter()
            .find(|owner| owner.symbol == requirement_parent)
            .expect("boundary callback owner was found above");
        let requirement_row = typed
            .trait_machine_signatures(requirement_trait)
            .iter()
            .find(|row| row.symbol == *requirement)
            .ok_or_else(|| {
                format!(
                    "private layout conformance `{}` callback requirement was not retained in its declaring trait",
                    selected.display_name()
                )
            })?;

        let closed_application =
            typed_trees_to_checked_trees::close_conformance_application(typed, selected)
                .map_err(|diagnostic| diagnostic.to_string())?;
        let slot_identity = format!(
            "{}#{:016x}",
            typed.normalized_hermetic_symbol_identity(selected.symbol)?,
            closed_application.report_fingerprint,
        );
        if normalized
            .iter()
            .any(|existing: &PrivateCallbackLayoutDemandReport| {
                existing.slot_identity == slot_identity
            })
        {
            return Err(format!(
                "policy `{policy_name}` places private callback slot `{}` more than once",
                selected.display_name()
            ));
        }
        normalized.push(PrivateCallbackLayoutDemandReport {
            slot_application: closed_application,
            slot_identity,
            layout_subject_identity: typed
                .normalized_hermetic_symbol_identity(conformance.carrier_symbol)?,
            callback_requirement_identity: typed
                .normalized_trait_requirement_overload_identity(requirement_trait, requirement_row)
                .identity(),
            offset: receipt.offset,
        });
    }
    normalized.sort_unstable_by(|left, right| left.slot_identity.cmp(&right.slot_identity));
    Ok(normalized)
}
