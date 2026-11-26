// Named-text recovery restoring bounded typed policy meaning.
// `PackagePolicyTextRecoveryLimits` pairs a text-expansion byte ceiling with the underlying policy owned-storage budget (shared by reconstructed binary storage, typed allocations, and canonical verification scratch while input and text verification stay borrowed).

use crate::encoding::encode::encoder::text::{
    HEADER, MAXIMUM_MARKUP_DEPTH, MAXIMUM_TEXT_BYTES, Writer, label, render,
};
use crate::encoding::{
    PackagePolicyRecoveryError, PackagePolicyRecoveryError as Error, PackagePolicyRecoveryLimits,
    PackagePolicyRecoveryUsage,
};
use crate::record::PackagePolicyBaseline;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackagePolicyTextRecoveryLimits {
    maximum_text_bytes: usize,
    policy: PackagePolicyRecoveryLimits,
}

impl PackagePolicyTextRecoveryLimits {
    pub const fn new(maximum_text_bytes: usize, policy: PackagePolicyRecoveryLimits) -> Self {
        Self {
            maximum_text_bytes,
            policy,
        }
    }
}

impl Default for PackagePolicyTextRecoveryLimits {
    fn default() -> Self {
        Self::new(MAXIMUM_TEXT_BYTES, PackagePolicyRecoveryLimits::default())
    }
}

impl PackagePolicyBaseline {
    pub fn recover_text_with_usage(
        text: &str,
        limits: PackagePolicyTextRecoveryLimits,
    ) -> Result<(Self, PackagePolicyRecoveryUsage), PackagePolicyRecoveryError> {
        let maximum = limits.maximum_text_bytes.min(MAXIMUM_TEXT_BYTES);
        if text.len() > maximum {
            return Err(PackagePolicyRecoveryError::InputTooLarge);
        }
        let mut policy_limits = limits.policy.bounded();
        let (binary, reserved) = binary(text, policy_limits)?;
        policy_limits.maximum_owned_bytes = policy_limits
            .maximum_owned_bytes
            .checked_sub(reserved)
            .ok_or(PackagePolicyRecoveryError::AllocationLimitExceeded)?;
        let (policy, mut usage) = Self::recover_canonical_with_usage(&binary, policy_limits)?;
        render(&policy, Writer::new(maximum, Some(text)))
            .map_err(|_| PackagePolicyRecoveryError::NonCanonicalEncoding)?;
        usage.owned_bytes = usage
            .owned_bytes
            .checked_add(reserved)
            .ok_or(PackagePolicyRecoveryError::LengthOverflow)?;
        Ok((policy, usage))
    }
}

// Zero-allocation lexical scanner and escape decoder for named-text policy framing.
// `quoted` borrows the encoded field slice so counting and decoding can each traverse it without allocating a temporary unescaped string.

pub(in crate::encoding::recovery) struct Tokens<'text> {
    remaining: &'text [u8],
}

impl<'text> Tokens<'text> {
    pub(in crate::encoding::recovery) fn new(text: &'text str) -> Self {
        Self {
            remaining: text.as_bytes(),
        }
    }

    pub(in crate::encoding::recovery) fn whitespace(&mut self) {
        let count = self
            .remaining
            .iter()
            .take_while(|byte| byte.is_ascii_whitespace())
            .count();
        self.remaining = &self.remaining[count..];
    }

    pub(in crate::encoding::recovery) fn atom(&mut self) -> Result<&'text str, Error> {
        self.whitespace();
        let count = self
            .remaining
            .iter()
            .take_while(|byte| byte.is_ascii_graphic() && **byte != b'"')
            .count();
        if count == 0 || count > 64 {
            return Err(Error::InvalidValue);
        }
        let (token, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        std::str::from_utf8(token).map_err(|_| Error::InvalidUtf8)
    }

    pub(in crate::encoding::recovery) fn expect(&mut self, expected: &str) -> Result<(), Error> {
        if self.atom()? == expected {
            Ok(())
        } else {
            Err(Error::InvalidValue)
        }
    }

    pub(in crate::encoding::recovery) fn done(&mut self) -> bool {
        self.whitespace();
        self.remaining.is_empty()
    }

    pub(in crate::encoding::recovery) fn quoted(&mut self) -> Result<&'text [u8], Error> {
        self.whitespace();
        if self.remaining.first() != Some(&b'"') {
            return Err(Error::InvalidValue);
        }
        let source = &self.remaining[1..];
        let mut index = 0;
        loop {
            match source.get(index).copied().ok_or(Error::UnexpectedEnd)? {
                b'"' => {
                    self.remaining = &source[index + 1..];
                    return Ok(&source[..index]);
                }
                b'\\' => {
                    index += 1;
                    match source.get(index).copied().ok_or(Error::UnexpectedEnd)? {
                        b'"' | b'\\' => index += 1,
                        b'x' => {
                            nibble(*source.get(index + 1).ok_or(Error::UnexpectedEnd)?)?;
                            nibble(*source.get(index + 2).ok_or(Error::UnexpectedEnd)?)?;
                            index += 3;
                        }
                        _ => return Err(Error::InvalidValue),
                    }
                }
                0x20..=0x7e => index += 1,
                _ => return Err(Error::InvalidValue),
            }
        }
    }
}

pub(in crate::encoding::recovery) fn decoded(value: &[u8]) -> impl Iterator<Item = u8> + '_ {
    let mut index = 0;
    std::iter::from_fn(move || {
        let byte = *value.get(index)?;
        index += 1;
        if byte != b'\\' {
            return Some(byte);
        }
        let escaped = value[index];
        index += 1;
        if escaped != b'x' {
            return Some(escaped);
        }
        let byte = nibble(value[index]).expect("validated escape") * 16
            + nibble(value[index + 1]).expect("validated escape");
        index += 2;
        Some(byte)
    })
}

pub(in crate::encoding::recovery) fn nibble(byte: u8) -> Result<u8, Error> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(Error::InvalidValue),
    }
}

// Generic markup framing decoder reconstructing canonical binary scalar streams from named-text policies.
// Names and container shape are subsequently verified by exact policy rerender so this reader cannot interpret a renamed field as accepted policy meaning.

pub(in crate::encoding) fn binary(
    text: &str,
    limits: PackagePolicyRecoveryLimits,
) -> Result<(Vec<u8>, usize), Error> {
    let body = text.strip_prefix(HEADER).ok_or(Error::UnsupportedVersion)?;
    let mut reader = Reader {
        tokens: Tokens::new(body),
        output: Vec::new(),
        reserved: 0,
        depth: 0,
        limits: limits.bounded(),
    };
    while !reader.tokens.done() {
        let token = reader.tokens.atom()?;
        reader.value(token)?;
    }
    if reader.depth != 0 {
        return Err(Error::UnexpectedEnd);
    }
    Ok((reader.output, reader.reserved))
}

pub(in crate::encoding::recovery::policy::text) struct Reader<'text> {
    tokens: Tokens<'text>,
    output: Vec<u8>,
    reserved: usize,
    depth: usize,
    limits: PackagePolicyRecoveryLimits,
}

impl Reader<'_> {
    pub(in crate::encoding::recovery) fn reserve(&mut self, count: usize) -> Result<(), Error> {
        let required = self
            .output
            .len()
            .checked_add(count)
            .ok_or(Error::LengthOverflow)?;
        if required > self.limits.maximum_bytes {
            return Err(Error::InputTooLarge);
        }
        if required > self.limits.maximum_owned_bytes {
            return Err(Error::AllocationLimitExceeded);
        }
        if required > self.reserved {
            let desired = self
                .reserved
                .saturating_mul(2)
                .max(64)
                .max(required)
                .min(self.limits.maximum_bytes)
                .min(self.limits.maximum_owned_bytes);
            self.output
                .try_reserve_exact(desired - self.output.len())
                .map_err(|_| Error::AllocationFailed)?;
            self.reserved = desired;
        }
        Ok(())
    }

    pub(in crate::encoding::recovery) fn append(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.reserve(bytes.len())?;
        self.output.extend_from_slice(bytes);
        Ok(())
    }

    pub(in crate::encoding::recovery) fn number<T: FromStr>(&mut self) -> Result<T, Error> {
        self.tokens.atom()?.parse().map_err(|_| Error::InvalidValue)
    }

    pub(in crate::encoding::recovery) fn open(&mut self) -> Result<(), Error> {
        self.tokens.expect("{")?;
        if self.depth == MAXIMUM_MARKUP_DEPTH {
            return Err(Error::NestingLimitExceeded);
        }
        self.depth += 1;
        Ok(())
    }

    pub(in crate::encoding::recovery) fn value(&mut self, token: &str) -> Result<(), Error> {
        match token {
            "field" | "record" => {
                if !label(self.tokens.atom()?) {
                    return Err(Error::InvalidValue);
                }
                self.open()
            }
            "item" => self.open(),
            "}" => {
                self.depth = self.depth.checked_sub(1).ok_or(Error::InvalidValue)?;
                Ok(())
            }
            "sequence" => {
                let count: u64 = self.number()?;
                if count > self.limits.maximum_sequence_elements as u64 {
                    return Err(Error::ElementLimitExceeded);
                }
                self.append(&count.to_le_bytes())?;
                self.open()
            }
            "option" => match self.tokens.atom()? {
                "none" => self.append(&[0]),
                "some" => {
                    self.append(&[1])?;
                    self.open()
                }
                _ => Err(Error::InvalidTag),
            },
            "tag" => {
                if !label(self.tokens.atom()?) {
                    return Err(Error::InvalidValue);
                }
                let value: u8 = self.number()?;
                self.append(&[value])
            }
            "bool" => match self.tokens.atom()? {
                "false" => self.append(&[0]),
                "true" => self.append(&[1]),
                _ => Err(Error::InvalidTag),
            },
            "u8" => {
                let value: u8 = self.number()?;
                self.append(&[value])
            }
            "u16" => {
                let value: u16 = self.number()?;
                self.append(&value.to_le_bytes())
            }
            "u32" => {
                let value: u32 = self.number()?;
                self.append(&value.to_le_bytes())
            }
            "u64" => {
                let value: u64 = self.number()?;
                self.append(&value.to_le_bytes())
            }
            "i64" => {
                let value: i64 = self.number()?;
                self.append(&value.to_le_bytes())
            }
            "u128" => {
                let value: u128 = self.number()?;
                self.append(&value.to_le_bytes())
            }
            "i128" => {
                let value: i128 = self.number()?;
                self.append(&value.to_le_bytes())
            }
            "string" | "bytes" | "fixed" => {
                let encoded = self.tokens.quoted()?;
                let length = decoded(encoded).count();
                if length > self.limits.maximum_field_bytes {
                    return Err(Error::FieldTooLarge);
                }
                if token != "fixed" {
                    self.append(&(length as u64).to_le_bytes())?;
                }
                self.reserve(length)?;
                self.output.extend(decoded(encoded));
                Ok(())
            }
            "digest" => {
                let value = self.tokens.atom()?;
                if value.len() != 64 {
                    return Err(Error::InvalidIdentity);
                }
                let mut digest = [0; 32];
                for (target, pair) in digest.iter_mut().zip(value.as_bytes().as_chunks::<2>().0) {
                    *target = nibble(pair[0])? * 16 + nibble(pair[1])?;
                }
                self.append(&digest)
            }
            _ => Err(Error::InvalidValue),
        }
    }
}
