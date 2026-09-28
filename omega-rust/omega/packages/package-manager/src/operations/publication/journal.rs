//! Local recovery bytes for the two fixed package files, without audit authority.
//!
//! Each present row is `name length\n`, exactly `length` raw bytes, and `\n`.
//! Only `before-lock` may instead be `before-lock absent\n` with no payload.

use super::{PackagePublicationError, PackagePublicationLimits};

const HEADER: &[u8] = b"omega-package-transaction 1\n";
const BEFORE_BUILD: &[u8] = b"before-build ";
const AFTER_BUILD: &[u8] = b"after-build ";
const BEFORE_LOCK: &[u8] = b"before-lock ";
const AFTER_LOCK: &[u8] = b"after-lock ";
const ABSENT: &[u8] = b"absent\n";
const DECIMAL_CAPACITY: usize = usize::MAX.ilog10() as usize + 1;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct PackageFileJournal {
    pub before_build: Vec<u8>,
    pub after_build: Vec<u8>,
    pub before_lock: Option<Vec<u8>>,
    pub after_lock: Vec<u8>,
}

impl PackageFileJournal {
    pub(super) fn encode(
        &self,
        limits: PackagePublicationLimits,
    ) -> Result<Vec<u8>, PackagePublicationError> {
        let rows = [
            (BEFORE_BUILD, Some(self.before_build.as_slice())),
            (AFTER_BUILD, Some(self.after_build.as_slice())),
            (BEFORE_LOCK, self.before_lock.as_deref()),
            (AFTER_LOCK, Some(self.after_lock.as_slice())),
        ];
        let mut total = HEADER.len();
        for (name, payload) in rows {
            let mut decimal = [0; DECIMAL_CAPACITY];
            let row_length = if let Some(payload) = payload {
                if payload.len() > limits.maximum_file_bytes {
                    return Err(PackagePublicationError::ByteLimitExceeded);
                }
                payload
                    .len()
                    .checked_add(decimal_bytes(payload.len(), &mut decimal).len())
                    .and_then(|length| length.checked_add(2))
                    .ok_or(PackagePublicationError::ByteLimitExceeded)?
            } else {
                ABSENT.len()
            };
            total = total
                .checked_add(name.len())
                .and_then(|length| length.checked_add(row_length))
                .ok_or(PackagePublicationError::ByteLimitExceeded)?;
        }
        if total > limits.maximum_journal_bytes {
            return Err(PackagePublicationError::ByteLimitExceeded);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total)
            .map_err(|_| PackagePublicationError::AllocationFailed)?;
        bytes.extend_from_slice(HEADER);
        for (name, payload) in rows {
            bytes.extend_from_slice(name);
            if let Some(payload) = payload {
                let mut decimal = [0; DECIMAL_CAPACITY];
                bytes.extend_from_slice(decimal_bytes(payload.len(), &mut decimal));
                bytes.push(b'\n');
                bytes.extend_from_slice(payload);
                bytes.push(b'\n');
            } else {
                bytes.extend_from_slice(ABSENT);
            }
        }
        Ok(bytes)
    }

    pub(super) fn recover(
        bytes: &[u8],
        limits: PackagePublicationLimits,
    ) -> Result<Self, PackagePublicationError> {
        if bytes.len() > limits.maximum_journal_bytes {
            return Err(PackagePublicationError::ByteLimitExceeded);
        }
        let mut remaining = bytes
            .strip_prefix(HEADER)
            .ok_or(PackagePublicationError::InvalidJournal("invalid header"))?;
        let before_build = read_required(&mut remaining, BEFORE_BUILD, &limits)?;
        let after_build = read_required(&mut remaining, AFTER_BUILD, &limits)?;
        consume(&mut remaining, BEFORE_LOCK)?;
        let before_lock = if let Some(rest) = remaining.strip_prefix(ABSENT) {
            remaining = rest;
            None
        } else {
            Some(read_payload(&mut remaining, &limits)?)
        };
        let after_lock = read_required(&mut remaining, AFTER_LOCK, &limits)?;
        if !remaining.is_empty() {
            return Err(PackagePublicationError::InvalidJournal("trailing bytes"));
        }

        // Validate the complete envelope before allocating any recovered fields.
        Ok(Self {
            before_build: copy_payload(before_build)?,
            after_build: copy_payload(after_build)?,
            before_lock: before_lock.map(copy_payload).transpose()?,
            after_lock: copy_payload(after_lock)?,
        })
    }
}

fn decimal_bytes(mut length: usize, storage: &mut [u8; DECIMAL_CAPACITY]) -> &[u8] {
    let mut start = storage.len();
    loop {
        start -= 1;
        storage[start] = b'0' + (length % 10) as u8;
        length /= 10;
        if length == 0 {
            return &storage[start..];
        }
    }
}

fn consume(remaining: &mut &[u8], prefix: &[u8]) -> Result<(), PackagePublicationError> {
    *remaining = remaining
        .strip_prefix(prefix)
        .ok_or(PackagePublicationError::InvalidJournal("unexpected row"))?;
    Ok(())
}

fn read_required<'a>(
    remaining: &mut &'a [u8],
    name: &[u8],
    limits: &PackagePublicationLimits,
) -> Result<&'a [u8], PackagePublicationError> {
    consume(remaining, name)?;
    read_payload(remaining, limits)
}

fn read_payload<'a>(
    remaining: &mut &'a [u8],
    limits: &PackagePublicationLimits,
) -> Result<&'a [u8], PackagePublicationError> {
    let end = remaining.iter().position(|byte| *byte == b'\n').ok_or(
        PackagePublicationError::InvalidJournal("missing length terminator"),
    )?;
    let digits = &remaining[..end];
    if digits.is_empty()
        || (digits.len() > 1 && digits[0] == b'0')
        || !digits.iter().all(u8::is_ascii_digit)
    {
        return Err(PackagePublicationError::InvalidJournal(
            "noncanonical length",
        ));
    }
    let mut length = 0usize;
    for digit in digits {
        length = length
            .checked_mul(10)
            .and_then(|length| length.checked_add(usize::from(*digit - b'0')))
            .ok_or(PackagePublicationError::InvalidJournal("length overflow"))?;
    }
    if length > limits.maximum_file_bytes {
        return Err(PackagePublicationError::ByteLimitExceeded);
    }
    let payload_and_rest = &remaining[end + 1..];
    let payload = payload_and_rest
        .get(..length)
        .ok_or(PackagePublicationError::InvalidJournal("truncated payload"))?;
    *remaining = payload_and_rest[length..].strip_prefix(b"\n").ok_or(
        PackagePublicationError::InvalidJournal("missing payload terminator"),
    )?;
    Ok(payload)
}

fn copy_payload(payload: &[u8]) -> Result<Vec<u8>, PackagePublicationError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(payload.len())
        .map_err(|_| PackagePublicationError::AllocationFailed)?;
    bytes.extend_from_slice(payload);
    Ok(bytes)
}

#[cfg(test)]
mod tests;
