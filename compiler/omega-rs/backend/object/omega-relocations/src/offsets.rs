mod data_addresses;
mod external_calls;
mod runtime_frame;
mod runtime_storage;
mod runtime_text;
mod wire_decode;
mod wire_encode;

#[derive(Clone, Copy)]
pub(crate) enum CallPlanSource<'plan> {
    CompatibilityOracle,
    Authoritative(&'plan omega_calling_conventions::CallPlan),
}

impl<'plan> CallPlanSource<'plan> {
    pub(crate) const fn authoritative(self) -> Option<&'plan omega_calling_conventions::CallPlan> {
        match self {
            Self::CompatibilityOracle => None,
            Self::Authoritative(plan) => Some(plan),
        }
    }
}

pub(super) use data_addresses::*;
pub(super) use external_calls::*;
pub(super) use runtime_storage::*;
pub(super) use runtime_text::*;
pub(super) use wire_decode::*;
pub(super) use wire_encode::*;
