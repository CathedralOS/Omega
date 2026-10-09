//! Completed calling-policy output, independent of its source evaluator.
//!
//! A policy producer retains the validated ABI plan together with the exact
//! application identity and value geometry. Native consumers join this output
//! to their requirements; they do not retain source handles or re-run the
//! policy evaluator. This is an in-process product, not foreign admission.

use crate::{BoundaryValueField, BoundaryValueShape, ValidatedBoundaryEntryPlan};
use target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallingApplication {
    pub requirement_identity: String,
    pub target: NativeTarget,
    pub report_fingerprint: u64,
    pub commitment: [u8; 32],
    pub plan: ValidatedBoundaryEntryPlan,
    pub signature: BoundarySignatureGeometry,
    pub opaque_applications: Vec<BoundaryOpaqueApplication>,
    pub callback_context_closed: bool,
}

/// The source-free shape graph used to classify the application. An ABI size
/// alone cannot distinguish a scalar from an equal-sized storage record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundarySignatureGeometry {
    pub shapes: Vec<BoundaryValueShape>,
    pub fields: Vec<BoundaryValueField>,
    pub parameters: Vec<u16>,
    pub result: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryOpaqueApplication {
    pub shape_root: u16,
    pub application_report_fingerprint: u64,
    pub selected_application_commitment: [u8; 32],
}
