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

impl StructuralTerm {
    /// Stable key of an already authenticated completed scalar observation.
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
            match value {
                StructuralTerm::Opaque(_) => false,
                StructuralTerm::ObservedValue { value, .. } => available(value, depth + 1),
                StructuralTerm::Projection { subject, .. } => available(subject, depth + 1),
                StructuralTerm::Constructor { fields, .. } => {
                    fields.iter().all(|(_, value)| available(value, depth + 1))
                }
                StructuralTerm::Application { arguments, .. }
                | StructuralTerm::CallProjection { arguments, .. } => {
                    arguments.iter().all(|value| available(value, depth + 1))
                }
                StructuralTerm::IndexedValue {
                    collection, index, ..
                } => available(collection, depth + 1) && available(index, depth + 1),
                StructuralTerm::ScalarBinary { left, right, .. } => {
                    available(left, depth + 1) && available(right, depth + 1)
                }
                _ => true,
            }
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
            } => {
                let Self::CapturedValue {
                    expression: other_expression,
                    callable: other_callable,
                    occurrence: other_occurrence,
                    result_type: other_result_type,
                } = other
                else {
                    return false;
                };
                (expression, callable, occurrence, result_type)
                    == (
                        other_expression,
                        other_callable,
                        other_occurrence,
                        other_result_type,
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
            } => formatter
                .debug_struct("CapturedValue")
                .field("expression", expression)
                .field("callable", callable)
                .field("occurrence", occurrence)
                .field("result_type", result_type)
                .finish(),
            Self::BoundProjection { subject, path } => formatter
                .debug_struct("BoundProjection")
                .field("subject", subject)
                .field("path", path)
                .finish(),
            Self::Integer(value) => formatter.debug_tuple("Integer").field(value).finish(),
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
