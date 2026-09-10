//! Array payload identities live in the structural namespace, not scalar slots.
//!
//! Reserving a place does not establish its value. Only the constructor effect
//! in the selected evaluation path does so; Terminal independently checks that
//! this producer dominates each use. No array payload crosses a block parameter.

use super::*;
use checked_trees::CheckedScalarComputationStructuralArgument;
use checked_trees::expression::ExpressionHandle;

#[derive(Clone)]
pub(crate) struct Slot {
    pub(super) expression: ExpressionHandle,
    pub(super) place: PlaceId,
    pub(super) structural_type: StructuralTypeId,
    pub(super) leaf_type: ScalarType,
    pub(super) leaf_count: u64,
}

pub(crate) fn extend_elements(
    plans: &checked_trees::CheckedScalarComputationPlans,
    arguments: &[CheckedScalarComputationStructuralArgument],
    pending: &mut Vec<Computation>,
) -> Result<(), LoweringError> {
    for argument in arguments {
        if let CheckedScalarComputationStructuralArgument::Array { elements, .. } = argument {
            pending.extend(
                plans
                    .operands
                    .span(*elements)
                    .ok_or(LoweringError::Unsupported(
                        "computed array elements have a stale span",
                    ))?,
            );
        }
    }
    Ok(())
}

pub(crate) fn prepare(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Vec<Slot>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut slots: Vec<Slot> = Vec::new();
    let roots = plans
        .roots
        .iter()
        .filter_map(|(_, root)| (root.machine == machine).then_some(root.root))
        .collect::<Vec<_>>();
    for handle in super::reachable_nodes(checked, &roots)? {
        let CheckedScalarComputationKind::Call {
            structural_arguments,
            ..
        } = plans.nodes.get(handle).kind
        else {
            continue;
        };
        for argument in plans
            .structural_arguments
            .span(structural_arguments)
            .ok_or(LoweringError::Unsupported(
                "computed array call has stale structural arguments",
            ))?
        {
            let CheckedScalarComputationStructuralArgument::Array {
                expression,
                type_reference,
                ..
            } = argument
            else {
                continue;
            };
            let identity = checked.normalized_type_identity(*type_reference);
            let declaration = types
                .iter()
                .find(|declaration| declaration.identity == identity.as_str())
                .ok_or(LoweringError::Unsupported(
                    "computed array has no structural type namespace",
                ))?;
            let (leaf_type, leaf_count) = layout(declaration.id, types)?;
            if let Some(existing) = slots.iter().find(|slot| slot.expression == *expression) {
                if existing.structural_type != declaration.id {
                    return unsupported("computed array occurrence has conflicting types");
                }
                continue;
            }
            slots.push(Slot {
                expression: *expression,
                place: place_id(allocate_dense(next_place)?),
                structural_type: declaration.id,
                leaf_type,
                leaf_count,
            });
        }
    }
    Ok(slots)
}

fn layout(
    mut current: StructuralTypeId,
    types: &[StructuralTypeDeclaration],
) -> Result<(ScalarType, u64), LoweringError> {
    let mut count = Some(1u64);
    let mut empty = false;
    for depth in 0..types.len() {
        let declaration = types
            .iter()
            .find(|declaration| declaration.id == current)
            .ok_or(LoweringError::Unsupported(
                "computed array element type is absent",
            ))?;
        match declaration.shape {
            StructuralTypeShape::FixedArray { element, length } => {
                // A later empty dimension suppresses product overflow, never
                // validation of the complete type beneath that dimension.
                empty |= length == 0;
                count = count.and_then(|count| count.checked_mul(length));
                current = element;
            }
            StructuralTypeShape::PrimitiveScalar(primitive) if depth != 0 => {
                return Ok((
                    primitive,
                    if empty {
                        0
                    } else {
                        count.ok_or(LoweringError::Unsupported(
                            "computed array leaf count overflows",
                        ))?
                    },
                ));
            }
            _ => return unsupported("computed array requires a fixed primitive array shape"),
        }
    }
    unsupported("computed array type is cyclic")
}

pub(crate) fn emit(
    effects: &[LoweredScalarArrayConstruction],
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    let types = values
        .iter()
        .map(|value| value.scalar_type)
        .collect::<Vec<_>>();
    for effect in effects {
        let mut elements = Vec::with_capacity(effect.elements.len());
        for element in &effect.elements {
            validate_direct_parameter_types(element, &types)?;
            if !matches!(element, LoweredDirectExpression::Parameter { .. }) {
                return unsupported("array construction requires completed scalar leaves");
            }
            elements.push(emit_direct_expression(
                element, values, next_value, operations,
            ));
        }
        let id = operations.allocate();
        operations.push(Operation {
            id,
            result: OperationResult::Structural(StructuralOperationResult {
                place: effect.place,
                structural_type: effect.structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishScalarArray { elements },
        });
    }
    Ok(())
}

pub(crate) fn declarations(
    operations: &[Operation],
) -> impl Iterator<Item = StructuralPlaceDeclaration> + '_ {
    operations.iter().filter_map(|operation| {
        if !matches!(operation.kind, OperationKind::EstablishScalarArray { .. }) {
            return None;
        }
        let OperationResult::Structural(result) = &operation.result else {
            return None;
        };
        Some(StructuralPlaceDeclaration {
            id: result.place,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation.id,
                structural_type: result.structural_type,
            },
        })
    })
}
