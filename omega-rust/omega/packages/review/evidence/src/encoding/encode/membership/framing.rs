//! Inverse of the evidence owner's length-framed semantic wrappers. Only
//! designated identity fields enter here; strings in literals/paths are opaque.

use super::{Observer, PackagePolicyMembershipError as Error, visitor::Visitor};
use psi_core::PackageKeyIdentity;

struct Fields<'a>(&'a str);
impl<'a> Fields<'a> {
    fn next(&mut self) -> Result<&'a str, Error> {
        let (length, rest) = self.0.split_once(':').ok_or(Error::MalformedIdentity)?;
        if length.is_empty() || (length.len() > 1 && length.starts_with('0')) {
            return Err(Error::MalformedIdentity);
        }
        let mut count = 0usize;
        for byte in length.bytes() {
            if !byte.is_ascii_digit() {
                return Err(Error::MalformedIdentity);
            }
            count = count
                .checked_mul(10)
                .and_then(|count| count.checked_add(usize::from(byte - b'0')))
                .ok_or(Error::MalformedIdentity)?;
        }
        let value = rest.get(..count).ok_or(Error::MalformedIdentity)?;
        self.0 = rest.get(count..).ok_or(Error::MalformedIdentity)?;
        Ok(value)
    }
    fn finish(self) -> Result<(), Error> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(Error::MalformedIdentity)
        }
    }
}

fn ordinal(value: &str) -> Result<(), Error> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(Error::MalformedIdentity);
    }
    value
        .parse::<u32>()
        .map(|_| ())
        .map_err(|_| Error::MalformedIdentity)
}

impl<Contains: FnMut(PackageKeyIdentity) -> bool> Visitor<Contains> {
    pub(super) fn visit_type(&mut self, identity: &str) -> Result<(), Error> {
        if identity.len() > 4 * 1024 * 1024 {
            return Err(Error::InvalidPolicy);
        }
        if !identity.as_bytes().first().is_some_and(u8::is_ascii_digit) {
            return self.runtime(identity);
        }
        self.nested(|visitor| {
            let mut fields = Fields(identity);
            if fields.next()? != "signature-type" {
                return Err(Error::MalformedIdentity);
            }
            visitor.runtime(fields.next()?)?;
            visitor.topology(fields.next()?)?;
            fields.finish()
        })
    }

    pub(super) fn visit_name(&mut self, name: &str) -> Result<(), Error> {
        if name.len() > 4 * 1024 * 1024 {
            return Err(Error::InvalidPolicy);
        }
        self.nested(|visitor| {
            if !name.as_bytes().first().is_some_and(u8::is_ascii_digit) {
                return Ok(());
            }
            let mut fields = Fields(name);
            match fields.next()? {
                "conformance-caller-binder" => {
                    visitor.owner_label(fields.next()?)?;
                    visitor.visit_name(fields.next()?)?;
                }
                "callback-static-parameter" => {
                    visitor.visit_name(fields.next()?)?;
                    ordinal(fields.next()?)?;
                }
                "boundary-operator-policy" => {
                    visitor.visit_name(fields.next()?)?;
                    // These are unqualified overload coordinates. The source-
                    // qualified signature is retained separately as typed fields.
                    fields.next()?;
                    fields.next()?;
                }
                "conformance-callable" => {
                    // Original declaration paths and dispatch-overload spelling
                    // are coordinates, not a second package-qualified type API.
                    for _ in 0..4 {
                        if fields.next()?.is_empty() {
                            return Err(Error::MalformedIdentity);
                        }
                    }
                    loop {
                        let mut child = Fields(fields.next()?);
                        match child.next()? {
                            "parameter" => {
                                visitor.visit_type(child.next()?)?;
                                if child.next()?.is_empty() {
                                    return Err(Error::MalformedIdentity);
                                }
                                child.finish()?;
                            }
                            "result-type" => {
                                visitor.visit_type(child.next()?)?;
                                child.finish()?;
                                break;
                            }
                            "result-none" => {
                                child.finish()?;
                                break;
                            }
                            _ => return Err(Error::MalformedIdentity),
                        }
                    }
                }
                _ => return Err(Error::MalformedIdentity),
            }
            fields.finish()
        })
    }

    fn owner_label(&mut self, label: &str) -> Result<(), Error> {
        let (kind, digest) = label.split_once(':').ok_or(Error::MalformedIdentity)?;
        if digest.len() != 64 {
            return Err(Error::MalformedIdentity);
        }
        let mut bytes = [0; 32];
        for (slot, pair) in bytes.iter_mut().zip(digest.as_bytes().chunks_exact(2)) {
            let digit = |byte| match byte {
                b'0'..=b'9' => Ok(byte - b'0'),
                b'a'..=b'f' => Ok(byte - b'a' + 10),
                _ => Err(Error::MalformedIdentity),
            };
            *slot = digit(pair[0])? * 16 + digit(pair[1])?;
        }
        match kind {
            "package" => self
                .package(PackageKeyIdentity::from_digest(bytes).ok_or(Error::MalformedIdentity)?),
            "toolchain-source" => Ok(()),
            _ => Err(Error::MalformedIdentity),
        }
    }

    fn topology(&mut self, topology: &str) -> Result<(), Error> {
        self.nested(|visitor| {
            if matches!(
                topology,
                "named" | "unit" | "dynamic-trait" | "const-expression" | "elided"
            ) {
                return Ok(());
            }
            if let Some(value) = topology.strip_prefix("binder:") {
                return ordinal(value);
            }
            let mut fields = Fields(topology);
            match fields.next()? {
                "reference" => {
                    visitor.topology(fields.next()?)?;
                    visitor.topology(fields.next()?)?;
                }
                "array" | "slice" => visitor.topology(fields.next()?)?,
                "generic" => {
                    while !fields.0.is_empty() {
                        visitor.topology(fields.next()?)?;
                    }
                }
                "constrained" => {
                    visitor.topology(fields.next()?)?;
                    while !fields.0.is_empty() {
                        visitor.constraint(fields.next()?)?;
                    }
                }
                _ => return Err(Error::MalformedIdentity),
            }
            fields.finish()
        })
    }

    fn constraint(&mut self, value: &str) -> Result<(), Error> {
        self.nested(|visitor| {
            let mut arguments = Fields(value);
            let mut label = Fields(arguments.next()?);
            match label.next()? {
                "declared-domain" => {
                    visitor.owner_label(label.next()?)?;
                    visitor.visit_name(label.next()?)?;
                }
                "compiler-domain" => {
                    if label.next()?.is_empty() || label.next()?.is_empty() {
                        return Err(Error::MalformedIdentity);
                    }
                }
                _ => return Err(Error::MalformedIdentity),
            }
            label.finish()?;
            // The producer emits these topology rows only for domains with arguments.
            visitor.topology(arguments.next()?)?;
            while !arguments.0.is_empty() {
                visitor.topology(arguments.next()?)?;
            }
            Ok(())
        })
    }
}
