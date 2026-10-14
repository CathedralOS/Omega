//! Exact runtime correspondence for faithful quotient definitions and the
//! bounded direct-lift inclusion rung.
//!
//! Both judgments accept only direct public parameters and preserve
//! mutable/borrow mode while matching quotient carriers through the retained
//! representative static application. Faithful `define` remains declaration-
//! order preserving; `lift` may explicitly select, permute, and repeat direct
//! members of the public telescope or supply a closed boolean, integer with an
//! explicit or exact target-derived landing, float with an explicit or exact
//! target-derived format, immutable-image byte string to its exact shared byte
//! view or bounded value-domain buffer, or a canonically context-landed byte
//! array, direct Boolean-literal array, exact depth-two byte/Boolean/integer/
//! float-literal array, an exact depth-three Boolean tensor, any remaining
//! recursively nested exact primitive fixed-array literal, or exactly landed
//! integer/float-literal array to its exact fixed-array representative
//! position.
//! Neither policy infers or selects a relation, contract proof, or
//! representative operation.

use super::{
    ExactQuotientRelation, PositionalRelation, RelationPlanError, RepresentativeStaticBinding,
    RepresentativeTelescope,
};
use numerics::literals::{FloatFormat, IntegerLanding, IntegerLiteral, LandedIntegerType};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// `expression` is a literal when the lift wants a target-driven read; a
/// nonliteral expression leaves the closed lift to other recognizers.
fn literal_target_mismatch_or_none<T>(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    position: usize,
) -> Result<Option<T>, RelationPlanError> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(_) | ExpressionNode::Integer(_) | ExpressionNode::Float(_) => {
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position))
        }
        _ => Ok(None),
    }
}

use super::static_application::substituted_type_matches;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DefineRuntimePosition {
    pub(super) public_parameter: SymbolHandle,
    pub(super) representative_parameter: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::proof_contracts::quotients) struct DefineRuntimeCorrespondence {
    pub(super) positions: Vec<DefineRuntimePosition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ClosedIntegerArrayElement {
    pub(super) spelling: String,
    pub(super) landing: IntegerLanding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ClosedFloatArrayElement {
    pub(super) spelling: String,
    pub(super) landing: FloatFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ClosedRecursiveArrayElement {
    Boolean(bool),
    Byte(u8),
    Integer(ClosedIntegerArrayElement),
    Float(ClosedFloatArrayElement),
    Array(std::sync::Arc<[ClosedRecursiveArrayElement]>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ClosedLiftLiteral {
    Boolean(bool),
    Integer {
        spelling: String,
        landing: IntegerLanding,
    },
    Float {
        spelling: String,
        landing: FloatFormat,
    },
    ByteString {
        bytes: std::sync::Arc<[u8]>,
        target_type: typed_trees::type_identity::NormalizedTypeIdentity,
    },
    FixedByteArray {
        bytes: std::sync::Arc<[u8]>,
        target_type: typed_trees::type_identity::NormalizedTypeIdentity,
    },
    BooleanArray {
        values: std::sync::Arc<[bool]>,
        target_type: typed_trees::type_identity::NormalizedTypeIdentity,
    },
    NestedBooleanArray {
        rows: std::sync::Arc<[std::sync::Arc<[bool]>]>,
        target_type: typed_trees::type_identity::NormalizedTypeIdentity,
    },
    NestedFixedByteArray {
        rows: std::sync::Arc<[std::sync::Arc<[u8]>]>,
        target_type: typed_trees::type_identity::NormalizedTypeIdentity,
    },
    NestedIntegerArray {
        rows: std::sync::Arc<[std::sync::Arc<[ClosedIntegerArrayElement]>]>,
        target_type: typed_trees::type_identity::NormalizedTypeIdentity,
    },
    NestedFloatArray {
        rows: std::sync::Arc<[std::sync::Arc<[ClosedFloatArrayElement]>]>,
        target_type: typed_trees::type_identity::NormalizedTypeIdentity,
    },
    BooleanTensor3 {
        planes: std::sync::Arc<[std::sync::Arc<[std::sync::Arc<[bool]>]>]>,
        target_type: typed_trees::type_identity::NormalizedTypeIdentity,
    },
    RecursivePrimitiveArray {
        elements: std::sync::Arc<[ClosedRecursiveArrayElement]>,
        target_type: typed_trees::type_identity::NormalizedTypeIdentity,
    },
    IntegerArray {
        elements: std::sync::Arc<[ClosedIntegerArrayElement]>,
        target_type: typed_trees::type_identity::NormalizedTypeIdentity,
    },
    FloatArray {
        elements: std::sync::Arc<[ClosedFloatArrayElement]>,
        target_type: typed_trees::type_identity::NormalizedTypeIdentity,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DirectLiftArgumentSource {
    PublicParameter(SymbolHandle),
    Literal(ClosedLiftLiteral),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DirectLiftRuntimePosition {
    pub(super) source: DirectLiftArgumentSource,
    pub(super) representative_parameter: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::proof_contracts::quotients) struct DirectLiftRuntimeCorrespondence {
    pub(super) positions: Vec<DirectLiftRuntimePosition>,
}

pub(super) fn derive_define_runtime_correspondence(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    call: &TableCallExpression,
    input_relations: &[PositionalRelation],
    result_relation: PositionalRelation,
    representative: &RepresentativeTelescope,
) -> Result<DefineRuntimeCorrespondence, RelationPlanError> {
    derive_define_position_runtime_correspondence(
        program,
        machine,
        state,
        call,
        input_relations,
        result_relation,
        representative,
    )
    .map(|positions| DefineRuntimeCorrespondence { positions })
}

pub(super) fn derive_direct_lift_runtime_correspondence(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    call: &TableCallExpression,
    input_relations: &[PositionalRelation],
    result_relation: PositionalRelation,
    representative: &RepresentativeTelescope,
) -> Result<DirectLiftRuntimeCorrespondence, RelationPlanError> {
    if !program.machine_type_parameters(machine).is_empty() {
        return Err(RelationPlanError::DirectLiftOwnerRequiresSubstitution);
    }
    let public_parameters = program.public_state_parameters(state).collect::<Vec<_>>();
    let arguments = program.expression_table.expression_handles(call.arguments);
    if arguments.len() != representative.parameters.len()
        || input_relations.len() != arguments.len()
    {
        return Err(RelationPlanError::DirectLiftRuntimeArityMismatch);
    }
    if has_duplicate_parameter_symbols(public_parameters.iter().map(|parameter| parameter.symbol))
        || has_duplicate_parameter_symbols(
            representative
                .parameters
                .iter()
                .map(|parameter| parameter.symbol),
        )
    {
        return Err(RelationPlanError::DirectLiftParameterIdentityNotUnique);
    }

    let mut positions = Vec::with_capacity(arguments.len());
    for (position, ((argument, relation), representative_parameter)) in arguments
        .iter()
        .zip(input_relations)
        .zip(&representative.parameters)
        .enumerate()
    {
        let source =
            if let Some(argument_symbol) = direct_public_parameter_symbol(program, *argument) {
                let public = public_parameters
                    .iter()
                    .copied()
                    .find(|parameter| parameter.symbol == argument_symbol)
                    .ok_or(RelationPlanError::DirectLiftArgumentIsNotPublicParameter(
                        position,
                    ))?;
                if public.is_mutable != representative_parameter.is_mutable {
                    return Err(RelationPlanError::DirectLiftParameterModeMismatch(position));
                }
                if !input_relation_matches_public_type(program, *relation, public.type_reference)
                    || !input_relation_matches_representative_type(
                        program,
                        *relation,
                        representative_parameter.type_reference,
                        &representative.static_application.bindings,
                    )
                {
                    return Err(RelationPlanError::DirectLiftParameterTypeMismatch(position));
                }
                DirectLiftArgumentSource::PublicParameter(public.symbol)
            } else {
                if representative_parameter.is_mutable {
                    return Err(RelationPlanError::DirectLiftParameterModeMismatch(position));
                }
                let literal = closed_lift_literal_for_representative(
                    program,
                    *argument,
                    representative_parameter.type_reference,
                    position,
                )?
                .ok_or(
                    RelationPlanError::DirectLiftArgumentIsNotPublicParameter(position),
                )?;
                if *relation
                    != PositionalRelation::ExactEquality(representative_parameter.type_reference)
                {
                    return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
                }
                DirectLiftArgumentSource::Literal(literal)
            };
        positions.push(DirectLiftRuntimePosition {
            source,
            representative_parameter: representative_parameter.symbol,
        });
    }
    if !input_relation_matches_representative_type(
        program,
        result_relation,
        representative.return_type,
        &representative.static_application.bindings,
    ) {
        return Err(RelationPlanError::DirectLiftResultTypeMismatch);
    }
    Ok(DirectLiftRuntimeCorrespondence { positions })
}

#[allow(clippy::too_many_arguments)]

/// Close every literal element under the shared leaf shape — the element
/// mapping both fixed-array lanes of the correspondence walk run.
fn closed_array_elements(
    program: &dyn crate::CheckingInput,
    elements: &[ExpressionHandle],
    element_type: TypeReferenceHandle,
    leaf_type: TypeReferenceHandle,
    leaf_primitive: PrimitiveType,
    position: usize,
) -> Result<Vec<ClosedRecursiveArrayElement>, RelationPlanError> {
    elements
        .iter()
        .map(|element| {
            closed_recursive_array_element(
                program,
                *element,
                element_type,
                leaf_type,
                leaf_primitive,
                position,
            )
        })
        .collect()
}

fn derive_define_position_runtime_correspondence(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    call: &TableCallExpression,
    input_relations: &[PositionalRelation],
    result_relation: PositionalRelation,
    representative: &RepresentativeTelescope,
) -> Result<Vec<DefineRuntimePosition>, RelationPlanError> {
    if !program.machine_type_parameters(machine).is_empty() {
        return Err(RelationPlanError::DefineOwnerRequiresSubstitution);
    }
    let public_parameters = program.public_state_parameters(state).collect::<Vec<_>>();
    let arguments = program.expression_table.expression_handles(call.arguments);
    let arity_matches = arguments.len() == representative.parameters.len()
        && input_relations.len() == arguments.len()
        && public_parameters.len() == arguments.len();
    if !arity_matches {
        return Err(RelationPlanError::DefineRuntimeArityMismatch);
    }
    if has_duplicate_parameter_symbols(public_parameters.iter().map(|parameter| parameter.symbol))
        || has_duplicate_parameter_symbols(
            representative
                .parameters
                .iter()
                .map(|parameter| parameter.symbol),
        )
    {
        return Err(RelationPlanError::DefineParameterIdentityNotUnique);
    }

    let mut positions = Vec::with_capacity(arguments.len());
    for (position, ((argument, relation), representative_parameter)) in arguments
        .iter()
        .zip(input_relations)
        .zip(&representative.parameters)
        .enumerate()
    {
        let argument_symbol = direct_public_parameter_symbol(program, *argument).ok_or(
            RelationPlanError::DefineArgumentIsNotPublicParameter(position),
        )?;
        let public = public_parameters[position];
        if argument_symbol != public.symbol {
            return Err(RelationPlanError::DefineArgumentOrderMismatch(position));
        }
        if public.is_mutable != representative_parameter.is_mutable {
            return Err(RelationPlanError::DefineParameterModeMismatch(position));
        }
        if !input_relation_matches_public_type(program, *relation, public.type_reference)
            || !input_relation_matches_representative_type(
                program,
                *relation,
                representative_parameter.type_reference,
                &representative.static_application.bindings,
            )
        {
            return Err(RelationPlanError::DefineParameterTypeMismatch(position));
        }
        positions.push(DefineRuntimePosition {
            public_parameter: public.symbol,
            representative_parameter: representative_parameter.symbol,
        });
    }
    if !input_relation_matches_representative_type(
        program,
        result_relation,
        representative.return_type,
        &representative.static_application.bindings,
    ) {
        return Err(RelationPlanError::DefineResultTypeMismatch);
    }
    Ok(positions)
}

pub(super) fn closed_lift_literal_for_representative(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    representative_type: TypeReferenceHandle,
    position: usize,
) -> Result<Option<ClosedLiftLiteral>, RelationPlanError> {
    if let Some(elements) = program.expression_table.array_literal_node(expression) {
        let TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(width),
        } = program
            .type_reference_table
            .type_reference(representative_type)
        else {
            return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
        };
        let elements = program.expression_table.expression_handles(*elements);
        if elements.len() != *width {
            return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
        }
        if let TypeReferenceNode::FixedArray {
            element_type: row_type,
            length: typed_trees::types::FixedArrayLength::Literal(plane_height),
        } = program.type_reference_table.type_reference(*element_type)
            && let TypeReferenceNode::FixedArray {
                element_type: leaf_type,
                length: typed_trees::types::FixedArrayLength::Literal(row_width),
            } = program.type_reference_table.type_reference(*row_type)
            && exact_primitive_type(program, *leaf_type) == Some(PrimitiveType::Bool)
        {
            let planes = elements
                .iter()
                .map(|plane| {
                    let rows = program.expression_table.array_literal_node(*plane)?;
                    let rows = program.expression_table.expression_handles(*rows);
                    if rows.len() != *plane_height {
                        return None;
                    }
                    rows.iter()
                        .map(|row| {
                            let leaves = program.expression_table.array_literal_node(*row)?;
                            let leaves = program.expression_table.expression_handles(*leaves);
                            if leaves.len() != *row_width {
                                return None;
                            }
                            leaves
                                .iter()
                                .map(|leaf| {
                                    let value = program.expression_table.boolean_node(*leaf)?;
                                    Some(*value)
                                })
                                .collect::<Option<Vec<_>>>()
                                .map(std::sync::Arc::from)
                        })
                        .collect::<Option<Vec<_>>>()
                        .map(std::sync::Arc::from)
                })
                .collect::<Option<Vec<_>>>()
                .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position))?;
            return Ok(Some(ClosedLiftLiteral::BooleanTensor3 {
                planes: planes.into(),
                target_type: program.normalized_type_identity(representative_type),
            }));
        }
        if let Some((depth, leaf_type, leaf_primitive)) =
            literal_fixed_array_leaf(program, representative_type)
            && depth >= 3
        {
            let values = closed_array_elements(
                program,
                elements,
                *element_type,
                leaf_type,
                leaf_primitive,
                position,
            )?;
            return Ok(Some(ClosedLiftLiteral::RecursivePrimitiveArray {
                elements: values.into(),
                target_type: program.normalized_type_identity(representative_type),
            }));
        }
        if let TypeReferenceNode::FixedArray {
            element_type: row_element_type,
            length: typed_trees::types::FixedArrayLength::Literal(row_width),
        } = program.type_reference_table.type_reference(*element_type)
        {
            let row_primitive = exact_primitive_type(program, *row_element_type)
                .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position))?;
            if row_primitive == PrimitiveType::U8 {
                let rows = nested_literal_rows(program, elements, *row_width, position)?
                    .into_iter()
                    .map(|row_elements| {
                        canonical_fixed_bytes(program, row_elements).map(std::sync::Arc::from)
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position))?;
                return Ok(Some(ClosedLiftLiteral::NestedFixedByteArray {
                    rows: rows.into(),
                    target_type: program.normalized_type_identity(representative_type),
                }));
            }
            if crate::value_custody::literals::primitive_landing_type(row_primitive).is_some() {
                let rows = nested_literal_rows(program, elements, *row_width, position)?
                    .into_iter()
                    .map(|row_elements| {
                        row_elements
                            .iter()
                            .map(|element| {
                                closed_integer_array_element(
                                    program,
                                    *element,
                                    *row_element_type,
                                    row_primitive,
                                    position,
                                )
                            })
                            .collect::<Result<Vec<_>, _>>()
                            .map(std::sync::Arc::from)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(Some(ClosedLiftLiteral::NestedIntegerArray {
                    rows: rows.into(),
                    target_type: program.normalized_type_identity(representative_type),
                }));
            }
            if matches!(row_primitive, PrimitiveType::F32 | PrimitiveType::F64) {
                let rows = nested_literal_rows(program, elements, *row_width, position)?
                    .into_iter()
                    .map(|row_elements| {
                        row_elements
                            .iter()
                            .map(|element| {
                                closed_float_array_element(
                                    program,
                                    *element,
                                    row_primitive,
                                    position,
                                )
                            })
                            .collect::<Result<Vec<_>, _>>()
                            .map(std::sync::Arc::from)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(Some(ClosedLiftLiteral::NestedFloatArray {
                    rows: rows.into(),
                    target_type: program.normalized_type_identity(representative_type),
                }));
            }
            if row_primitive != PrimitiveType::Bool {
                return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
            }
            let rows = nested_literal_rows(program, elements, *row_width, position)?
                .into_iter()
                .map(|row_elements| {
                    closed_boolean_array_elements(program, row_elements).map(std::sync::Arc::from)
                })
                .collect::<Option<Vec<_>>>()
                .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position))?;
            return Ok(Some(ClosedLiftLiteral::NestedBooleanArray {
                rows: rows.into(),
                target_type: program.normalized_type_identity(representative_type),
            }));
        }
        let element_primitive = exact_primitive_type(program, *element_type)
            .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position))?;
        if element_primitive == PrimitiveType::Bool {
            let values = closed_boolean_array_elements(program, elements)
                .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position))?;
            return Ok(Some(ClosedLiftLiteral::BooleanArray {
                values: values.into(),
                target_type: program.normalized_type_identity(representative_type),
            }));
        }
        if element_primitive == PrimitiveType::U8
            && let Some(bytes) = canonical_fixed_bytes(program, elements)
        {
            return Ok(Some(ClosedLiftLiteral::FixedByteArray {
                bytes: bytes.into(),
                target_type: program.normalized_type_identity(representative_type),
            }));
        }
        if crate::value_custody::literals::primitive_landing_type(element_primitive).is_some() {
            let elements = elements
                .iter()
                .map(|element| {
                    closed_integer_array_element(
                        program,
                        *element,
                        *element_type,
                        element_primitive,
                        position,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(Some(ClosedLiftLiteral::IntegerArray {
                elements: elements.into(),
                target_type: program.normalized_type_identity(representative_type),
            }));
        }
        if matches!(element_primitive, PrimitiveType::F32 | PrimitiveType::F64) {
            let elements = elements
                .iter()
                .map(|element| {
                    closed_float_array_element(program, *element, element_primitive, position)
                })
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(Some(ClosedLiftLiteral::FloatArray {
                elements: elements.into(),
                target_type: program.normalized_type_identity(representative_type),
            }));
        }
        return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
    }
    if let Some(bytes) = program.expression_table.string_node(expression) {
        let exact_target = exact_shared_byte_slice(program, representative_type)
            || crate::value_custody::expression_types::bounded_byte_buffer_capacity(
                program,
                representative_type,
            )
            .is_some_and(|capacity| bytes.len() <= capacity);
        return exact_target
            .then(|| ClosedLiftLiteral::ByteString {
                bytes: bytes.clone(),
                target_type: program.normalized_type_identity(representative_type),
            })
            .map(Some)
            .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
    }
    let TypeReferenceNode::Named { name, .. } = program
        .type_reference_table
        .type_reference(representative_type)
    else {
        return literal_target_mismatch_or_none(program, expression, position);
    };
    let Some(primitive) = PrimitiveType::from_name(name.as_str()) else {
        return literal_target_mismatch_or_none(program, expression, position);
    };
    // Atomic aliases also report an underlying primitive. This rung accepts
    // only the exact scalar spelling, never an adapted wrapper.
    if name.as_str() != primitive.name() {
        return literal_target_mismatch_or_none(program, expression, position);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) if primitive == PrimitiveType::Bool => {
            Ok(Some(ClosedLiftLiteral::Boolean(*value)))
        }
        ExpressionNode::Boolean(_) => {
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position))
        }
        ExpressionNode::Integer(literal) => {
            let landing =
                exact_integer_landing(program, representative_type, primitive, literal, position)?;
            Ok(Some(ClosedLiftLiteral::Integer {
                spelling: literal.text().to_owned(),
                landing,
            }))
        }
        ExpressionNode::Float(literal) => {
            let landing = exact_float_landing(primitive, literal, position)?;
            Ok(Some(ClosedLiftLiteral::Float {
                spelling: literal.text().to_owned(),
                landing,
            }))
        }
        _ => Ok(None),
    }
}

fn exact_shared_byte_slice(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return false;
    };
    if *access != language_core::ReferenceAccess::Shared {
        return false;
    }
    let TypeReferenceNode::Slice { element_type } =
        program.type_reference_table.type_reference(*referee)
    else {
        return false;
    };
    matches!(
        program
            .type_reference_table
            .type_reference(*element_type),
        TypeReferenceNode::Named { name, .. } if name.as_str() == PrimitiveType::U8.name()
    )
}

fn exact_primitive_type(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let TypeReferenceNode::Named { name, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    PrimitiveType::from_name(name.as_str()).filter(|primitive| name.as_str() == primitive.name())
}

fn literal_fixed_array_leaf(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<(usize, TypeReferenceHandle, PrimitiveType)> {
    let mut depth = 0;
    let mut current = type_reference;
    loop {
        match program.type_reference_table.type_reference(current) {
            TypeReferenceNode::FixedArray {
                element_type,
                length: typed_trees::types::FixedArrayLength::Literal(_),
            } => {
                depth += 1;
                current = *element_type;
            }
            TypeReferenceNode::FixedArray { .. } => return None,
            _ => return exact_primitive_type(program, current).map(|leaf| (depth, current, leaf)),
        }
    }
}

fn closed_recursive_array_element(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    target_type: TypeReferenceHandle,
    leaf_type: TypeReferenceHandle,
    leaf_primitive: PrimitiveType,
    position: usize,
) -> Result<ClosedRecursiveArrayElement, RelationPlanError> {
    match program.type_reference_table.type_reference(target_type) {
        TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(width),
        } => {
            let Some(elements) = program.expression_table.array_literal_node(expression) else {
                return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
            };
            let elements = program.expression_table.expression_handles(*elements);
            if elements.len() != *width {
                return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
            }
            closed_array_elements(
                program,
                elements,
                *element_type,
                leaf_type,
                leaf_primitive,
                position,
            )
            .map(std::sync::Arc::from)
            .map(ClosedRecursiveArrayElement::Array)
        }
        _ if target_type == leaf_type
            && exact_primitive_type(program, target_type) == Some(leaf_primitive) =>
        {
            match (
                leaf_primitive,
                program.expression_table.expression(expression),
            ) {
                (PrimitiveType::Bool, ExpressionNode::Boolean(value)) => {
                    Ok(ClosedRecursiveArrayElement::Boolean(*value))
                }
                (PrimitiveType::U8, ExpressionNode::Integer(_)) => {
                    let byte = canonical_fixed_bytes(program, std::slice::from_ref(&expression))
                        .and_then(|bytes| bytes.into_iter().next())
                        .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position))?;
                    Ok(ClosedRecursiveArrayElement::Byte(byte))
                }
                (primitive, ExpressionNode::Integer(literal))
                    if crate::value_custody::literals::primitive_landing_type(primitive)
                        .is_some() =>
                {
                    let landing =
                        exact_integer_landing(program, target_type, primitive, literal, position)?;
                    Ok(ClosedRecursiveArrayElement::Integer(
                        ClosedIntegerArrayElement {
                            spelling: literal.text().to_owned(),
                            landing,
                        },
                    ))
                }
                (
                    primitive @ (PrimitiveType::F32 | PrimitiveType::F64),
                    ExpressionNode::Float(literal),
                ) => {
                    let landing = exact_float_landing(primitive, literal, position)?;
                    Ok(ClosedRecursiveArrayElement::Float(
                        ClosedFloatArrayElement {
                            spelling: literal.text().to_owned(),
                            landing,
                        },
                    ))
                }
                _ => Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
            }
        }
        _ => Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
    }
}

/// The shared integer-element arm of the flat and nested integer array
/// lifts: unwrap the literal and land it under `element_type`.
/// A nested array literal's elements each unwrapped to their row's spelled
/// elements, rejecting a row that is not itself a literal or whose width
/// disagrees with the declared shape.
fn nested_literal_rows<'program>(
    program: &'program dyn crate::CheckingInput,
    elements: &[ExpressionHandle],
    row_width: usize,
    position: usize,
) -> Result<Vec<&'program [ExpressionHandle]>, RelationPlanError> {
    elements
        .iter()
        .map(|row| {
            let Some(row_elements) = program.expression_table.array_literal_node(*row) else {
                return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
            };
            let row_elements = program.expression_table.expression_handles(*row_elements);
            if row_elements.len() != row_width {
                return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
            }
            Ok(row_elements)
        })
        .collect()
}

fn closed_integer_array_element(
    program: &dyn crate::CheckingInput,
    element: ExpressionHandle,
    element_type: TypeReferenceHandle,
    primitive: PrimitiveType,
    position: usize,
) -> Result<ClosedIntegerArrayElement, RelationPlanError> {
    let Some(literal) = program.expression_table.integer_node(element) else {
        return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
    };
    let landing = exact_integer_landing(program, element_type, primitive, literal, position)?;
    Ok(ClosedIntegerArrayElement {
        spelling: literal.text().to_owned(),
        landing,
    })
}

/// The shared float-element arm of the flat and nested float array lifts.
fn closed_float_array_element(
    program: &dyn crate::CheckingInput,
    element: ExpressionHandle,
    primitive: PrimitiveType,
    position: usize,
) -> Result<ClosedFloatArrayElement, RelationPlanError> {
    let ExpressionNode::Float(literal) = program.expression_table.expression(element) else {
        return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
    };
    let landing = exact_float_landing(primitive, literal, position)?;
    Ok(ClosedFloatArrayElement {
        spelling: literal.text().to_owned(),
        landing,
    })
}

/// The shared boolean-element arm of the flat and nested boolean array
/// lifts: every element an exact boolean literal.
fn closed_boolean_array_elements(
    program: &dyn crate::CheckingInput,
    elements: &[ExpressionHandle],
) -> Option<Vec<bool>> {
    elements
        .iter()
        .map(|element| {
            let value = program.expression_table.boolean_node(*element)?;
            Some(*value)
        })
        .collect()
}

fn canonical_fixed_bytes(
    program: &dyn crate::CheckingInput,
    elements: &[ExpressionHandle],
) -> Option<Vec<u8>> {
    elements
        .iter()
        .map(|element| {
            let literal = program.expression_table.integer_node(*element)?;
            let value = literal
                .value_u64()
                .and_then(|value| u8::try_from(value).ok())?;
            (literal.landing().is_none() && literal.text() == value.to_string()).then_some(value)
        })
        .collect()
}

fn exact_integer_landing(
    program: &dyn crate::CheckingInput,
    target_type: TypeReferenceHandle,
    primitive: PrimitiveType,
    literal: &IntegerLiteral,
    position: usize,
) -> Result<IntegerLanding, RelationPlanError> {
    let target_domain = program.arithmetic_domain_for_type_reference(target_type);
    let landing = match literal.landing() {
        Some(landing) => landing,
        None => IntegerLanding {
            landed_type: crate::value_custody::literals::primitive_landing_type(primitive)
                .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position))?,
            domain: target_domain,
        },
    };
    if landed_primitive(landing.landed_type) != primitive
        || landing.domain != target_domain
        || !integer_literal_fits(literal, landing.landed_type)
    {
        return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
    }
    Ok(landing)
}

fn exact_float_landing(
    primitive: PrimitiveType,
    literal: &numerics::literals::FloatLiteral,
    position: usize,
) -> Result<FloatFormat, RelationPlanError> {
    let expected = match primitive {
        PrimitiveType::F32 => FloatFormat::F32,
        PrimitiveType::F64 => FloatFormat::F64,
        _ => return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
    };
    let landing = literal.landing().unwrap_or(expected);
    if landing != expected {
        return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
    }
    Ok(landing)
}

pub(super) fn landed_primitive(landed: LandedIntegerType) -> PrimitiveType {
    PrimitiveType::from_landed_integer(landed)
}

fn integer_literal_fits(literal: &IntegerLiteral, landed: LandedIntegerType) -> bool {
    let width = landed.bit_width();
    if landed.is_signed() {
        literal.value_i64().is_some_and(|value| {
            width == 64 || {
                let minimum = -(1i64 << (width - 1));
                let maximum = (1i64 << (width - 1)) - 1;
                (minimum..=maximum).contains(&value)
            }
        })
    } else {
        !literal.text().starts_with('-')
            && literal
                .value_u64()
                .is_some_and(|value| width == 64 || value < (1u64 << width))
    }
}

fn has_duplicate_parameter_symbols(symbols: impl IntoIterator<Item = SymbolHandle>) -> bool {
    let mut seen = Vec::new();
    for symbol in symbols {
        if seen.contains(&symbol) {
            return true;
        }
        seen.push(symbol);
    }
    false
}

pub(super) fn direct_public_parameter_symbol(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let expression = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => inner.target,
        _ => expression,
    };
    let path = program.expression_table.name_path_node(expression)?;
    (program
        .expression_table
        .name_path_members(path.members)
        .len()
        == 1
        && path.symbol.is_valid())
    .then_some(path.symbol)
}

fn input_relation_matches_representative_type(
    program: &dyn crate::CheckingInput,
    relation: PositionalRelation,
    representative_type: TypeReferenceHandle,
    substitutions: &[RepresentativeStaticBinding],
) -> bool {
    match relation {
        PositionalRelation::ExactEquality(public_type) => {
            substituted_type_matches(program, representative_type, public_type, substitutions)
        }
        PositionalRelation::Quotient(relation) => {
            quotient_carrier_matches_type(program, relation, representative_type, substitutions)
        }
    }
}

fn input_relation_matches_public_type(
    program: &dyn crate::CheckingInput,
    relation: PositionalRelation,
    public_type: TypeReferenceHandle,
) -> bool {
    let relation_type = match relation {
        PositionalRelation::Quotient(relation) => relation.quotient_type,
        PositionalRelation::ExactEquality(type_reference) => type_reference,
    };
    program.normalized_type_identity(relation_type) == program.normalized_type_identity(public_type)
}

fn quotient_carrier_matches_type(
    program: &dyn crate::CheckingInput,
    relation: ExactQuotientRelation,
    representative_type: TypeReferenceHandle,
    substitutions: &[RepresentativeStaticBinding],
) -> bool {
    if !matches!(
        program
            .type_reference_table
            .type_reference(relation.quotient_type),
        TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
    ) {
        // Borrow/reference carrier substitution needs an exact shell-preserving
        // rewrite; do not erase that mode by unwrapping here.
        return false;
    }
    let Some(quotient) = crate::declarations::data_queries::data_definition_by_symbol(
        program,
        relation.quotient_symbol,
    ) else {
        return false;
    };
    let Some(metadata) = quotient.quotient.as_ref() else {
        return false;
    };
    let Some(carrier_symbol) =
        crate::proof_contracts::quotients::equivalence_selection::base_data_symbol(
            program,
            metadata.carrier,
        )
    else {
        return false;
    };
    let Some(carrier) =
        crate::declarations::data_queries::data_definition_by_symbol(program, carrier_symbol)
    else {
        return false;
    };
    quotient.properties.multiplicity == carrier.properties.multiplicity
        && substituted_type_matches(
            program,
            representative_type,
            metadata.carrier,
            substitutions,
        )
}
