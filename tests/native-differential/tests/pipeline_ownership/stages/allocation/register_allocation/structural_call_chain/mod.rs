//! Optimizer module role: stage group.
//! Scalar-result calls carrying borrowed structural arguments, with values live
//! across each call.

mod allocation;
mod corruption;
mod encoding_layout;
mod fixture;
mod publication;
