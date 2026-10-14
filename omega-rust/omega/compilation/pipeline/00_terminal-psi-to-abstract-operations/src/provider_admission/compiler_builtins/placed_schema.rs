//! Join an untrusted selected scalar schema to its independently admitted
//! Terminal declaration. A digest of the supplied schema cannot make this join.

use effects::provider_plan::ProviderPlan;
use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, ScalarType};
use terminal_psi::{
    BoundaryMachineDeclaration, StructuralAccess, TerminalModule, TerminalPlacedAccessorKind,
};
use terminal_semantics::PlacedScalarBoundaryDemand;

pub(super) fn validate(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
    plan: &ProviderPlan,
    demand: &PlacedScalarBoundaryDemand,
) -> Result<(), String> {
    let invalid = || {
        "placed scalar selected schema does not match its Terminal signature and effects".to_owned()
    };
    let schema = &plan.schema;
    let [method] = schema.methods.as_slice() else {
        return Err(invalid());
    };
    let requirement = module
        .identities
        .text(boundary.identity)
        .ok_or_else(invalid)?;
    let accessor = module
        .structural_type(demand.accessor)
        .and_then(|declaration| module.identities.text(declaration.identity))
        .filter(|identity| !identity.is_empty())
        .ok_or_else(invalid)?;
    let borrow = match demand.access {
        StructuralAccess::SharedBorrow => "ref",
        StructuralAccess::MutableBorrow => "ref-mut",
        _ => return Err(invalid()),
    };
    let scalar = primitive_identity(demand.scalar);
    let mut parameters = vec![format!("{borrow}({accessor})")];
    let name = match demand.operation {
        TerminalPlacedAccessorKind::Read => "read",
        TerminalPlacedAccessorKind::Take => "take",
        TerminalPlacedAccessorKind::Write => {
            parameters.push(scalar.clone());
            "write"
        }
    };
    let has_result = demand.operation != TerminalPlacedAccessorKind::Write;
    // These declarations are the closed, synchronous, caller-RAM operations.
    // Extra effect/claim/calling surfaces need a different authenticated catalog;
    // an equal byte codec does not license treating them as this implementation.
    if schema.identities.text(method.requirement_identity) != requirement
        || method.name != name
        || schema.identities.text(schema.trait_name)
            != format!("{}::{name}", method.requirement_owner)
        || schema.trait_package_identity != plan.origin_package_identity
        || method.requirement_owner_package_identity != schema.trait_package_identity
        || !schema.receiver_applications.is_empty()
        || method.parameter_count != parameters.len()
        || method.parameter_type_identities.len() != parameters.len()
        || !method
            .parameter_type_identities
            .iter()
            .zip(&parameters)
            .all(|(actual, expected)| schema.identities.text(*actual) == expected)
        || method.has_result != has_result
        || if has_result {
            schema.identities.text(method.result_type_identity) != scalar
        } else {
            method.result_type_identity != Default::default()
        }
        || method.completion != boundary.completion
        || method.terminates_guarantee != boundary.unconditional_termination
        || !method.termination_premises.is_empty()
        || !method.entry_claims.is_empty()
        || !method.result_claims.is_empty()
        || !method.service_reach.is_empty()
        || !method.synchronous_invocations.is_empty()
        || !boundary.requires.is_empty()
        || !boundary.scalar_requires.is_empty()
        || !boundary.scalar_ensures.is_empty()
        || !boundary.program_local_root_introductions.is_empty()
        || !boundary.content_guarantees.is_empty()
        || !boundary.crash_routes.is_empty()
        || !boundary.fixed_service_reach.is_empty()
        || !boundary.published_service_ceiling.is_empty()
        || boundary.owning_service.is_some()
        || method.may_suspend
        || method.may_block
        || method.calling_plan_report_fingerprint.is_some()
        || method.calling_plan_commitment.is_some()
    {
        return Err(invalid());
    }
    Ok(())
}

/// The ordinary normalized builtin type grammar used by the checked producer.
/// Qualified or nominal value identities do not silently collapse to a codec.
fn primitive_identity(scalar: ScalarType) -> String {
    let name = match scalar {
        ScalarType::Boolean => "bool".to_owned(),
        ScalarType::Integer(integer) if integer.is_address() => "addr".to_owned(),
        ScalarType::Integer(integer) => format!(
            "{}{}",
            if integer.sign() == IntegerSign::Signed {
                "i"
            } else {
                "u"
            },
            integer.bits()
        ),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => "f32".to_owned(),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => "f64".to_owned(),
    };
    format!("named(name({name}))")
}
