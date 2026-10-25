//! Execute admitted fixed-frame native adapters in a private provider process.
//!
//! This host integration owns real native libraries, shared physical homes and
//! process containment. It consumes source-free products and never invokes the
//! compiler. Terminal Psi retains semantic ownership, loan and completion
//! admission; an IPC response alone establishes none of those obligations.

#![deny(unsafe_op_in_unsafe_fn)]

mod channel;
mod native_code;
mod native_provider_runtime;
mod shared_backing;
mod wire;
mod worker;
pub use native_provider_runtime::{
    NativeAdapterId, NativeArgument, NativeHomeId, NativeProviderError, NativeProviderProcess,
};
pub use worker::run_worker_if_requested;

#[cfg(test)]
mod physical_tests;
