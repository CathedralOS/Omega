//! Structural call signatures derive access from declarations, never from layout alone.

use std::collections::{BTreeMap, BTreeSet};

use calling_conventions::{CallPlan, CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use semantic_vocabulary::StructuralTypeId;
use target::NativeTarget;
use terminal_psi::{StructuralParameterDeclaration, StructuralTypeDeclaration};

use super::structural_layout::{structural_parameter_shape, structural_shape};
use crate::LoweringError;

pub(crate) struct StructuralCallSignature {
    signature: CallSignature,
    scalar_parameter_count: usize,
}

impl StructuralCallSignature {
    pub(crate) fn derive(
        scalar_shapes: &[ValueShape],
        parameters: &[StructuralParameterDeclaration],
        result: Option<ValueShape>,
        declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
        active: &mut BTreeSet<StructuralTypeId>,
    ) -> Result<Self, LoweringError> {
        let mut shapes = scalar_shapes.to_vec();
        for parameter in parameters {
            let referent =
                structural_shape(parameter.structural_type, declarations, cache, active)?;
            shapes.push(structural_parameter_shape(referent, parameter.access));
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

    /// Append `pairs` trailing `{instance, table}` pointer words — one pair
    /// per declared dynamic descriptor parameter — before the plan is
    /// evaluated. `function_signature` owns the semantic rows that bind these
    /// placements back to their parameter identities.
    pub(crate) fn push_dynamic_parameter_words(&mut self, pairs: usize, pointer_shape: ValueShape) {
        for _ in 0..pairs {
            self.signature.parameters.push(pointer_shape);
            self.signature.parameters.push(pointer_shape);
        }
    }

    pub(crate) fn plan(&self, target: NativeTarget) -> Result<CallPlan, LoweringError> {
        evaluate_call_plan(CallingPolicy::native_for_target(target), &self.signature)
            .map_err(LoweringError::AbiPlan)
    }
}
