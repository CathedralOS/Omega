#![forbid(unsafe_code)]

//! Compiler-owned aggregate accounting for one build-evaluation session.
//!
//! This account measures deterministic evaluator fuel, retained BuildLog
//! bytes, and canonical filesystem operation attempts. It does not claim to
//! bound host CPU time, memory, or another process resource.

use std::sync::Arc;
use std::sync::Mutex;

pub const BUILD_EVALUATION_SPONSOR_LIMITS_SCHEMA_VERSION: u32 = 3;

/// Immutable limits for one shared build-evaluation resource account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildEvaluationSponsorLimits {
    maximum_fuel_units: u64,
    maximum_build_log_bytes: u64,
    maximum_filesystem_operation_attempts: u64,
}

impl BuildEvaluationSponsorLimits {
    /// Construct the version-3 limit schema.
    pub const fn new(
        maximum_fuel_units: u64,
        maximum_build_log_bytes: u64,
        maximum_filesystem_operation_attempts: u64,
    ) -> Option<Self> {
        if maximum_fuel_units == 0
            || maximum_build_log_bytes == 0
            || maximum_filesystem_operation_attempts == 0
        {
            return None;
        }
        Some(Self {
            maximum_fuel_units,
            maximum_build_log_bytes,
            maximum_filesystem_operation_attempts,
        })
    }

    pub const fn schema_version(self) -> u32 {
        BUILD_EVALUATION_SPONSOR_LIMITS_SCHEMA_VERSION
    }

    pub const fn maximum_fuel_units(self) -> u64 {
        self.maximum_fuel_units
    }

    /// Aggregate bytes emitted through the compiler-owned BuildLog facet.
    /// This does not claim to bound temporary evaluator data.
    pub const fn maximum_build_log_bytes(self) -> u64 {
        self.maximum_build_log_bytes
    }

    /// Aggregate canonical filesystem calls whose attempt rows may be
    /// retained. This is a vector/cardinality bound, not memory containment.
    pub const fn maximum_filesystem_operation_attempts(self) -> u64 {
        self.maximum_filesystem_operation_attempts
    }
}

#[derive(Debug)]
struct BuildEvaluationSponsorAccount {
    limits: BuildEvaluationSponsorLimits,
    consumed: Mutex<BuildEvaluationSponsorConsumption>,
}

#[derive(Debug, Default)]
struct BuildEvaluationSponsorConsumption {
    fuel_units: u64,
    build_log_bytes: u64,
    filesystem_operation_attempts: u64,
}

/// Cloneable compiler-side authority for one aggregate resource account.
///
/// Every clone refers to the same atomically synchronized account. Evaluated
/// Omega code cannot inspect this Rust object or its remaining allowance.
#[derive(Debug, Clone)]
pub struct BuildEvaluationSponsor {
    account: Arc<BuildEvaluationSponsorAccount>,
}

impl BuildEvaluationSponsor {
    pub fn new(limits: BuildEvaluationSponsorLimits) -> Self {
        Self {
            account: Arc::new(BuildEvaluationSponsorAccount {
                limits,
                consumed: Mutex::new(BuildEvaluationSponsorConsumption::default()),
            }),
        }
    }

    pub fn limits(&self) -> BuildEvaluationSponsorLimits {
        self.account.limits
    }

    pub fn consumed_fuel_units(&self) -> u64 {
        match self.account.consumed.lock() {
            Ok(consumed) => consumed.fuel_units,
            Err(poisoned) => poisoned.into_inner().fuel_units,
        }
    }

    pub fn consumed_build_log_bytes(&self) -> u64 {
        match self.account.consumed.lock() {
            Ok(consumed) => consumed.build_log_bytes,
            Err(poisoned) => poisoned.into_inner().build_log_bytes,
        }
    }

    pub fn consumed_filesystem_operation_attempts(&self) -> u64 {
        match self.account.consumed.lock() {
            Ok(consumed) => consumed.filesystem_operation_attempts,
            Err(poisoned) => poisoned.into_inner().filesystem_operation_attempts,
        }
    }

    pub(crate) fn charge_fuel_unit(&self) -> Result<(), String> {
        let maximum = self.account.limits.maximum_fuel_units();
        let mut consumed = self.account.consumed.lock().map_err(|_| {
            "build-evaluation aggregate fuel sponsor account is unavailable".to_owned()
        })?;
        if consumed.fuel_units >= maximum {
            return Err(format!(
                "build-evaluation aggregate fuel sponsor exhausted at {maximum} fuel units"
            ));
        }
        consumed.fuel_units += 1;
        Ok(())
    }

    pub(crate) fn charge_build_log_bytes(&self, bytes: u64) -> Result<(), String> {
        let maximum = self.account.limits.maximum_build_log_bytes();
        let mut consumed = self.account.consumed.lock().map_err(|_| {
            "build-evaluation aggregate BuildLog sponsor account is unavailable".to_owned()
        })?;
        let Some(candidate) = consumed.build_log_bytes.checked_add(bytes) else {
            return Err("build-evaluation aggregate BuildLog accounting overflowed".to_owned());
        };
        if candidate > maximum {
            return Err(format!(
                "build-evaluation aggregate BuildLog sponsor exhausted at {maximum} bytes"
            ));
        }
        consumed.build_log_bytes = candidate;
        Ok(())
    }

    pub(crate) fn charge_filesystem_operation_attempt(&self) -> Result<(), String> {
        let maximum = self.account.limits.maximum_filesystem_operation_attempts();
        let mut consumed = self.account.consumed.lock().map_err(|_| {
            "build-evaluation aggregate filesystem-attempt sponsor account is unavailable"
                .to_owned()
        })?;
        if consumed.filesystem_operation_attempts >= maximum {
            return Err(format!(
                "build-evaluation aggregate filesystem-attempt sponsor exhausted at {maximum} attempts"
            ));
        }
        consumed.filesystem_operation_attempts += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_reject_zero_and_expose_version_three_schema() {
        assert_eq!(BuildEvaluationSponsorLimits::new(0, 9, 11), None);
        assert_eq!(BuildEvaluationSponsorLimits::new(7, 0, 11), None);
        assert_eq!(BuildEvaluationSponsorLimits::new(7, 9, 0), None);
        let limits = BuildEvaluationSponsorLimits::new(7, 9, 11).expect("nonzero limits");
        assert_eq!(limits.schema_version(), 3);
        assert_eq!(limits.maximum_fuel_units(), 7);
        assert_eq!(limits.maximum_build_log_bytes(), 9);
        assert_eq!(limits.maximum_filesystem_operation_attempts(), 11);
    }

    #[test]
    fn clones_share_exact_aggregate_exhaustion() {
        let sponsor = BuildEvaluationSponsor::new(
            BuildEvaluationSponsorLimits::new(2, 3, 4).expect("nonzero limits"),
        );
        let clone = sponsor.clone();

        sponsor.charge_fuel_unit().expect("first unit");
        clone.charge_fuel_unit().expect("second unit");
        assert_eq!(sponsor.consumed_fuel_units(), 2);
        assert_eq!(
            clone.charge_fuel_unit().expect_err("account is exhausted"),
            "build-evaluation aggregate fuel sponsor exhausted at 2 fuel units"
        );
        assert_eq!(clone.consumed_fuel_units(), 2);
    }

    #[test]
    fn clones_share_exact_aggregate_build_log_exhaustion() {
        let sponsor = BuildEvaluationSponsor::new(
            BuildEvaluationSponsorLimits::new(2, 3, 4).expect("nonzero limits"),
        );
        let clone = sponsor.clone();

        sponsor.charge_build_log_bytes(1).expect("first byte");
        clone.charge_build_log_bytes(2).expect("remaining bytes");
        assert_eq!(sponsor.consumed_build_log_bytes(), 3);
        assert_eq!(
            clone
                .charge_build_log_bytes(1)
                .expect_err("BuildLog account is exhausted"),
            "build-evaluation aggregate BuildLog sponsor exhausted at 3 bytes"
        );
        assert_eq!(clone.consumed_build_log_bytes(), 3);
    }

    #[test]
    fn clones_share_exact_aggregate_filesystem_attempt_exhaustion() {
        let sponsor = BuildEvaluationSponsor::new(
            BuildEvaluationSponsorLimits::new(2, 3, 2).expect("nonzero limits"),
        );
        let clone = sponsor.clone();

        sponsor
            .charge_filesystem_operation_attempt()
            .expect("first attempt");
        clone
            .charge_filesystem_operation_attempt()
            .expect("second attempt");
        assert_eq!(sponsor.consumed_filesystem_operation_attempts(), 2);
        assert_eq!(
            clone
                .charge_filesystem_operation_attempt()
                .expect_err("attempt account is exhausted"),
            "build-evaluation aggregate filesystem-attempt sponsor exhausted at 2 attempts"
        );
        assert_eq!(clone.consumed_filesystem_operation_attempts(), 2);
    }

    #[test]
    fn observation_recovers_a_poisoned_account_while_charging_fails_closed() {
        let sponsor = BuildEvaluationSponsor::new(
            BuildEvaluationSponsorLimits::new(2, 3, 4).expect("nonzero limits"),
        );
        let poisoning_clone = sponsor.clone();
        let _ = std::thread::spawn(move || {
            let _guard = poisoning_clone
                .account
                .consumed
                .lock()
                .expect("fresh account");
            panic!("poison account");
        })
        .join();

        assert_eq!(sponsor.consumed_fuel_units(), 0);
        assert_eq!(
            sponsor
                .charge_fuel_unit()
                .expect_err("a poisoned account must fail closed"),
            "build-evaluation aggregate fuel sponsor account is unavailable"
        );
    }
}
