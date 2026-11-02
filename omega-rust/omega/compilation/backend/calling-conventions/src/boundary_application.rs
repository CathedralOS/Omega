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
mod native_telescope;
mod recovery;
mod terminal_telescope;
pub use callback_catalog::{
    BoundaryCallbackLayout, CallbackDataGeometry, CallbackInlineFieldGeometry, CallbackSlotGeometry,
};
pub use native_telescope::{NativeParameterTelescope, validate_fresh_native_parameter_identity};
pub use recovery::BoundaryCallingApplicationCandidate;
pub use terminal_telescope::TerminalCallingTelescope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallingApplication {
    pub abi: semantic_vocabulary::CallingAbiIdentity,
    pub requirement_identity: String,
    pub target: NativeTarget,
    pub report_fingerprint: u64,
    pub commitment: [u8; 32],
    pub plan: ValidatedBoundaryEntryPlan,
    /// Semantic prefix used to locate a table, never passed to its callee.
    /// Public telescope ordinals remain unchanged; native-only lanes survive.
    pub dispatch_only_parameter_count: u32,
    pub signature: BoundarySignatureGeometry,
    pub opaque_applications: Vec<BoundaryOpaqueApplication>,
    /// Complete producer context, including binders and demands whose absence
    /// from a candidate plan must still make that candidate fail validation.
    pub callback_context: crate::CallbackMaterializationContext,
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
    pub callback_bindings: Arena<BoundaryCallbackBinding>,
    pub callback_binding_rows: HandleSpan<BoundaryCallbackBinding>,
}

/// Validated public-native to physical-call projection. Only a semantic prefix
/// can be omitted; callback lanes keep their declared order and identity.
pub struct NativeCallProjection<'geometry> {
    parameters: &'geometry [BoundaryNativeParameterGeometry],
    dispatch_only_parameter_count: u32,
}

impl<'geometry> NativeCallProjection<'geometry> {
    pub fn new(
        parameters: &'geometry [BoundaryNativeParameterGeometry],
        semantic_parameter_count: usize,
        dispatch_only_parameter_count: u32,
    ) -> Result<Self, String> {
        let dispatch_count = usize::try_from(dispatch_only_parameter_count)
            .map_err(|_| "dispatch prefix exceeds usize")?;
        if dispatch_count > semantic_parameter_count {
            return Err("dispatch prefix exceeds the public semantic telescope".into());
        }
        let mut semantic_seen = vec![false; semantic_parameter_count];
        for (ordinal, parameter) in parameters.iter().enumerate() {
            if usize::try_from(parameter.native_ordinal).ok() != Some(ordinal)
                || parameters[..ordinal]
                    .iter()
                    .any(|prior| prior.identity == parameter.identity)
            {
                return Err(
                    "native call projection repeats an identity or changes row order".into(),
                );
            }
            if let crate::BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal } =
                parameter.origin
            {
                let seen = semantic_seen
                    .get_mut(
                        usize::try_from(formal_ordinal)
                            .map_err(|_| "semantic formal exceeds usize")?,
                    )
                    .ok_or("native call projection has an out-of-range semantic formal")?;
                if std::mem::replace(seen, true) {
                    return Err("native call projection repeats a semantic formal".into());
                }
            }
        }
        if semantic_seen.contains(&false) {
            return Err("native call projection lost a public semantic formal".into());
        }
        Ok(Self {
            parameters,
            dispatch_only_parameter_count,
        })
    }

    fn is_dispatch_only(&self, parameter: &BoundaryNativeParameterGeometry) -> bool {
        matches!(parameter.origin,
            crate::BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal }
            if formal_ordinal < self.dispatch_only_parameter_count)
    }

    pub fn parameters(
        &self,
    ) -> impl Iterator<Item = &'geometry BoundaryNativeParameterGeometry> + '_ {
        self.parameters
            .iter()
            .filter(|parameter| !self.is_dispatch_only(parameter))
    }

    pub fn parameter_ordinal(&self, public_ordinal: u32) -> Result<Option<usize>, String> {
        let ordinal =
            usize::try_from(public_ordinal).map_err(|_| "native ordinal exceeds usize")?;
        let parameter = self
            .parameters
            .get(ordinal)
            .ok_or("native ordinal is outside the public telescope")?;
        if self.is_dispatch_only(parameter) {
            return Ok(None);
        }
        let omitted = self.parameters[..ordinal]
            .iter()
            .filter(|parameter| self.is_dispatch_only(parameter))
            .count();
        Ok(Some(ordinal - omitted))
    }
}

impl BoundaryCallingApplication {
    pub fn call_projection(&self) -> Result<NativeCallProjection<'_>, String> {
        NativeCallProjection::new(
            self.signature
                .native_parameters
                .span(self.signature.native_parameter_rows)
                .ok_or("calling application has stale native telescope storage")?,
            self.signature.parameters.len(),
            self.dispatch_only_parameter_count,
        )
    }
}

/// Exact source binder correspondence retained by the calling-policy producer.
/// Structural machine parameters also consume ordinals, so vector position and
/// compact binder IDs cannot reconstruct the authored correspondence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallbackBinding {
    pub binder: crate::StaticMachineBinderId,
    pub requirement: crate::CallbackRequirementId,
    pub static_machine_ordinal: u32,
    pub requirement_identity: String,
    pub invokes_synchronously: bool,
}

impl Default for BoundaryCallbackBinding {
    fn default() -> Self {
        Self {
            binder: crate::StaticMachineBinderId::new(1).expect("nonzero arena sentinel"),
            requirement: crate::CallbackRequirementId::new(1).expect("nonzero arena sentinel"),
            static_machine_ordinal: 0,
            requirement_identity: String::new(),
            invokes_synchronously: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryNativeParameterShape {
    Semantic(u16),
    TargetFunctionPointer { byte_size: u16, alignment: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryNativeParameterGeometry {
    pub identity: crate::NativeParameterId,
    pub native_ordinal: u32,
    pub shape: BoundaryNativeParameterShape,
    pub origin: crate::BoundaryNativeParameterOrigin,
}

impl BoundaryNativeParameterGeometry {
    pub const fn identity(&self) -> crate::NativeParameterId {
        self.identity
    }

    pub const fn native_ordinal(&self) -> u32 {
        self.native_ordinal
    }

    pub const fn shape(&self) -> BoundaryNativeParameterShape {
        self.shape
    }

    pub const fn origin(&self) -> crate::BoundaryNativeParameterOrigin {
        self.origin
    }
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
