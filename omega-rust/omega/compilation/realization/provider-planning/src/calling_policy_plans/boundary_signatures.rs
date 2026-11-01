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
    CallbackRequirementId, NativeCallbackDemand, NativeParameterId, StaticMachineBinderId,
    ValidatedBoundaryEntryPlan,
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
        let mut carrying_parameters = Vec::new();
        for (formal_ordinal, root) in self.parameters.iter().copied().enumerate() {
            if let Some(path) =
                boundary_shape_path(&self.shapes, &self.fields, root, representation.shape_root)?
            {
                carrying_parameters.push((formal_ordinal, root, path));
            }
        }
        let result_path = match self.result {
            Some(root) => {
                boundary_shape_path(&self.shapes, &self.fields, root, representation.shape_root)?
            }
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
            let placement = validated
                .plan()
                .call
                .parameters
                .get(
                    usize::try_from(native.native_ordinal).map_err(|_| {
                        "opaque representation native ordinal exceeds usize".to_owned()
                    })?,
                )
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
            path: result_path.expect("one validated result occurrence has a path"),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryNativeParameter {
    pub(crate) identity: NativeParameterId,
    pub(crate) native_ordinal: u32,
    pub(crate) shape: BoundaryNativeParameterShape,
    pub(crate) origin: BoundaryNativeParameterOrigin,
}

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
