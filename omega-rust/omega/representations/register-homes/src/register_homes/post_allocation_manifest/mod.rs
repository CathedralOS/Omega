//! Durable post-allocation optimization manifest: record shape, canonical
//! identity, persistence, and human rendering. Projection and validation live
//! in the producing transform.

mod codec;
mod error;
mod identity;
mod model;
mod rendering;

pub use error::*;
pub use model::*;

#[cfg(test)]
mod tests;
