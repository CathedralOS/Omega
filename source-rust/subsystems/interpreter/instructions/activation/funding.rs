//! Bound each path through the checked worker before taking the caller's loan.
//! The interpreter stores control words as bytes so an arbitrary slice start
//! needs no machine-pointer alignment. Structural carrier geometry still comes
//! from this program's admitted target.
use super::results::{RetainedBacking, RetainedDescriptor};
use crate::boundary::errors::malformed;
use crate::engine::execution::TerminalExecution;
use crate::storage::byte_sequences::binding::ByteSequenceBinding;
use crate::storage::provider_storage::value_image::geometry::ImageGeometry;
use crate::{TerminalInterpretError, TerminalScalarValue};
use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, IntegerType, IntegerValue, ScalarType};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    TerminalActivationOrigin, TerminalActivationStorage, TerminalPreparedActivationEntry,
    TerminalTaskFrame, TerminalTaskResidentValue, TerminalTaskValueType,
};

// Each frame carries its saved instruction position even when it has no locals.
const CONTROL_BYTES: u64 = 8;

pub(super) fn supports(
    execution: &TerminalExecution<'_>,
    selected: TerminalPreparedActivationEntry,
    backing: &RetainedBacking,
) -> Result<bool, TerminalInterpretError> {
    if selected.storage != TerminalActivationStorage::Fixed {
        return Ok(false);
    }
    let demand = execution
        .program
        .catalogs
        .execution
        .task_activation_demands
        .iter()
        .find(|d| d.origin == TerminalActivationOrigin::Caller(selected))
        .ok_or(malformed())?;
    let geometry = ImageGeometry::for_execution(execution);
    let size = |value: TerminalTaskValueType| -> Option<u64> {
        Some(match value {
            TerminalTaskValueType::Unit => 0,
            TerminalTaskValueType::Structural(id) => geometry.layout(id).ok()?.0,
            TerminalTaskValueType::Scalar(ScalarType::Boolean) => 1,
            TerminalTaskValueType::Scalar(ScalarType::Integer(ty)) => {
                u64::from(execution.closed_integer(ty).ok()?.bits()).div_ceil(8)
            }
            TerminalTaskValueType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)) => 4,
            TerminalTaskValueType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)) => 8,
            // The checked scalar stores a descriptor identity in this machine.
            TerminalTaskValueType::Scalar(ScalarType::Callable(_)) => 8,
        })
    };
    let bytes = |values: &[TerminalTaskResidentValue]| {
        values
            .iter()
            .try_fold(0u64, |n, v| n.checked_add(size(v.value_type)?))
    };
    let frames: BTreeMap<_, _> = demand
        .frames
        .iter()
        .map(|frame| (frame.identity, frame))
        .collect();
    let mut sizes = BTreeMap::new();
    for frame in &demand.frames {
        let Some(peak) = frame_bytes(frame, &bytes) else {
            return Ok(false);
        };
        sizes.insert(frame.identity, peak);
    }
    Ok(path_bound(demand.root_frame, &frames, &sizes)
        .is_some_and(|needed| needed <= backing.len() && backing.alignment() >= 1))
}

fn frame_bytes(
    frame: &TerminalTaskFrame,
    bytes: &impl Fn(&[TerminalTaskResidentValue]) -> Option<u64>,
) -> Option<u64> {
    if !frame.unresolved_calls.is_empty() {
        return None;
    }
    let mut peak = CONTROL_BYTES;
    for state in &frame.states {
        let base = CONTROL_BYTES.checked_add(bytes(&state.resident)?)?;
        peak = peak.max(base);
        for args in &state.crossing_arguments {
            peak = peak.max(base.checked_add(bytes(args)?)?);
        }
    }
    Some(peak)
}

// Postorder without host recursion on an artifact-provided call graph. The
// local cost is incurred on each path; sibling calls can reuse the same bytes.
fn path_bound(
    root: u64,
    frames: &BTreeMap<u64, &TerminalTaskFrame>,
    local: &BTreeMap<u64, u64>,
) -> Option<u64> {
    let mut entering = BTreeSet::new();
    let mut finished = BTreeMap::new();
    let mut pending = vec![(root, false)];
    while let Some((id, leaving)) = pending.pop() {
        if finished.contains_key(&id) {
            continue;
        }
        let frame = frames.get(&id)?;
        if leaving {
            let child = frame.calls.iter().try_fold(0u64, |peak, call| {
                Some(peak.max(*finished.get(&call.callee)?))
            })?;
            finished.insert(id, local.get(&id)?.checked_add(child)?);
            entering.remove(&id);
        } else {
            if !entering.insert(id) {
                return None;
            }
            pending.push((id, true));
            pending.extend(frame.calls.iter().map(|call| (call.callee, false)));
        }
    }
    finished.get(&root).copied()
}

impl RetainedBacking {
    pub(super) fn put_position(
        &self,
        execution: &mut TerminalExecution<'_>,
        frame: usize,
        position: usize,
    ) -> Result<(), TerminalInterpretError> {
        let bytes = u64::try_from(position)
            .map_err(|_| malformed())?
            .to_le_bytes();
        self.write_storage(
            execution,
            frame.checked_mul(bytes.len()).ok_or(malformed())?,
            &bytes,
        )
    }

    pub(super) fn write_storage(
        &self,
        execution: &mut TerminalExecution<'_>,
        offset: usize,
        bytes: &[u8],
    ) -> Result<(), TerminalInterpretError> {
        for (within, &byte) in bytes.iter().enumerate() {
            let index = offset.checked_add(within).ok_or(malformed())?;
            if u64::try_from(index).map_err(|_| malformed())? >= self.len() {
                return Err(malformed());
            }
            match self.binding() {
                RetainedDescriptor::Bytes(binding) => {
                    execution.write_retained_byte(binding, index, byte)?
                }
                RetainedDescriptor::Elements(view) => execution.write_element_view_scalar(
                    view,
                    index,
                    &[],
                    TerminalScalarValue::Integer {
                        scalar_type: IntegerType::new(IntegerSign::Unsigned, 8)
                            .map_err(|_| malformed())?,
                        value: IntegerValue::Unsigned(u128::from(byte)),
                    },
                )?,
            }
        }
        Ok(())
    }

    pub(super) fn position(
        &self,
        execution: &TerminalExecution<'_>,
        frame: usize,
    ) -> Result<usize, TerminalInterpretError> {
        let mut encoded = [0u8; 8];
        self.read_storage(
            execution,
            frame.checked_mul(encoded.len()).ok_or(malformed())?,
            &mut encoded,
        )?;
        usize::try_from(u64::from_le_bytes(encoded)).map_err(|_| malformed())
    }

    pub(super) fn read_storage(
        &self,
        execution: &TerminalExecution<'_>,
        offset: usize,
        bytes: &mut [u8],
    ) -> Result<(), TerminalInterpretError> {
        for (within, byte) in bytes.iter_mut().enumerate() {
            let index = offset.checked_add(within).ok_or(malformed())?;
            if u64::try_from(index).map_err(|_| malformed())? >= self.len() {
                return Err(malformed());
            }
            *byte = match self.binding() {
                RetainedDescriptor::Bytes(ByteSequenceBinding::MutableArray {
                    array,
                    offset,
                    ..
                }) => {
                    let offset = usize::try_from(*offset).map_err(|_| malformed())?;
                    execution.stored_byte_array_read(
                        array,
                        offset.checked_add(index).ok_or(malformed())?,
                    )?
                }
                RetainedDescriptor::Bytes(binding) => *execution
                    .exclusive_owner_bytes(binding)?
                    .0
                    .get(index)
                    .ok_or(malformed())?,
                RetainedDescriptor::Elements(view) => {
                    let ty = IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?;
                    let TerminalScalarValue::Integer {
                        value: IntegerValue::Unsigned(value),
                        ..
                    } = execution.read_element_view_scalar(
                        view,
                        index,
                        &[],
                        ScalarType::Integer(ty),
                    )?
                    else {
                        return Err(malformed());
                    };
                    u8::try_from(value).map_err(|_| malformed())?
                }
            };
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_psi::{TerminalIdentityIndex, TerminalTaskFrameCall};

    fn frame(identity: u64, calls: &[u64]) -> TerminalTaskFrame {
        TerminalTaskFrame {
            identity,
            machine_callable_identity: TerminalIdentityIndex(0),
            entry: TerminalIdentityIndex(1),
            states: Vec::new(),
            calls: calls
                .iter()
                .map(|id| TerminalTaskFrameCall { callee: *id })
                .collect(),
            unresolved_calls: Vec::new(),
        }
    }

    #[test]
    fn fixed_storage_covers_every_enterable_frame_and_reuses_space_for_siblings() {
        let frame_list = [
            frame(1, &[2, 3]),
            frame(2, &[4]),
            frame(3, &[4]),
            frame(4, &[]),
        ];
        let frames = frame_list
            .iter()
            .map(|frame| (frame.identity, frame))
            .collect();
        let sizes = BTreeMap::from([(1, 8), (2, 32), (3, 64), (4, 16)]);
        assert_eq!(path_bound(1, &frames, &sizes), Some(88));
        let cyclic = frame(4, &[1]);
        let mut frames = frames;
        frames.insert(4, &cyclic);
        assert_eq!(path_bound(1, &frames, &sizes), None);
        frames.remove(&4);
        assert_eq!(path_bound(1, &frames, &sizes), None);
    }
}
