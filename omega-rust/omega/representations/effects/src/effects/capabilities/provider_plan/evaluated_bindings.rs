//! Evaluated binding usages and receipts, foreign imports and syscalls.

use crate::effects::capabilities::foreign_locator::{
    ForeignLocatorIdentityDigest, NormalizedForeignLocator,
};
use sha2::Digest;
use sha2::Sha256;
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

/// Complete deterministic evaluator usage retained by one successful foreign
/// binding materialization. The evaluator owns these values; this carrier only
/// preserves them in provider identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluatedBindingUsage {
    usage_schema_version: u32,
    step_schedule_marker: u32,
    fuel_units: u64,
    fuel_ceiling: u64,
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
        fuel_units: u64,
        fuel_ceiling: u64,
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
            fuel_units,
            fuel_ceiling,
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

    pub const fn fuel_units(self) -> u64 {
        self.fuel_units
    }

    pub const fn fuel_ceiling(self) -> u64 {
        self.fuel_ceiling
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

/// Durable receipt for one exact source evaluation and binding
/// materialization. Arena handles, source spans, and raw traces deliberately
/// remain outside this identity carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedBindingReceipt {
    producer_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    producer_callable_identity: String,
    producer_closure_digest: EvaluatedBindingProducerClosureDigest,
    evaluator_semantics_marker: u32,
    evaluation_usage: EvaluatedBindingUsage,
    evaluation_digest: EvaluatedBindingEvaluationDigest,
    materializer_schema_version: u32,
    materialization_digest: EvaluatedBindingMaterializationDigest,
    locator_identity_digest: ForeignLocatorIdentityDigest,
}

impl EvaluatedBindingReceipt {
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn from_evaluation(
        producer_package: Option<semantic_vocabulary::PackageKeyIdentity>,
        producer_callable_identity: String,
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
        Ok(Self {
            producer_package,
            producer_callable_identity,
            producer_closure_digest,
            evaluator_semantics_marker,
            evaluation_usage,
            evaluation_digest,
            materializer_schema_version,
            materialization_digest,
            locator_identity_digest,
        })
    }

    pub const fn producer_package(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        self.producer_package
    }

    pub fn producer_callable_identity(&self) -> &str {
        &self.producer_callable_identity
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

    /// Collision-resistant commitment to every retained receipt field.
    pub fn identity_digest(&self) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(b"omega.evaluated-binding-receipt.sha256.v1\0");
        match self.producer_package {
            Some(package) => {
                digest.update([1]);
                receipt_hash_bytes(&mut digest, &package.digest());
            }
            None => digest.update([0]),
        }
        receipt_hash_bytes(&mut digest, self.producer_callable_identity.as_bytes());
        receipt_hash_bytes(&mut digest, &self.producer_closure_digest.as_bytes());
        digest.update(self.evaluator_semantics_marker.to_le_bytes());
        let usage = self.evaluation_usage;
        digest.update(usage.usage_schema_version.to_le_bytes());
        digest.update(usage.step_schedule_marker.to_le_bytes());
        digest.update(usage.fuel_units.to_le_bytes());
        digest.update(usage.fuel_ceiling.to_le_bytes());
        digest.update(usage.build_log_bytes.to_le_bytes());
        digest.update(usage.filesystem_operation_attempts.to_le_bytes());
        digest.update(usage.peak_live_cells.to_le_bytes());
        digest.update(usage.peak_live_text_bytes.to_le_bytes());
        digest.update(usage.result_cells.to_le_bytes());
        digest.update(usage.result_text_bytes.to_le_bytes());
        receipt_hash_bytes(&mut digest, &self.evaluation_digest.as_bytes());
        digest.update(self.materializer_schema_version.to_le_bytes());
        receipt_hash_bytes(&mut digest, &self.materialization_digest.as_bytes());
        receipt_hash_bytes(&mut digest, &self.locator_identity_digest.as_bytes());
        digest.finalize().into()
    }
}

fn receipt_hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(
        u64::try_from(bytes.len())
            .expect("evaluated binding receipt field length fits u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
}

/// Atomic normalized import plus the receipt that produced it. Construction
/// rejects a receipt committed to any different locator.
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

/// Atomic normalized syscall number plus the receipt that produced it.
/// Construction rejects non-Linux targets, numbers outside the downstream
/// syscall carrier, and receipts committed to any different target or number.
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
                "evaluated Binding::Syscall is not applicable to selected target `{}`",
                target.target_name(),
            ));
        }
        let number = u32::try_from(number)
            .map_err(|_| "evaluated Binding::Syscall number does not fit u32".to_owned())?;
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
