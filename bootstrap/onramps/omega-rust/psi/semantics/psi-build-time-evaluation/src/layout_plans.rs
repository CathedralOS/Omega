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
use psi_typed_trees::TypedTrees;

use crate::BuildTimeAdmissionPlan;

mod owned_value_encoding;
mod plan_validation;
mod schema_reflection;
mod schema_value;

use owned_value_encoding::{encode_typed_owned_value, exact_struct_fields};
pub(crate) use plan_validation::validate_plan;
pub use schema_reflection::normalized_schema_identity;
#[allow(unused_imports)]
pub(crate) use schema_reflection::{RepeatedFieldInfo, SchemaFieldInfo, schema_fields};
use schema_reflection::{
    checked_align_up, primitive_byte_size, reflected_field_layout, reflected_nested_member_layout,
};
pub(crate) use schema_value::build_schema_value;
use schema_value::field_key;

const SCHEMA_FIELD_CAPACITY: usize = 32;

pub fn compute_layout_plan(
    typed: &TypedTrees,
    policy_machine: &str,
    schema_data: &str,
) -> Result<LayoutPlanReport, String> {
    compute_layout_plan_with_optional_authority(typed, policy_machine, schema_data, None, None)
}

pub fn compute_layout_plan_with_authority(
    typed: &TypedTrees,
    policy_machine: &str,
    schema_data: &str,
    selection_authority: std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>,
    custody: crate::BuildTimeInvocationCustody,
) -> Result<LayoutPlanReport, String> {
    compute_layout_plan_with_optional_authority(
        typed,
        policy_machine,
        schema_data,
        Some(selection_authority),
        Some(custody),
    )
}

fn compute_layout_plan_with_optional_authority(
    typed: &TypedTrees,
    policy_machine: &str,
    schema_data: &str,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    custody: Option<crate::BuildTimeInvocationCustody>,
) -> Result<LayoutPlanReport, String> {
    let (schema_fields, schema_identity) = schema_fields(typed, schema_data)?;
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

    let plan = psi_checked_interpreter::evaluate_build_time_machine(
        typed,
        policy_machine,
        vec![schema_value],
    )
    .map_err(|reason| format!("build-time evaluation of `{policy_machine}` failed: {reason}"))?;

    validate_plan(&plan, &schema_fields, schema_identity, policy_machine)
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
    if layout.schema_identity != normalized_schema_identity(typed, data) {
        return Err(MaterializationDiagnostic(format!(
            "layout schema identity does not match typed data `{schema_data}`"
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
    use super::{normalized_schema_identity, schema_fields};
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn semantic_schema_identity_distinguishes_common_and_payload_field_relevance() {
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
            normalized_schema_identity(&typed, data)
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
