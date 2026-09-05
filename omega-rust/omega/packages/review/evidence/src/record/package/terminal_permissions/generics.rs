//! Exact declaration binder association without parsing or allocating identities.

use super::PackagePolicyTerminalService;
use std::fmt::Write;

pub(super) fn validate(service: &PackagePolicyTerminalService) -> Result<(), &'static str> {
    for method in &service.methods {
        if method.signature.schema_lifetime_parameter_count != service.lifetime_parameter_count
            || method.signature.schema_arguments.len() != service.static_parameters.len()
            || (!service.static_parameters.is_empty() && method.calling.is_some())
            || method
                .parameter_type_identities
                .iter()
                .map(String::as_str)
                .ne(method
                    .signature
                    .parameters
                    .iter()
                    .map(|parameter| parameter.type_identity.canonical.as_str()))
            || method.result_type_identity.as_deref()
                != method
                    .signature
                    .result
                    .as_ref()
                    .map(|result| result.canonical.as_str())
        {
            return Err("terminal permission method changes its service declaration telescope");
        }
        for (ordinal, argument) in method.signature.schema_arguments.iter().enumerate() {
            let mut comparison = IdentityComparison {
                remaining: &argument.canonical,
            };
            if write_service_parameter_identity(&mut comparison, ordinal).is_err()
                || !comparison.remaining.is_empty()
            {
                return Err(
                    "terminal permission root argument is not its exact declaration binder",
                );
            }
        }
    }
    Ok(())
}

/// The typed Named projector produces named(name(service-parameter:N)) with
/// Named lifetime topology. Reuse semantic framing rather than decoding text.
pub(crate) fn write_service_parameter_identity(
    writer: &mut impl Write,
    ordinal: usize,
) -> std::fmt::Result {
    let mut runtime = StackIdentity {
        bytes: [0; 96],
        length: 0,
    };
    write!(runtime, "named(name(service-parameter:{ordinal}))")?;
    let runtime =
        std::str::from_utf8(&runtime.bytes[..runtime.length]).map_err(|_| std::fmt::Error)?;
    crate::record::write_framed_identity(writer, "signature-type", [runtime, "named"])
}

struct StackIdentity {
    bytes: [u8; 96],
    length: usize,
}

impl Write for StackIdentity {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        let end = self
            .length
            .checked_add(value.len())
            .ok_or(std::fmt::Error)?;
        self.bytes
            .get_mut(self.length..end)
            .ok_or(std::fmt::Error)?
            .copy_from_slice(value.as_bytes());
        self.length = end;
        Ok(())
    }
}

struct IdentityComparison<'a> {
    remaining: &'a str,
}

impl Write for IdentityComparison<'_> {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.remaining = self.remaining.strip_prefix(value).ok_or(std::fmt::Error)?;
        Ok(())
    }
}
