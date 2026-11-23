//! Validate the plan a selected policy returned before any domain may use it.
//!
//! The caller supplies the value that executing the policy actually returned
//! for one frozen invocation. Nothing here runs the policy, reads its body or
//! consults the constructor it reached: `wiki/spec/resources/access.md` says
//! the returned data is not a validation certificate, and validation trusts no
//! intermediate calculation. Each rule below reads the returned value alone
//! and compares it with the invocation this package froze.
//!
//! A plan this validation cannot yet judge completely is refused, never
//! narrowed. Rules over regions, callers and services have no validated
//! consumer, so a plan that carries any of them stays pending.
use omega_prepared::{PlanIssuance, PreparedTrees, PreparedValue};
use omega_resolved::DeclarationId;
use omega_toolchain::core_interface;
use omega_typed::{TypedField, TypedTrees, ValueType};
use std::sync::Arc;

const NOT_A_PLAN: &str = "returned value is not a core AccessPlan record";
const MALFORMED: &str = "returned plan field does not have its declared shape";
const COUNT_DISAGREES: &str = "returned plan count disagrees with its occupied slots";

/// Read an execution result against this domain's semantic schema. Issuance
/// is published only after the package driver supplies the bound receipt.
pub fn read_returned_access_plan(
    prepared: &PreparedTrees<'_>,
    domain: &DeclarationId,
    returned: &PreparedValue,
) -> Result<PlanIssuance, &'static str> {
    let invocation = prepared
        .policy_invocation(domain)
        .ok_or("access plan has no frozen invocation")?;
    let argument = schema_image(invocation.schema).ok_or("policy schema has no byte view")?;
    read_plan(prepared.typed(), argument, returned).map(|(_, issuance)| issuance)
}

fn schema_image(schema: &PreparedValue) -> Option<&Arc<[u8]>> {
    let PreparedValue::Record { owner, fields } = schema else { return None };
    match fields.as_ref() {
        [PreparedValue::SharedBytes(image)] if owner == core_interface().id(r"reflect\TypeSchema") => Some(image),
        _ => None,
    }
}

/// Pair a returned record with the fields its core declaration lists. The
/// toolchain's own typed forest supplies the roster, so a plan from a
/// declaration with more or fewer fields cannot be read as this one.
fn declared_fields<'a>(
    typed: &'a TypedTrees<'_>,
    owner: &str,
    value: &'a PreparedValue,
) -> Option<impl Iterator<Item = (&'a TypedField, &'a PreparedValue)>> {
    let PreparedValue::Record { owner: actual, fields } = value else {
        return None;
    };
    if actual != core_interface().id(owner) {
        return None;
    }
    let declared = typed.library?.record_fields.get(actual)?;
    (declared.len() == fields.len()).then(|| declared.iter().zip(fields.iter()))
}

fn read_plan(
    typed: &TypedTrees<'_>,
    argument: &Arc<[u8]>,
    returned: &PreparedValue,
) -> Result<(Arc<[u8]>, PlanIssuance), &'static str> {
    let core = core_interface();
    let is = |field: &TypedField, name: &str| &field.declaration == core.id(name);
    let (mut schema, mut issuance) = (None, None);
    // A count is validated against the slots it governs, whichever comes first.
    let (mut occupied, mut counts) = ([None; 3], [None; 3]);
    for (field, value) in declared_fields(typed, "AccessPlan", returned).ok_or(NOT_A_PLAN)? {
        if is(field, "AccessPlan::schema") {
            schema = Some(returned_schema(value, argument)?);
        } else if is(field, "AccessPlan::issuance") {
            issuance = Some(returned_issuance(value)?);
        } else if is(field, "AccessPlan::reach") {
            check_reach(typed, value)?;
        } else if let Some(axis) = [
            "AccessPlan::fields",
            "AccessPlan::readers",
            "AccessPlan::writers",
        ]
        .iter()
        .position(|name| is(field, name))
        {
            let empty = core.id([
                "AccessFieldSlot::Empty",
                "AccessRuleSlot::Empty",
                "AccessRuleSlot::Empty",
            ][axis]);
            occupied[axis] = Some(occupied_slots(&field.ty, value, empty)?);
        } else if let Some(axis) = [
            "AccessPlan::field_count",
            "AccessPlan::reader_count",
            "AccessPlan::writer_count",
        ]
        .iter()
        .position(|name| is(field, name))
        {
            counts[axis] = Some(count(value)?);
        } else {
            // A field added to the core plan may restrict access. Publishing a
            // domain without reading it would drop that restriction.
            return Err("returned plan has a field that validation does not cover");
        }
    }
    for (occupied, count) in occupied.into_iter().zip(counts) {
        let (Some(occupied), Some(count)) = (occupied, count) else {
            return Err(NOT_A_PLAN);
        };
        if count != occupied {
            return Err(COUNT_DISAGREES);
        }
        if count != 0 {
            return Err("returned plan has field, reader or writer rules, which need validated regions and callers");
        }
    }
    Ok((schema.ok_or(NOT_A_PLAN)?, issuance.ok_or(NOT_A_PLAN)?))
}

/// The plan borrows the schema argument for its whole region. Equal bytes in
/// another allocation are a different borrow, so only the argument itself matches.
fn returned_schema(
    value: &PreparedValue,
    argument: &Arc<[u8]>,
) -> Result<Arc<[u8]>, &'static str> {
    let image = schema_image(value).ok_or(MALFORMED)?;
    if Arc::ptr_eq(image, argument) {
        Ok(image.clone())
    } else if image == argument {
        Err("returned plan's schema is a copy outside the region of this invocation's schema")
    } else {
        Err("returned plan describes a different schema")
    }
}

fn returned_issuance(value: &PreparedValue) -> Result<PlanIssuance, &'static str> {
    let core = core_interface();
    let PreparedValue::Sum { owner, case, fields } = value else {
        return Err(MALFORMED);
    };
    if owner != core.id("Issuance") {
        return Err(MALFORMED);
    }
    match fields.as_ref() {
        [] if case == core.id("Issuance::Unrestricted") => Ok(PlanIssuance::Unrestricted),
        [PreparedValue::Array(routes)] if case == core.id("Issuance::Only") => {
            if routes.is_empty() {
                Ok(PlanIssuance::Nobody)
            } else {
                // A route is a `CallableInfo`. Preparation freezes no descriptor
                // (`DescriptorStorageUnavailable`), so a returned one cannot be
                // matched to the callable typing selected.
                Err("returned plan's issuance routes need frozen callable descriptors")
            }
        }
        _ => Err(MALFORMED),
    }
}

/// The number of slots in a fixed-capacity roster that hold an entry.
fn occupied_slots(
    declared: &ValueType,
    value: &PreparedValue,
    empty: &DeclarationId,
) -> Result<u64, &'static str> {
    let (ValueType::Array(element, capacity), PreparedValue::Array(slots)) = (declared, value) else {
        return Err(MALFORMED);
    };
    let (slot_data, _) = element.nominal_parts().ok_or(MALFORMED)?;
    if u64::try_from(slots.len()) != Ok(*capacity) {
        return Err(MALFORMED);
    }
    let mut occupied = 0;
    for slot in slots.iter() {
        match slot {
            PreparedValue::Sum { owner, case, fields } if owner == slot_data => {
                if case != empty {
                    occupied += 1;
                } else if !fields.is_empty() {
                    return Err(MALFORMED);
                }
            }
            _ => return Err(MALFORMED),
        }
    }
    Ok(occupied)
}

fn count(value: &PreparedValue) -> Result<u64, &'static str> {
    match value {
        PreparedValue::Integer(count) => u64::try_from(count).map_err(|_| MALFORMED),
        _ => Err(MALFORMED),
    }
}

/// A new plan starts with empty additional reach. A plan that names services
/// needs each identity validated against the domain's boundary requirements.
fn check_reach(
    typed: &TypedTrees<'_>,
    value: &PreparedValue,
) -> Result<(), &'static str> {
    let core = core_interface();
    let (mut named, mut service_count) = (None, None);
    for (field, value) in declared_fields(typed, "BoundaryReach", value).ok_or(MALFORMED)? {
        if &field.declaration == core.id("BoundaryReach::services") {
            let (ValueType::Array(_, capacity), PreparedValue::Array(services)) = (&field.ty, value) else {
                return Err(MALFORMED);
            };
            if u64::try_from(services.len()) != Ok(*capacity) {
                return Err(MALFORMED);
            }
            let mut nonzero = false;
            for service in services.iter() {
                nonzero |= count(service)? != 0;
            }
            named = Some(nonzero);
        } else if &field.declaration == core.id("BoundaryReach::service_count") {
            service_count = Some(count(value)?);
        } else {
            return Err("returned plan has a field that validation does not cover");
        }
    }
    match (named, service_count) {
        (Some(false), Some(0)) => Ok(()),
        (Some(true), Some(0)) => Err(COUNT_DISAGREES),
        (Some(_), Some(_)) => Err("returned plan names boundary reach, which needs validated service identities"),
        _ => Err(MALFORMED),
    }
}
