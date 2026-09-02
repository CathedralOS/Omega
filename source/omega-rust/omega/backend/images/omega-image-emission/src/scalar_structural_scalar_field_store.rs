//! Independent object replay for the direct mutable-self scalar-store carrier.

use omega_calling_conventions::{IndirectPointerLocation, ValueClass, ValueLocation};
use omega_machine_code::{MachineCodeFunction, SemanticCodeSite};
use omega_target::{Architecture, NativeTarget};
use psi_core::{IntegerSign, IntegerType};

use crate::ObjectError;
use crate::instruction_loads::{aarch64_terminal_register, x86_terminal_register};
use crate::unit_structural_scalar_field_store::integer_bits;

pub(super) fn validate_scalar_structural_scalar_field_store(
    target: NativeTarget,
    function: &MachineCodeFunction,
) -> Result<(), ObjectError> {
    let Some(store) = &function.scalar_structural_scalar_field_store else {
        return Ok(());
    };
    let invalid = || ObjectError::InvalidScalarStructuralScalarFieldStoreEvidence(function.machine);
    let parameter_index = usize::try_from(store.destination.position).map_err(|_| invalid())?;
    let parameter = function
        .scalar_structural_parameters
        .get(parameter_index)
        .ok_or_else(invalid)?;
    let home = function
        .scalar_structural_parameter_homes
        .get(parameter_index)
        .ok_or_else(invalid)?;
    let width = store
        .scalar_type
        .bits()
        .checked_div(8)
        .ok_or_else(invalid)?;
    if !store.destination.is_self
        || function.attachment != Some(store.destination.structural_type)
        || store.destination.access != psi_terminal::StructuralAccess::MutableBorrow
        || !matches!(
            store.destination.multiplicity,
            psi_terminal::StructuralMultiplicity::Unrestricted
                | psi_terminal::StructuralMultiplicity::Affine
        )
        || !store.destination.qualifications.is_empty()
        || !store.destination.projected_qualifications.is_empty()
        || !store.path.is_empty()
        || parameter.place != store.destination.place
        || parameter.structural_type != store.destination.structural_type
        || parameter.multiplicity != store.destination.multiplicity
        || parameter.shape.class != ValueClass::BorrowedReference
        || home.place != parameter.place
        || home.structural_type != parameter.structural_type
        || home.multiplicity != parameter.multiplicity
        || home.shape != parameter.shape
        || !home.indirect
        || home.source != store.destination_placement
        || store.destination_placement.shape != parameter.shape
        || !matches!(
            store.destination_placement.locations.as_slice(),
            [ValueLocation::Indirect {
                copy_stack_byte_offset: None,
                ..
            }]
        )
        || !matches!(store.scalar_type.bits(), 8 | 16 | 32 | 64)
        || store.scalar_type.is_address()
        || !store.scalar_type.admits(store.value)
        || store
            .field_byte_offset
            .checked_add(u32::from(width))
            .is_none_or(|end| end > u32::from(parameter.shape.byte_size))
        || !function
            .provenance
            .operations
            .contains(&store.defining_operation)
        || !function
            .provenance
            .operations
            .contains(&store.psi_operation)
        || store.operation_ordinal != 1
        || store.code_offset != 0
        || exact_attribution_count(function, store) != 1
    {
        return Err(invalid());
    }
    let bits = integer_bits(store.scalar_type, store.value).ok_or_else(invalid)?;
    let (expected_store, expected_function) = match target.architecture {
        Architecture::X86_64 => expected_x86(store, width, bits),
        Architecture::Aarch64 => expected_aarch64(store, width, bits),
    }
    .ok_or_else(invalid)?;
    if store.byte_count == 0
        || store.byte_count != expected_store.len()
        || store.bytes != expected_store
        || function.bytes != expected_function
    {
        return Err(invalid());
    }
    Ok(())
}

fn exact_attribution_count(
    function: &MachineCodeFunction,
    store: &omega_machine_code::ScalarStructuralScalarFieldStoreRecord,
) -> usize {
    function
        .semantic_code_attribution
        .iter()
        .filter(|row| {
            row.site == SemanticCodeSite::Operation(store.psi_operation)
                && row.operation_ordinal == store.operation_ordinal
                && row.code_offset == store.code_offset
                && row.byte_count == store.byte_count
        })
        .count()
}

fn expected_x86(
    store: &omega_machine_code::ScalarStructuralScalarFieldStoreRecord,
    width: u16,
    bits: u64,
) -> Option<(Vec<u8>, Vec<u8>)> {
    const ADDRESS_REGISTER: u8 = 10;
    const READ_ADDRESS_REGISTER: u8 = 11;
    const VALUE_REGISTER: u8 = 11;
    let [ValueLocation::Indirect { pointer, .. }] =
        store.destination_placement.locations.as_slice()
    else {
        return None;
    };
    let mut bytes = vec![0x49, 0xb8 | (VALUE_REGISTER & 7)];
    bytes.extend_from_slice(&bits.to_le_bytes());
    let store_base = match *pointer {
        IndirectPointerLocation::Register(register) => x86_terminal_register(register)?,
        IndirectPointerLocation::Stack {
            stack_byte_offset, ..
        } => {
            x86_stack_load(
                &mut bytes,
                ADDRESS_REGISTER,
                stack_byte_offset.checked_add(8)?,
            );
            ADDRESS_REGISTER
        }
    };
    x86_memory_store(
        &mut bytes,
        VALUE_REGISTER,
        store_base,
        store.field_byte_offset,
        width,
    )?;
    let store_bytes = bytes.clone();
    let read_base = match *pointer {
        IndirectPointerLocation::Register(register) => x86_terminal_register(register)?,
        IndirectPointerLocation::Stack {
            stack_byte_offset, ..
        } => {
            x86_stack_load(
                &mut bytes,
                READ_ADDRESS_REGISTER,
                stack_byte_offset.checked_add(8)?,
            );
            READ_ADDRESS_REGISTER
        }
    };
    x86_memory_load(&mut bytes, 0, read_base, store.field_byte_offset, width)?;
    x86_normalize(&mut bytes, store.scalar_type)?;
    bytes.push(0xc3);
    Some((store_bytes, bytes))
}

fn x86_stack_load(bytes: &mut Vec<u8>, register: u8, offset: u32) {
    bytes.extend_from_slice(&[0x48 | (((register >> 3) & 1) << 2), 0x8b]);
    if offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x44 | ((register & 7) << 3), 0x24, offset as u8]);
    } else {
        bytes.extend_from_slice(&[0x84 | ((register & 7) << 3), 0x24]);
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
}

fn x86_width_prefix(bytes: &mut Vec<u8>, register: u8, base: u8, width: u16) -> Option<()> {
    let extension = (((register >> 3) & 1) << 2) | ((base >> 3) & 1);
    match width {
        1 | 4 => bytes.push(0x40 | extension),
        2 => bytes.extend_from_slice(&[0x66, 0x40 | extension]),
        8 => bytes.push(0x48 | extension),
        _ => return None,
    }
    Some(())
}

fn x86_memory_store(
    bytes: &mut Vec<u8>,
    source: u8,
    base: u8,
    offset: u32,
    width: u16,
) -> Option<()> {
    x86_width_prefix(bytes, source, base, width)?;
    bytes.push(if width == 1 { 0x88 } else { 0x89 });
    x86_modrm(bytes, source, base, offset);
    Some(())
}

fn x86_memory_load(
    bytes: &mut Vec<u8>,
    destination: u8,
    base: u8,
    offset: u32,
    width: u16,
) -> Option<()> {
    match width {
        1 => {
            bytes.push(0x40 | (((destination >> 3) & 1) << 2) | ((base >> 3) & 1));
            bytes.extend_from_slice(&[0x0f, 0xb6]);
        }
        2 => {
            bytes.push(0x66);
            bytes.push(0x40 | (((destination >> 3) & 1) << 2) | ((base >> 3) & 1));
            bytes.extend_from_slice(&[0x0f, 0xb7]);
        }
        4 => {
            bytes.push(0x40 | (((destination >> 3) & 1) << 2) | ((base >> 3) & 1));
            bytes.push(0x8b);
        }
        8 => {
            bytes.push(0x48 | (((destination >> 3) & 1) << 2) | ((base >> 3) & 1));
            bytes.push(0x8b);
        }
        _ => return None,
    }
    x86_modrm(bytes, destination, base, offset);
    Some(())
}

fn x86_modrm(bytes: &mut Vec<u8>, register: u8, base: u8, offset: u32) {
    if offset == 0 && (base & 7) != 5 {
        bytes.push(((register & 7) << 3) | (base & 7));
    } else if offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x40 | ((register & 7) << 3) | (base & 7), offset as u8]);
    } else {
        bytes.push(0x80 | ((register & 7) << 3) | (base & 7));
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
}

fn x86_normalize(bytes: &mut Vec<u8>, scalar_type: IntegerType) -> Option<()> {
    match (scalar_type.sign(), scalar_type.bits()) {
        (_, 64) => {}
        (IntegerSign::Unsigned, 8) => bytes.extend_from_slice(&[0x25, 0xff, 0, 0, 0]),
        (IntegerSign::Unsigned, 16) => bytes.extend_from_slice(&[0x25, 0xff, 0xff, 0, 0]),
        (IntegerSign::Unsigned, 32) => bytes.extend_from_slice(&[0x89, 0xc0]),
        (IntegerSign::Signed, 8) => bytes.extend_from_slice(&[0x48, 0x0f, 0xbe, 0xc0]),
        (IntegerSign::Signed, 16) => bytes.extend_from_slice(&[0x48, 0x0f, 0xbf, 0xc0]),
        (IntegerSign::Signed, 32) => bytes.extend_from_slice(&[0x48, 0x63, 0xc0]),
        _ => return None,
    }
    Some(())
}

fn expected_aarch64(
    store: &omega_machine_code::ScalarStructuralScalarFieldStoreRecord,
    width: u16,
    bits: u64,
) -> Option<(Vec<u8>, Vec<u8>)> {
    const ADDRESS_REGISTER: u8 = 17;
    const READ_ADDRESS_REGISTER: u8 = 9;
    const VALUE_REGISTER: u8 = 16;
    let [ValueLocation::Indirect { pointer, .. }] =
        store.destination_placement.locations.as_slice()
    else {
        return None;
    };
    let mut instructions = Vec::new();
    for chunk in 0..4 {
        let immediate = ((bits >> (chunk * 16)) & 0xffff) as u32;
        if chunk == 0 || immediate != 0 {
            let base = if chunk == 0 { 0xd280_0000 } else { 0xf280_0000 };
            instructions
                .push(base | ((chunk as u32) << 21) | (immediate << 5) | u32::from(VALUE_REGISTER));
        }
    }
    let store_base = match *pointer {
        IndirectPointerLocation::Register(register) => aarch64_terminal_register(register)?,
        IndirectPointerLocation::Stack {
            stack_byte_offset, ..
        } => {
            instructions.push(aarch64_access(
                0xf940_0000,
                ADDRESS_REGISTER,
                31,
                stack_byte_offset,
                8,
            )?);
            ADDRESS_REGISTER
        }
    };
    instructions.push(aarch64_access(
        aarch64_store_base(width)?,
        VALUE_REGISTER,
        store_base,
        store.field_byte_offset,
        width,
    )?);
    let store_bytes = instructions
        .iter()
        .flat_map(|instruction| instruction.to_le_bytes())
        .collect::<Vec<_>>();
    let read_base = match *pointer {
        IndirectPointerLocation::Register(register) => aarch64_terminal_register(register)?,
        IndirectPointerLocation::Stack {
            stack_byte_offset, ..
        } => {
            instructions.push(aarch64_access(
                0xf940_0000,
                READ_ADDRESS_REGISTER,
                31,
                stack_byte_offset,
                8,
            )?);
            READ_ADDRESS_REGISTER
        }
    };
    instructions.push(aarch64_access(
        aarch64_load_base(width)?,
        0,
        read_base,
        store.field_byte_offset,
        width,
    )?);
    if store.scalar_type.bits() != 64 {
        let base = match store.scalar_type.sign() {
            IntegerSign::Signed => 0x9340_0000,
            IntegerSign::Unsigned => 0xd340_0000,
        };
        instructions.push(base | (u32::from(store.scalar_type.bits() - 1) << 10));
    }
    instructions.push(0xd65f_03c0);
    Some((
        store_bytes,
        instructions
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect(),
    ))
}

fn aarch64_load_base(width: u16) -> Option<u32> {
    Some(match width {
        1 => 0x3940_0000,
        2 => 0x7940_0000,
        4 => 0xb940_0000,
        8 => 0xf940_0000,
        _ => return None,
    })
}

fn aarch64_store_base(width: u16) -> Option<u32> {
    Some(match width {
        1 => 0x3900_0000,
        2 => 0x7900_0000,
        4 => 0xb900_0000,
        8 => 0xf900_0000,
        _ => return None,
    })
}

fn aarch64_access(
    base: u32,
    register: u8,
    address_register: u8,
    offset: u32,
    width: u16,
) -> Option<u32> {
    let scale = u32::from(width);
    (scale != 0 && offset.is_multiple_of(scale) && offset / scale <= 0xfff).then_some(
        base | ((offset / scale) << 10) | (u32::from(address_register) << 5) | u32::from(register),
    )
}
