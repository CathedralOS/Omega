//! Source-authored layout policy evaluation and validation.
//!
//! Reflect the schema, admit and execute its policy, then validate the returned
//! layout and private callback demands.

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

/// Evaluate one exact checked policy body. A family interface is never replaced
/// with a selected member here; the application owner evaluates its full roster.
pub fn compute_native_layout_plan_for_machine(
    typed: &TypedTrees,
    machine_symbol: symbols::SymbolHandle,
    schema_data: &str,
    selection: Option<crate::AuthorizedSelection>,
) -> Result<NativeLayoutPlanReport, String> {
    let (fields, fingerprint) = schema_fields(typed, schema_data)?;
    compute_native_layout_plan_with_schema(
        typed,
        machine_symbol,
        schema_data,
        &fields,
        fingerprint,
        selection,
    )
}

/// The family installer captures this exact field slice as the fold input and
/// uses it for every member, rather than reconstructing arguments per member.
pub(super) fn compute_native_layout_plan_with_schema(
    typed: &TypedTrees,
    machine_symbol: symbols::SymbolHandle,
    schema_data: &str,
    schema_fields: &[super::schema_reflection::SchemaFieldInfo],
    schema_report_fingerprint: u64,
    selection: Option<crate::AuthorizedSelection>,
) -> Result<NativeLayoutPlanReport, String> {
    let machine = typed
        .machine_by_symbol(machine_symbol)
        .ok_or_else(|| "layout policy lost its exact machine declaration".to_owned())?;
    let policy_machine = machine.name.as_str();
    let (selection_authority, custody) = match selection {
        Some(selection) => (Some(selection.authority), Some(selection.custody)),
        None => (None, None),
    };
    let schema_value = build_schema_value(typed, schema_data, schema_fields)?;

    let admission = BuildTimeAdmissionPlan::infer(typed, selection_authority);
    match custody {
        Some(custody) => admission.require_common_floor_for_invocation(typed, machine, custody)?,
        None => admission.require_common_floor(typed, machine)?,
    }

    let evaluation = checked_interpreter::evaluate_build_time_machine(
        typed,
        super::policy_arguments::policy_request(typed, machine, vec![schema_value])?,
    )
    .map_err(|reason| format!("build-time evaluation of `{policy_machine}` failed: {reason}"))?;

    let layout = validate_plan(
        evaluation.value(),
        schema_fields,
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
            .conformance_by_symbol(selected.symbol)
            .ok_or_else(|| {
                format!(
                    "policy `{policy_name}` selected private layout conformance `{}` whose declaration was not retained",
                    selected.display_name()
                )
            })?;
        let trait_definition = typed
            .trait_definition_by_symbol(conformance.trait_symbol)
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
                "private layout conformance `{}` must implement core `PrivateCallbackSlot<Layout, Requirement>`",
                selected.display_name()
            ));
        }
        // `PrivateCallbackSlot<Layout, machine Requirement>`: the layout policy
        // type is an ordinary trait argument, and only that policy's own plan
        // may place the slot.
        let arguments = typed
            .type_reference_table
            .type_reference_handles(conformance.arguments);
        let [layout_argument, requirement_argument] = arguments else {
            return Err(format!(
                "private layout conformance `{}` must carry exactly one layout argument and one callback-requirement argument",
                selected.display_name()
            ));
        };
        let layout = typed.type_reference_table.type_symbol(*layout_argument);
        if !layout.is_valid() || layout != policy_machine.attached_data_symbol {
            return Err(format!(
                "policy `{policy_name}` selected private slot `{}` for layout `{}`, but the active layout producer is attached to `{}`",
                selected.display_name(),
                typed.display_type_reference(*layout_argument),
                typed.symbols.name(policy_machine.attached_data_symbol),
            ));
        }
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
            .trait_definition_by_symbol(requirement_parent)
            .expect("boundary callback owner was found above");
        let requirement_row = typed
            .trait_machine_signature_by_symbol(requirement_trait, *requirement)
            .ok_or_else(|| {
                format!(
                    "private layout conformance `{}` callback requirement was not retained in its declaring trait",
                    selected.display_name()
                )
            })?;

        let closed_application =
            validation::conformance_applications::close_conformance_application(typed, selected)
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
            layout_subject_identity: typed.normalized_hermetic_symbol_identity(layout)?,
            callback_requirement_identity: typed
                .trait_requirement_callable_identity_text(requirement_trait, requirement_row),
            offset: receipt.offset,
        });
    }
    normalized.sort_unstable_by(|left, right| left.slot_identity.cmp(&right.slot_identity));
    Ok(normalized)
}
