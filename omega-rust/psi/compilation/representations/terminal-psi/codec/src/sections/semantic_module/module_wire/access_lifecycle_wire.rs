//! Whole-value operations share the normalized placement wire with field views.
//! The boundary coordinate describes demand, not an installed memory resource.

use super::super::CodecError;
use super::super::wire::{Reader, Writer, decode_canonical, decode_counted};
use terminal_psi::{
    TerminalAccessLifecycleExecution, TerminalAccessLifecycleOperation,
    TerminalAccessLifecyclePlan, TerminalAccessStorage, TerminalIdentityIndex,
};

/// Retain the exact whole-value operation contracts in native program identity.
/// These bytes describe demand; encoding grants no backing or completion.
pub fn encode_access_lifecycle_catalog(
    plans: &[TerminalAccessLifecyclePlan],
) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    writer.counted("access lifecycle plans", plans, encode)?;
    Ok(writer.finish())
}

/// Decode retained demand without establishing a resource or lifecycle result.
pub fn decode_access_lifecycle_catalog(
    bytes: &[u8],
) -> Result<Vec<TerminalAccessLifecyclePlan>, CodecError> {
    decode_canonical(
        bytes,
        |reader| decode_counted(reader, decode),
        |plans| encode_access_lifecycle_catalog(plans),
    )
}

pub(super) fn encode(
    writer: &mut Writer,
    plan: &TerminalAccessLifecyclePlan,
) -> Result<(), CodecError> {
    writer.id(plan.boundary);
    writer.u8(match plan.operation {
        TerminalAccessLifecycleOperation::Initialize => 0,
        TerminalAccessLifecycleOperation::MoveOut => 1,
        TerminalAccessLifecycleOperation::View => 2,
    });
    match &plan.execution {
        TerminalAccessLifecycleExecution::SelectedProvider => writer.u8(0),
        TerminalAccessLifecycleExecution::CompilerPrimitive {
            profile_type,
            profile_requirement,
        } => {
            writer.u8(1);
            writer.u8(u8::from(profile_type.is_some()));
            if let Some(profile_type) = profile_type {
                writer.id(*profile_type);
            }
            writer.u8(u8::from(profile_requirement.is_some()));
            if let Some(requirement) = profile_requirement {
                writer.id(requirement.boundary);
                writer.u32(requirement.owner.identity().0);
                writer.u32(requirement.callable.0);
            }
        }
    }
    writer.id(plan.schema);
    writer.ids("placement image fields", &plan.image_fields)?;
    writer.id(plan.storage.carrier);
    writer.id(plan.storage.base);
    writer.id(plan.storage.length);
    writer.id(plan.storage.granted);
    if plan.operation == TerminalAccessLifecycleOperation::View {
        if plan.storage.vacant.is_some() || plan.storage.resident.is_some() {
            return Err(CodecError::NonCanonicalEncoding);
        }
    } else {
        let (vacant, resident) = plan
            .storage
            .owned_content()
            .map_err(|_| CodecError::NonCanonicalEncoding)?;
        writer.id(vacant);
        writer.id(resident);
    }
    for identity in [
        plan.policy_identity,
        plan.policy_plan_machine_identity,
        plan.schema_identity,
    ] {
        writer.u32(identity.0);
    }
    super::access_view_plan_wire::encode_placement(writer, &plan.placement)
}

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<TerminalAccessLifecyclePlan, CodecError> {
    let boundary = reader.id("access lifecycle boundary")?;
    let operation = match reader.u8()? {
        0 => TerminalAccessLifecycleOperation::Initialize,
        1 => TerminalAccessLifecycleOperation::MoveOut,
        2 => TerminalAccessLifecycleOperation::View,
        tag => return Err(CodecError::InvalidTag("access lifecycle operation", tag)),
    };
    let execution = match reader.u8()? {
        0 => TerminalAccessLifecycleExecution::SelectedProvider,
        1 => {
            let profile_type = match reader.u8()? {
                0 => None,
                1 => Some(reader.id("placement profile type")?),
                tag => return Err(CodecError::InvalidTag("placement profile presence", tag)),
            };
            let profile_requirement = match reader.u8()? {
                0 => None,
                1 => Some(terminal_psi::TerminalAccessProfileRequirement {
                    boundary: reader.id("placement profile requirement")?,
                    owner: terminal_psi::NominalDeclarationIdentity::new(TerminalIdentityIndex(
                        reader.u32()?,
                    )),
                    callable: TerminalIdentityIndex(reader.u32()?),
                }),
                tag => {
                    return Err(CodecError::InvalidTag(
                        "placement profile requirement presence",
                        tag,
                    ));
                }
            };
            TerminalAccessLifecycleExecution::CompilerPrimitive {
                profile_type,
                profile_requirement,
            }
        }
        tag => return Err(CodecError::InvalidTag("access lifecycle execution", tag)),
    };
    let schema = reader.id("access lifecycle schema")?;
    let image_fields = super::super::wire::decode_ids(reader, "placement image field")?;
    let storage = TerminalAccessStorage {
        carrier: reader.id("placement extent carrier")?,
        base: reader.id("placement extent base field")?,
        length: reader.id("placement extent length field")?,
        granted: reader.id("placement extent grant domain")?,
        vacant: if operation == TerminalAccessLifecycleOperation::View {
            None
        } else {
            Some(reader.id("placement extent vacant domain")?)
        },
        resident: if operation == TerminalAccessLifecycleOperation::View {
            None
        } else {
            Some(reader.id("placement extent resident domain")?)
        },
    };
    let policy_identity = TerminalIdentityIndex(reader.u32()?);
    let policy_plan_machine_identity = TerminalIdentityIndex(reader.u32()?);
    let schema_identity = TerminalIdentityIndex(reader.u32()?);
    let placement = super::access_view_plan_wire::decode_placement(reader)?;
    Ok(TerminalAccessLifecyclePlan {
        boundary,
        operation,
        execution,
        schema,
        image_fields,
        storage,
        policy_identity,
        policy_plan_machine_identity,
        schema_identity,
        placement,
    })
}
