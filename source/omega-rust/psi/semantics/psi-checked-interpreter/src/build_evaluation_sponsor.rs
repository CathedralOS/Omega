#![forbid(unsafe_code)]

//! Compiler-owned aggregate fuel accounting for one build-evaluation session.
//!
//! This account measures deterministic evaluator fuel units. It does not
//! claim to bound host CPU time, memory, or any other process resource.

use std::sync::Arc;
use std::sync::Mutex;

pub const BUILD_EVALUATION_SPONSOR_LIMITS_SCHEMA_VERSION: u32 = 1;

/// Immutable limits for one shared build-evaluation fuel account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildEvaluationSponsorLimits {
    maximum_fuel_units: u64,
}

impl BuildEvaluationSponsorLimits {
    /// Construct the version-1 limit schema.
    pub const fn new(maximum_fuel_units: u64) -> Option<Self> {
        if maximum_fuel_units == 0 {
            return None;
        }
        Some(Self { maximum_fuel_units })
    }

    pub const fn schema_version(self) -> u32 {
        BUILD_EVALUATION_SPONSOR_LIMITS_SCHEMA_VERSION
    }

    pub const fn maximum_fuel_units(self) -> u64 {
        self.maximum_fuel_units
    }
}

#[derive(Debug)]
struct BuildEvaluationSponsorAccount {
    limits: BuildEvaluationSponsorLimits,
    consumed_fuel_units: Mutex<u64>,
}

/// Cloneable compiler-side authority for one aggregate fuel account.
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
                consumed_fuel_units: Mutex::new(0),
            }),
        }
    }

    pub fn limits(&self) -> BuildEvaluationSponsorLimits {
        self.account.limits
    }

    pub fn consumed_fuel_units(&self) -> u64 {
        match self.account.consumed_fuel_units.lock() {
            Ok(consumed) => *consumed,
            Err(poisoned) => *poisoned.into_inner(),
        }
    }

    pub(crate) fn charge_fuel_unit(&self) -> Result<(), String> {
        let maximum = self.account.limits.maximum_fuel_units();
        let mut consumed = self.account.consumed_fuel_units.lock().map_err(|_| {
            "build-evaluation aggregate fuel sponsor account is unavailable".to_owned()
        })?;
        if *consumed >= maximum {
            return Err(format!(
                "build-evaluation aggregate fuel sponsor exhausted at {maximum} fuel units"
            ));
        }
        *consumed += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_reject_zero_and_expose_version_one_schema() {
        assert_eq!(BuildEvaluationSponsorLimits::new(0), None);
        let limits = BuildEvaluationSponsorLimits::new(7).expect("nonzero limit");
        assert_eq!(limits.schema_version(), 1);
        assert_eq!(limits.maximum_fuel_units(), 7);
    }

    #[test]
    fn clones_share_exact_aggregate_exhaustion() {
        let sponsor = BuildEvaluationSponsor::new(
            BuildEvaluationSponsorLimits::new(2).expect("nonzero limit"),
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
    fn observation_recovers_a_poisoned_account_while_charging_fails_closed() {
        let sponsor = BuildEvaluationSponsor::new(
            BuildEvaluationSponsorLimits::new(2).expect("nonzero limit"),
        );
        let poisoning_clone = sponsor.clone();
        let _ = std::thread::spawn(move || {
            let _guard = poisoning_clone
                .account
                .consumed_fuel_units
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
