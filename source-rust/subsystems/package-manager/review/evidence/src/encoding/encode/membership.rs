// Source-graph package membership verification across retained semantic package owners without admission or declaration claims.

#[cfg(test)]
mod tests;
mod visitor;

use self::PackagePolicyMembershipError as Error;
use self::visitor::Visitor;
use crate::record::PackagePolicyBaseline;
use semantic_vocabulary::PackageKeyIdentity;

pub(super) trait Observer {
    fn package(&mut self, package: PackageKeyIdentity) -> Result<(), PackagePolicyMembershipError>;
    fn type_identity(&mut self, identity: &str) -> Result<(), PackagePolicyMembershipError>;
    fn declared_type_identity(
        &mut self,
        identity: &str,
        lifetimes: usize,
    ) -> Result<(), PackagePolicyMembershipError>;
    fn static_argument(&mut self, identity: &str) -> Result<(), PackagePolicyMembershipError>;
    fn service_application(&mut self, identity: &str) -> Result<(), PackagePolicyMembershipError>;
    fn nominal_path(&mut self, path: &str) -> Result<(), PackagePolicyMembershipError>;
}

impl PackagePolicyBaseline {
    pub fn validate_package_membership(
        &self,
        contains: impl FnMut(PackageKeyIdentity) -> bool,
        limits: PackagePolicyMembershipLimits,
    ) -> Result<PackagePolicyMembershipUsage, PackagePolicyMembershipFailure> {
        let mut visitor = visitor::Visitor::new(contains, limits);
        let mut encoder = super::encoder::Encoder::policy_membership(&mut visitor);
        let result = super::baseline::framed_policy(&mut encoder, self);
        let membership_error = encoder.membership_error();
        drop(encoder);
        if let Some(error) = membership_error {
            return Err(PackagePolicyMembershipFailure::new(
                error,
                visitor.unreadable_identity(),
            ));
        }
        result.map_err(|_| PackagePolicyMembershipError::InvalidPolicy)?;
        Ok(visitor.usage())
    }
}

// Inverse reader of length-framed semantic identity wrappers (`signature-type`, `conformance-callable`, and topology constraints) for package membership verification.

pub(in crate::encoding::encode::membership) struct Fields<'a>(&'a str);
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

pub(in crate::encoding::encode::membership) fn ordinal(value: &str) -> Result<(), Error> {
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
    pub(super) fn visit_type(
        &mut self,
        identity: &str,
        lifetimes: Option<usize>,
    ) -> Result<(), Error> {
        if identity.len() > 4 * 1024 * 1024 {
            return Err(Error::InvalidPolicy);
        }
        if !identity.as_bytes().first().is_some_and(u8::is_ascii_digit) {
            return self.canonical_type(identity, lifetimes);
        }
        self.nested(|visitor| {
            let mut fields = Fields(identity);
            if fields.next()? != "signature-type" {
                return Err(Error::MalformedIdentity);
            }
            visitor.canonical_type(fields.next()?, None)?;
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
                    fields.next()?;
                    fields.next()?;
                }
                "conformance-callable" => {
                    for _ in 0..4 {
                        if fields.next()?.is_empty() {
                            return Err(Error::MalformedIdentity);
                        }
                    }
                    loop {
                        let mut child = Fields(fields.next()?);
                        match child.next()? {
                            "parameter" => {
                                visitor.visit_type(child.next()?, None)?;
                                if child.next()?.is_empty() {
                                    return Err(Error::MalformedIdentity);
                                }
                                child.finish()?;
                            }
                            "result-type" => {
                                visitor.visit_type(child.next()?, None)?;
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
        for (slot, pair) in bytes.iter_mut().zip(digest.as_bytes().as_chunks::<2>().0) {
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
            visitor.topology(arguments.next()?)?;
            while !arguments.0.is_empty() {
                visitor.topology(arguments.next()?)?;
            }
            Ok(())
        })
    }
}

// Work budgets, usage counters, failure kinds, and bounded malformed-identity excerpts for semantic package-owner membership verification.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackagePolicyMembershipLimits {
    pub(super) maximum_owned_bytes: usize,
    pub(super) maximum_identity_nodes: usize,
    pub(super) maximum_depth: usize,
}

impl PackagePolicyMembershipLimits {
    pub const fn new(
        maximum_owned_bytes: usize,
        maximum_identity_nodes: usize,
        maximum_depth: usize,
    ) -> Self {
        Self {
            maximum_owned_bytes,
            maximum_identity_nodes,
            maximum_depth,
        }
    }
    pub const fn maximum_owned_bytes(self) -> usize {
        self.maximum_owned_bytes
    }
    pub const fn maximum_identity_nodes(self) -> usize {
        self.maximum_identity_nodes
    }
    pub const fn maximum_depth(self) -> usize {
        self.maximum_depth
    }
    pub(super) fn bounded(self) -> Self {
        Self::new(
            self.maximum_owned_bytes.min(64 * 1024 * 1024),
            self.maximum_identity_nodes.min(2 * 128 * 1024 * 1024),
            self.maximum_depth.min(128),
        )
    }
}

impl Default for PackagePolicyMembershipLimits {
    fn default() -> Self {
        Self::new(64 * 1024 * 1024, 1_048_576, 128)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PackagePolicyMembershipUsage {
    pub(super) owned_bytes: usize,
    pub(super) identity_nodes: usize,
}
impl PackagePolicyMembershipUsage {
    pub const fn owned_bytes(self) -> usize {
        self.owned_bytes
    }
    pub const fn identity_nodes(self) -> usize {
        self.identity_nodes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackagePolicyMembershipError {
    UnknownPackage { package: PackageKeyIdentity },
    OwnedBytesLimitExceeded,
    IdentityNodeLimitExceeded,
    NestingLimitExceeded,
    MalformedIdentity,
    AllocationFailed,
    InvalidPolicy,
}
impl std::fmt::Display for PackagePolicyMembershipError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "package policy membership: {self:?}")
    }
}
impl std::error::Error for PackagePolicyMembershipError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyMembershipFailure {
    error: PackagePolicyMembershipError,
    identity: Option<MalformedIdentityExcerpt>,
}

impl PackagePolicyMembershipFailure {
    pub(super) fn new(
        error: PackagePolicyMembershipError,
        identity: Option<MalformedIdentityExcerpt>,
    ) -> Self {
        let identity =
            identity.filter(|_| error == PackagePolicyMembershipError::MalformedIdentity);
        Self { error, identity }
    }
    pub const fn error(&self) -> PackagePolicyMembershipError {
        self.error
    }
    pub fn identity(&self) -> Option<&MalformedIdentityExcerpt> {
        self.identity.as_ref()
    }
}

impl From<PackagePolicyMembershipError> for PackagePolicyMembershipFailure {
    fn from(error: PackagePolicyMembershipError) -> Self {
        Self::new(error, None)
    }
}

impl std::fmt::Display for PackagePolicyMembershipFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.error)?;
        match &self.identity {
            Some(identity) => write!(formatter, " in {identity}"),
            None => Ok(()),
        }
    }
}
impl std::error::Error for PackagePolicyMembershipFailure {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MalformedIdentityExcerpt {
    field: &'static str,
    length: usize,
    start: usize,
    end: usize,
    stopped: Option<usize>,
    text: String,
}

impl MalformedIdentityExcerpt {
    const WINDOW_BEFORE_STOP: usize = 192;
    const WINDOW: usize = 256;

    pub(super) fn new(field: &'static str, identity: &str, stopped: Option<usize>) -> Self {
        let stop = stopped.map(|stop| stop.min(identity.len()));
        let mut start = stop.map_or(0, |stop| stop.saturating_sub(Self::WINDOW_BEFORE_STOP));
        let mut end = identity.len().min(start + Self::WINDOW);
        while !identity.is_char_boundary(start) {
            start += 1;
        }
        while !identity.is_char_boundary(end) {
            end -= 1;
        }
        start = start.min(end);
        Self {
            field,
            length: identity.len(),
            start,
            end,
            stopped: stop,
            text: identity[start..end]
                .chars()
                .map(|character| {
                    if character.is_control() {
                        '?'
                    } else {
                        character
                    }
                })
                .collect(),
        }
    }
    pub const fn field(&self) -> &'static str {
        self.field
    }
    pub const fn identity_length(&self) -> usize {
        self.length
    }
    pub const fn stopped_at(&self) -> Option<usize> {
        self.stopped
    }
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl std::fmt::Display for MalformedIdentityExcerpt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} of {} bytes", self.field, self.length)?;
        if let Some(stopped) = self.stopped {
            write!(formatter, ", unreadable at byte {stopped}")?;
        }
        write!(
            formatter,
            ", bytes {}..{}: \"{}\"",
            self.start, self.end, self.text
        )
    }
}
