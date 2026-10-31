//! Source-authored layout policy evaluation and validation.
//!
//! Reflect the schema, admit and execute its policy, then validate the returned
//! layout and private callback demands.

use typed_trees::TypedTrees;

use super::schema_reflection::{build_schema_value, schema_fields};
use crate::BuildTimeAdmissionPlan;
use crate::build_time_evaluation::type_reference_table;

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
        None,
        &Default::default(),
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
    shared_admission: Option<&BuildTimeAdmissionPlan>,
    accounting: &crate::evaluation_accounting::EvaluationAccounting,
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

    let owned_admission;
    let admission = match shared_admission {
        Some(admission) => admission,
        None => {
            owned_admission = BuildTimeAdmissionPlan::infer(typed, selection_authority);
            &owned_admission
        }
    };
    match custody {
        Some(custody) => admission.require_common_floor_for_invocation(typed, machine, custody)?,
        None => admission.require_common_floor(typed, machine)?,
    }

    let evaluation = accounting
        .evaluate(
            checked_interpreter::BuildMachineProgram::Typed(typed),
            super::policy_arguments::policy_request(typed, machine, vec![schema_value])?,
        )
        .map_err(|reason| {
            format!("build-time evaluation of `{policy_machine}` failed: {reason}")
        })?;

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
        None,
    )?;
    Ok(NativeLayoutPlanReport {
        layout,
        private_callback_demands,
    })
}

pub(crate) fn normalize_private_callback_demands(
    typed: &TypedTrees,
    policy_machine: &typed_trees::machine::Machine,
    receipts: &[checked_interpreter::PrivateLayoutPlacementReceipt],
    policy_name: &str,
    selected_implementation: Option<(
        &typed_trees::expression::StaticMachineArgument,
        symbols::SymbolHandle,
    )>,
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
        let arguments = type_reference_table(typed).type_reference_handles(conformance.arguments);
        let [_semantic_argument, layout_argument, requirement_argument] = arguments else {
            return Err(format!(
                "private layout conformance `{}` must retain semantic type, fixed-layout implementation and callback requirement",
                selected.display_name()
            ));
        };
        let closed_application =
            validation::conformance_applications::close_conformance_application(typed, selected)
                .map_err(|diagnostic| diagnostic.to_string())?;
        let (active, active_owner) = selected_implementation.ok_or_else(||
            "private callback placement requires the exact selected fixed-layout implementation application".to_owned())?;
        let expected = closed_trait_argument_identity(
            typed,
            &closed_application,
            *layout_argument,
            policy_machine.symbol,
        )?;
        let actual = typed_trees::type_identity::portable_applied_index_identity(
            typed,
            typed_trees::type_identity::IndexIdentityArgument::StaticArgument(active),
            active_owner,
            &[],
        )
        .ok_or("active layout policy has no complete portable implementation identity")?;
        if expected != actual {
            return Err(format!(
                "policy `{policy_name}` selected private slot `{}` for a different exact fixed-layout implementation application",
                selected.display_name()
            ));
        }
        let layout_subject_identity = typed
            .type_identity_text(actual)
            .ok_or("active layout application lost its portable identity")?;
        let typed_trees::types::TypeReferenceNode::Named {
            symbol: requirement,
            ..
        } = type_reference_table(typed).type_reference(*requirement_argument)
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
            layout_subject_identity,
            callback_requirement_identity: typed
                .trait_requirement_callable_identity_text(requirement_trait, requirement_row),
            offset: receipt.offset,
        });
    }
    normalized.sort_unstable_by(|left, right| left.slot_identity.cmp(&right.slot_identity));
    Ok(normalized)
}

/// Interpret the relationship's actual argument under its own complete selected
/// telescope. Equal report strings cannot substitute a different map capture.
fn closed_trait_argument_identity(
    typed: &TypedTrees,
    application: &typed_trees::typed_trees::ClosedConformanceApplication,
    argument: typed_trees::types::TypeReferenceHandle,
    owner: symbols::SymbolHandle,
) -> Result<typed_trees::type_identity::TypeIdentityId, String> {
    use typed_trees::type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument};
    let declaration = typed
        .conformance_by_symbol(application.declaration)
        .ok_or("private layout slot lost its exact map declaration")?;
    let substitutions = typed
        .conformance_type_parameters(declaration)
        .iter()
        .zip(&application.arguments)
        .map(|(parameter, argument)| {
            (
                parameter.symbol,
                IndexIdentityArgument::StaticArgument(argument),
            )
        })
        .collect::<Vec<_>>();
    let lifetimes = declaration
        .lifetime_parameters
        .iter()
        .zip(&application.lifetime_arguments)
        .map(|(parameter, actual)| (parameter.as_str().to_owned(), actual.clone()))
        .collect::<Vec<_>>();
    typed_trees::type_identity::portable_applied_index_identity(
        typed,
        // This authenticated formal selects implementation code. Read its
        // carrier/code role, just as the active static operand does; a named
        // value-index identity would describe a different kind of operand.
        IndexIdentityArgument::TypeArgument(argument),
        owner,
        &[AppliedTypeIdentityScope {
            declaration: declaration.symbol,
            arguments: &substitutions,
            lifetimes: &lifetimes,
        }],
    )
    .ok_or("private layout slot has no complete implementation argument identity".into())
}
