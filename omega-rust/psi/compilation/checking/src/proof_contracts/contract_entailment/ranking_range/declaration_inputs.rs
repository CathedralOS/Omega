//! A declared integer view receives one typed input, not necessarily an integer.
//! Its demanded scalar reads are resolved in the declaration before callers
//! capture their values. This describes no loaded value and proves no access,
//! selector bound, input qualification, or arithmetic formation.

use crate::CheckingInput;
use symbols::SymbolHandle;
use typed_trees::measure::MeasureDefinition;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

mod reads;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredInputRead {
    pub coordinate: crate::RankSubjectCoordinate,
    pub type_reference: TypeReferenceHandle,
    pub primitive_type: PrimitiveType,
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
        });
        return Some(shape);
    }
    let observed = crate::value_custody::expression_types::readable_observation_type_reference(
        program,
        formal.type_reference,
    )?;
    program.closed_data_definition(observed)?;
    reads::collect(program, measure, &mut shape)?;
    Some(shape)
}
