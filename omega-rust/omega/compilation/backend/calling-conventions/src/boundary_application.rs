//! Completed calling-policy output, independent of its source evaluator.
//!
//! A policy producer retains the validated ABI plan together with the exact
//! application identity and value geometry. Native consumers join this output
//! to their requirements; they do not retain source handles or re-run the
//! policy evaluator. This is an in-process product, not foreign admission.

use crate::{BoundaryValueField, BoundaryValueShape, ValidatedBoundaryEntryPlan};
use arena::{Arena, HandleSpan};
use target::NativeTarget;
mod callback_admission;
mod callback_catalog;
mod identity;
pub use callback_catalog::{
    BoundaryCallbackLayout, CallbackDataGeometry, CallbackInlineFieldGeometry, CallbackSlotGeometry,
};

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
    pub native_parameters: Arena<BoundaryNativeParameterGeometry>,
    pub native_parameter_rows: HandleSpan<BoundaryNativeParameterGeometry>,
    pub callback_layouts: Arena<BoundaryCallbackLayout>,
    pub callback_layout_rows: HandleSpan<BoundaryCallbackLayout>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryNativeParameterShape {
    Semantic(u16),
    TargetFunctionPointer { byte_size: u16, alignment: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryNativeParameterGeometry {
    pub identity: crate::NativeParameterId,
    pub native_ordinal: u32,
    pub shape: BoundaryNativeParameterShape,
    pub origin: crate::BoundaryNativeParameterOrigin,
}

impl Default for BoundaryNativeParameterGeometry {
    fn default() -> Self {
        Self {
            identity: crate::NativeParameterId::new(1).expect("nonzero arena sentinel"),
            native_ordinal: 0,
            shape: BoundaryNativeParameterShape::Semantic(0),
            origin: crate::BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal: 0 },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryOpaqueApplication {
    pub shape_root: u16,
    pub application_report_fingerprint: u64,
    pub selected_application_commitment: [u8; 32],
    pub conformance_application_commitment: [u8; 32],
    pub representation_schema_version: u16,
    pub checked_semantic_copy: bool,
}
