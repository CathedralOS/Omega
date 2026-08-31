//! Programmable-layout evaluation and validation.
//!
//! The compiler materializes `Schema`, invokes a build-time-admissible policy
//! machine at build time, and validates the returned `Plan` before any consumer
//! trusts it. Plans are keyed by compiler-issued field identities rather than
//! array position, so policies may reorder entries or fragment one logical
//! field.

pub use psi_checked_interpreter::BuildTimeValue;
use psi_layout_plans::{
    AggregateFieldSchema, AggregateFieldValue, ByteOrder, MaterializationDiagnostic,
    materialize_aggregate_layout_into,
};
#[allow(unused_imports)]
pub use psi_layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};
pub use psi_layout_plans::{NativeLayoutPlanReport, PrivateCallbackLayoutDemandReport};
use psi_typed_trees::TypedTrees;

use crate::BuildTimeAdmissionPlan;

mod const_materializable;
mod const_record_with_nested_sum_materializable;
mod const_record_with_sum_materializable;
mod const_sum_materializable;
mod owned_value_encoding;
mod plan_validation;
mod schema_reflection;
mod schema_value;

pub use const_materializable::{
    ValidatedConstMaterialization, validate_const_materializable_typed_owned_layout,
};
pub use const_record_with_nested_sum_materializable::{
    ValidatedConstDepthFourNestedSumOccurrenceMaterialization,
    ValidatedConstDepthThreeNestedSumOccurrenceMaterialization,
    ValidatedConstDepthTwoNestedSumOccurrenceMaterialization,
    ValidatedConstNestedSumRecordOccurrenceMaterialization,
    ValidatedConstRecordWithDepthFourNestedSumsMaterialization,
    ValidatedConstRecordWithDepthThreeNestedSumMaterialization,
    ValidatedConstRecordWithDepthThreeNestedSumsMaterialization,
    ValidatedConstRecordWithDepthTwoNestedSumMaterialization,
    ValidatedConstRecordWithDepthTwoNestedSumsMaterialization,
    ValidatedConstRecordWithNestedSumRecordMaterialization,
    ValidatedConstRecordWithNestedSumRecordsMaterialization,
    validate_const_materializable_record_with_depth_four_nested_sums,
    validate_const_materializable_record_with_depth_three_nested_sum,
    validate_const_materializable_record_with_depth_three_nested_sums,
    validate_const_materializable_record_with_depth_two_nested_sum,
    validate_const_materializable_record_with_depth_two_nested_sums,
    validate_const_materializable_record_with_nested_sum_record,
    validate_const_materializable_record_with_nested_sum_records,
};
pub use const_record_with_sum_materializable::{
    ValidatedConstRecordSumArrayElementMaterialization,
    ValidatedConstRecordSumArrayElementSelection, ValidatedConstRecordSumArrayFieldMaterialization,
    ValidatedConstRecordSumFieldMaterialization, ValidatedConstRecordWithSumArrayMaterialization,
    ValidatedConstRecordWithSumArraysMaterialization, ValidatedConstRecordWithSumMaterialization,
    validate_const_materializable_record_with_conventional_sum,
    validate_const_materializable_record_with_conventional_sum_array,
    validate_const_materializable_record_with_conventional_sum_arrays,
    validate_const_materializable_record_with_conventional_sums,
};
pub use const_sum_materializable::{
    ValidatedConstSumMaterialization, validate_const_materializable_conventional_sum,
};
use owned_value_encoding::{encode_typed_owned_value, exact_struct_fields};
pub(crate) use plan_validation::validate_plan;
pub use schema_reflection::normalized_schema_report_fingerprint;
#[allow(unused_imports)]
pub(crate) use schema_reflection::{RepeatedFieldInfo, SchemaFieldInfo, schema_fields};
use schema_reflection::{
    checked_align_up, primitive_byte_size, reflected_field_layout, reflected_nested_member_layout,
};
pub(crate) use schema_value::{build_schema_value, local_schema_field_discriminator};

const SCHEMA_FIELD_CAPACITY: usize = 32;

pub fn compute_layout_plan(
    typed: &TypedTrees,
    policy_machine: &str,
    schema_data: &str,
) -> Result<LayoutPlanReport, String> {
    let report = compute_native_layout_plan_with_optional_authority(
        typed,
        policy_machine,
        schema_data,
        None,
        None,
    )?;
    if !report.private_callback_demands.is_empty() {
        return Err(format!(
            "policy `{policy_machine}` produced private native-layout demands; consume it through the native layout-plan path so those demands cannot be discarded"
        ));
    }
    Ok(report.layout)
}

pub fn compute_layout_plan_with_authority(
    typed: &TypedTrees,
    policy_machine: &str,
    schema_data: &str,
    selection_authority: std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>,
    custody: crate::BuildTimeInvocationCustody,
) -> Result<LayoutPlanReport, String> {
    let report = compute_native_layout_plan_with_optional_authority(
        typed,
        policy_machine,
        schema_data,
        Some(selection_authority),
        Some(custody),
    )?;
    if !report.private_callback_demands.is_empty() {
        return Err(format!(
            "policy `{policy_machine}` produced private native-layout demands; consume it through the native layout-plan path so those demands cannot be discarded"
        ));
    }
    Ok(report.layout)
}

/// Evaluate one native layout policy while retaining source-authored private
/// callback destinations. The demands are target-neutral here: the selected
/// calling-plan realization later supplies callback-address size/alignment and
/// proves final bounds/non-overlap.
pub fn compute_native_layout_plan(
    typed: &TypedTrees,
    policy_machine: &str,
    schema_data: &str,
) -> Result<NativeLayoutPlanReport, String> {
    compute_native_layout_plan_with_optional_authority(
        typed,
        policy_machine,
        schema_data,
        None,
        None,
    )
}

pub fn compute_native_layout_plan_with_authority(
    typed: &TypedTrees,
    policy_machine: &str,
    schema_data: &str,
    selection_authority: std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>,
    custody: crate::BuildTimeInvocationCustody,
) -> Result<NativeLayoutPlanReport, String> {
    compute_native_layout_plan_with_optional_authority(
        typed,
        policy_machine,
        schema_data,
        Some(selection_authority),
        Some(custody),
    )
}

fn compute_native_layout_plan_with_optional_authority(
    typed: &TypedTrees,
    policy_machine: &str,
    schema_data: &str,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    custody: Option<crate::BuildTimeInvocationCustody>,
) -> Result<NativeLayoutPlanReport, String> {
    let (schema_fields, schema_report_fingerprint) = schema_fields(typed, schema_data)?;
    let schema_value = build_schema_value(typed, schema_data, &schema_fields)?;

    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == policy_machine)
        .ok_or_else(|| format!("no machine named `{policy_machine}` exists"))?;
    let admission =
        BuildTimeAdmissionPlan::infer_with_selection_authority(typed, selection_authority);
    match custody {
        Some(custody) => admission.require_common_floor_for_invocation(typed, machine, custody)?,
        None => admission.require_common_floor(typed, machine)?,
    }

    let evaluation = psi_checked_interpreter::evaluate_build_time_machine_with_operation_receipts(
        typed,
        policy_machine,
        vec![schema_value],
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
    policy_machine: &psi_typed_trees::machine::Machine,
    receipts: &[psi_checked_interpreter::PrivateLayoutPlacementReceipt],
    policy_name: &str,
) -> Result<Vec<PrivateCallbackLayoutDemandReport>, String> {
    let mut normalized = Vec::with_capacity(receipts.len());
    for receipt in receipts {
        let selected = &receipt.selected_slot;
        if selected.const_literal.is_some()
            || selected.evidence_projection.is_some()
            || !selected.symbol.is_valid()
            || typed.symbols.get(selected.symbol).kind != psi_symbols::SymbolKind::Conformance
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
                != Some(psi_source::SourceOrigin::Toolchain)
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
        let psi_typed_trees::types::TypeReferenceNode::Named {
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
            || typed.symbols.get(*requirement).kind != psi_symbols::SymbolKind::State
        {
            return Err(format!(
                "private layout conformance `{}` callback argument is not one exact trait requirement",
                selected.display_name()
            ));
        }
        let requirement_parent = typed.symbols.get(*requirement).parent;
        if !requirement_parent.is_valid()
            || typed.symbols.get(requirement_parent).kind != psi_symbols::SymbolKind::Trait
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
            psi_typed_trees_to_checked_trees::close_conformance_application(typed, selected)
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

/// Materializes one compiler-checked, source-owned record through a normalized
/// layout. Psi supplies the typed semantic value and derives every native field
/// extent; the Omega realization seam supplies byte order. Source code never
/// supplies physical field bytes or offsets.
pub fn materialize_typed_owned_layout_into(
    typed: &TypedTrees,
    schema_data: &str,
    layout: &LayoutPlanReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    destination: &mut [u8],
) -> Result<(), MaterializationDiagnostic> {
    let data = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == schema_data)
        .ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "no typed data definition named `{schema_data}` exists"
            ))
        })?;
    if layout.schema_report_fingerprint != normalized_schema_report_fingerprint(typed, data) {
        return Err(MaterializationDiagnostic(format!(
            "layout schema report fingerprint does not match typed data `{schema_data}`"
        )));
    }
    let (schema_fields, _) =
        schema_fields(typed, schema_data).map_err(MaterializationDiagnostic)?;
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(MaterializationDiagnostic(format!(
            "typed owned value for `{schema_data}` is not a record"
        )));
    };
    if type_name != schema_data {
        return Err(MaterializationDiagnostic(format!(
            "typed owned value `{type_name}` does not match schema `{schema_data}`"
        )));
    }
    let supplied = exact_struct_fields(schema_data, fields)?;
    let members = typed.data_members(data);
    if members
        .iter()
        .any(|member| matches!(member, psi_typed_trees::data::DataMember::Variant(_)))
    {
        return Err(MaterializationDiagnostic(format!(
            "typed owned materialization does not yet admit sum cases in `{schema_data}`"
        )));
    }
    let physical_fields = members
        .iter()
        .filter_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) if !field.relevance.is_erased() => {
                Some(field)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if supplied.len() != members.len() {
        return Err(MaterializationDiagnostic(format!(
            "typed owned value `{schema_data}` has {} fields, expected {}",
            supplied.len(),
            members.len()
        )));
    }
    for member in members {
        let psi_typed_trees::data::DataMember::Field(field) = member else {
            unreachable!("sum cases rejected above")
        };
        if !supplied.contains_key(field.name.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "typed owned value `{schema_data}` has no field `{}`",
                field.name
            )));
        }
    }

    let mut schemas = Vec::with_capacity(schema_fields.len());
    let mut values = Vec::with_capacity(schema_fields.len());
    for field in &physical_fields {
        if schema_fields
            .iter()
            .any(|reflected| reflected.name == field.name.as_str())
        {
            continue;
        }
        let field_value = supplied.get(field.name.as_str()).ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "typed owned value `{schema_data}` has no field `{}`",
                field.name
            ))
        })?;
        let bytes = encode_typed_owned_value(
            typed,
            field.type_reference,
            field_value,
            byte_order,
            &mut Vec::new(),
        )?;
        if !bytes.is_empty() {
            return Err(MaterializationDiagnostic(format!(
                "typed field `{}` was omitted from layout despite deriving {} physical bytes",
                field.name,
                bytes.len()
            )));
        }
    }
    for reflected in &schema_fields {
        let field = physical_fields
            .iter()
            .find(|field| field.name.as_str() == reflected.name)
            .expect("schema reflection retained the same physical fields");
        let field_value = supplied.get(reflected.name.as_str()).ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "typed owned value `{schema_data}` has no field `{}`",
                reflected.name
            ))
        })?;
        let bytes = encode_typed_owned_value(
            typed,
            field.type_reference,
            field_value,
            byte_order,
            &mut Vec::new(),
        )?;
        let byte_size = u64::try_from(bytes.len()).map_err(|_| {
            MaterializationDiagnostic(format!(
                "typed field `{}` extent cannot be represented as u64",
                reflected.name
            ))
        })?;
        if byte_size != reflected.size {
            return Err(MaterializationDiagnostic(format!(
                "typed field `{}` encoded to {byte_size} bytes, expected {}",
                reflected.name, reflected.size
            )));
        }
        schemas.push(match (reflected.repeated, reflected.identity) {
            (Some(repeated), Some(identity)) => AggregateFieldSchema::new_repeated_numbered(
                &reflected.name,
                identity,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (Some(repeated), None) => AggregateFieldSchema::new_repeated(
                &reflected.name,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (None, Some(identity)) => {
                AggregateFieldSchema::new_numbered(&reflected.name, identity, reflected.size)?
            }
            (None, None) => AggregateFieldSchema::new(&reflected.name, reflected.size)?,
        });
        values.push(AggregateFieldValue::new(&reflected.name, bytes)?);
    }
    materialize_aggregate_layout_into(layout, &schemas, &values, destination)
}

/// Evaluates an effect-free, zero-argument source machine to obtain an owned
/// typed value, then materializes it through the selected layout. The compiler
/// owns both the evaluation boundary and type-directed byte derivation.
pub fn evaluate_and_materialize_typed_owned_layout_into(
    typed: &TypedTrees,
    value_machine: &str,
    schema_data: &str,
    layout: &LayoutPlanReport,
    byte_order: ByteOrder,
    destination: &mut [u8],
) -> Result<(), MaterializationDiagnostic> {
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == value_machine)
        .ok_or_else(|| {
            MaterializationDiagnostic(format!("no machine named `{value_machine}` exists"))
        })?;
    let states = typed.machine_states(machine);
    let [entry] = states else {
        return Err(MaterializationDiagnostic(format!(
            "typed owned value machine `{value_machine}` must have one state"
        )));
    };
    if !typed.state_parameters(entry).is_empty() {
        return Err(MaterializationDiagnostic(format!(
            "typed owned value machine `{value_machine}` must take no arguments"
        )));
    }
    BuildTimeAdmissionPlan::infer(typed)
        .require_common_floor(typed, machine)
        .map_err(MaterializationDiagnostic)?;
    let value = psi_checked_interpreter::evaluate_build_time_machine(typed, value_machine, vec![])
        .map_err(|reason| {
            MaterializationDiagnostic(format!(
                "build-time evaluation of typed owned value `{value_machine}` failed: {reason}"
            ))
        })?;
    materialize_typed_owned_layout_into(typed, schema_data, layout, &value, byte_order, destination)
}

#[cfg(test)]
mod tests {
    use super::{normalized_schema_report_fingerprint, schema_fields};
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn semantic_schema_report_fingerprint_distinguishes_common_and_payload_field_relevance() {
        let source = r#"
            data CommonRelevant { proof: i32; }
            data CommonErased { proof [erased]: i32; }
            data PayloadRelevant { case Certified(proof: i32); }
            data PayloadErased { case Certified(proof [erased]: i32); }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let identity = |name: &str| {
            let data = typed
                .data_definitions()
                .iter()
                .find(|data| data.name.as_str() == name)
                .expect("data definition");
            normalized_schema_report_fingerprint(&typed, data)
        };

        assert_ne!(identity("CommonRelevant"), identity("CommonErased"));
        assert_ne!(identity("PayloadRelevant"), identity("PayloadErased"));
    }

    #[test]
    fn schema_reflection_rejects_quotients_directly_and_as_nested_records() {
        let source = r#"
            data Carrier { case Unit; }
            proposition same(left: Carrier, right: Carrier) = left == right;
            data Bucket = Carrier % same;
            data Envelope { bucket: Bucket; tag: u8; }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");

        let direct = schema_fields(&typed, "Bucket").expect_err("quotient schema must reject");
        assert!(direct.contains("schema reflection cannot observe quotient `Bucket`"));

        let nested = schema_fields(&typed, "Envelope")
            .expect_err("a nested quotient must not acquire a reflected layout");
        assert!(nested.contains("field `bucket` is neither a supported primitive"));
    }
}
