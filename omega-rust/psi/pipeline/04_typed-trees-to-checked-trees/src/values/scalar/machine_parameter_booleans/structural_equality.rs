//! Structural equality of two structural parameter places.
//!
//! Equal records have equal fields; equal sums select the same case and then
//! have equal payloads. `lower` decomposes one `==` into leaf comparisons
//! (Boolean, integer, IEEE, byte-sequence, and payloadless-sum leaves) joined
//! by `and` across fields and `or` across cases. Recursive data is refused.

use super::structural_paths::{
    path_type_reference, payloadless_sum_cases, structural_record_fields,
};
use crate::values::scalar::expression_facts::is_integer;
use crate::values::scalar::structural_fields::{structural_data, structural_parameter_field_path};
use checked_trees::{
    CheckedBooleanExpression, CheckedIeeeFloatComparisonKind, CheckedIntegerComparisonKind,
    CheckedScalarExpression, CheckedStructuralParameterField,
    CheckedStructuralPredicatePathSegment,
};
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField, DataMember, DataShapeKind};
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

/// `left == right` over two structural parameter places of one data type, or
/// `None` when either side is not such a place or the type cannot decompose.
pub(super) fn lower(
    program: &TypedTrees,
    parameters: &[StateParameter],
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    let mut left_path = Vec::new();
    let mut right_path = Vec::new();
    let left_parameter =
        structural_parameter_field_path(program, parameters, left, &mut left_path)?;
    let right_parameter =
        structural_parameter_field_path(program, parameters, right, &mut right_path)?;
    let left_type = path_type_reference(program, parameters, left_parameter, &left_path)?;
    let right_type = path_type_reference(program, parameters, right_parameter, &right_path)?;
    let left_data = structural_data(program, left_type)?;
    let right_data = structural_data(program, right_type)?;
    if left_data.symbol != right_data.symbol || left_data.name != right_data.name {
        return None;
    }
    let mut decomposition = Decomposition {
        program,
        left_parameter,
        right_parameter,
        allow_direct_nested_mixed: left_path.is_empty()
            && right_path.is_empty()
            && matches!(
                DataDefinition::shape_kind_from_members(program.data_members(left_data)),
                DataShapeKind::Record
            ),
        nested_mixed_seen: false,
        visiting: Vec::new(),
    };
    let mut comparisons = Vec::new();
    decomposition.append(left_type, &left_path, &right_path, &mut comparisons)?;
    Some(conjunction(comparisons).unwrap_or(CheckedBooleanExpression::Constant(true)))
}

/// One equality's decomposition state: the two compared parameters, the data
/// types already on the path (recursion is refused), and the nested
/// mixed-shape allowance.
struct Decomposition<'program> {
    program: &'program TypedTrees,
    left_parameter: u32,
    right_parameter: u32,
    /// A mixed-shape value (fields and cases) nested under record fields is
    /// admitted only when the compared roots are whole record parameters.
    allow_direct_nested_mixed: bool,
    nested_mixed_seen: bool,
    visiting: Vec<symbols::SymbolHandle>,
}

impl Decomposition<'_> {
    /// Append the comparisons proving the values at `left_path` and
    /// `right_path`, both of `type_reference`, equal.
    fn append(
        &mut self,
        type_reference: TypeReferenceHandle,
        left_path: &[CheckedStructuralPredicatePathSegment],
        right_path: &[CheckedStructuralPredicatePathSegment],
        comparisons: &mut Vec<CheckedBooleanExpression>,
    ) -> Option<()> {
        let program = self.program;
        if append_direct_structural_leaf_equality(
            program,
            self.left_parameter,
            self.right_parameter,
            type_reference,
            left_path.to_vec(),
            right_path.to_vec(),
            comparisons,
        )
        .is_some()
        {
            return Some(());
        }
        if let Some(cases) = payloadless_sum_cases(program, type_reference) {
            comparisons.push(CheckedBooleanExpression::PayloadlessSumEqual {
                left: self.left_place(left_path),
                right: self.right_place(right_path),
                cases,
            });
            return Some(());
        }
        let data = structural_data(program, type_reference)?;
        if !data.symbol.is_valid() || self.visiting.contains(&data.symbol) {
            return None;
        }
        self.visiting.push(data.symbol);
        let result = self.append_data(data, type_reference, left_path, right_path, comparisons);
        self.visiting.pop();
        result
    }

    fn append_data(
        &mut self,
        data: &DataDefinition,
        type_reference: TypeReferenceHandle,
        left_path: &[CheckedStructuralPredicatePathSegment],
        right_path: &[CheckedStructuralPredicatePathSegment],
        comparisons: &mut Vec<CheckedBooleanExpression>,
    ) -> Option<()> {
        let program = self.program;
        let members = program.data_members(data);
        match DataDefinition::shape_kind_from_members(members) {
            DataShapeKind::Empty => Some(()),
            DataShapeKind::Record => {
                for field in structural_record_fields(program, type_reference)? {
                    self.append_field(field, None, left_path, right_path, comparisons)?;
                }
                Some(())
            }
            DataShapeKind::Enum => {
                if members
                    .iter()
                    .any(|member| !matches!(member, DataMember::Variant(_)))
                {
                    return None;
                }
                self.append_cases(members, left_path, right_path, comparisons)
            }
            DataShapeKind::Mixed => {
                // The bounded nested mixed-shape slice permits one through
                // fourteen direct record fields before the sole mixed
                // occurrence. Deeper records, case payloads, and two mixed
                // siblings retain their fail-closed fence until their
                // independent path and replay canaries land.
                if !left_path.is_empty() || !right_path.is_empty() {
                    if !is_bounded_nested_mixed_field_path_pair(left_path, right_path)
                        || !self.allow_direct_nested_mixed
                        || self.nested_mixed_seen
                    {
                        return None;
                    }
                    self.nested_mixed_seen = true;
                }
                for member in members {
                    let DataMember::Field(field) = member else {
                        continue;
                    };
                    if field.relevance.is_erased() {
                        return None;
                    }
                    self.append_field(field, None, left_path, right_path, comparisons)?;
                }
                self.append_cases(members, left_path, right_path, comparisons)
            }
        }
    }

    /// Equal sums select the same case and then have equal payloads: one
    /// `and` arm per case, joined by `or`.
    fn append_cases(
        &mut self,
        members: &[DataMember],
        left_path: &[CheckedStructuralPredicatePathSegment],
        right_path: &[CheckedStructuralPredicatePathSegment],
        comparisons: &mut Vec<CheckedBooleanExpression>,
    ) -> Option<()> {
        let mut arms = Vec::new();
        for member in members {
            let DataMember::Variant(variant) = member else {
                continue;
            };
            let case = variant.path_identity();
            let mut arm = vec![
                CheckedBooleanExpression::StructuralCaseMembership {
                    subject: self.left_place(left_path),
                    case: case.clone(),
                },
                CheckedBooleanExpression::StructuralCaseMembership {
                    subject: self.right_place(right_path),
                    case: case.clone(),
                },
            ];
            for field in self.program.data_payload_fields(variant) {
                if field.relevance.is_erased() {
                    return None;
                }
                self.append_field(field, Some(&case), left_path, right_path, &mut arm)?;
            }
            arms.push(conjunction(arm)?);
        }
        comparisons.push(disjunction(arms)?);
        Some(())
    }

    /// Compare one field on both sides, under `case` when it is a payload
    /// field.
    fn append_field(
        &mut self,
        field: &DataField,
        case: Option<&str>,
        left_path: &[CheckedStructuralPredicatePathSegment],
        right_path: &[CheckedStructuralPredicatePathSegment],
        comparisons: &mut Vec<CheckedBooleanExpression>,
    ) -> Option<()> {
        let extend = |path: &[CheckedStructuralPredicatePathSegment]| {
            let mut path = path.to_vec();
            if let Some(case) = case {
                path.push(CheckedStructuralPredicatePathSegment::Case(case.to_owned()));
            }
            path.push(CheckedStructuralPredicatePathSegment::Field(
                field.path_identity(),
            ));
            path
        };
        self.append(
            field.type_reference,
            &extend(left_path),
            &extend(right_path),
            comparisons,
        )
    }

    fn left_place(
        &self,
        path: &[CheckedStructuralPredicatePathSegment],
    ) -> CheckedStructuralParameterField {
        CheckedStructuralParameterField {
            parameter_position: self.left_parameter,
            path: path.to_vec(),
        }
    }

    fn right_place(
        &self,
        path: &[CheckedStructuralPredicatePathSegment],
    ) -> CheckedStructuralParameterField {
        CheckedStructuralParameterField {
            parameter_position: self.right_parameter,
            path: path.to_vec(),
        }
    }
}

fn conjunction(parts: Vec<CheckedBooleanExpression>) -> Option<CheckedBooleanExpression> {
    let mut parts = parts.into_iter();
    let first = parts.next()?;
    Some(
        parts.fold(first, |left, right| CheckedBooleanExpression::And {
            left: Box::new(left),
            right: Box::new(right),
        }),
    )
}

fn disjunction(parts: Vec<CheckedBooleanExpression>) -> Option<CheckedBooleanExpression> {
    let mut parts = parts.into_iter();
    let first = parts.next()?;
    Some(
        parts.fold(first, |left, right| CheckedBooleanExpression::Or {
            left: Box::new(left),
            right: Box::new(right),
        }),
    )
}

fn append_direct_structural_leaf_equality(
    program: &TypedTrees,
    left_parameter: u32,
    right_parameter: u32,
    type_reference: TypeReferenceHandle,
    left: Vec<CheckedStructuralPredicatePathSegment>,
    right: Vec<CheckedStructuralPredicatePathSegment>,
    comparisons: &mut Vec<CheckedBooleanExpression>,
) -> Option<()> {
    match program.primitive_type_reference(type_reference) {
        Some(PrimitiveType::Bool) => {
            comparisons.push(CheckedBooleanExpression::Equal {
                left: Box::new(CheckedBooleanExpression::StructuralParameterField {
                    parameter_position: left_parameter,
                    path: left,
                }),
                right: Box::new(CheckedBooleanExpression::StructuralParameterField {
                    parameter_position: right_parameter,
                    path: right,
                }),
            });
        }
        Some(primitive_type)
            if is_integer(primitive_type) && primitive_type != PrimitiveType::Addr =>
        {
            comparisons.push(CheckedBooleanExpression::IntegerComparison {
                kind: CheckedIntegerComparisonKind::Equal,
                left: Box::new(CheckedScalarExpression::StructuralParameterField {
                    parameter_position: left_parameter,
                    path: left,
                    primitive_type,
                }),
                right: Box::new(CheckedScalarExpression::StructuralParameterField {
                    parameter_position: right_parameter,
                    path: right,
                    primitive_type,
                }),
            });
        }
        Some(primitive_type)
            if matches!(primitive_type, PrimitiveType::F32 | PrimitiveType::F64) =>
        {
            let mut left = CheckedStructuralParameterField {
                parameter_position: left_parameter,
                path: left,
            };
            let mut right = CheckedStructuralParameterField {
                parameter_position: right_parameter,
                path: right,
            };
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            comparisons.push(CheckedBooleanExpression::IeeeFloatComparison {
                kind: CheckedIeeeFloatComparisonKind::Equal,
                primitive_type,
                left,
                right,
            });
        }
        Some(_) => return None,
        None if crate::execution::terminal_unit::types::byte_sequence_carrier(
            program,
            type_reference,
            &[],
        )
        .is_some() =>
        {
            let mut left = CheckedStructuralParameterField {
                parameter_position: left_parameter,
                path: left,
            };
            let mut right = CheckedStructuralParameterField {
                parameter_position: right_parameter,
                path: right,
            };
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            comparisons.push(CheckedBooleanExpression::ByteSequenceEqual { left, right });
        }
        None => return None,
    }
    Some(())
}

fn is_bounded_nested_mixed_field_path_pair(
    left: &[CheckedStructuralPredicatePathSegment],
    right: &[CheckedStructuralPredicatePathSegment],
) -> bool {
    const MAX_ENCLOSING_FIELDS: usize = 14;

    !left.is_empty()
        && left.len() == right.len()
        && left.len() <= MAX_ENCLOSING_FIELDS
        && left
            .iter()
            .all(|segment| matches!(segment, CheckedStructuralPredicatePathSegment::Field(_)))
        && right
            .iter()
            .all(|segment| matches!(segment, CheckedStructuralPredicatePathSegment::Field(_)))
}
