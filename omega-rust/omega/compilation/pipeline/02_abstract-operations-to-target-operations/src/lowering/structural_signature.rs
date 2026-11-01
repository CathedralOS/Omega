//! Structural call signatures derive access from declarations, never from layout alone.
//!
//! [Structural access](../../../../../../../wiki/spec/terminal-psi/structural_access.md)
//! requires one exhaustive access classifier that every structural-call
//! producer uses. `StructuralCallSignature::parameter_shape` is that
//! classifier, and `StructuralCallPlan` has no constructor outside
//! `StructuralCallSignature::plan`, so a prepared call cannot carry a plan
//! evaluated from shapes the classifier did not derive. Scalar-only signatures
//! stay ordinary `CallSignature`s.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Deref;

use calling_conventions::{CallPlan, CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use semantic_vocabulary::StructuralTypeId;
use target::NativeTarget;
use terminal_psi::{StructuralAccess, StructuralTypeDeclaration};

use super::structural_layout::{scalar_storage_type, structural_shape};
use crate::LoweringError;

/// The calling lane a structural parameter crosses. Internal loans of a
/// primitive scalar carry a descriptor; foreign loans stay thin pointers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParameterLane {
    Internal,
    Foreign,
}

pub(crate) struct StructuralCallSignature {
    signature: CallSignature,
    scalar_parameter_count: usize,
}

/// A call plan evaluated from a [`StructuralCallSignature`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StructuralCallPlan(CallPlan);

impl StructuralCallPlan {
    pub(crate) fn into_call_plan(self) -> CallPlan {
        self.0
    }
}

impl Deref for StructuralCallPlan {
    type Target = CallPlan;

    fn deref(&self) -> &CallPlan {
        &self.0
    }
}

impl StructuralCallSignature {
    /// The ABI shape of one structural parameter of `structural_type` whose
    /// referent has shape `referent`, passed with `access` across `lane`.
    pub(crate) fn parameter_shape(
        lane: ParameterLane,
        structural_type: StructuralTypeId,
        referent: ValueShape,
        access: StructuralAccess,
        declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    ) -> ValueShape {
        match access {
            StructuralAccess::Owned => referent,
            StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow => {
                if lane == ParameterLane::Internal
                    && scalar_storage_type(structural_type, declarations).is_some()
                {
                    ValueShape::borrowed_scalar_reference()
                } else {
                    ValueShape::borrowed_reference(referent.byte_size, referent.alignment)
                }
            }
        }
    }

    pub(crate) fn derive(
        scalar_shapes: &[ValueShape],
        formals: impl IntoIterator<Item = (StructuralTypeId, StructuralAccess)>,
        result: Option<ValueShape>,
        declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
        active: &mut BTreeSet<StructuralTypeId>,
    ) -> Result<Self, LoweringError> {
        let mut shapes = scalar_shapes.to_vec();
        for (structural_type, access) in formals {
            let referent = structural_shape(structural_type, declarations, cache, active)?;
            shapes.push(Self::parameter_shape(
                ParameterLane::Internal,
                structural_type,
                referent,
                access,
                declarations,
            ));
        }
        Ok(Self {
            signature: CallSignature {
                parameters: shapes,
                result,
            },
            scalar_parameter_count: scalar_shapes.len(),
        })
    }

    pub(crate) fn structural_shapes(&self) -> &[ValueShape] {
        &self.signature.parameters[self.scalar_parameter_count..]
    }

    pub(crate) fn plan(&self, target: NativeTarget) -> Result<StructuralCallPlan, LoweringError> {
        evaluate_call_plan(CallingPolicy::native_for_target(target), &self.signature)
            .map(StructuralCallPlan)
            .map_err(LoweringError::AbiPlan)
    }
}
