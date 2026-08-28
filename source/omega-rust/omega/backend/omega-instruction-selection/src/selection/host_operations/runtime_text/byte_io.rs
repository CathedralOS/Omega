use crate::InstructionSelectionInput;
use omega_abstract_operations::SelectedInstructionKind;
use omega_calling_conventions::PlatformCallData;
use omega_layout::DataShape;
use omega_platform_interface::{HostCall, HostCallArgumentKind};

use crate::selection::storage_places::{
    classify_scalar_value_type_in_table, resolve_runtime_storage_place_in_table,
};

/// std console `read_byte() -> ByteRead` (PlatformCallData::SingleByteRead):
/// the composite instruction owns the whole result -- it pre-zeroes the sum
/// slot (the zero state IS `Eof`), reads fd 0 straight into the payload
/// word, and writes tag 1 only when a byte arrived. Serves the
/// `let r: ByteRead = self.console.read_byte()` local shape (the slot at
/// this statement's LocalStorage); anything else returns None and the
/// emission-planning blocker refuses the compile loudly.
pub(in crate::selection::host_operations) fn runtime_byte_read(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
) -> Option<SelectedInstructionKind> {
    if !matches!(host_call.data, PlatformCallData::SingleByteRead) {
        return None;
    }

    let slot = input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == dispatch_index.unwrap_or(0)
                && slot.source_key == host_call.source_key
                && slot.statement_index == host_call.statement_index
                && matches!(
                    slot.kind,
                    omega_runtime_storage::RuntimeFrameSlotKind::LocalStorage
                ))
            .then_some(slot)
        })?;
    let payload_offset = byte_read_payload_offset(input)?;
    // The slot must hold the full sum (tag + payload word); a narrower slot
    // is some other statement shape -- refuse rather than clobber.
    if slot.byte_size < payload_offset + core::mem::size_of::<i32>() {
        return None;
    }

    Some(SelectedInstructionKind::ReadRuntimeByte {
        target_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
        target_offset: slot.byte_offset,
        payload_offset,
    })
}

/// std console `write_byte(b)` (PlatformCallData::SingleByteWrite): one byte
/// to stdout straight from the argument's storage (LE low byte), from the
/// source storage of an exact authored `u8 as i32`, or from the staged 1-byte
/// data object for an integer-literal argument.
pub(in crate::selection::host_operations) fn runtime_byte_write(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
) -> Option<SelectedInstructionKind> {
    if !matches!(host_call.data, PlatformCallData::SingleByteWrite) {
        return None;
    }

    let arguments = input.host_calls.arguments.span(host_call.arguments)?;
    let first_argument = arguments.first()?;

    // Integer literals arrive PRE-RESOLVED as the Integer argument kind (the
    // host-call lowering folds them before any plan sees the call); the
    // staged 1-byte data object carries the value.
    if matches!(first_argument.kind, HostCallArgumentKind::Integer(_)) {
        let (literal, _) = input.data.objects.iter().find(|(_, data_object)| {
            data_object.source_key == host_call.source_key
                && data_object.source_statement == host_call.statement_index
        })?;
        return Some(SelectedInstructionKind::WriteRuntimeByte {
            source_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            source_offset: 0,
            literal,
            source_is_place: false,
        });
    }

    let HostCallArgumentKind::Expression(expression) = &first_argument.kind else {
        return None;
    };

    // `write_byte` accepts i32 while the checked Console adapters walk u8
    // views. Omega forbids implicit widening, so this path is deliberately
    // limited to the exact checked `u8 as i32` source form. The conversion is
    // value-preserving and the byte operation observes only the low byte;
    // reading the original u8 place is therefore the exact runtime meaning and
    // avoids manufacturing an unowned scratch value. Other casts continue to
    // the serve-or-refuse blocker.
    let storage_expression = match input.host_calls.expressions.expression(*expression) {
        psi_checked_trees::expression::ExpressionNode::Cast(cast)
            if !cast.form.is_recast()
                && input.program.primitive_type_reference(cast.target_type)
                    == Some(psi_checked_trees::types::PrimitiveType::I32)
                && classify_scalar_value_type_in_table(
                    input,
                    dispatch_index.unwrap_or(0),
                    host_call.source_key,
                    &input.host_calls.expressions,
                    cast.value,
                ) == Some(psi_checked_trees::types::PrimitiveType::U8) =>
        {
            cast.value
        }
        psi_checked_trees::expression::ExpressionNode::Cast(_) => return None,
        _ => *expression,
    };

    let place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index.unwrap_or(0),
        host_call.source_key,
        &input.host_calls.expressions,
        storage_expression,
    )?;
    Some(SelectedInstructionKind::WriteRuntimeByte {
        source_region: place.region,
        source_offset: place.byte_offset,
        literal: omega_abstract_operations::AbstractDataObjectHandle::invalid(),
        source_is_place: true,
    })
}

/// The `Byte(value)` payload word's ABSOLUTE offset inside a `ByteRead`
/// value, from the layout plan (tag at 0, payload packed after -- 4 today,
/// but the layout plan stays the single authority).
fn byte_read_payload_offset(input: &InstructionSelectionInput<'_>) -> Option<usize> {
    let data_layout = input
        .layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.name.as_str() == "ByteRead")
        .map(|(_, data_layout)| data_layout)?;
    let DataShape::Enum { variants, .. } = &data_layout.shape else {
        return None;
    };
    let variants = input.layouts.variants.span(*variants)?;
    let byte_variant = variants
        .iter()
        .find(|variant| variant.name.as_str() == "Byte")?;
    let fields = input.layouts.fields.span(byte_variant.fields)?;
    fields
        .iter()
        .find(|field| field.name.as_str() == "value")
        .map(|field| field.offset)
}
