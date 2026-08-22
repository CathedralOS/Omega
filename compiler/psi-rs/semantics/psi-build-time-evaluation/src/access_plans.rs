//! Build-time evaluation of source-authored placed-access policies.
//!
//! Source policy records are ordinary values. This module is the compiler
//! boundary that reflects a schema, evaluates an effect-free `Access::plan` or
//! `Placement::plan` machine, and converts the result into the sealed
//! normalized model consumed by admission and lowering.

use std::collections::BTreeSet;

use psi_access_plans::{
    AccessExposure, AccessPlan, AtomicPermissions, BoundaryReach, BoundaryServiceReachId,
    ExternalRead, FieldAccess, PlacementPlan, ValidatedAccessPlan, ValidatedPlacementPlan,
    validate_access_plan, validate_placement_plan,
};
use psi_checked_interpreter::BuildTimeValue;
use psi_layout_plans::{
    LayoutPlacementReport, LayoutPlanReport, layout_plan_reports_match_for_replay,
};
use psi_typed_trees::TypedTrees;

use crate::BuildTimeAdmissionPlan;
use crate::layout_plans::{SchemaFieldInfo, build_schema_value, schema_fields, validate_plan};

mod layout_value;

use layout_value::build_layout_plan_value;

const ACCESS_FIELD_CAPACITY: usize = 32;
const BOUNDARY_REACH_CAPACITY: usize = 32;

/// Evaluate and validate one source `Access::plan` machine against an already
/// validated layout for the same reflected schema.
pub fn compute_access_plan(
    typed: &TypedTrees,
    policy_machine: &str,
    schema_data: &str,
    layout: &LayoutPlanReport,
) -> Result<ValidatedAccessPlan, String> {
    let (schema_fields, schema_identity) = schema_fields(typed, schema_data)?;
    if layout.schema_identity != schema_identity {
        return Err(format!(
            "validated layout schema identity {} does not match reflected schema `{schema_data}` identity {schema_identity}",
            layout.schema_identity
        ));
    }
    // Reject an aliased or malformed retained field-identity set before
    // canonical replay can replace numbered presentation names.
    AccessPlan::inaccessible(layout).map_err(|diagnostic| {
        format!(
            "layout supplied to access policy `{policy_machine}` is not a canonical field-identity set: {diagnostic}"
        )
    })?;
    let schema_value = build_schema_value(typed, schema_data, &schema_fields)?;
    let layout_value = build_layout_plan_value(layout, &schema_fields)?;
    let canonical_layout = validate_plan(
        &layout_value,
        &schema_fields,
        schema_identity,
        policy_machine,
    )?;
    // Numbered source names are presentation. Acceptance still replays the
    // exact structure rather than trusting its compact report/cache hash.
    if !layout_plan_reports_match_for_replay(&canonical_layout, layout) {
        return Err(format!(
            "layout supplied to access policy `{policy_machine}` is not the canonical validated layout for schema `{schema_data}`"
        ));
    }
    let plan = evaluate_policy(
        typed,
        policy_machine,
        vec![schema_value, layout_value],
        "access",
    )?;
    validate_access_value(&plan, &schema_fields, &canonical_layout, policy_machine)
}

/// Evaluate and validate one source `Placement::plan` machine. The returned
/// layout is validated before its access value is normalized against it; the
/// complete layout/access/reach tuple then receives one placement identity.
pub fn compute_placement_plan(
    typed: &TypedTrees,
    policy_machine: &str,
    schema_data: &str,
) -> Result<ValidatedPlacementPlan, String> {
    let (schema_fields, schema_identity) = schema_fields(typed, schema_data)?;
    let schema_value = build_schema_value(typed, schema_data, &schema_fields)?;
    let plan = evaluate_policy(typed, policy_machine, vec![schema_value], "placement")?;
    let fields = struct_fields(&plan).map_err(|reason| invalid(policy_machine, reason))?;
    let layout_value =
        named_field(fields, "layout").map_err(|reason| invalid(policy_machine, reason))?;
    let layout = validate_plan(
        layout_value,
        &schema_fields,
        schema_identity,
        policy_machine,
    )?;
    let access_value =
        named_field(fields, "access").map_err(|reason| invalid(policy_machine, reason))?;
    let access = validate_access_value(access_value, &schema_fields, &layout, policy_machine)?;
    let reach_value =
        named_field(fields, "reach").map_err(|reason| invalid(policy_machine, reason))?;
    let reach =
        parse_boundary_reach(reach_value).map_err(|reason| invalid(policy_machine, reason))?;

    // Reuse the exact source decisions retained by the validated access plan;
    // only the normalizer can seal them into a placement identity.
    validate_placement_plan(PlacementPlan {
        layout,
        access: access.plan().clone(),
        reach,
    })
    .map_err(|diagnostic| invalid(policy_machine, diagnostic.to_string()))
}

fn evaluate_policy(
    typed: &TypedTrees,
    policy_machine: &str,
    arguments: Vec<BuildTimeValue>,
    policy_kind: &str,
) -> Result<BuildTimeValue, String> {
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == policy_machine)
        .ok_or_else(|| format!("no machine named `{policy_machine}` exists"))?;
    BuildTimeAdmissionPlan::infer(typed).require_common_floor(typed, machine)?;
    psi_checked_interpreter::evaluate_build_time_machine(typed, policy_machine, arguments).map_err(
        |reason| {
            format!(
                "build-time evaluation of {policy_kind} policy `{policy_machine}` failed: {reason}"
            )
        },
    )
}

fn validate_access_value(
    value: &BuildTimeValue,
    schema_fields: &[SchemaFieldInfo],
    layout: &LayoutPlanReport,
    policy_machine: &str,
) -> Result<ValidatedAccessPlan, String> {
    let fields = struct_fields(value).map_err(|reason| invalid(policy_machine, reason))?;
    let field_count = uint_value(
        named_field(fields, "field_count").map_err(|reason| invalid(policy_machine, reason))?,
        "field_count",
    )
    .map_err(|reason| invalid(policy_machine, reason))?;
    if field_count != schema_fields.len() as u64 {
        return Err(invalid(
            policy_machine,
            format!(
                "access field_count is {field_count}, but reflected schema requires exactly {} decisions",
                schema_fields.len()
            ),
        ));
    }
    if field_count > ACCESS_FIELD_CAPACITY as u64 {
        return Err(invalid(
            policy_machine,
            format!(
                "access field_count {field_count} exceeds bootstrap capacity {ACCESS_FIELD_CAPACITY}"
            ),
        ));
    }
    let entries = array_value(
        named_field(fields, "entries").map_err(|reason| invalid(policy_machine, reason))?,
        "entries",
    )
    .map_err(|reason| invalid(policy_machine, reason))?;
    if entries.len() < field_count as usize {
        return Err(invalid(
            policy_machine,
            format!(
                "access field_count is {field_count}, but the plan carries only {} entries",
                entries.len()
            ),
        ));
    }

    let mut normalized =
        AccessPlan::inaccessible(layout).map_err(|diagnostic| diagnostic.to_string())?;
    let mut seen = BTreeSet::new();
    for (entry_index, value) in entries.iter().take(field_count as usize).enumerate() {
        let entry_fields = struct_fields(value).map_err(|reason| {
            invalid(
                policy_machine,
                format!("access entry {entry_index} {reason}"),
            )
        })?;
        let key = uint_value(
            named_field(entry_fields, "key").map_err(|reason| invalid(policy_machine, reason))?,
            "key",
        )
        .map_err(|reason| invalid(policy_machine, reason))?;
        let Some(schema_field) = schema_fields.iter().find(|field| field.key == key) else {
            return Err(invalid(
                policy_machine,
                format!("access entry {entry_index} refers to unknown schema field key {key}"),
            ));
        };
        if !seen.insert(key) {
            return Err(invalid(
                policy_machine,
                format!(
                    "access plan contains more than one decision for field `{}`",
                    schema_field.name
                ),
            ));
        }
        let source_access = named_field(entry_fields, "access")
            .map_err(|reason| invalid(policy_machine, reason))?;
        let access = parse_field_access(source_access, schema_field, layout)
            .map_err(|reason| invalid(policy_machine, reason))?;
        let normalized_key = normalized
            .entries()
            .iter()
            .find(|entry| entry.field() == schema_field.name)
            .map(|entry| entry.key())
            .ok_or_else(|| {
                invalid(
                    policy_machine,
                    format!(
                        "validated layout has no canonical access slot for field `{}`",
                        schema_field.name
                    ),
                )
            })?;
        normalized
            .set(normalized_key, access)
            .map_err(|diagnostic| invalid(policy_machine, diagnostic.to_string()))?;
    }
    if seen.len() != schema_fields.len() {
        let missing = schema_fields
            .iter()
            .filter(|field| !seen.contains(&field.key))
            .map(|field| format!("`{}`", field.name))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(invalid(
            policy_machine,
            format!("access plan omits schema field decisions for {missing}"),
        ));
    }

    validate_access_plan(normalized, layout)
        .map_err(|diagnostic| invalid(policy_machine, diagnostic.to_string()))
}

fn parse_field_access(
    value: &BuildTimeValue,
    schema_field: &SchemaFieldInfo,
    layout: &LayoutPlanReport,
) -> Result<FieldAccess, String> {
    let BuildTimeValue::Case { variant, payload } = value else {
        return Err(format!(
            "access for field `{}` is not a FieldAccess case",
            schema_field.name
        ));
    };
    match short_name(variant) {
        "Inaccessible" => {
            require_empty_payload(payload, "Inaccessible")?;
            Ok(FieldAccess::Inaccessible)
        }
        "Stable" => {
            require_scalar_access_field(schema_field)?;
            let width = transfer_width_bits(schema_field, layout)?;
            Ok(FieldAccess::Stable {
                transfer_width_bits: width,
                read: bool_payload(payload, "read")?,
                write: bool_payload(payload, "write")?,
                exposure: exposure_payload(payload)?,
            })
        }
        "External" => {
            require_scalar_access_field(schema_field)?;
            let width = transfer_width_bits(schema_field, layout)?;
            Ok(FieldAccess::External {
                transfer_width_bits: width,
                read: external_read_payload(payload)?,
                write: bool_payload(payload, "write")?,
                exposure: exposure_payload(payload)?,
            })
        }
        "Atomic" => {
            require_scalar_access_field(schema_field)?;
            let width = transfer_width_bits(schema_field, layout)?;
            let operations = payload_field(payload, "operations")?;
            Ok(FieldAccess::Atomic {
                transfer_width_bits: width,
                operations: parse_atomic_permissions(operations)?,
                exposure: exposure_payload(payload)?,
            })
        }
        other => Err(format!(
            "access for field `{}` uses unknown FieldAccess case `{other}`",
            schema_field.name
        )),
    }
}

fn require_scalar_access_field(schema_field: &SchemaFieldInfo) -> Result<(), String> {
    schema_field.primitive.map(|_| ()).ok_or_else(|| {
        format!(
            "field `{}` is aggregate; the current access vocabulary admits only Inaccessible for aggregate fields",
            schema_field.name
        )
    })
}

fn transfer_width_bits(
    schema_field: &SchemaFieldInfo,
    layout: &LayoutPlanReport,
) -> Result<u16, String> {
    let placements = layout
        .entries
        .iter()
        .filter(|entry| entry.field == schema_field.name)
        .map(|entry| entry.placement)
        .collect::<Vec<_>>();
    let width = match placements.as_slice() {
        [LayoutPlacementReport::At { .. }] => schema_field
            .size
            .checked_mul(8)
            .ok_or_else(|| format!("field `{}` transfer width overflows", schema_field.name))?,
        [LayoutPlacementReport::IntegerAt { stored_width, .. }] => *stored_width,
        [
            LayoutPlacementReport::Bits {
                container_width, ..
            },
            ..,
        ] => *container_width,
        [] => {
            return Err(format!(
                "field `{}` has no validated layout placement",
                schema_field.name
            ));
        }
        _ => schema_field
            .size
            .checked_mul(8)
            .ok_or_else(|| format!("field `{}` transfer width overflows", schema_field.name))?,
    };
    u16::try_from(width).map_err(|_| {
        format!(
            "field `{}` transfer width {width} does not fit the normalized access vocabulary",
            schema_field.name
        )
    })
}

fn parse_atomic_permissions(value: &BuildTimeValue) -> Result<AtomicPermissions, String> {
    let fields = struct_fields(value)?;
    Ok(AtomicPermissions {
        load: bool_named_field(fields, "load")?,
        store: bool_named_field(fields, "store")?,
        fetch_add: bool_named_field(fields, "fetch_add")?,
        fetch_sub: bool_named_field(fields, "fetch_sub")?,
        fetch_xor: bool_named_field(fields, "fetch_xor")?,
        fetch_or: bool_named_field(fields, "fetch_or")?,
        fetch_and: bool_named_field(fields, "fetch_and")?,
        swap: bool_named_field(fields, "swap")?,
        compare_exchange: bool_named_field(fields, "compare_exchange")?,
        compare_exchange_once: bool_named_field(fields, "compare_exchange_once")?,
        try_exchange: bool_named_field(fields, "try_exchange")?,
        try_exchange_once: bool_named_field(fields, "try_exchange_once")?,
    })
}

fn exposure_payload(payload: &[(String, BuildTimeValue)]) -> Result<AccessExposure, String> {
    let value = payload_field(payload, "exposure")?;
    let BuildTimeValue::Case { variant, payload } = value else {
        return Err("FieldAccess exposure is not an Exposure case".into());
    };
    require_empty_payload(payload, short_name(variant))?;
    match short_name(variant) {
        "Exported" => Ok(AccessExposure::Exported),
        "BindingPrivate" => Ok(AccessExposure::BindingPrivate),
        other => Err(format!("unknown Exposure case `{other}`")),
    }
}

fn external_read_payload(payload: &[(String, BuildTimeValue)]) -> Result<ExternalRead, String> {
    let value = payload_field(payload, "read")?;
    let BuildTimeValue::Case { variant, payload } = value else {
        return Err("external read policy is not an ExternalRead case".into());
    };
    require_empty_payload(payload, short_name(variant))?;
    match short_name(variant) {
        "None" => Ok(ExternalRead::None),
        "Read" => Ok(ExternalRead::Read),
        "Take" => Ok(ExternalRead::Take),
        other => Err(format!("unknown ExternalRead case `{other}`")),
    }
}

fn parse_boundary_reach(value: &BuildTimeValue) -> Result<BoundaryReach, String> {
    let fields = struct_fields(value)?;
    let service_count = uint_value(named_field(fields, "service_count")?, "service_count")?;
    if service_count > BOUNDARY_REACH_CAPACITY as u64 {
        return Err(format!(
            "boundary service_count {service_count} exceeds bootstrap capacity {BOUNDARY_REACH_CAPACITY}"
        ));
    }
    let services = array_value(named_field(fields, "services")?, "services")?;
    if services.len() < service_count as usize {
        return Err(format!(
            "boundary service_count is {service_count}, but the plan carries only {} service entries",
            services.len()
        ));
    }
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::with_capacity(service_count as usize);
    for (index, value) in services.iter().take(service_count as usize).enumerate() {
        let identity = uint_value(value, format!("services[{index}]").as_str())?;
        if !seen.insert(identity) {
            return Err(format!(
                "boundary reach repeats service identity {identity}"
            ));
        }
        normalized.push(
            BoundaryServiceReachId::from_normalized_identity(identity)
                .map_err(|diagnostic| diagnostic.to_string())?,
        );
    }
    Ok(BoundaryReach::from_services(normalized))
}

fn struct_fields(value: &BuildTimeValue) -> Result<&[(String, BuildTimeValue)], String> {
    match value {
        BuildTimeValue::Struct { fields, .. } => Ok(fields),
        other => Err(format!("is not a record value: {other:?}")),
    }
}

fn named_field<'a>(
    fields: &'a [(String, BuildTimeValue)],
    name: &str,
) -> Result<&'a BuildTimeValue, String> {
    fields
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value)
        .ok_or_else(|| format!("carries no `{name}` field"))
}

fn payload_field<'a>(
    fields: &'a [(String, BuildTimeValue)],
    name: &str,
) -> Result<&'a BuildTimeValue, String> {
    fields
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value)
        .ok_or_else(|| format!("case payload carries no `{name}` field"))
}

fn uint_value(value: &BuildTimeValue, name: &str) -> Result<u64, String> {
    match value {
        BuildTimeValue::Int(value) => Ok(*value as u64),
        other => Err(format!("`{name}` is not an integer: {other:?}")),
    }
}

fn bool_named_field(fields: &[(String, BuildTimeValue)], name: &str) -> Result<bool, String> {
    match named_field(fields, name)? {
        BuildTimeValue::Bool(value) => Ok(*value),
        other => Err(format!("`{name}` is not a bool: {other:?}")),
    }
}

fn bool_payload(payload: &[(String, BuildTimeValue)], name: &str) -> Result<bool, String> {
    match payload_field(payload, name)? {
        BuildTimeValue::Bool(value) => Ok(*value),
        other => Err(format!("case payload `{name}` is not a bool: {other:?}")),
    }
}

fn array_value<'a>(value: &'a BuildTimeValue, name: &str) -> Result<&'a [BuildTimeValue], String> {
    match value {
        BuildTimeValue::Array(values) => Ok(values),
        other => Err(format!("`{name}` is not an array: {other:?}")),
    }
}

fn require_empty_payload(payload: &[(String, BuildTimeValue)], case: &str) -> Result<(), String> {
    if payload.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "nullary case `{case}` unexpectedly carries a payload"
        ))
    }
}

fn short_name(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

fn invalid(policy_machine: &str, reason: impl AsRef<str>) -> String {
    format!(
        "policy `{policy_machine}` produced an invalid placed-access plan: {}",
        reason.as_ref()
    )
}
