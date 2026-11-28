//! Resolve local activation calls within one object text section. Moving a whole
//! object preserves both the caller-to-routine and routine-to-worker distances.
use super::{Error, host};
use crate::{ObjectForeignCall, ObjectPublication};
use std::borrow::Cow;
use target_vocabulary::{FixedActivationAction, NormalizedForeignCallMechanism};

/// Replay each issued routine against the placed call and its reached worker.
/// Programs without activation calls share the original bytes.
pub fn resolved_text<'a>(
    source: &'a ObjectPublication,
    calls: &[ObjectForeignCall],
) -> Result<Cow<'a, [u8]>, Error> {
    let original = source.object().text_section.bytes();
    let fields = &source.object().unresolved_fixed_activation_calls;
    let routines = source.fixed_activations();
    let fixed_calls: Vec<_> = calls
        .iter()
        .filter(|call| {
            matches!(
                call.binding.mechanism,
                NormalizedForeignCallMechanism::FixedActivation(_)
            )
        })
        .collect();
    if fields.len() != fixed_calls.len() {
        return Err(Error::Mismatch(
            "fixed call field roster differs from its physical calls",
        ));
    }
    if fields.is_empty() && routines.is_empty() {
        return Ok(Cow::Borrowed(original));
    }
    if source.text().target != target::NativeTarget::linux_x64() {
        return Err(Error::Unsupported(
            "fixed activation placement has no target contract",
        ));
    }
    let mut resolved = original.to_vec();
    let mut offsets = Vec::with_capacity(routines.len());
    for routine in routines {
        let NormalizedForeignCallMechanism::FixedActivation(call) = &routine.binding().mechanism
        else {
            return Err(Error::Mismatch(
                "local activation routine lost its selected boundary",
            ));
        };
        if !fixed_calls
            .iter()
            .any(|physical| &physical.binding == routine.binding())
            || routine.bytes().is_empty()
            || routine.binding().fixed_activation_stack_bytes()
                != Some(routine.caller_stack_bytes())
        {
            return Err(Error::Mismatch(
                "fixed activation routine differs from its reached calling plan",
            ));
        }
        let start = resolved.len();
        resolved.extend_from_slice(routine.bytes());
        match (call.action, routine.worker_call()) {
            (FixedActivationAction::Step { worker, .. }, Some((offset, selected)))
                if selected == worker =>
            {
                let target = source.text().function(worker).ok_or(Error::Mismatch(
                    "fixed activation worker was not placed in this program",
                ))?;
                // The local field must be inside this routine, even when previous routines exist.
                let end = offset.checked_add(4).ok_or(Error::Overflow)?;
                if offset == 0 || end > routine.bytes().len() {
                    return Err(Error::Mismatch(
                        "fixed activation worker call lies outside its routine",
                    ));
                }
                resolve_call(
                    &mut resolved,
                    start.checked_add(offset).ok_or(Error::Overflow)?,
                    host(target.section_offset)?,
                )?;
            }
            (
                FixedActivationAction::Prepare { .. }
                | FixedActivationAction::Park { .. }
                | FixedActivationAction::Resume { .. },
                None,
            ) => {}
            _ => {
                return Err(Error::Mismatch(
                    "fixed activation lost its exact worker entry",
                ));
            }
        }
        offsets.push(start);
    }
    for field in fields {
        let offset = host(field.field_section_offset)?;
        let mut matching = fixed_calls.iter().filter(|call| {
            call.machine == field.caller
                && call.owner == target_vocabulary::CallSiteOwner::from(field.invocation_site)
                && call.text_offset == offset
        });
        let (Some(call), None) = (matching.next(), matching.next()) else {
            return Err(Error::Mismatch(
                "fixed activation field lacks one reached call",
            ));
        };
        let mut matching = routines
            .iter()
            .zip(&offsets)
            .filter(|(routine, _)| routine.binding() == &call.binding);
        let (Some((_, &target)), None) = (matching.next(), matching.next()) else {
            return Err(Error::Mismatch(
                "fixed activation field lacks one issued routine",
            ));
        };
        if field.field_byte_width != 4
            || field.addend != 0
            || field.field_section_offset.checked_add(4)
                != Some(field.next_instruction_section_offset)
            || field.opcode_section_offset.checked_add(1) != Some(field.field_section_offset)
            || offset.checked_add(4).is_none_or(|end| end > original.len())
        {
            return Err(Error::Mismatch(
                "fixed activation field is not a direct local call",
            ));
        }
        resolve_call(&mut resolved, offset, target)?;
    }
    Ok(Cow::Owned(resolved))
}

/// All offsets are section-relative. Only the selected four-byte operand may change.
fn resolve_call(text: &mut [u8], field: usize, target: usize) -> Result<(), Error> {
    let opcode = field.checked_sub(1).ok_or(Error::Overflow)?;
    let end = field.checked_add(4).ok_or(Error::Overflow)?;
    if text.get(opcode..end) != Some(&[0xe8, 0, 0, 0, 0]) {
        return Err(Error::Mismatch(
            "fixed activation does not have the issued call field",
        ));
    }
    let displacement = i64::try_from(target)
        .map_err(|_| Error::Overflow)?
        .checked_sub(i64::try_from(end).map_err(|_| Error::Overflow)?)
        .and_then(|delta| i32::try_from(delta).ok())
        .ok_or(Error::Overflow)?;
    text[field..end].copy_from_slice(&displacement.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_object_can_link_a_caller_forward_and_its_selected_worker_backward() {
        let mut text = [0x90, 0xe8, 0, 0, 0, 0, 0xc3, 0xe8, 0, 0, 0, 0, 0xc3];
        resolve_call(&mut text, 2, 7).unwrap();
        resolve_call(&mut text, 8, 0).unwrap();
        assert_eq!(i32::from_le_bytes(text[2..6].try_into().unwrap()), 1);
        assert_eq!(i32::from_le_bytes(text[8..12].try_into().unwrap()), -12);
        assert_eq!(text[6], 0xc3);
    }

    #[test]
    fn a_moved_or_prepopulated_field_does_not_change_text() {
        for (mut text, field) in [
            ([0xe8, 1, 0, 0, 0], 1),
            ([0xe8, 0, 0, 0, 0], 2),
            ([0x90, 0, 0, 0, 0], 1),
        ] {
            let original = text;
            assert!(resolve_call(&mut text, field, 0).is_err());
            assert_eq!(text, original);
        }
    }
}
