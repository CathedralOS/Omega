use super::tokens::{Tokens, decoded, nibble};
use crate::encoding::encode::encoder::text::{HEADER, MAXIMUM_MARKUP_DEPTH, label};
use crate::encoding::{PackagePolicyRecoveryError as Error, PackagePolicyRecoveryLimits};

/// The generic grammar reconstructs the existing canonical scalar stream.
/// Names and container shape are subsequently checked by exact policy rerender;
/// this reader cannot interpret a renamed field as accepted policy meaning.
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

struct Reader<'text> {
    tokens: Tokens<'text>,
    output: Vec<u8>,
    reserved: usize,
    depth: usize,
    limits: PackagePolicyRecoveryLimits,
}

impl Reader<'_> {
    fn reserve(&mut self, count: usize) -> Result<(), Error> {
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

    fn append(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.reserve(bytes.len())?;
        self.output.extend_from_slice(bytes);
        Ok(())
    }

    fn number<T: std::str::FromStr>(&mut self) -> Result<T, Error> {
        self.tokens.atom()?.parse().map_err(|_| Error::InvalidValue)
    }

    fn open(&mut self) -> Result<(), Error> {
        self.tokens.expect("{")?;
        if self.depth == MAXIMUM_MARKUP_DEPTH {
            return Err(Error::NestingLimitExceeded);
        }
        self.depth += 1;
        Ok(())
    }

    fn value(&mut self, token: &str) -> Result<(), Error> {
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
