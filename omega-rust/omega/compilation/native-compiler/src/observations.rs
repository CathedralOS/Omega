// Invocation-local counts of actual native coordinator operations across Omega
// 00 through 09, fragment emission, and image emission. These observations
// never supply semantic inputs, reuse decisions, or admission evidence; failed
// invocations retain their entered stages on Drop, and independent callable
// bodies remain visible as separate `NativePipelineUnit::Callable` scopes.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use target::TargetProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePipelineUnit {
    Application,
    Callable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePipelineInvocation {
    pub profile: TargetProfile,
    pub unit: NativePipelineUnit,
    pub stages: [u64; 10], // entry counts for numbered Omega 00 through 09, in order
    pub fragment_emission_attempts: u64,
    pub image_emission_attempts: u64,
    pub completed: bool,
}

static ENABLED: AtomicBool = AtomicBool::new(false);
static INVOCATIONS: Mutex<Vec<NativePipelineInvocation>> = Mutex::new(Vec::new());

pub fn native_pipeline_invocations() -> Vec<NativePipelineInvocation> {
    ENABLED.store(true, Ordering::Relaxed);
    INVOCATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

#[derive(Clone, Copy)]
pub(crate) enum NativePipelineStage {
    TerminalToAbstract = 0,
    AbstractOptimization = 1,
    AbstractToTarget = 2,
    InstructionSelection = 3,
    SelectedOptimization = 4,
    RegisterAllocation = 5,
    PostAllocationMachine = 6,
    SelectedFormEncoding = 7,
    ResolvedLayout = 8,
    LayoutOptimization = 9,
    FragmentEmission = 10,
    ImageEmission = 11,
}

pub(crate) struct NativePipelineObservation {
    profile: TargetProfile,
    unit: NativePipelineUnit,
    counts: [AtomicU64; 12],
    completed: AtomicBool,
}

impl NativePipelineObservation {
    pub(crate) fn new(profile: TargetProfile, unit: NativePipelineUnit) -> Self {
        Self {
            profile,
            unit,
            counts: std::array::from_fn(|_| AtomicU64::new(0)),
            completed: AtomicBool::new(false),
        }
    }

    pub(crate) fn enter(&self, stage: NativePipelineStage) {
        self.counts[stage as usize].fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn complete(&self) {
        self.completed.store(true, Ordering::Relaxed);
    }
}

impl Drop for NativePipelineObservation {
    fn drop(&mut self) {
        if !ENABLED.load(Ordering::Relaxed) {
            return;
        }
        let observation = NativePipelineInvocation {
            profile: self.profile,
            unit: self.unit,
            stages: std::array::from_fn(|stage| self.counts[stage].load(Ordering::Relaxed)),
            fragment_emission_attempts: self.counts[10].load(Ordering::Relaxed),
            image_emission_attempts: self.counts[11].load(Ordering::Relaxed),
            completed: self.completed.load(Ordering::Relaxed),
        };
        INVOCATIONS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(observation);
    }
}
