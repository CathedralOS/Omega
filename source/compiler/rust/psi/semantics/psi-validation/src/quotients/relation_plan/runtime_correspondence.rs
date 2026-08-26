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
//! array or direct Boolean-literal array to its exact fixed-array
//! representative position.
//! Neither policy infers or selects a relation, contract proof, or
//! representative operation.

use super::{
    ExactQuotientRelation, InputRelation, RelationPlanError, RepresentativeStaticBinding,
    RepresentativeTelescope,
};
use psi_numerics::literals::{FloatFormat, IntegerLanding, IntegerLiteral, LandedIntegerType};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

use super::static_application::substituted_type_matches;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DefineRuntimePosition {
    pub(super) public_parameter: SymbolHandle,
    pub(super) representative_parameter: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::quotients) struct DefineRuntimeCorrespondence {
    pub(super) positions: Vec<DefineRuntimePosition>,
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
        target_type: psi_typed_trees::type_identity::NormalizedTypeIdentity,
    },
    FixedByteArray {
        bytes: std::sync::Arc<[u8]>,
        target_type: psi_typed_trees::type_identity::NormalizedTypeIdentity,
    },
    BooleanArray {
        values: std::sync::Arc<[bool]>,
        target_type: psi_typed_trees::type_identity::NormalizedTypeIdentity,
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
pub(in crate::quotients) struct DirectLiftRuntimeCorrespondence {
    pub(super) positions: Vec<DirectLiftRuntimePosition>,
}

pub(super) fn derive_define_runtime_correspondence(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &TableCallExpression,
    input_relations: &[InputRelation],
    result_relation: ExactQuotientRelation,
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
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &TableCallExpression,
    input_relations: &[InputRelation],
    result_relation: ExactQuotientRelation,
    representative: &RepresentativeTelescope,
) -> Result<DirectLiftRuntimeCorrespondence, RelationPlanError> {
    if !program.machine_type_parameters(machine).is_empty() {
        return Err(RelationPlanError::DirectLiftOwnerRequiresSubstitution);
    }
    let public_parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_const)
        .collect::<Vec<_>>();
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
        let source = if let Some(argument_symbol) =
            direct_public_parameter_symbol(program, *argument)
        {
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
            if representative_parameter.is_mutable || representative_parameter.is_self {
                return Err(RelationPlanError::DirectLiftParameterModeMismatch(position));
            }
            let literal = closed_lift_literal_for_representative(
                program,
                *argument,
                representative_parameter.type_reference,
                position,
            )?
            .ok_or(RelationPlanError::DirectLiftArgumentIsNotPublicParameter(
                position,
            ))?;
            if *relation != InputRelation::ExactEquality(representative_parameter.type_reference) {
                return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
            }
            DirectLiftArgumentSource::Literal(literal)
        };
        positions.push(DirectLiftRuntimePosition {
            source,
            representative_parameter: representative_parameter.symbol,
        });
    }
    if !quotient_carrier_matches_type(
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
fn derive_define_position_runtime_correspondence(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &TableCallExpression,
    input_relations: &[InputRelation],
    result_relation: ExactQuotientRelation,
    representative: &RepresentativeTelescope,
) -> Result<Vec<DefineRuntimePosition>, RelationPlanError> {
    if !program.machine_type_parameters(machine).is_empty() {
        return Err(RelationPlanError::DefineOwnerRequiresSubstitution);
    }
    let public_parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_const)
        .collect::<Vec<_>>();
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
    if !quotient_carrier_matches_type(
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
    program: &TypedTrees,
    expression: ExpressionHandle,
    representative_type: TypeReferenceHandle,
    position: usize,
) -> Result<Option<ClosedLiftLiteral>, RelationPlanError> {
    if let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(expression)
    {
        let TypeReferenceNode::FixedArray {
            element_type,
            length: psi_typed_trees::types::FixedArrayLength::Literal(width),
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
        if program.primitive_type_reference(*element_type) == Some(PrimitiveType::Bool) {
            let values = elements
                .iter()
                .map(|element| {
                    let ExpressionNode::Boolean(value) =
                        program.expression_table.expression(*element)
                    else {
                        return None;
                    };
                    Some(*value)
                })
                .collect::<Option<Vec<_>>>()
                .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position))?;
            return Ok(Some(ClosedLiftLiteral::BooleanArray {
                values: values.into(),
                target_type: program.normalized_type_identity(representative_type),
            }));
        }
        if program.primitive_type_reference(*element_type) != Some(PrimitiveType::U8) {
            return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
        }
        let bytes = elements
            .iter()
            .map(|element| {
                let ExpressionNode::Integer(literal) =
                    program.expression_table.expression(*element)
                else {
                    return None;
                };
                let value = literal
                    .value_u64()
                    .and_then(|value| u8::try_from(value).ok())?;
                (literal.landing().is_none() && literal.text() == value.to_string())
                    .then_some(value)
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(RelationPlanError::DirectLiftLiteralTargetMismatch(position))?;
        return Ok(Some(ClosedLiftLiteral::FixedByteArray {
            bytes: bytes.into(),
            target_type: program.normalized_type_identity(representative_type),
        }));
    }
    if let ExpressionNode::String(bytes) = program.expression_table.expression(expression) {
        let exact_target = exact_shared_byte_slice(program, representative_type)
            || crate::expression_types::bounded_byte_buffer_capacity(program, representative_type)
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
        return match program.expression_table.expression(expression) {
            ExpressionNode::Boolean(_) | ExpressionNode::Integer(_) | ExpressionNode::Float(_) => {
                Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position))
            }
            _ => Ok(None),
        };
    };
    let Some(primitive) = PrimitiveType::from_name(name.as_str()) else {
        return match program.expression_table.expression(expression) {
            ExpressionNode::Boolean(_) | ExpressionNode::Integer(_) | ExpressionNode::Float(_) => {
                Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position))
            }
            _ => Ok(None),
        };
    };
    // Atomic aliases also report an underlying primitive. This rung accepts
    // only the exact scalar spelling, never an adapted wrapper.
    if name.as_str() != primitive.name() {
        return match program.expression_table.expression(expression) {
            ExpressionNode::Boolean(_) | ExpressionNode::Integer(_) | ExpressionNode::Float(_) => {
                Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position))
            }
            _ => Ok(None),
        };
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) if primitive == PrimitiveType::Bool => {
            Ok(Some(ClosedLiftLiteral::Boolean(*value)))
        }
        ExpressionNode::Boolean(_) => {
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position))
        }
        ExpressionNode::Integer(literal) => {
            let target_domain = program.arithmetic_domain_for_type_reference(representative_type);
            let landing = match literal.landing() {
                Some(landing) => landing,
                None => IntegerLanding {
                    landed_type: integer_primitive_landing(primitive)
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
            Ok(Some(ClosedLiftLiteral::Integer {
                spelling: literal.text().to_owned(),
                landing,
            }))
        }
        ExpressionNode::Float(literal) => {
            let expected = match primitive {
                PrimitiveType::F32 => FloatFormat::F32,
                PrimitiveType::F64 => FloatFormat::F64,
                _ => {
                    return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
                }
            };
            let landing = literal.landing().unwrap_or(expected);
            if landing != expected {
                return Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position));
            }
            Ok(Some(ClosedLiftLiteral::Float {
                spelling: literal.text().to_owned(),
                landing,
            }))
        }
        _ => Ok(None),
    }
}

fn exact_shared_byte_slice(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return false;
    };
    if *access != psi_language_core::ReferenceAccess::Shared {
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

fn landed_primitive(landed: LandedIntegerType) -> PrimitiveType {
    match landed {
        LandedIntegerType::I8 => PrimitiveType::I8,
        LandedIntegerType::I16 => PrimitiveType::I16,
        LandedIntegerType::I32 => PrimitiveType::I32,
        LandedIntegerType::I64 => PrimitiveType::I64,
        LandedIntegerType::U8 => PrimitiveType::U8,
        LandedIntegerType::U16 => PrimitiveType::U16,
        LandedIntegerType::U32 => PrimitiveType::U32,
        LandedIntegerType::U64 => PrimitiveType::U64,
        LandedIntegerType::Addr => PrimitiveType::Addr,
    }
}

fn integer_primitive_landing(primitive: PrimitiveType) -> Option<LandedIntegerType> {
    Some(match primitive {
        PrimitiveType::I8 => LandedIntegerType::I8,
        PrimitiveType::I16 => LandedIntegerType::I16,
        PrimitiveType::I32 => LandedIntegerType::I32,
        PrimitiveType::I64 => LandedIntegerType::I64,
        PrimitiveType::U8 => LandedIntegerType::U8,
        PrimitiveType::U16 => LandedIntegerType::U16,
        PrimitiveType::U32 => LandedIntegerType::U32,
        PrimitiveType::U64 => LandedIntegerType::U64,
        PrimitiveType::Addr => LandedIntegerType::Addr,
        _ => return None,
    })
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
                .is_some_and(|value| width == 64 || value <= (1u64 << width) - 1)
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

fn direct_public_parameter_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let expression = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => inner.target,
        _ => expression,
    };
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    (program
        .expression_table
        .name_path_members(path.members)
        .len()
        == 1
        && path.symbol.is_valid())
    .then_some(path.symbol)
}

fn input_relation_matches_representative_type(
    program: &TypedTrees,
    relation: InputRelation,
    representative_type: TypeReferenceHandle,
    substitutions: &[RepresentativeStaticBinding],
) -> bool {
    match relation {
        InputRelation::ExactEquality(public_type) => {
            substituted_type_matches(program, representative_type, public_type, substitutions)
        }
        InputRelation::Quotient(relation) => {
            quotient_carrier_matches_type(program, relation, representative_type, substitutions)
        }
    }
}

fn input_relation_matches_public_type(
    program: &TypedTrees,
    relation: InputRelation,
    public_type: TypeReferenceHandle,
) -> bool {
    let relation_type = match relation {
        InputRelation::Quotient(relation) => relation.quotient_type,
        InputRelation::ExactEquality(type_reference) => type_reference,
    };
    program.normalized_type_identity(relation_type) == program.normalized_type_identity(public_type)
}

fn quotient_carrier_matches_type(
    program: &TypedTrees,
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
    let Some(quotient) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == relation.quotient_symbol)
    else {
        return false;
    };
    let Some(metadata) = quotient.quotient.as_ref() else {
        return false;
    };
    let Some(carrier_symbol) = super::super::base_data_symbol(program, metadata.carrier) else {
        return false;
    };
    let Some(carrier) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == carrier_symbol)
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
