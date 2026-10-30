//! Structural equality compares selected meaning and value identity.
//!
//! ScalarBinary raw operand references are observation authority, not semantic
//! identity: an authenticated &u32 input and its captured u32 value have the
//! same selected scalar meaning. Every consumer checks that authority before
//! using equality; transport retains it and resolution checks it again.
//! ObservedValue also preserves its authenticated input metadata without
//! changing the wrapped scalar identity. Debug retains the same semantic view
//! because existing polynomial normalization uses it as a deterministic key.

use super::StructuralTerm;

#[cfg(test)]
mod resolution_payload_tests {
    use super::*;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

    #[test]
    fn payload_comparison_keeps_hidden_descriptors_and_nested_authority() {
        let integer = |value| StructuralTerm::Integer(numerics::bignum::BigInt::from_i64(value));
        let observed = |reference, descriptor| StructuralTerm::ObservedValue {
            observed_type: TypeReferenceHandle::from_arena_index(reference),
            value: Box::new(integer(7)),
            descriptor,
        };
        let nested = |value| StructuralTerm::Constructor {
            data: "Carrier".to_owned(),
            case: "Only".to_owned(),
            fields: vec![("value".to_owned(), value)],
        };
        let binary = |reference| StructuralTerm::ScalarBinary {
            references: [TypeReferenceHandle::from_arena_index(reference); 2],
            operator: typed_trees::expression::BinaryOperator::Add,
            meaning: [(
                PrimitiveType::I32,
                numerics::arithmetic::ArithmeticDomain::Exact,
            ); 2],
            left: Box::new(integer(1)),
            right: Box::new(integer(2)),
        };
        let pairs = [
            (observed(1, None), observed(2, None)),
            (observed(1, None), observed(1, Some(Box::new(integer(1))))),
            (
                observed(1, Some(Box::new(integer(1)))),
                observed(1, Some(Box::new(integer(2)))),
            ),
            (
                observed(1, Some(Box::new(observed(1, Some(Box::new(integer(1))))))),
                observed(1, Some(Box::new(observed(1, Some(Box::new(integer(2))))))),
            ),
            (binary(1), binary(2)),
            (
                StructuralTerm::Float(numerics::literals::FloatLiteral::parse("1.25f32").unwrap()),
                StructuralTerm::Float(numerics::literals::FloatLiteral::parse("1.25f64").unwrap()),
            ),
        ];
        for (left, right) in pairs {
            let (left, right) = (nested(left), nested(right));
            assert_eq!(
                left, right,
                "the semantic comparison must erase this metadata"
            );
            assert!(left.same_resolution_payload(&left.clone()));
            assert!(!left.same_resolution_payload(&right));
            assert!(!right.same_resolution_payload(&left));
        }
    }
}

impl StructuralTerm {
    /// Recurrence compares the complete retained resolver input. Semantic
    /// equality alone erases observation authority and float landing metadata;
    /// those payloads must not collapse two different pending questions.
    pub(super) fn same_resolution_payload(&self, other: &Self) -> bool {
        if std::mem::discriminant(self) != std::mem::discriminant(other) || self != other {
            return false;
        }
        self.same_resolution_metadata(other)
    }

    // Semantic equality already walked the values. Descriptors need their
    // own full comparison because semantic equality deliberately omits them.
    fn same_resolution_metadata(&self, other: &Self) -> bool {
        if std::mem::discriminant(self) != std::mem::discriminant(other) {
            return false;
        }
        if let (
            Self::ObservedValue {
                observed_type,
                value,
                descriptor,
            },
            Self::ObservedValue {
                observed_type: other_type,
                value: other_value,
                descriptor: other_descriptor,
            },
        ) = (self, other)
        {
            return observed_type == other_type
                && value.same_resolution_metadata(other_value)
                && match (descriptor, other_descriptor) {
                    (None, None) => true,
                    (Some(left), Some(right)) => left.same_resolution_payload(right),
                    _ => false,
                };
        }
        let metadata_matches = match (self, other) {
            (
                Self::ScalarBinary { references, .. },
                Self::ScalarBinary {
                    references: other_references,
                    ..
                },
            ) => references == other_references,
            (
                Self::IndexedValue { element_type, .. },
                Self::IndexedValue {
                    element_type: other_type,
                    ..
                },
            ) => element_type == other_type,
            (Self::Float(value), Self::Float(other_value)) => {
                value.landing() == other_value.landing()
            }
            _ => true,
        };
        if !metadata_matches {
            return false;
        }
        let mut children = Vec::new();
        self.for_each_child(|child| children.push(child));
        let mut index = 0;
        let mut same = true;
        other.for_each_child(|child| {
            same &= children
                .get(index)
                .is_some_and(|value| value.same_resolution_metadata(child));
            index += 1;
        });
        same && index == children.len()
    }

    /// Visit every immediate child subterm of `self` once, in declaration
    /// order: a constructor's fields, an application's arguments, and the one
    /// or two operand terms the observation variants carry. A shell's observed
    /// value and descriptor are edges; a consumer that compares semantic
    /// identity strips the shell with `semantic_value` first, and a visited
    /// child may itself be a shell whose consumer looks through it.
    ///
    /// This is the single owner of the term's child-edge relation. The
    /// occurrence search, the application collectors, the law-shape checks,
    /// the containment probe and the uncertified-projection walk used to
    /// re-enumerate the same variant table in each copy, so every added
    /// variant or edge had to be mirrored in all of them; they now walk the
    /// term through this one.
    pub(in crate::proof_contracts::contract_entailment) fn for_each_child<'term>(
        &'term self,
        mut visit: impl FnMut(&'term StructuralTerm),
    ) {
        self.visit_children(&mut |child| {
            visit(child);
            false
        });
    }

    /// Whether any immediate child satisfies `predicate`, stopping at the
    /// first match.
    pub(in crate::proof_contracts::contract_entailment) fn any_child<'term>(
        &'term self,
        mut predicate: impl FnMut(&'term StructuralTerm) -> bool,
    ) -> bool {
        self.visit_children(&mut predicate)
    }

    /// The one child-edge table. Its variants are the ones every earlier walk
    /// already descended: a constructor's fields, an application's arguments,
    /// indexed values, measures, projections, embeddings, binary operators and
    /// a shell's observed value plus descriptor; leaves carry none. A caller
    /// that must ignore observation identity strips the shell with
    /// [`Self::semantic_value`] first, exactly as the identity relation does.
    fn visit_children<'term>(
        &'term self,
        visit: &mut impl FnMut(&'term StructuralTerm) -> bool,
    ) -> bool {
        match self {
            Self::ObservedValue {
                value, descriptor, ..
            } => {
                visit(value)
                    || descriptor
                        .as_ref()
                        .is_some_and(|descriptor| visit(descriptor))
            }
            self_ => match self_.semantic_value() {
                Self::Constructor { fields, .. } => fields.iter().any(|(_, value)| visit(value)),
                Self::Application { arguments, .. } | Self::CallProjection { arguments, .. } => {
                    arguments.iter().any(|argument| visit(argument))
                }
                Self::IndexedValue {
                    collection, index, ..
                } => visit(collection) || visit(index),
                Self::CollectionMeasure {
                    collection: subject,
                    ..
                }
                | Self::Projection { subject, .. }
                | Self::IntegerEmbedding { value: subject, .. }
                | Self::ScalarRepresentation { value: subject, .. } => visit(subject),
                Self::ScalarBinary { left, right, .. }
                | Self::MathematicalBinary { left, right, .. } => visit(left) || visit(right),
                _ => false,
            },
        }
    }

    /// Stable key of an already authenticated completed value observation.
    /// Reuse this owner's semantic formatting, also used by polynomial keys:
    /// it erases observation-only shells exactly as semantic equality does.
    /// Event handles and capture sequence remain part of the key, so another
    /// prefix can compare the same snapshot without renaming a later read.
    pub(in crate::proof_contracts::contract_entailment) fn observation_identity_key(
        &self,
    ) -> Option<String> {
        fn available(value: &StructuralTerm, depth: usize) -> bool {
            if depth >= 64 {
                return false;
            }
            // A shell costs one depth step, as the per-variant recursion did.
            let mut value = value;
            let mut depth = depth;
            while let StructuralTerm::ObservedValue { value: inner, .. } = value {
                depth += 1;
                if depth >= 64 {
                    return false;
                }
                value = inner;
            }
            !matches!(value, StructuralTerm::Opaque(_))
                && !value.any_child(|child| !available(child, depth + 1))
        }
        available(self, 0).then(|| format!("{:?}", self.semantic_value()))
    }
}

impl Eq for StructuralTerm {}

impl PartialEq for StructuralTerm {
    fn eq(&self, other: &Self) -> bool {
        let other = other.semantic_value();
        match self {
            Self::ObservedValue { value, .. } => value.as_ref().eq(other),
            Self::Variable(value) => {
                let Self::Variable(other_value) = other else {
                    return false;
                };
                value == other_value
            }
            Self::BoundValue(value) => {
                let Self::BoundValue(other_value) = other else {
                    return false;
                };
                value == other_value
            }
            Self::CapturedValue {
                expression,
                callable,
                occurrence,
                result_type,
                integer_range,
            } => {
                let Self::CapturedValue {
                    expression: other_expression,
                    callable: other_callable,
                    occurrence: other_occurrence,
                    result_type: other_result_type,
                    integer_range: other_range,
                } = other
                else {
                    return false;
                };
                (expression, callable, occurrence, result_type, integer_range)
                    == (
                        other_expression,
                        other_callable,
                        other_occurrence,
                        other_result_type,
                        other_range,
                    )
            }
            Self::BoundProjection { subject, path } => {
                let Self::BoundProjection {
                    subject: other_subject,
                    path: other_path,
                } = other
                else {
                    return false;
                };
                (subject, path) == (other_subject, other_path)
            }
            Self::Integer(value) => {
                let Self::Integer(other_value) = other else {
                    return false;
                };
                value == other_value
            }
            Self::ScalarRepresentation {
                source,
                target,
                value,
            } => {
                matches!(other, Self::ScalarRepresentation { source: other_source, target: other_target, value: other_value }
                    if source == other_source && target == other_target && value == other_value)
            }
            Self::IntegerEmbedding { reference, value } => {
                matches!(other, Self::IntegerEmbedding { reference: other_reference, value: other_value }
                    if reference == other_reference && value == other_value)
            }
            Self::MathematicalBinary {
                operator,
                left,
                right,
            } => {
                matches!(other, Self::MathematicalBinary { operator: other_operator, left: other_left, right: other_right }
                    if operator == other_operator && left == other_left && right == other_right)
            }
            Self::Float(value) => {
                let Self::Float(other_value) = other else {
                    return false;
                };
                value == other_value
            }
            Self::Constructor { data, case, fields } => {
                let Self::Constructor {
                    data: other_data,
                    case: other_case,
                    fields: other_fields,
                } = other
                else {
                    return false;
                };
                (data, case, fields) == (other_data, other_case, other_fields)
            }
            Self::Application {
                result_type,
                target,
                selections,
                machine,
                arguments,
            } => {
                let Self::Application {
                    result_type: other_result_type,
                    target: other_target,
                    selections: other_selections,
                    machine: other_machine,
                    arguments: other_arguments,
                } = other
                else {
                    return false;
                };
                (result_type, target, selections, machine, arguments)
                    == (
                        other_result_type,
                        other_target,
                        other_selections,
                        other_machine,
                        other_arguments,
                    )
            }
            Self::CallProjection {
                selections,
                target,
                machine,
                result_type,
                field,
                field_name,
                arguments,
            } => {
                let Self::CallProjection {
                    selections: other_selections,
                    target: other_target,
                    machine: other_machine,
                    result_type: other_result_type,
                    field: other_field,
                    field_name: other_field_name,
                    arguments: other_arguments,
                } = other
                else {
                    return false;
                };
                (
                    selections,
                    target,
                    machine,
                    result_type,
                    field,
                    field_name,
                    arguments,
                ) == (
                    other_selections,
                    other_target,
                    other_machine,
                    other_result_type,
                    other_field,
                    other_field_name,
                    other_arguments,
                )
            }
            Self::IndexedValue {
                element_identity,
                collection,
                index,
                ..
            } => {
                let Self::IndexedValue {
                    element_identity: other_identity,
                    collection: other_collection,
                    index: other_index,
                    ..
                } = other
                else {
                    return false;
                };
                (element_identity, collection, index)
                    == (other_identity, other_collection, other_index)
            }
            Self::Projection { subject, path } => {
                let Self::Projection {
                    subject: other_subject,
                    path: other_path,
                } = other
                else {
                    return false;
                };
                (subject, path) == (other_subject, other_path)
            }
            Self::CollectionMeasure {
                measure,
                collection,
            } => {
                let Self::CollectionMeasure {
                    measure: other_measure,
                    collection: other_collection,
                } = other
                else {
                    return false;
                };
                (measure, collection) == (other_measure, other_collection)
            }
            Self::ScalarBinary {
                operator,
                meaning,
                left,
                right,
                ..
            } => {
                let Self::ScalarBinary {
                    operator: other_operator,
                    meaning: other_meaning,
                    left: other_left,
                    right: other_right,
                    ..
                } = other
                else {
                    return false;
                };
                (operator, meaning, left, right)
                    == (other_operator, other_meaning, other_left, other_right)
            }
            Self::Opaque(value) => {
                let Self::Opaque(other_value) = other else {
                    return false;
                };
                value == other_value
            }
        }
    }
}

impl std::fmt::Debug for StructuralTerm {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ObservedValue { value, .. } => std::fmt::Debug::fmt(value, formatter),
            Self::Variable(value) => formatter.debug_tuple("Variable").field(value).finish(),
            Self::BoundValue(value) => formatter.debug_tuple("BoundValue").field(value).finish(),
            Self::CapturedValue {
                expression,
                callable,
                occurrence,
                result_type,
                integer_range,
            } => formatter
                .debug_struct("CapturedValue")
                .field("expression", expression)
                .field("callable", callable)
                .field("occurrence", occurrence)
                .field("result_type", result_type)
                .field("integer_range", integer_range)
                .finish(),
            Self::BoundProjection { subject, path } => formatter
                .debug_struct("BoundProjection")
                .field("subject", subject)
                .field("path", path)
                .finish(),
            Self::Integer(value) => formatter.debug_tuple("Integer").field(value).finish(),
            Self::ScalarRepresentation {
                source,
                target,
                value,
            } => formatter
                .debug_struct("ScalarRepresentation")
                .field("source", source)
                .field("target", target)
                .field("value", value)
                .finish(),
            Self::IntegerEmbedding { reference, value } => formatter
                .debug_struct("IntegerEmbedding")
                .field("reference", reference)
                .field("value", value)
                .finish(),
            Self::MathematicalBinary {
                operator,
                left,
                right,
            } => formatter
                .debug_struct("MathematicalBinary")
                .field("operator", operator)
                .field("left", left)
                .field("right", right)
                .finish(),
            Self::Float(value) => formatter.debug_tuple("Float").field(value).finish(),
            Self::Constructor { data, case, fields } => formatter
                .debug_struct("Constructor")
                .field("data", data)
                .field("case", case)
                .field("fields", fields)
                .finish(),
            Self::Application {
                result_type,
                target,
                selections,
                machine,
                arguments,
            } => formatter
                .debug_struct("Application")
                .field("result_type", result_type)
                .field("target", target)
                .field("selections", selections)
                .field("machine", machine)
                .field("arguments", arguments)
                .finish(),
            Self::CallProjection {
                selections,
                target,
                machine,
                result_type,
                field,
                field_name,
                arguments,
            } => formatter
                .debug_struct("CallProjection")
                .field("selections", selections)
                .field("target", target)
                .field("machine", machine)
                .field("result_type", result_type)
                .field("field", field)
                .field("field_name", field_name)
                .field("arguments", arguments)
                .finish(),
            Self::IndexedValue {
                element_identity,
                collection,
                index,
                ..
            } => formatter
                .debug_struct("IndexedValue")
                .field("element_identity", element_identity)
                .field("collection", collection)
                .field("index", index)
                .finish(),
            Self::Projection { subject, path } => formatter
                .debug_struct("Projection")
                .field("subject", subject)
                .field("path", path)
                .finish(),
            Self::CollectionMeasure {
                measure,
                collection,
            } => formatter
                .debug_struct("CollectionMeasure")
                .field("measure", measure)
                .field("collection", collection)
                .finish(),
            Self::ScalarBinary {
                operator,
                meaning,
                left,
                right,
                ..
            } => formatter
                .debug_struct("ScalarBinary")
                .field("operator", operator)
                .field("meaning", meaning)
                .field("left", left)
                .field("right", right)
                .finish(),
            Self::Opaque(value) => formatter.debug_tuple("Opaque").field(value).finish(),
        }
    }
}
