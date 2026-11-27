// Invocation-local counts of actual native coordinator operations across native
// selection through layout, fragment emission, and image emission. These observations
// never supply semantic inputs, reuse decisions, or admission evidence; failed
// invocations retain their entered stages on Drop, and independent callable
// bodies remain visible as separate `NativePipelineUnit::Callable` scopes.

use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "test-support")]
use std::sync::{Mutex, atomic::AtomicBool};
use target::TargetProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePipelineUnit {
    Application,
    Callable,
}

#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePipelineInvocation {
    pub profile: TargetProfile,
    pub unit: NativePipelineUnit,
    pub stages: [u64; 8],
    pub fragment_emission_attempts: u64,
    pub image_emission_attempts: u64,
    pub completed: bool,
}

#[cfg(feature = "test-support")]
static ENABLED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "test-support")]
static INVOCATIONS: Mutex<Vec<NativePipelineInvocation>> = Mutex::new(Vec::new());

#[cfg(feature = "test-support")]
pub fn native_pipeline_invocations() -> Vec<NativePipelineInvocation> {
    ENABLED.store(true, Ordering::Relaxed);
    INVOCATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

#[derive(Clone, Copy)]
pub enum NativePipelineStage {
    TargetBoundProgramToAbstract = 0,
    AbstractToTarget = 1,
    InstructionSelection = 2,
    SelectedOptimization = 3,
    RegisterAllocation = 4,
    PostAllocationMachine = 5,
    SelectedFormEncoding = 6,
    ResolvedLayout = 7,
    FragmentEmission = 8,
    ImageEmission = 9,
}

pub struct NativePipelineObservation {
    #[cfg(feature = "test-support")]
    profile: TargetProfile,
    #[cfg(feature = "test-support")]
    unit: NativePipelineUnit,
    counts: [AtomicU64; 10],
    #[cfg(feature = "test-support")]
    completed: AtomicBool,
}

impl NativePipelineObservation {
    pub fn new(profile: TargetProfile, unit: NativePipelineUnit) -> Self {
        #[cfg(not(feature = "test-support"))]
        let _ = (profile, unit);
        Self {
            #[cfg(feature = "test-support")]
            profile,
            #[cfg(feature = "test-support")]
            unit,
            counts: std::array::from_fn(|_| AtomicU64::new(0)),
            #[cfg(feature = "test-support")]
            completed: AtomicBool::new(false),
        }
    }

    pub fn counter(&self, stage: NativePipelineStage) -> &AtomicU64 {
        &self.counts[stage as usize]
    }

    pub fn enter(&self, stage: NativePipelineStage) {
        self.counts[stage as usize].fetch_add(1, Ordering::Relaxed);
    }

    pub fn complete(&self) {
        #[cfg(feature = "test-support")]
        self.completed.store(true, Ordering::Relaxed);
    }
}

#[cfg(feature = "test-support")]
impl Drop for NativePipelineObservation {
    fn drop(&mut self) {
        if !ENABLED.load(Ordering::Relaxed) {
            return;
        }
        let observation = NativePipelineInvocation {
            profile: self.profile,
            unit: self.unit,
            stages: std::array::from_fn(|stage| self.counts[stage].load(Ordering::Relaxed)),
            fragment_emission_attempts: self.counts[NativePipelineStage::FragmentEmission as usize].load(Ordering::Relaxed),
            image_emission_attempts: self.counts[NativePipelineStage::ImageEmission as usize].load(Ordering::Relaxed),
            completed: self.completed.load(Ordering::Relaxed),
        };
        INVOCATIONS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(observation);
    }
}
