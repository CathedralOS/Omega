//! Spill concrete live SSA scalars to the borrowed stack. Runtime-only
//! identities (capabilities, executable descriptors and host addresses) keep
//! their checked interpreter metadata; serializing their numbers could forge
//! authority when restoring a worker.
use super::results::RetainedBacking;
use crate::TerminalScalarValue;
use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::engine::execution::TerminalExecution;
use crate::frame_store::ParkedFrames;
use semantic_vocabulary::{
    IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerValue, ScalarType, ValueId,
};
use terminal_psi::TerminalSuspensionPlace;
use wire::digest_encoder::DomainDigestEncoder;

struct ScalarHome {
    frame: usize,
    value: ValueId,
    ty: ScalarType,
    width: u8,
}

pub(super) struct ScalarSpills {
    start: usize,
    homes: Vec<ScalarHome>,
    commitment: [u8; 32],
}

fn commitment() -> DomainDigestEncoder {
    DomainDigestEncoder::new(b"omega.interpreter.owned-activation.scalar-spills.v1")
}

impl ScalarSpills {
    pub(super) fn save(
        execution: &mut TerminalExecution<'_>,
        backing: &RetainedBacking,
        frames: &ParkedFrames,
        before_entry: bool,
    ) -> Result<Self, TerminalInterpretError> {
        let program = std::sync::Arc::clone(&execution.program);
        let stack: Vec<_> = execution.call_stack.parked_homes(frames).collect();
        // Control words for every live frame occupy the beginning of this loan.
        let start = stack.len().checked_mul(8).ok_or(malformed())?;
        let mut offset = start;
        let mut digest = commitment();
        let mut homes = Vec::new();
        for (index, frame) in stack.into_iter().enumerate() {
            let saved = execution.call_stack.get(frame).ok_or(malformed())?;
            let machine = program.verified_machine(saved.current_machine)?;
            let mut live = Vec::new();
            if before_entry {
                live.extend(machine.parameters.iter().map(|parameter| parameter.id));
            } else {
                let position = backing.position(execution, index)?;
                // The leaf is stopped at its park call. Ancestors saved the
                // following return position when entering that nested call.
                let call = if index == 0 {
                    Some(position)
                } else {
                    position.checked_sub(1)
                };
                let operation =
                    call.and_then(|at| machine.block(&saved.current)?.operations.get(at));
                if let Some((op, crossing)) =
                    operation.and_then(|op| Some((op, op.suspension_crossing?)))
                {
                    let plans = &program.catalogs.execution.suspension_call_plans;
                    let key = plans
                        .binary_search_by_key(&(op.id, crossing), |plan| {
                            (plan.operation, plan.crossing)
                        })
                        .map_err(|_| malformed())?;
                    live.extend(
                        plans[key]
                            .live_values
                            .iter()
                            .filter_map(|row| match row.place {
                                TerminalSuspensionPlace::Scalar(id) => Some(id),
                                TerminalSuspensionPlace::Structural(_) => None,
                            }),
                    );
                }
            }
            live.sort_unstable();
            live.dedup();
            for id in live {
                let value = *execution
                    .call_stack
                    .get(frame)
                    .and_then(|f| f.values.get(&id))
                    .ok_or(malformed())?;
                let Some((bytes, width)) = encode(execution, value)? else {
                    continue;
                };
                backing.write_storage(execution, offset, &bytes[..usize::from(width)])?;
                digest.raw(&bytes[..usize::from(width)]);
                offset = offset.checked_add(usize::from(width)).ok_or(malformed())?;
                execution
                    .call_stack
                    .get_mut(frame)
                    .ok_or(malformed())?
                    .values
                    .remove(&id)
                    .ok_or(malformed())?;
                homes.push(ScalarHome {
                    frame,
                    value: id,
                    ty: value.scalar_type(),
                    width,
                });
            }
        }
        Ok(Self {
            start,
            homes,
            commitment: digest.finish(),
        })
    }

    pub(super) fn restore(
        &self,
        execution: &mut TerminalExecution<'_>,
        backing: &RetainedBacking,
    ) -> Result<(), TerminalInterpretError> {
        let mut offset = self.start;
        let mut digest = commitment();
        for home in &self.homes {
            let width = usize::from(home.width);
            let mut bytes = [0; 16];
            backing.read_storage(execution, offset, &mut bytes[..width])?;
            digest.raw(&bytes[..width]);
            let value = decode(home.ty, &bytes[..width])?;
            execution
                .call_stack
                .get_mut(home.frame)
                .ok_or(malformed())?
                .values
                .insert_vacant(home.value, value)?;
            offset = offset.checked_add(width).ok_or(malformed())?;
        }
        if digest.finish() != self.commitment {
            // No frame is reattached until its complete saved frontier checks.
            return Err(malformed());
        }
        Ok(())
    }
}

fn encode(
    execution: &TerminalExecution<'_>,
    value: TerminalScalarValue,
) -> Result<Option<([u8; 16], u8)>, TerminalInterpretError> {
    let (bits, width) = match value {
        TerminalScalarValue::Boolean(value) => (u128::from(value), 1),
        TerminalScalarValue::Integer {
            scalar_type: ty,
            value,
        } if !ty.is_address() => {
            let bits = match value {
                IntegerValue::Signed(n) => u128::from_le_bytes(n.to_le_bytes()),
                IntegerValue::Unsigned(n) => n,
            };
            let width = execution.closed_integer(ty)?.bits().div_ceil(8);
            (bits, u8::try_from(width).map_err(|_| malformed())?)
        }
        TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(bits)) => (u128::from(bits), 4),
        TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(bits)) => (u128::from(bits), 8),
        _ => return Ok(None),
    };
    Ok(Some((bits.to_le_bytes(), width)))
}

fn decode(ty: ScalarType, encoded: &[u8]) -> Result<TerminalScalarValue, TerminalInterpretError> {
    let mut bytes = [0; 16];
    if matches!(ty, ScalarType::Integer(ty) if ty.sign() == IntegerSign::Signed)
        && encoded.last().is_some_and(|byte| byte & 0x80 != 0)
    {
        bytes.fill(0xff);
    }
    bytes
        .get_mut(..encoded.len())
        .ok_or(malformed())?
        .copy_from_slice(encoded);
    let bits = u128::from_le_bytes(bytes);
    Ok(match ty {
        ScalarType::Boolean if bits <= 1 => TerminalScalarValue::Boolean(bits != 0),
        ScalarType::Integer(scalar_type) if !scalar_type.is_address() => {
            TerminalScalarValue::Integer {
                scalar_type,
                value: match scalar_type.sign() {
                    IntegerSign::Signed => IntegerValue::Signed(i128::from_le_bytes(bytes)),
                    IntegerSign::Unsigned => IntegerValue::Unsigned(bits),
                },
            }
        }
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => TerminalScalarValue::IeeeFloat(
            IeeeFloatValue::Binary32(u32::try_from(bits).map_err(|_| malformed())?),
        ),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => TerminalScalarValue::IeeeFloat(
            IeeeFloatValue::Binary64(u64::try_from(bits).map_err(|_| malformed())?),
        ),
        _ => return Err(malformed()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::IntegerType;

    #[test]
    fn restoring_a_concrete_spill_preserves_signed_values_and_float_payloads() {
        let signed = IntegerType::new(IntegerSign::Signed, 12).unwrap();
        assert_eq!(
            decode(ScalarType::Integer(signed), &[0x01, 0xf8]).unwrap(),
            TerminalScalarValue::Integer {
                scalar_type: signed,
                value: IntegerValue::Signed(-2047)
            }
        );
        let nan = 0x7fc00037u32;
        assert_eq!(
            decode(
                ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
                &nan.to_le_bytes()
            )
            .unwrap(),
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(nan))
        );
        assert!(decode(ScalarType::Boolean, &[2]).is_err());
    }
}
