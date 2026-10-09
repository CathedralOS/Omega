//! Runtime invocation receipts: binding one activation to the provider
//! operation and plan that started it.

use crate::{
      ExecutorPreservationAxis};

pub(crate) const fn axis_label(axis: ExecutorPreservationAxis) -> &'static str {
    match axis {
        ExecutorPreservationAxis::Cpu => "CPU",
        ExecutorPreservationAxis::HostThread => "host-thread",
    }
}
