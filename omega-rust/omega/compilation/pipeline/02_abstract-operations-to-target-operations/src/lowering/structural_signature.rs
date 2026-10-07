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

    pub(crate) fn plan(&self, target: NativeTarget) -> Result<CallPlan, LoweringError> {
        evaluate_call_plan(CallingPolicy::native_for_target(target), &self.signature)
            .map_err(LoweringError::AbiPlan)
    }
}
