use semantic_vocabulary::MachineId;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedConformanceApplication {
    pub owner: MachineId,
    pub declaration_identity: String,
    pub telescope: Vec<ClosedConformanceParameterBinding>,
    pub subject_identity: Option<String>,
    pub trait_identity: String,
    pub trait_lifetime_arguments: Vec<String>,
    pub trait_arguments: Vec<String>,
    /// Ordered standalone replay registry derived from the exact checked
    /// source-machine closure. Rows name entries by canonical callable
    /// identity without duplicating the artifact-local machine coordinate.
    pub realization_callables: Vec<ClosedConformanceRealizationCallable>,
    pub rows: Vec<ClosedConformanceRow>,
    /// Historical compact report/index coordinate. It cannot authorize a
    /// dispatch or replay without the adjacent strong commitment.
    pub report_fingerprint: u64,
    /// Domain-separated SHA-256 commitment to the exact source-free
    /// application structure.
    pub commitment: ClosedConformanceApplicationCommitment,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedConformanceApplicationCommitment([u8; 32]);

impl ClosedConformanceApplicationCommitment {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == [0; 32]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClosedConformanceParameterKind {
    Lifetime,
    Type,
    Const,
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedConformanceParameterBinding {
    pub parameter: String,
    pub kind: ClosedConformanceParameterKind,
    pub argument: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedConformanceRow {
    pub declaring_trait_identity: String,
    /// Canonical normalized overload identity of the public requirement.
    pub public_requirement_identity: String,
    /// Declaration path retained separately for exact row-map replay.
    pub requirement_identity: String,
    pub realization_identity: String,
    /// Reference into the owning application's standalone callable registry.
    /// Rows outside the bounded static named-witness lane remain map-free.
    pub realization_callable_identity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedConformanceRealizationCallable {
    /// Canonical checked callable identity, not a declaration display path.
    pub source_callable_identity: String,
    /// Artifact-local Terminal machine emitted for that exact callable.
    pub machine: MachineId,
    /// Source-derived matched requirement/realization result class for the
    /// bounded static requirement cohort.
    /// Callable identity intentionally excludes return type, so this separately
    /// committed value prevents coordinated scalar-result retargeting.
    pub result: ClosedConformanceCallableResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClosedConformanceCallableResult {
    Unit,
    I32,
    Bool,
}

pub fn closed_conformance_application_report_fingerprint(
    application: &ClosedConformanceApplication,
) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn push(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend((value.len() as u64).to_le_bytes());
        bytes.extend(value.as_bytes());
    }

    let mut bytes = Vec::new();
    push(&mut bytes, &application.declaration_identity);
    push(
        &mut bytes,
        application
            .subject_identity
            .as_deref()
            .unwrap_or("<subjectless>"),
    );
    push(&mut bytes, &application.trait_identity);
    bytes.extend((application.trait_lifetime_arguments.len() as u64).to_le_bytes());
    for argument in &application.trait_lifetime_arguments {
        push(&mut bytes, argument);
    }
    bytes.extend((application.telescope.len() as u64).to_le_bytes());
    for binding in &application.telescope {
        push(&mut bytes, &binding.parameter);
        bytes.push(match binding.kind {
            ClosedConformanceParameterKind::Lifetime => 1,
            ClosedConformanceParameterKind::Type => 2,
            ClosedConformanceParameterKind::Const => 3,
            ClosedConformanceParameterKind::Machine => 4,
        });
        push(&mut bytes, &binding.argument);
    }
    bytes.extend((application.trait_arguments.len() as u64).to_le_bytes());
    for argument in &application.trait_arguments {
        push(&mut bytes, argument);
    }
    bytes.extend((application.realization_callables.len() as u64).to_le_bytes());
    for callable in &application.realization_callables {
        push(&mut bytes, &callable.source_callable_identity);
        bytes.extend(callable.machine.get().to_le_bytes());
        bytes.push(match callable.result {
            ClosedConformanceCallableResult::Unit => 1,
            ClosedConformanceCallableResult::I32 => 2,
            ClosedConformanceCallableResult::Bool => 3,
        });
    }
    bytes.extend((application.rows.len() as u64).to_le_bytes());
    for row in &application.rows {
        push(&mut bytes, &row.declaring_trait_identity);
        push(&mut bytes, &row.public_requirement_identity);
        push(&mut bytes, &row.requirement_identity);
        push(&mut bytes, &row.realization_identity);
        bytes.push(u8::from(row.realization_callable_identity.is_some()));
        if let Some(identity) = &row.realization_callable_identity {
            push(&mut bytes, identity);
        }
    }
    bytes.into_iter().fold(OFFSET, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(PRIME)
    })
}

/// Authority-bearing identity for a closed conformance application.
///
/// The owner is deliberately outside this commitment: ownership is an exact
/// independent join, while this value commits to the reusable semantic
/// application structure itself.
pub fn closed_conformance_application_commitment(
    application: &ClosedConformanceApplication,
) -> ClosedConformanceApplicationCommitment {
    fn push(digest: &mut Sha256, value: &str) {
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(value.as_bytes());
    }

    let mut digest = Sha256::new();
    digest.update(b"omega.psi.terminal.closed-conformance-application.v3\0");
    push(&mut digest, &application.declaration_identity);
    push(
        &mut digest,
        application
            .subject_identity
            .as_deref()
            .unwrap_or("<subjectless>"),
    );
    push(&mut digest, &application.trait_identity);
    digest.update((application.trait_lifetime_arguments.len() as u64).to_le_bytes());
    for argument in &application.trait_lifetime_arguments {
        push(&mut digest, argument);
    }
    digest.update((application.telescope.len() as u64).to_le_bytes());
    for binding in &application.telescope {
        push(&mut digest, &binding.parameter);
        digest.update([match binding.kind {
            ClosedConformanceParameterKind::Lifetime => 1,
            ClosedConformanceParameterKind::Type => 2,
            ClosedConformanceParameterKind::Const => 3,
            ClosedConformanceParameterKind::Machine => 4,
        }]);
        push(&mut digest, &binding.argument);
    }
    digest.update((application.trait_arguments.len() as u64).to_le_bytes());
    for argument in &application.trait_arguments {
        push(&mut digest, argument);
    }
    digest.update((application.realization_callables.len() as u64).to_le_bytes());
    for callable in &application.realization_callables {
        push(&mut digest, &callable.source_callable_identity);
        digest.update(callable.machine.get().to_le_bytes());
        digest.update([match callable.result {
            ClosedConformanceCallableResult::Unit => 1,
            ClosedConformanceCallableResult::I32 => 2,
            ClosedConformanceCallableResult::Bool => 3,
        }]);
    }
    digest.update((application.rows.len() as u64).to_le_bytes());
    for row in &application.rows {
        push(&mut digest, &row.declaring_trait_identity);
        push(&mut digest, &row.public_requirement_identity);
        push(&mut digest, &row.requirement_identity);
        push(&mut digest, &row.realization_identity);
        digest.update([u8::from(row.realization_callable_identity.is_some())]);
        if let Some(identity) = &row.realization_callable_identity {
            push(&mut digest, identity);
        }
    }
    ClosedConformanceApplicationCommitment::from_digest(digest.finalize().into())
}
