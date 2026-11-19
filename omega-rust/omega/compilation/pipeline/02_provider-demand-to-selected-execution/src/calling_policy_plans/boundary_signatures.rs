//! Match opaque occurrences to their exact physical placements.

use crate::calling_policy_plans::opaque_representations::{
    BoundaryOpaqueRepresentationMovement, BoundaryOpaqueRepresentationMovementRole,
    BoundaryOpaqueRepresentationPathElement,
};
use calling_conventions::ValidatedBoundaryEntryPlan;
pub use calling_conventions::{
    BoundaryNativeParameterOrigin, BoundaryValueClass, BoundaryValueField, BoundaryValueShape,
};

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
        dispatch_only_formal: Option<u32>,
    ) -> Result<BoundaryOpaqueRepresentationMovement, String> {
        let mut carrying_parameters = Vec::new();
        for (formal_ordinal, root) in self.parameters.iter().copied().enumerate() {
            if let Some(path) = boundary_shape_path(self.shapes, self.fields, root, shape_root)? {
                carrying_parameters.push((formal_ordinal, root, path));
            }
        }
        let result_path = match self.result {
            Some(root) => boundary_shape_path(self.shapes, self.fields, root, shape_root)?,
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
            let projection = calling_conventions::NativeCallProjection::new(
                self.native_parameters,
                self.parameters.len(),
                dispatch_only_formal,
            )?;
            let wire_ordinal = projection
                .parameter_ordinal(native.native_ordinal)?
                .ok_or("opaque occurrence belongs to a dispatch-only formal")?;
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

pub use calling_conventions::BoundaryNativeParameterShape;

#[cfg(test)]
mod tests {
    use super::*;
    use calling_conventions::{
        BoundaryNativeParameterGeometry, CallSignature, CallbackRequirementId, CallingPolicy,
        NativeParameterId, StaticMachineBinderId, ValueShape,
        evaluate_ordinary_boundary_entry_plan,
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
            let movement = geometry.movement(1, &plan, Some(0)).unwrap();
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
                    .movement(0, &plan, Some(0))
                    .unwrap_err()
                    .contains("dispatch-only formal")
            );
        }
    }
}
