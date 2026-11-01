//! Boundary signatures: the value classes, shapes and fields a boundary
//! call carries, the materialized signature, and how signatures are
//! collected from the typed program and its calling-policy machines.

use crate::calling_policy_plans::callback_layout_catalog::BoundaryCallbackLayoutEntry;
use crate::calling_policy_plans::opaque_representations::{
    BoundaryOpaqueRepresentationMovement, BoundaryOpaqueRepresentationMovementRole,
    BoundaryOpaqueRepresentationPathElement, BoundaryOpaqueRepresentationUse,
};
pub use calling_conventions::{
    BoundaryNativeParameterOrigin, BoundaryValueClass, BoundaryValueField, BoundaryValueShape,
};
use calling_conventions::{
    CallbackRequirementId, NativeCallbackDemand, StaticMachineBinderId, ValidatedBoundaryEntryPlan,
};
use target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedBoundarySignature {
    pub(crate) abi: semantic_vocabulary::CallingAbiIdentity,
    pub(crate) owner_requirement_identity: String,
    pub(crate) native_target: NativeTarget,
    pub(crate) opaque_representations: Vec<BoundaryOpaqueRepresentationUse>,
    pub(crate) shapes: Vec<BoundaryValueShape>,
    pub(crate) fields: Vec<BoundaryValueField>,
    pub(crate) parameters: Vec<u16>,
    pub(crate) callback_binders: Vec<BoundaryCallbackBinder>,
    pub(crate) callback_demands: Vec<NativeCallbackDemand>,
    pub(crate) callback_layout_catalog: Vec<BoundaryCallbackLayoutEntry>,
    pub(crate) native_parameters: Vec<BoundaryNativeParameter>,
    pub(crate) direct_callback_parameters: Vec<BoundaryDirectCallbackParameter>,
    pub(crate) result: Option<u16>,
}

impl MaterializedBoundarySignature {
    pub fn abi(&self) -> &semantic_vocabulary::CallingAbiIdentity {
        &self.abi
    }
    /// Named layout-owned destinations in native-demand order. Direct callback
    /// parameters have no layout field and therefore contribute no entry here.
    /// Typed slot applications survive without retaining a complete LayoutPlan.
    pub fn callback_layout_catalog(&self) -> &[BoundaryCallbackLayoutEntry] {
        &self.callback_layout_catalog
    }

    pub fn owner_requirement_identity(&self) -> &str {
        &self.owner_requirement_identity
    }

    pub const fn native_target(&self) -> NativeTarget {
        self.native_target
    }

    /// Compiler-derived opaque representations actually used by value while
    /// materializing this exact boundary signature. An unused build selection
    /// is deliberately absent.
    pub fn opaque_representation_uses(&self) -> &[BoundaryOpaqueRepresentationUse] {
        &self.opaque_representations
    }

    pub fn shapes(&self) -> &[BoundaryValueShape] {
        &self.shapes
    }

    pub fn fields(&self) -> &[BoundaryValueField] {
        &self.fields
    }

    pub fn parameters(&self) -> &[u16] {
        &self.parameters
    }

    pub const fn result(&self) -> Option<u16> {
        self.result
    }

    /// Rejoin one exact opaque shape node to the top-level semantic occurrence
    /// and target placement that carries it. This consumes a replay-validated
    /// plan; equal-looking layouts or an aggregate plan digest are not enough.
    pub fn opaque_representation_movement(
        &self,
        representation: &BoundaryOpaqueRepresentationUse,
        validated: &ValidatedBoundaryEntryPlan,
    ) -> Result<BoundaryOpaqueRepresentationMovement, String> {
        if self
            .opaque_representations
            .iter()
            .filter(|candidate| *candidate == representation)
            .count()
            != 1
        {
            return Err(
                "opaque representation use is not one exact materialized-signature occurrence"
                    .to_owned(),
            );
        }
        BoundaryOpaqueMovementGeometry {
            shapes: &self.shapes,
            fields: &self.fields,
            parameters: &self.parameters,
            result: self.result,
            native_parameters: &self.native_parameters,
        }
        .movement(representation.shape_root, validated, 0)
    }
}

/// Borrowed geometry shared by Source review and canonical ABI publication.
/// It contains no Source identities or representation-selection authority.
pub(super) struct BoundaryOpaqueMovementGeometry<'geometry> {
    pub shapes: &'geometry [BoundaryValueShape],
    pub fields: &'geometry [BoundaryValueField],
    pub parameters: &'geometry [u16],
    pub result: Option<u16>,
    pub native_parameters: &'geometry [calling_conventions::BoundaryNativeParameterGeometry],
}

impl BoundaryOpaqueMovementGeometry<'_> {
    pub fn movement(
        &self,
        shape_root: u16,
        validated: &ValidatedBoundaryEntryPlan,
        dispatch_only_parameter_count: usize,
    ) -> Result<BoundaryOpaqueRepresentationMovement, String> {
        let mut carrying_parameters = Vec::new();
        for (formal_ordinal, root) in self.parameters.iter().copied().enumerate() {
            if let Some(path) = boundary_shape_path(&self.shapes, &self.fields, root, shape_root)? {
                carrying_parameters.push((formal_ordinal, root, path));
            }
        }
        let result_path = match self.result {
            Some(root) => boundary_shape_path(&self.shapes, &self.fields, root, shape_root)?,
            None => None,
        };
        if carrying_parameters.len() + usize::from(result_path.is_some()) != 1 {
            return Err(format!(
                "opaque representation shape node belongs to {} top-level boundary occurrences; expected one",
                carrying_parameters.len() + usize::from(result_path.is_some()),
            ));
        }
        if let Some((formal_ordinal, root, path)) = carrying_parameters.into_iter().next() {
            let formal_ordinal = u32::try_from(formal_ordinal)
                .map_err(|_| "opaque representation formal ordinal exceeds u32".to_owned())?;
            let matching = self
                .native_parameters
                .iter()
                .filter(|parameter| {
                    matches!(
                        parameter.origin,
                        BoundaryNativeParameterOrigin::SemanticFormal {
                            formal_ordinal: candidate,
                        } if candidate == formal_ordinal
                    ) && matches!(
                        parameter.shape,
                        BoundaryNativeParameterShape::Semantic(candidate) if candidate == root
                    )
                })
                .collect::<Vec<_>>();
            let [native] = matching.as_slice() else {
                return Err(format!(
                    "opaque representation formal parameter maps to {} native parameters; expected one",
                    matching.len(),
                ));
            };
            if usize::try_from(formal_ordinal).map_err(|_| "opaque formal ordinal exceeds usize")?
                < dispatch_only_parameter_count
            {
                return Err("opaque occurrence belongs to a dispatch-only formal".into());
            }
            // Public native ordinals remain ABI identity. The wire plan omits
            // dispatch-only semantic formals, but not native-only callbacks.
            let removed = self
                .native_parameters
                .iter()
                .filter(|parameter| {
                    parameter.native_ordinal < native.native_ordinal
                        && matches!(parameter.origin,
                        BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal }
                        if usize::try_from(formal_ordinal).is_ok_and(|ordinal|
                            ordinal < dispatch_only_parameter_count))
                })
                .count();
            let wire_ordinal = usize::try_from(native.native_ordinal)
                .map_err(|_| "opaque native ordinal exceeds usize")?
                .checked_sub(removed)
                .ok_or("opaque movement has an invalid public-to-wire projection")?;
            let placement = validated
                .plan()
                .call
                .parameters
                .get(wire_ordinal)
                .ok_or_else(|| {
                    "opaque representation native parameter has no validated placement".to_owned()
                })?
                .clone();
            return Ok(BoundaryOpaqueRepresentationMovement {
                role: BoundaryOpaqueRepresentationMovementRole::Parameter {
                    formal_ordinal,
                    native_ordinal: native.native_ordinal,
                },
                path,
                placement,
            });
        }
        let placement =
            validated.plan().call.result.clone().ok_or_else(|| {
                "opaque representation result has no validated placement".to_owned()
            })?;
        Ok(BoundaryOpaqueRepresentationMovement {
            role: BoundaryOpaqueRepresentationMovementRole::Result,
            path: result_path.ok_or("opaque result occurrence lost its checked path")?,
            placement,
        })
    }
}

pub(crate) fn boundary_shape_path(
    shapes: &[BoundaryValueShape],
    fields: &[BoundaryValueField],
    root: u16,
    target: u16,
) -> Result<Option<Vec<BoundaryOpaqueRepresentationPathElement>>, String> {
    let shape = shapes
        .get(usize::from(root))
        .ok_or_else(|| "boundary shape graph contains an out-of-range node".to_owned())?;
    if root == target {
        return Ok(Some(Vec::new()));
    }
    match shape.class {
        BoundaryValueClass::Integer | BoundaryValueClass::Float | BoundaryValueClass::Reference => {
            Ok(None)
        }
        BoundaryValueClass::FixedArray { element, .. } => {
            if element >= root {
                return Err("boundary fixed-array shape graph is not acyclic postorder".to_owned());
            }
            let Some(mut path) = boundary_shape_path(shapes, fields, element, target)? else {
                return Ok(None);
            };
            path.insert(
                0,
                BoundaryOpaqueRepresentationPathElement::FixedArrayElement,
            );
            Ok(Some(path))
        }
        BoundaryValueClass::Record {
            first_field,
            field_count,
        } => {
            let start = usize::from(first_field);
            let end = start
                .checked_add(usize::from(field_count))
                .ok_or_else(|| "boundary record field range overflowed".to_owned())?;
            let children = fields
                .get(start..end)
                .ok_or_else(|| "boundary record shape has an out-of-range field span".to_owned())?;
            let mut found = None;
            for (ordinal, child) in children.iter().enumerate() {
                if child.shape >= root {
                    return Err("boundary record shape graph is not acyclic postorder".to_owned());
                }
                if let Some(mut path) = boundary_shape_path(shapes, fields, child.shape, target)? {
                    path.insert(
                        0,
                        BoundaryOpaqueRepresentationPathElement::RecordField {
                            ordinal: u16::try_from(ordinal).map_err(|_| {
                                "boundary record field ordinal exceeds u16".to_owned()
                            })?,
                        },
                    );
                    if found.replace(path).is_some() {
                        return Err(
                            "opaque representation shape node is reachable through multiple record fields"
                                .to_owned(),
                        );
                    }
                }
            }
            Ok(found)
        }
    }
}

pub(crate) use calling_conventions::BoundaryNativeParameterGeometry as BoundaryNativeParameter;

pub use calling_conventions::BoundaryNativeParameterShape;

pub use language_semantics::callback_declarations::BoundaryDirectCallbackDeclaration as BoundaryDirectCallbackParameter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallbackBinder {
    pub binder: StaticMachineBinderId,
    pub requirement: CallbackRequirementId,
    pub static_machine_ordinal: u32,
    pub parameter_symbol: symbols::SymbolHandle,
    pub requirement_trait: symbols::SymbolHandle,
    pub requirement_machine: symbols::SymbolHandle,
    pub requirement_identity: String,
    pub invokes_synchronously: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use calling_conventions::{
        BoundaryNativeParameterGeometry, CallSignature, CallingPolicy, NativeParameterId,
        ValueShape, evaluate_ordinary_boundary_entry_plan,
    };

    #[test]
    fn dispatch_projection_preserves_public_ordinals_and_native_only_lanes() {
        let shapes = [
            BoundaryValueShape {
                class: BoundaryValueClass::Reference,
                byte_size: 8,
                alignment: 8,
            },
            BoundaryValueShape {
                class: BoundaryValueClass::Integer,
                byte_size: 8,
                alignment: 8,
            },
        ];
        for has_callback in [false, true] {
            let mut native = vec![BoundaryNativeParameterGeometry {
                identity: NativeParameterId::new(1).unwrap(),
                native_ordinal: 0,
                shape: BoundaryNativeParameterShape::Semantic(0),
                origin: BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal: 0 },
            }];
            if has_callback {
                native.push(BoundaryNativeParameterGeometry {
                    identity: NativeParameterId::new(2).unwrap(),
                    native_ordinal: 1,
                    shape: BoundaryNativeParameterShape::TargetFunctionPointer {
                        byte_size: 8,
                        alignment: 8,
                    },
                    origin: BoundaryNativeParameterOrigin::PrivateCallback {
                        binder: StaticMachineBinderId::new(1).unwrap(),
                        requirement: CallbackRequirementId::new(1).unwrap(),
                    },
                });
            }
            let ordinal = u32::try_from(native.len()).unwrap();
            native.push(BoundaryNativeParameterGeometry {
                identity: NativeParameterId::new(u64::from(ordinal) + 1).unwrap(),
                native_ordinal: ordinal,
                shape: BoundaryNativeParameterShape::Semantic(1),
                origin: BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal: 1 },
            });
            let classified = CallSignature {
                parameters: vec![ValueShape::integer(8, 8); native.len() - 1],
                result: None,
            };
            let plan =
                evaluate_ordinary_boundary_entry_plan(CallingPolicy::Aapcs64, &classified).unwrap();
            let geometry = BoundaryOpaqueMovementGeometry {
                shapes: &shapes,
                fields: &[],
                parameters: &[0, 1],
                result: None,
                native_parameters: &native,
            };
            let movement = geometry.movement(1, &plan, 1).unwrap();
            assert_eq!(
                movement.role(),
                BoundaryOpaqueRepresentationMovementRole::Parameter {
                    formal_ordinal: 1,
                    native_ordinal: ordinal,
                }
            );
            assert_eq!(
                movement.placement(),
                plan.plan().call.parameters.last().unwrap()
            );
            assert!(
                geometry
                    .movement(0, &plan, 1)
                    .unwrap_err()
                    .contains("dispatch-only formal")
            );
        }
    }
}
