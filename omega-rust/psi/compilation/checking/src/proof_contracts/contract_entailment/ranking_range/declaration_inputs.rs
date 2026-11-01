//! A declared integer view receives one typed input, not necessarily an integer.
//! Its demanded scalar reads are resolved in the declaration before callers
//! capture their values. This describes no loaded value and proves no access,
//! selector bound, input qualification, or arithmetic formation.

use crate::CheckingInput;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::measure::MeasureDefinition;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredInputRead {
    pub coordinate: crate::RankSubjectCoordinate,
    pub type_reference: TypeReferenceHandle,
    pub primitive_type: PrimitiveType,
    /// Repeated syntax reads one captured coordinate; every authored occurrence
    /// remains bound so normalization cannot silently read another namespace.
    pub occurrences: Vec<ExpressionHandle>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredInputShape {
    pub parameter: SymbolHandle,
    pub type_reference: TypeReferenceHandle,
    pub reads: Vec<DeclaredInputRead>,
}

/// Preserve the ordinary argument-type boundary, including nominal identity,
/// generic applications and qualifications. A readable reference may supply an
/// observation of its referent; this supplies neither a load nor ownership.
pub fn declared_input_type_matches(
    program: &dyn CheckingInput,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
) -> bool {
    if !program.type_reference_table.contains_type_reference(actual)
        || !program
            .type_reference_table
            .contains_type_reference(required)
        || !actual.is_valid()
        || !required.is_valid()
    {
        return false;
    }
    let Some(actual) = crate::value_custody::expression_types::readable_observation_type_reference(
        program, actual,
    ) else {
        return false;
    };
    // Scalar qualifications are independently formed on the actual value by
    // the integer-application owner, rather than inferred from type identity.
    if let Some(carrier) = program.primitive_type_reference(required) {
        return carrier.is_fixed_integer()
            && program.primitive_type_reference(actual) == Some(carrier)
            && program.arithmetic_domain_for_type_reference(actual)
                == program.arithmetic_domain_for_type_reference(required);
    }
    crate::value_custody::expression_types::completed_value_type_matches(program, actual, required)
}

pub fn declared_input_shape(
    program: &dyn CheckingInput,
    measure: &MeasureDefinition,
) -> Option<DeclaredInputShape> {
    let formal = measure.parameter.as_ref()?;
    let mut shape = DeclaredInputShape {
        parameter: formal.symbol,
        type_reference: formal.type_reference,
        reads: Vec::new(),
    };
    if let Some(primitive_type) = program.primitive_type_reference(formal.type_reference) {
        primitive_type.is_fixed_integer().then_some(())?;
        shape.reads.push(DeclaredInputRead {
            coordinate: crate::RankSubjectCoordinate {
                parameter: formal.symbol,
                path: Vec::new(),
            },
            type_reference: formal.type_reference,
            primitive_type,
            occurrences: Vec::new(),
        });
        return Some(shape);
    }
    let observed = crate::value_custody::expression_types::readable_observation_type_reference(
        program,
        formal.type_reference,
    )?;
    program.closed_data_definition(observed)?;
    let mut pending = program
        .expression_table
        .expression_handles(measure.body)
        .to_vec();
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        if let typed_trees::expression::ExpressionNode::Indexed(indexed) =
            program.expression_table.expression(expression)
        {
            let operand_types = [indexed.collection, indexed.index].map(|operand| {
                crate::parameter_expression_result_type_reference(
                    program,
                    measure.symbol,
                    std::slice::from_ref(formal),
                    operand,
                )
            });
            if !typed_trees::operator::builtin_index_read_is_spelled(
                program,
                measure.symbol,
                expression,
                &operand_types,
            ) {
                return None;
            }
        }
        if let Some(place) = crate::canonical_place_from_expression(program, expression)
            && place.root.as_symbol() == Some(formal.symbol)
            && let Some(geometry) = crate::structural_scalar_coordinate_in_type(
                program,
                place.root,
                formal.type_reference,
                &place.segments,
            )
        {
            geometry.primitive_type.is_fixed_integer().then_some(())?;
            if let Some(read) = shape
                .reads
                .iter_mut()
                .find(|read| read.coordinate.path == place.segments)
            {
                read.occurrences.push(expression);
            } else {
                shape.reads.push(DeclaredInputRead {
                    coordinate: crate::RankSubjectCoordinate {
                        parameter: formal.symbol,
                        path: place.segments,
                    },
                    type_reference: geometry.result_type_reference,
                    primitive_type: geometry.primitive_type,
                    occurrences: vec![expression],
                });
            }
        }
        crate::expression_traversal::executable_children(
            program,
            program.expression_table.expression(expression),
            |child| pending.push(child),
        );
    }
    Some(shape)
}
