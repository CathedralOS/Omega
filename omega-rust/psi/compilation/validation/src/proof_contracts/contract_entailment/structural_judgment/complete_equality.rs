//! Complete equality of resolver inputs, for reusing a completed query whose
//! inputs a sibling context rebuilt with equal contents in a fresh allocation.
//!
//! The completed-query table keyed its inputs by allocation identity. Sibling
//! proof contexts derive the same rewrite, substitution or stored value from
//! the same parent and each make their own `Arc`, so an identical question
//! asked again under an equal context always missed. A hit may instead accept
//! an owner whose contents are completely equal to the retained one.
//!
//! "Completely" is the point. `StructuralTerm`, `ConstructorIdentity` and
//! `FloatLiteral` keep deliberately partial `==` (semantic values that look
//! through observation wrappers, constructor carriers, literal spellings), so
//! a derive over them would not do. Every comparison here destructures every
//! field, so a new field fails to compile until it is compared, and any pair
//! of different variants compares unequal. A type without such a comparison
//! is matched by allocation identity only.

use super::{ConstructorIdentity, StructuralTerm};

/// Equality over every field, recursively; never a semantic or partial view.
pub(super) trait CompleteEq {
    fn complete_eq(&self, other: &Self) -> bool;
}

/// Types whose `PartialEq` is derived over every field, recursively, and
/// that contain no partially compared type.
macro_rules! derived_complete_eq {
    ($($type:ty),* $(,)?) => {
        $(impl CompleteEq for $type {
            fn complete_eq(&self, other: &Self) -> bool {
                self == other
            }
        })*
    };
}

derived_complete_eq!(
    String,
    bool,
    u64,
    usize,
    super::SymbolHandle,
    super::ExpressionHandle,
    super::TypeReferenceHandle,
    super::PrimitiveType,
    super::BinaryOperator,
    super::StaticMachineArgument,
    typed_trees::type_identity::TypeIdentityId,
    arena::Handle<typed_trees::statement::StatementNode>,
    numerics::bignum::BigInt,
    numerics::arithmetic::ArithmeticDomain,
    language_semantics::declaration_selection::CollectionMeasure,
    checked_trees::IntegerRangeRecipe,
);

impl<T: CompleteEq + ?Sized> CompleteEq for Box<T> {
    fn complete_eq(&self, other: &Self) -> bool {
        (**self).complete_eq(other)
    }
}

impl<T: CompleteEq + ?Sized> CompleteEq for std::sync::Arc<T> {
    fn complete_eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(self, other) || (**self).complete_eq(other)
    }
}

impl<T: CompleteEq> CompleteEq for Option<T> {
    fn complete_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Some(left), Some(right)) => left.complete_eq(right),
            (None, None) => true,
            _ => false,
        }
    }
}

impl<T: CompleteEq> CompleteEq for [T] {
    fn complete_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(left, right)| left.complete_eq(right))
    }
}

impl<T: CompleteEq> CompleteEq for Vec<T> {
    fn complete_eq(&self, other: &Self) -> bool {
        self.as_slice().complete_eq(other.as_slice())
    }
}

impl<T: CompleteEq, const N: usize> CompleteEq for [T; N] {
    fn complete_eq(&self, other: &Self) -> bool {
        self.as_slice().complete_eq(other.as_slice())
    }
}

impl<A: CompleteEq, B: CompleteEq> CompleteEq for (A, B) {
    fn complete_eq(&self, other: &Self) -> bool {
        self.0.complete_eq(&other.0) && self.1.complete_eq(&other.1)
    }
}

impl CompleteEq for numerics::literals::FloatLiteral {
    fn complete_eq(&self, other: &Self) -> bool {
        self.same_literal_completely(other)
    }
}

impl CompleteEq for ConstructorIdentity {
    fn complete_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Boolean(left), Self::Boolean(right)) => left == right,
            (
                Self::Nominal {
                    carrier,
                    reference,
                    declaration,
                    case,
                    member,
                },
                Self::Nominal {
                    carrier: other_carrier,
                    reference: other_reference,
                    declaration: other_declaration,
                    case: other_case,
                    member: other_member,
                },
            ) => {
                carrier.complete_eq(other_carrier)
                    && reference.complete_eq(other_reference)
                    && declaration.complete_eq(other_declaration)
                    && case.complete_eq(other_case)
                    && member.complete_eq(other_member)
            }
            _ => false,
        }
    }
}

impl CompleteEq for StructuralTerm {
    fn complete_eq(&self, other: &Self) -> bool {
        use StructuralTerm as T;
        match (self, other) {
            (T::Variable(left), T::Variable(right)) | (T::Opaque(left), T::Opaque(right)) => {
                left.complete_eq(right)
            }
            (
                T::ObservedValue {
                    observed_type,
                    value,
                    descriptor,
                },
                T::ObservedValue {
                    observed_type: other_observed_type,
                    value: other_value,
                    descriptor: other_descriptor,
                },
            ) => {
                observed_type.complete_eq(other_observed_type)
                    && value.complete_eq(other_value)
                    && descriptor.complete_eq(other_descriptor)
            }
            (T::BoundValue(left), T::BoundValue(right)) => left.complete_eq(right),
            (
                T::CapturedValue {
                    expression,
                    statement,
                    callable,
                    occurrence,
                    result_type,
                    integer_range,
                },
                T::CapturedValue {
                    expression: other_expression,
                    statement: other_statement,
                    callable: other_callable,
                    occurrence: other_occurrence,
                    result_type: other_result_type,
                    integer_range: other_integer_range,
                },
            ) => {
                expression.complete_eq(other_expression)
                    && statement.complete_eq(other_statement)
                    && callable.complete_eq(other_callable)
                    && occurrence.complete_eq(other_occurrence)
                    && result_type.complete_eq(other_result_type)
                    && integer_range.complete_eq(other_integer_range)
            }
            (
                T::BoundProjection { subject, path },
                T::BoundProjection {
                    subject: other_subject,
                    path: other_path,
                },
            ) => subject.complete_eq(other_subject) && path.complete_eq(other_path),
            (
                T::ScalarRepresentation {
                    source,
                    target,
                    value,
                },
                T::ScalarRepresentation {
                    source: other_source,
                    target: other_target,
                    value: other_value,
                },
            ) => {
                source.complete_eq(other_source)
                    && target.complete_eq(other_target)
                    && value.complete_eq(other_value)
            }
            (T::Integer(left), T::Integer(right)) => left.complete_eq(right),
            (
                T::RepeatedArray { length, element },
                T::RepeatedArray {
                    length: other_length,
                    element: other_element,
                },
            ) => length.complete_eq(other_length) && element.complete_eq(other_element),
            (
                T::IntegerEmbedding { reference, value },
                T::IntegerEmbedding {
                    reference: other_reference,
                    value: other_value,
                },
            ) => reference.complete_eq(other_reference) && value.complete_eq(other_value),
            (
                T::MathematicalBinary {
                    operator,
                    left,
                    right,
                },
                T::MathematicalBinary {
                    operator: other_operator,
                    left: other_left,
                    right: other_right,
                },
            ) => {
                operator.complete_eq(other_operator)
                    && left.complete_eq(other_left)
                    && right.complete_eq(other_right)
            }
            (T::Float(left), T::Float(right)) => left.complete_eq(right),
            (
                T::Constructor {
                    identity,
                    data,
                    case,
                    fields,
                },
                T::Constructor {
                    identity: other_identity,
                    data: other_data,
                    case: other_case,
                    fields: other_fields,
                },
            ) => {
                identity.complete_eq(other_identity)
                    && data.complete_eq(other_data)
                    && case.complete_eq(other_case)
                    && fields.complete_eq(other_fields)
            }
            (
                T::Application {
                    result_type,
                    target,
                    selections,
                    machine,
                    arguments,
                },
                T::Application {
                    result_type: other_result_type,
                    target: other_target,
                    selections: other_selections,
                    machine: other_machine,
                    arguments: other_arguments,
                },
            ) => {
                result_type.complete_eq(other_result_type)
                    && target.complete_eq(other_target)
                    && selections.complete_eq(other_selections)
                    && machine.complete_eq(other_machine)
                    && arguments.complete_eq(other_arguments)
            }
            (
                T::CallProjection {
                    selections,
                    target,
                    machine,
                    result_type,
                    field,
                    field_name,
                    arguments,
                },
                T::CallProjection {
                    selections: other_selections,
                    target: other_target,
                    machine: other_machine,
                    result_type: other_result_type,
                    field: other_field,
                    field_name: other_field_name,
                    arguments: other_arguments,
                },
            ) => {
                selections.complete_eq(other_selections)
                    && target.complete_eq(other_target)
                    && machine.complete_eq(other_machine)
                    && result_type.complete_eq(other_result_type)
                    && field.complete_eq(other_field)
                    && field_name.complete_eq(other_field_name)
                    && arguments.complete_eq(other_arguments)
            }
            (
                T::Projection { subject, path },
                T::Projection {
                    subject: other_subject,
                    path: other_path,
                },
            ) => subject.complete_eq(other_subject) && path.complete_eq(other_path),
            (
                T::CollectionMeasure {
                    measure,
                    collection,
                },
                T::CollectionMeasure {
                    measure: other_measure,
                    collection: other_collection,
                },
            ) => measure.complete_eq(other_measure) && collection.complete_eq(other_collection),
            (
                T::IndexedValue {
                    element_type,
                    element_identity,
                    collection,
                    index,
                },
                T::IndexedValue {
                    element_type: other_element_type,
                    element_identity: other_element_identity,
                    collection: other_collection,
                    index: other_index,
                },
            ) => {
                element_type.complete_eq(other_element_type)
                    && element_identity.complete_eq(other_element_identity)
                    && collection.complete_eq(other_collection)
                    && index.complete_eq(other_index)
            }
            (
                T::ScalarBinary {
                    references,
                    operator,
                    meaning,
                    left,
                    right,
                },
                T::ScalarBinary {
                    references: other_references,
                    operator: other_operator,
                    meaning: other_meaning,
                    left: other_left,
                    right: other_right,
                },
            ) => {
                references.complete_eq(other_references)
                    && operator.complete_eq(other_operator)
                    && meaning.complete_eq(other_meaning)
                    && left.complete_eq(other_left)
                    && right.complete_eq(other_right)
            }
            _ => false,
        }
    }
}
