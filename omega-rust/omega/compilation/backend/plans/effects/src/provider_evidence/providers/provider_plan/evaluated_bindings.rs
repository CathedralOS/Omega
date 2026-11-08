/*
We retain evaluated foreign binding usage metrics (`EvaluatedBindingUsage`),
durable evaluation receipts (`EvaluatedBindingReceipt`), and target-validated
foreign imports and syscalls (`EvaluatedForeignImport`, `EvaluatedForeignSyscall`,
`EvaluatedNativeBinding`). Each carrier verifies that its receipt commits to
the exact normalized locator or syscall coordinate before construction.
*/

use crate::provider_evidence::providers::foreign_locator::{
    ForeignLocatorIdentityDigest, NormalizedForeignLocator,
};
use target::TargetProfile;

macro_rules! evaluated_binding_digest {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            #[doc(hidden)]
            pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, String> {
                if bytes == [0; 32] {
                    return Err(concat!(stringify!($name), " cannot be zero").to_owned());
                }
                Ok(Self(bytes))
            }

            pub const fn as_bytes(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

evaluated_binding_digest!(EvaluatedBindingProducerClosureDigest);
evaluated_binding_digest!(EvaluatedBindingEvaluationDigest);
evaluated_binding_digest!(EvaluatedBindingMaterializationDigest);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EvaluatedBindingUsage {
    usage_schema_version: u32,
    step_schedule_marker: u32,
    work_units: u64,
    work_budget: u64,
    build_log_bytes: u64,
    filesystem_operation_attempts: u64,
    peak_live_cells: u64,
    peak_live_text_bytes: u64,
    result_cells: u64,
    result_text_bytes: u64,
}

impl EvaluatedBindingUsage {
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn from_evaluator(
        usage_schema_version: u32,
        step_schedule_marker: u32,
        work_units: u64,
        work_budget: u64,
        build_log_bytes: u64,
        filesystem_operation_attempts: u64,
        peak_live_cells: u64,
        peak_live_text_bytes: u64,
        result_cells: u64,
        result_text_bytes: u64,
    ) -> Result<Self, String> {
        if usage_schema_version == 0 || step_schedule_marker == 0 {
            return Err(
                "evaluated binding usage requires nonzero schema and step-schedule identities"
                    .to_owned(),
            );
        }
        Ok(Self {
            usage_schema_version,
            step_schedule_marker,
            work_units,
            work_budget,
            build_log_bytes,
            filesystem_operation_attempts,
            peak_live_cells,
            peak_live_text_bytes,
            result_cells,
            result_text_bytes,
        })
    }

    pub const fn usage_schema_version(self) -> u32 {
        self.usage_schema_version
    }

    pub const fn step_schedule_marker(self) -> u32 {
        self.step_schedule_marker
    }

    pub const fn work_units(self) -> u64 {
        self.work_units
    }

    pub const fn work_budget(self) -> u64 {
        self.work_budget
    }

    pub const fn build_log_bytes(self) -> u64 {
        self.build_log_bytes
    }

    pub const fn filesystem_operation_attempts(self) -> u64 {
        self.filesystem_operation_attempts
    }

    pub const fn peak_live_cells(self) -> u64 {
        self.peak_live_cells
    }

    pub const fn peak_live_text_bytes(self) -> u64 {
        self.peak_live_text_bytes
    }

    pub const fn result_cells(self) -> u64 {
        self.result_cells
    }

    pub const fn result_text_bytes(self) -> u64 {
        self.result_text_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildNativeBindingOrigin {
    pub requirement_identity: terminal_psi::TerminalIdentityIndex,
    pub source_occurrence_digest: [u8; 32],
    pub terminal_code: terminal_psi::TerminalPsiIdentity,
    pub work_schedule: u32,
    pub storage_schedule: u32,
    pub observation_digest: [u8; 32],
}

impl BuildNativeBindingOrigin {
    fn validate<'table>(
        &self,
        identities: &'table terminal_psi::TerminalIdentityTable,
    ) -> Result<&'table str, String> {
        let requirement = identities
            .text(self.requirement_identity)
            .filter(|requirement| !requirement.is_empty());
        let Some(requirement) = requirement else {
            return Err("native Build binding origin has no exact requirement identity".into());
        };
        if self.source_occurrence_digest == [0; 32]
            || self.terminal_code.program_fingerprint.as_bytes() == &[0; 32]
            || self.work_schedule == 0
            || self.storage_schedule == 0
            || self.observation_digest == [0; 32]
        {
            return Err("native Build binding origin is incomplete".into());
        }
        Ok(requirement)
    }
    fn feed(
        &self,
        requirement: &str,
        digest: &mut crate::provider_evidence::digest_encoder::DomainDigestEncoder,
    ) {
        digest.bytes(requirement.as_bytes());
        digest.raw(&self.source_occurrence_digest);
        digest.raw(&self.terminal_code.vocabulary_marker.get().to_le_bytes());
        digest.raw(self.terminal_code.program_fingerprint.as_bytes());
        digest.raw(&self.work_schedule.to_le_bytes());
        digest.raw(&self.storage_schedule.to_le_bytes());
        digest.raw(&self.observation_digest);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedBindingReceipt {
    build_origin: Option<BuildNativeBindingOrigin>,
    producer_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    producer_callable_identity: terminal_psi::TerminalIdentityIndex,
    producer_closure_digest: EvaluatedBindingProducerClosureDigest,
    evaluator_semantics_marker: u32,
    evaluation_usage: EvaluatedBindingUsage,
    evaluation_digest: EvaluatedBindingEvaluationDigest,
    materializer_schema_version: u32,
    materialization_digest: EvaluatedBindingMaterializationDigest,
    locator_identity_digest: ForeignLocatorIdentityDigest,
    identity: [u8; 32],
}

impl EvaluatedBindingReceipt {
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn from_evaluation(
        producer_package: Option<semantic_vocabulary::PackageKeyIdentity>,
        producer_callable_identity: &str,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        producer_closure_digest: EvaluatedBindingProducerClosureDigest,
        evaluator_semantics_marker: u32,
        evaluation_usage: EvaluatedBindingUsage,
        evaluation_digest: EvaluatedBindingEvaluationDigest,
        materializer_schema_version: u32,
        materialization_digest: EvaluatedBindingMaterializationDigest,
        locator_identity_digest: ForeignLocatorIdentityDigest,
    ) -> Result<Self, String> {
        if producer_callable_identity.is_empty() {
            return Err("evaluated binding receipt requires an exact producer identity".to_owned());
        }
        if evaluator_semantics_marker == 0 || materializer_schema_version == 0 {
            return Err(
                "evaluated binding receipt requires nonzero evaluator and materializer identities"
                    .to_owned(),
            );
        }
        let identity = receipt_identity_digest(
            producer_package,
            producer_callable_identity,
            producer_closure_digest,
            evaluator_semantics_marker,
            evaluation_usage,
            evaluation_digest,
            materializer_schema_version,
            materialization_digest,
            locator_identity_digest,
        );
        Ok(Self {
            build_origin: None,
            producer_package,
            producer_callable_identity: identities.intern(producer_callable_identity),
            producer_closure_digest,
            evaluator_semantics_marker,
            evaluation_usage,
            evaluation_digest,
            materializer_schema_version,
            materialization_digest,
            locator_identity_digest,
            identity,
        })
    }

    pub fn with_build_origin(
        mut self,
        origin: BuildNativeBindingOrigin,
        identities: &terminal_psi::TerminalIdentityTable,
    ) -> Result<Self, String> {
        let requirement = origin.validate(identities)?;
        if self.build_origin.is_some() || self.producer_package.is_none() {
            return Err(
                "native Build receipt has duplicate origin or no immutable supplier".into(),
            );
        }
        let mut digest = crate::provider_evidence::digest_encoder::DomainDigestEncoder::new(
            b"omega.build-native-binding-receipt.sha256.v1\0",
        );
        digest.raw(&self.identity);
        origin.feed(requirement, &mut digest);
        self.identity = digest.finish();
        self.build_origin = Some(origin);
        Ok(self)
    }

    pub fn reintern_producer(
        &self,
        source: &terminal_psi::TerminalIdentityTable,
        destination: &mut terminal_psi::TerminalIdentityTableBuilder,
    ) -> Result<Self, String> {
        let producer = source
            .text(self.producer_callable_identity)
            .filter(|name| !name.is_empty())
            .ok_or("native binding receipt lost its source producer identity")?;
        let mut receipt = self.clone();
        receipt.producer_callable_identity = destination.intern(producer);
        if let Some(origin) = &mut receipt.build_origin {
            let requirement = source
                .text(origin.requirement_identity)
                .filter(|name| !name.is_empty())
                .ok_or("native Build receipt lost its source requirement identity")?;
            origin.requirement_identity = destination.intern(requirement);
        }
        Ok(receipt)
    }

    pub const fn build_origin(&self) -> Option<&BuildNativeBindingOrigin> {
        self.build_origin.as_ref()
    }

    pub const fn producer_package(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        self.producer_package
    }

    pub const fn producer_callable_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.producer_callable_identity
    }

    pub const fn producer_closure_digest(&self) -> EvaluatedBindingProducerClosureDigest {
        self.producer_closure_digest
    }

    pub const fn evaluator_semantics_marker(&self) -> u32 {
        self.evaluator_semantics_marker
    }

    pub const fn evaluation_usage(&self) -> EvaluatedBindingUsage {
        self.evaluation_usage
    }

    pub const fn evaluation_digest(&self) -> EvaluatedBindingEvaluationDigest {
        self.evaluation_digest
    }

    pub const fn materializer_schema_version(&self) -> u32 {
        self.materializer_schema_version
    }

    pub const fn materialization_digest(&self) -> EvaluatedBindingMaterializationDigest {
        self.materialization_digest
    }

    pub const fn locator_identity_digest(&self) -> ForeignLocatorIdentityDigest {
        self.locator_identity_digest
    }

    pub const fn identity_digest(&self) -> [u8; 32] {
        self.identity
    }
}

#[allow(clippy::too_many_arguments)]
fn receipt_identity_digest(
    producer_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    producer_callable_identity: &str,
    producer_closure_digest: EvaluatedBindingProducerClosureDigest,
    evaluator_semantics_marker: u32,
    evaluation_usage: EvaluatedBindingUsage,
    evaluation_digest: EvaluatedBindingEvaluationDigest,
    materializer_schema_version: u32,
    materialization_digest: EvaluatedBindingMaterializationDigest,
    locator_identity_digest: ForeignLocatorIdentityDigest,
) -> [u8; 32] {
    let mut digest = crate::provider_evidence::digest_encoder::DomainDigestEncoder::new(
        b"omega.evaluated-binding-receipt.sha256.v1\0",
    );
    match producer_package {
        Some(package) => {
            digest.byte(1);
            receipt_hash_bytes(&mut digest, &package.digest());
        }
        None => digest.byte(0),
    }
    receipt_hash_bytes(&mut digest, producer_callable_identity.as_bytes());
    receipt_hash_bytes(&mut digest, &producer_closure_digest.as_bytes());
    digest.raw(&evaluator_semantics_marker.to_le_bytes());
    let usage = evaluation_usage;
    digest.raw(&usage.usage_schema_version.to_le_bytes());
    digest.raw(&usage.step_schedule_marker.to_le_bytes());
    digest.raw(&usage.work_units.to_le_bytes());
    digest.raw(&usage.work_budget.to_le_bytes());
    digest.raw(&usage.build_log_bytes.to_le_bytes());
    digest.raw(&usage.filesystem_operation_attempts.to_le_bytes());
    digest.raw(&usage.peak_live_cells.to_le_bytes());
    digest.raw(&usage.peak_live_text_bytes.to_le_bytes());
    digest.raw(&usage.result_cells.to_le_bytes());
    digest.raw(&usage.result_text_bytes.to_le_bytes());
    receipt_hash_bytes(&mut digest, &evaluation_digest.as_bytes());
    digest.raw(&materializer_schema_version.to_le_bytes());
    receipt_hash_bytes(&mut digest, &materialization_digest.as_bytes());
    receipt_hash_bytes(&mut digest, &locator_identity_digest.as_bytes());
    digest.finish()
}

fn receipt_hash_bytes(
    digest: &mut crate::provider_evidence::digest_encoder::DomainDigestEncoder,
    bytes: &[u8],
) {
    digest.bytes(bytes);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedForeignImport {
    locator: NormalizedForeignLocator,
    receipt: EvaluatedBindingReceipt,
}

impl EvaluatedForeignImport {
    #[doc(hidden)]
    pub fn from_retained_evidence(
        locator: NormalizedForeignLocator,
        receipt: EvaluatedBindingReceipt,
    ) -> Result<Self, String> {
        if receipt.locator_identity_digest() != locator.identity_digest() {
            return Err(
                "evaluated binding receipt does not commit to the supplied normalized locator"
                    .to_owned(),
            );
        }
        Ok(Self { locator, receipt })
    }

    pub const fn locator(&self) -> &NormalizedForeignLocator {
        &self.locator
    }

    pub const fn receipt(&self) -> &EvaluatedBindingReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedForeignSyscall {
    target: TargetProfile,
    number: i64,
    identity_digest: ForeignLocatorIdentityDigest,
    receipt: EvaluatedBindingReceipt,
}

impl EvaluatedForeignSyscall {
    #[doc(hidden)]
    pub fn from_retained_evidence(
        target: TargetProfile,
        number: u64,
        receipt: EvaluatedBindingReceipt,
    ) -> Result<Self, String> {
        if !matches!(target, TargetProfile::LinuxArm64 | TargetProfile::LinuxX64) {
            return Err(format!(
                "evaluated ForeignBinding::Syscall is not applicable to selected target `{}`",
                target.target_name(),
            ));
        }
        let number = u32::try_from(number)
            .map_err(|_| "evaluated ForeignBinding::Syscall number does not fit u32".to_owned())?;
        let identity_digest = evaluated_syscall_identity_digest(target, number);
        if receipt.locator_identity_digest() != identity_digest {
            return Err(
                "evaluated binding receipt does not commit to the supplied normalized syscall"
                    .to_owned(),
            );
        }
        Ok(Self {
            target,
            number: i64::from(number),
            identity_digest,
            receipt,
        })
    }

    pub const fn target(&self) -> TargetProfile {
        self.target
    }

    pub const fn number(&self) -> i64 {
        self.number
    }

    pub const fn identity_digest(&self) -> ForeignLocatorIdentityDigest {
        self.identity_digest
    }

    pub const fn receipt(&self) -> &EvaluatedBindingReceipt {
        &self.receipt
    }
}

#[doc(hidden)]
pub fn evaluated_syscall_identity_digest(
    target: TargetProfile,
    number: u32,
) -> ForeignLocatorIdentityDigest {
    target::evaluated_syscall_identity_digest(target, number)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluatedNativeBinding {
    Import(EvaluatedForeignImport),
    Syscall(EvaluatedForeignSyscall),
}

impl EvaluatedNativeBinding {
    pub fn reintern_producer(
        &self,
        source: &terminal_psi::TerminalIdentityTable,
        destination: &mut terminal_psi::TerminalIdentityTableBuilder,
    ) -> Result<Self, String> {
        let receipt = self.receipt().reintern_producer(source, destination)?;
        match self {
            Self::Import(import) => Ok(Self::Import(
                EvaluatedForeignImport::from_retained_evidence(import.locator().clone(), receipt)?,
            )),
            Self::Syscall(syscall) => Ok(Self::Syscall(
                EvaluatedForeignSyscall::from_retained_evidence(
                    syscall.target(),
                    u64::try_from(syscall.number())
                        .map_err(|_| "native syscall number is negative")?,
                    receipt,
                )?,
            )),
        }
    }

    pub const fn target(&self) -> target::TargetProfile {
        match self {
            Self::Import(evaluated) => evaluated.locator().target(),
            Self::Syscall(evaluated) => evaluated.target(),
        }
    }

    pub const fn receipt(&self) -> &EvaluatedBindingReceipt {
        match self {
            Self::Import(evaluated) => evaluated.receipt(),
            Self::Syscall(evaluated) => evaluated.receipt(),
        }
    }

    pub const fn identity_digest(&self) -> target::ForeignLocatorIdentityDigest {
        match self {
            Self::Import(evaluated) => evaluated.locator().identity_digest(),
            Self::Syscall(evaluated) => evaluated.identity_digest(),
        }
    }

    pub fn provider_binding(&self) -> super::ProviderBinding {
        match self {
            Self::Import(evaluated) => super::ProviderBinding::Import {
                evaluated: evaluated.clone(),
            },
            Self::Syscall(evaluated) => super::ProviderBinding::Syscall {
                number: evaluated.number(),
            },
        }
    }

    pub const fn as_import(&self) -> Option<&EvaluatedForeignImport> {
        match self {
            Self::Import(evaluated) => Some(evaluated),
            Self::Syscall(_) => None,
        }
    }

    pub const fn as_syscall(&self) -> Option<&EvaluatedForeignSyscall> {
        match self {
            Self::Import(_) => None,
            Self::Syscall(evaluated) => Some(evaluated),
        }
    }
}
