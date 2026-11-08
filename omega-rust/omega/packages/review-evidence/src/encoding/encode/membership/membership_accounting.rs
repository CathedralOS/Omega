use semantic_vocabulary::PackageKeyIdentity;

/// Lowerable work and requested-scratch ceilings for semantic owner descent.
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

/// Plain aggregate charges, not authority. Debit both before the next baseline.
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

/// A membership refusal with the identity text behind it. The error stays a
/// closed `Copy` value because the grammar's callbacks carry it through every
/// nested descent; the excerpt joins it once, where the descent returns. Only
/// `MalformedIdentity` has one: a limit or an unknown package already names
/// its cause, while a malformed identity names a rule and not the declaration
/// that broke it, and a lock reaches this check only after compiling every
/// reviewed package.
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

/// A bounded copy of the text the descent could not read, with the byte range
/// it covers in the whole identity. Identities reach 4 MiB; a failure report
/// keeps at most one window.
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

    /// Window `identity` around `stopped`, where the grammar gave up, or take
    /// its head when the failure is a framing rule with no position.
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
    /// Which retained field held the identity.
    pub const fn field(&self) -> &'static str {
        self.field
    }
    pub const fn identity_length(&self) -> usize {
        self.length
    }
    /// Byte offset where the grammar stopped, when it reports one.
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
