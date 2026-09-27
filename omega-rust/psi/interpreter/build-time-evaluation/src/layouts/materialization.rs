//! Typed owned values to exact layout bytes, with independently replayable custody.
//!
//! The existing record, sum, and nested-record materializers retain their own
//! admission restrictions. All share the schema reflection and byte encoder;
//! layout policy evaluation is owned by the sibling `layout_plans` module.

pub use checked_interpreter::BuildTimeValue;
use checked_interpreter::{BuildMachineEvaluationRequest, BuildTimeOperationEvaluation};
use layout_plans::{
    AggregateFieldSchema, AggregateFieldValue, ByteOrder, LayoutPlanReport,
    MaterializationDiagnostic, materialize_aggregate_layout_into,
};
use typed_trees::TypedTrees;

use super::schema_reflection::{
    RepeatedFieldInfo, checked_align_up, normalized_schema_report_fingerprint, primitive_byte_size,
    reflected_field_layout, reflected_nested_member_layout, schema_fields,
};
use crate::BuildTimeAdmissionPlan;

mod const_materializable;
mod const_record_with_nested_sum_materializable;
mod const_record_with_sum_materializable;
mod const_sum_materializable;
mod owned_value_encoding;

pub use const_materializable::{
    ValidatedConstMaterialization, validate_const_materializable_typed_owned_layout,
};
pub use const_record_with_nested_sum_materializable::{
    ValidatedConstNestedSumRecordOccurrenceMaterialization,
    ValidatedConstRecordArrayElementSelection, ValidatedConstRecordArrayFieldMaterialization,
    ValidatedConstRecordSumChildMaterialization,
    ValidatedConstRecordWithNestedSumRecordMaterialization,
    ValidatedConstRecordWithNestedSumRecordsMaterialization,
    ValidatedConstRecordWithRecursiveNestedSumsMaterialization,
    validate_const_materializable_record_with_nested_sum_record,
    validate_const_materializable_record_with_nested_sum_records,
    validate_const_materializable_record_with_recursive_nested_sums,
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
        .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
    {
        return Err(MaterializationDiagnostic(format!(
            "typed owned materialization does not yet admit sum cases in `{schema_data}`"
        )));
    }
    let physical_fields = members
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if !field.relevance.is_erased() => {
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
        let typed_trees::data::DataMember::Field(field) = member else {
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
    materialize_aggregate_layout_into(layout, &schemas, &values, byte_order, destination)
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
    let machine = typed.realized_machine_named(value_machine).ok_or_else(|| {
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
    BuildTimeAdmissionPlan::infer(typed, None)
        .require_common_floor(typed, machine)
        .map_err(|rejection| MaterializationDiagnostic(rejection.reason))?;
    let value = checked_interpreter::evaluate_build_time_machine(
        typed,
        BuildMachineEvaluationRequest::named(value_machine, vec![]),
    )
    .map(BuildTimeOperationEvaluation::into_value)
    .map_err(|reason| {
        MaterializationDiagnostic(format!(
            "build-time evaluation of typed owned value `{value_machine}` failed: {reason}"
        ))
    })?;
    materialize_typed_owned_layout_into(typed, schema_data, layout, &value, byte_order, destination)
}
