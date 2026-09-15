//! Decode each required access, register operation and displacement independently.
use super::X86_64SelectedFormEncodingError as Error;

pub(super) fn validate(
    bytes: &[u8],
    load: bool,
    width: u8,
    [base, value, scratch]: [u8; 3],
    offset: u32,
) -> Result<(), Error> {
    let mut remaining = bytes;
    if !load {
        register_operation(&mut remaining, 0x89, value, scratch)?;
    }
    for byte_index in 0..width {
        let destination = if load && byte_index == 0 {
            value
        } else {
            scratch
        };
        access(
            &mut remaining,
            load,
            destination,
            base,
            offset + u32::from(byte_index),
        )?;
        if load && byte_index != 0 {
            shift(&mut remaining, scratch, 4, byte_index * 8)?;
            register_operation(&mut remaining, 0x09, scratch, value)?;
        } else if !load && byte_index + 1 != width {
            shift(&mut remaining, scratch, 5, 8)?;
        }
    }
    if remaining.is_empty() {
        Ok(())
    } else {
        Err(Error::MalformedEncoding)
    }
}

fn take<'a>(bytes: &mut &'a [u8], count: usize) -> Result<&'a [u8], Error> {
    let (head, tail) = bytes
        .split_at_checked(count)
        .ok_or(Error::MalformedEncoding)?;
    *bytes = tail;
    Ok(head)
}

fn register_operation(
    bytes: &mut &[u8],
    opcode: u8,
    source: u8,
    destination: u8,
) -> Result<(), Error> {
    let encoded = take(bytes, 3)?;
    let prefix = encoded[0];
    let mode = encoded[2];
    if prefix & 0xfa != 0x48
        || encoded[1] != opcode
        || mode >> 6 != 3
        || ((mode >> 3) & 7) | ((prefix & 4) << 1) != source
        || (mode & 7) | ((prefix & 1) << 3) != destination
    {
        return Err(Error::EncodedFormMismatch);
    }
    Ok(())
}

fn shift(bytes: &mut &[u8], register: u8, operation: u8, amount: u8) -> Result<(), Error> {
    let encoded = take(bytes, 4)?;
    if encoded[0] & 0xfe != 0x48
        || encoded[1] != 0xc1
        || encoded[2] >> 6 != 3
        || (encoded[2] >> 3) & 7 != operation
        || (encoded[2] & 7) | ((encoded[0] & 1) << 3) != register
        || encoded[3] != amount
    {
        return Err(Error::EncodedFormMismatch);
    }
    Ok(())
}

fn access(bytes: &mut &[u8], load: bool, register: u8, base: u8, offset: u32) -> Result<(), Error> {
    let prefix = take(bytes, 1)?[0];
    if prefix & 0xfa != if load { 0x48 } else { 0x40 } {
        return Err(Error::EncodedFormMismatch);
    }
    if load && take(bytes, 1)?[0] != 0x0f {
        return Err(Error::EncodedFormMismatch);
    }
    let header = take(bytes, 2)?;
    let mode = header[1];
    if header[0] != if load { 0xb6 } else { 0x88 }
        || mode >> 6 != 2
        || ((mode >> 3) & 7) | ((prefix & 4) << 1) != register
        || (mode & 7) | ((prefix & 1) << 3) != base
    {
        return Err(Error::EncodedFormMismatch);
    }
    if mode & 7 == 4 && take(bytes, 1)?[0] != 0x24 {
        return Err(Error::EncodedFormMismatch);
    }
    let displacement =
        <[u8; 4]>::try_from(take(bytes, 4)?).map_err(|_| Error::MalformedEncoding)?;
    if i32::from_le_bytes(displacement) < 0 || u32::from_le_bytes(displacement) != offset {
        return Err(Error::EncodedFormMismatch);
    }
    Ok(())
}
