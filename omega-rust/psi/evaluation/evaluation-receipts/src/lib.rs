//! Completed evaluation records that source settlement and native planning read.
//!
//! A record identifies the accepted code, observations and actual invocation
//! charges. It carries no package resolver, source tree or execution capability.
#![forbid(unsafe_code)]

mod native_binding;
pub use native_binding::{NativeBindingMaterialization, NativeBuildEvaluation};
