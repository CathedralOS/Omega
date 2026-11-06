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

/// Borrow immediate term edges without allocating a list for each recursive
/// query. Variable arity borrows its existing storage; other
/// variants retain at most two references in their declared order.
enum StructuralChildren<'term> {
    Terms(std::slice::Iter<'term, StructuralTerm>),
    Fields(std::slice::Iter<'term, (String, StructuralTerm)>),
    Fixed {
        first: Option<&'term StructuralTerm>,
        second: Option<&'term StructuralTerm>,
    },
}

impl<'term> Iterator for StructuralChildren<'term> {
    type Item = &'term StructuralTerm;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Terms(terms) => terms.next(),
            Self::Fields(fields) => fields.next().map(|(_, value)| value),
            Self::Fixed { first, second } => first.take().or_else(|| second.take()),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let length = match self {
            Self::Terms(terms) => terms.len(),
            Self::Fields(fields) => fields.len(),
            Self::Fixed { first, second } => {
                usize::from(first.is_some()) + usize::from(second.is_some())
            }
        };
        (length, Some(length))
    }
}

impl ExactSizeIterator for StructuralChildren<'_> {}

pub(in crate::proof_contracts::contract_entailment) fn same_static_selection_payload(
    left: &[typed_trees::expression::StaticMachineArgument],
    right: &[typed_trees::expression::StaticMachineArgument],
) -> bool {
    left == right && same_static_selection_metadata(left, right)
}

// Ordinary static-argument equality compares a literal's numeric value. A
// resolver question additionally retains the carrier and arithmetic policy
// that this particular literal brought to its selected application. Applied
// owners can contain more static arguments, so preserve that metadata at
// every level without changing the language's semantic equality relation.
fn same_static_selection_metadata(
    left: &[typed_trees::expression::StaticMachineArgument],
    right: &[typed_trees::expression::StaticMachineArgument],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            let same_literal = match (&left.const_literal, &right.const_literal) {
                (None, None) => true,
                (Some(left), Some(right)) => left.landing() == right.landing(),
                _ => false,
            };
            same_literal
                && match (&left.application, &right.application) {
                    (None, None) => true,
                    (Some(left), Some(right)) => {
                        same_static_selection_metadata(&left.arguments, &right.arguments)
                    }
                    _ => false,
                }
        })
}

impl StructuralTerm {
    /// Recurrence compares the complete retained resolver input. Semantic
    /// equality alone erases observation authority and float landing metadata;
    /// those payloads must not collapse two different pending questions.
    pub(in crate::proof_contracts::contract_entailment) fn same_resolution_payload(
        &self,
        other: &Self,
    ) -> bool {
        StructuralIdentity::ResolutionInput.matches(self, other)
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
        self.children().for_each(&mut visit);
    }

    /// Whether any immediate child satisfies `predicate`, stopping at the
    /// first match.
    pub(in crate::proof_contracts::contract_entailment) fn any_child<'term>(
        &'term self,
        mut predicate: impl FnMut(&'term StructuralTerm) -> bool,
    ) -> bool {
        self.children().any(&mut predicate)
    }

    /// The one child-edge table. Its variants are the ones every earlier walk
    /// already descended: a constructor's fields, an application's arguments,
    /// indexed values, measures, projections, embeddings, binary operators and
    /// a shell's observed value plus descriptor; leaves carry none. A caller
    /// that must ignore observation identity strips the shell with
    /// [`Self::semantic_value`] first, exactly as the identity relation does.
    fn children(&self) -> StructuralChildren<'_> {
        match self {
            Self::ObservedValue {
                value, descriptor, ..
            } => StructuralChildren::Fixed {
                first: Some(value.as_ref()),
                second: descriptor.as_deref(),
            },
            self_ => match self_.semantic_value() {
                Self::RepeatedArray { element, .. } => StructuralChildren::Fixed {
                    first: element.as_deref(),
                    second: None,
                },
                Self::Constructor { fields, .. } => StructuralChildren::Fields(fields.iter()),
                Self::Application { arguments, .. } | Self::CallProjection { arguments, .. } => {
                    StructuralChildren::Terms(arguments.iter())
                }
                Self::IndexedValue {
                    collection, index, ..
                } => StructuralChildren::Fixed {
                    first: Some(collection.as_ref()),
                    second: Some(index.as_ref()),
                },
                Self::CollectionMeasure {
                    collection: subject,
                    ..
                }
                | Self::Projection { subject, .. }
                | Self::IntegerEmbedding { value: subject, .. }
                | Self::ScalarRepresentation { value: subject, .. } => StructuralChildren::Fixed {
                    first: Some(subject.as_ref()),
                    second: None,
                },
                Self::ScalarBinary { left, right, .. }
                | Self::MathematicalBinary { left, right, .. } => StructuralChildren::Fixed {
                    first: Some(left.as_ref()),
                    second: Some(right.as_ref()),
                },
                _ => StructuralChildren::Fixed {
                    first: None,
                    second: None,
                },
            },
        }
    }

    /// Whether the value's semantic identity is recorded without an opaque spelling.
    /// This checks representation only; callers still establish observation
    /// authority and the meaning of each selected operation.
    pub(in crate::proof_contracts::contract_entailment) fn has_observation_identity(&self) -> bool {
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
        available(self, 0)
    }

    /// Stable key of an already authenticated completed value observation.
    /// Reuse this owner's semantic formatting, also used by polynomial keys:
    /// it erases observation-only shells exactly as semantic equality does.
    /// Event handles and capture sequence remain part of the key, so another
    /// prefix can compare the same snapshot without renaming a later read.
    pub(in crate::proof_contracts::contract_entailment) fn observation_identity_key(
        &self,
    ) -> Option<String> {
        self.has_observation_identity()
            .then(|| format!("{:?}", self.semantic_value()))
    }
}

impl Eq for StructuralTerm {}

/// Value equality omits observation authority; resolver equality retains it.
/// Both relations share one term walk. Resolver comparison checks each value
/// together with its retained metadata; ordinary equality looks through wrappers.
#[derive(Clone, Copy)]
enum StructuralIdentity {
    SemanticValue,
    ResolutionInput,
}

impl StructuralIdentity {
    fn matches(self, left: &StructuralTerm, right: &StructuralTerm) -> bool {
        let (left, other) = match self {
            Self::SemanticValue => (left.semantic_value(), right.semantic_value()),
            Self::ResolutionInput => (left, right),
        };
        let retain_metadata = matches!(self, Self::ResolutionInput);
        match left {
            StructuralTerm::ObservedValue {
                observed_type,
                value,
                descriptor,
            } => {
                let StructuralTerm::ObservedValue {
                    observed_type: other_type,
                    value: other_value,
                    descriptor: other_descriptor,
                } = other
                else {
                    return false;
                };
                observed_type == other_type
                    && self.matches(value, other_value)
                    && self.optional_terms_match(descriptor, other_descriptor)
            }
            StructuralTerm::RepeatedArray { length, element } => matches!(
                other,
                StructuralTerm::RepeatedArray { length: other_length, element: other_element }
                    if length == other_length && self.optional_terms_match(element, other_element)
            ),
            StructuralTerm::Variable(value) => {
                let StructuralTerm::Variable(other_value) = other else {
                    return false;
                };
                value == other_value
            }
            StructuralTerm::BoundValue(value) => {
                let StructuralTerm::BoundValue(other_value) = other else {
                    return false;
                };
                value == other_value
            }
            StructuralTerm::CapturedValue {
                statement,
                expression,
                callable,
                occurrence,
                result_type,
                integer_range,
            } => {
                let StructuralTerm::CapturedValue {
                    statement: other_statement,
                    expression: other_expression,
                    callable: other_callable,
                    occurrence: other_occurrence,
                    result_type: other_result_type,
                    integer_range: other_range,
                } = other
                else {
                    return false;
                };
                (
                    expression,
                    statement,
                    callable,
                    occurrence,
                    result_type,
                    integer_range,
                ) == (
                    other_expression,
                    other_statement,
                    other_callable,
                    other_occurrence,
                    other_result_type,
                    other_range,
                )
            }
            StructuralTerm::BoundProjection { subject, path } => {
                let StructuralTerm::BoundProjection {
                    subject: other_subject,
                    path: other_path,
                } = other
                else {
                    return false;
                };
                (subject, path) == (other_subject, other_path)
            }
            StructuralTerm::Integer(value) => {
                let StructuralTerm::Integer(other_value) = other else {
                    return false;
                };
                value == other_value
            }
            StructuralTerm::ScalarRepresentation {
                source,
                target,
                value,
            } => {
                matches!(other, StructuralTerm::ScalarRepresentation { source: other_source, target: other_target, value: other_value }
                    if source == other_source && target == other_target && self.matches(value, other_value))
            }
            StructuralTerm::IntegerEmbedding { reference, value } => {
                matches!(other, StructuralTerm::IntegerEmbedding { reference: other_reference, value: other_value }
                    if reference == other_reference && self.matches(value, other_value))
            }
            StructuralTerm::MathematicalBinary {
                operator,
                left,
                right,
            } => {
                matches!(other, StructuralTerm::MathematicalBinary { operator: other_operator, left: other_left, right: other_right }
                    if operator == other_operator && self.matches(left, other_left) && self.matches(right, other_right))
            }
            StructuralTerm::Float(value) => {
                let StructuralTerm::Float(other_value) = other else {
                    return false;
                };
                value == other_value
                    && (!retain_metadata || value.landing() == other_value.landing())
            }
            StructuralTerm::Constructor {
                identity, fields, ..
            } => {
                let StructuralTerm::Constructor {
                    identity: other_identity,
                    fields: other_fields,
                    ..
                } = other
                else {
                    return false;
                };
                let same_identity = match self {
                    Self::SemanticValue => identity == other_identity,
                    Self::ResolutionInput => identity.same_resolution_payload(*other_identity),
                };
                same_identity
                    && fields.len() == other_fields.len()
                    && fields.iter().zip(other_fields).all(
                        |((name, value), (other_name, other_value))| {
                            name == other_name && self.matches(value, other_value)
                        },
                    )
            }
            StructuralTerm::Application {
                result_type,
                target,
                selections,
                machine,
                arguments,
            } => {
                let StructuralTerm::Application {
                    result_type: other_result_type,
                    target: other_target,
                    selections: other_selections,
                    machine: other_machine,
                    arguments: other_arguments,
                } = other
                else {
                    return false;
                };
                (result_type, target) == (other_result_type, other_target)
                    && self.selections_match(selections, other_selections)
                    && machine == other_machine
                    && self.term_lists_match(arguments, other_arguments)
            }
            StructuralTerm::CallProjection {
                selections,
                target,
                machine,
                result_type,
                field,
                field_name,
                arguments,
            } => {
                let StructuralTerm::CallProjection {
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
                self.selections_match(selections, other_selections)
                    && (target, machine, result_type, field, field_name)
                        == (
                            other_target,
                            other_machine,
                            other_result_type,
                            other_field,
                            other_field_name,
                        )
                    && self.term_lists_match(arguments, other_arguments)
            }
            StructuralTerm::IndexedValue {
                element_identity,
                element_type,
                collection,
                index,
            } => {
                let StructuralTerm::IndexedValue {
                    element_identity: other_identity,
                    element_type: other_type,
                    collection: other_collection,
                    index: other_index,
                } = other
                else {
                    return false;
                };
                element_identity == other_identity
                    && (!retain_metadata || element_type == other_type)
                    && self.matches(collection, other_collection)
                    && self.matches(index, other_index)
            }
            StructuralTerm::Projection { subject, path } => {
                let StructuralTerm::Projection {
                    subject: other_subject,
                    path: other_path,
                } = other
                else {
                    return false;
                };
                self.matches(subject, other_subject) && path == other_path
            }
            StructuralTerm::CollectionMeasure {
                measure,
                collection,
            } => {
                let StructuralTerm::CollectionMeasure {
                    measure: other_measure,
                    collection: other_collection,
                } = other
                else {
                    return false;
                };
                measure == other_measure && self.matches(collection, other_collection)
            }
            StructuralTerm::ScalarBinary {
                operator,
                meaning,
                left,
                right,
                references,
            } => {
                let StructuralTerm::ScalarBinary {
                    operator: other_operator,
                    meaning: other_meaning,
                    left: other_left,
                    right: other_right,
                    references: other_references,
                } = other
                else {
                    return false;
                };
                (operator, meaning) == (other_operator, other_meaning)
                    && (!retain_metadata || references == other_references)
                    && self.matches(left, other_left)
                    && self.matches(right, other_right)
            }
            StructuralTerm::Opaque(value) => {
                let StructuralTerm::Opaque(other_value) = other else {
                    return false;
                };
                value == other_value
            }
        }
    }

    fn optional_terms_match(
        self,
        left: &Option<Box<StructuralTerm>>,
        right: &Option<Box<StructuralTerm>>,
    ) -> bool {
        match (left.as_deref(), right.as_deref()) {
            (None, None) => true,
            (Some(left), Some(right)) => self.matches(left, right),
            _ => false,
        }
    }

    fn term_lists_match(self, left: &[StructuralTerm], right: &[StructuralTerm]) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(left, right)| self.matches(left, right))
    }

    fn selections_match(
        self,
        left: &[typed_trees::expression::StaticMachineArgument],
        right: &[typed_trees::expression::StaticMachineArgument],
    ) -> bool {
        match self {
            Self::SemanticValue => left == right,
            Self::ResolutionInput => same_static_selection_payload(left, right),
        }
    }
}

impl PartialEq for StructuralTerm {
    fn eq(&self, other: &Self) -> bool {
        StructuralIdentity::SemanticValue.matches(self, other)
    }
}

impl std::fmt::Debug for StructuralTerm {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ObservedValue { value, .. } => std::fmt::Debug::fmt(value, formatter),
            Self::RepeatedArray { length, element } => formatter
                .debug_struct("RepeatedArray")
                .field("length", length)
                .field("element", element)
                .finish(),
            Self::Variable(value) => formatter.debug_tuple("Variable").field(value).finish(),
            Self::BoundValue(value) => formatter.debug_tuple("BoundValue").field(value).finish(),
            Self::CapturedValue {
                statement,
                expression,
                callable,
                occurrence,
                result_type,
                integer_range,
            } => formatter
                .debug_struct("CapturedValue")
                .field("expression", expression)
                .field("statement", statement)
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
            Self::Constructor {
                identity, fields, ..
            } => formatter
                .debug_struct("Constructor")
                .field("identity", identity)
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

#[cfg(test)]
mod resolution_payload_tests {
    use super::*;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

    #[test]
    fn argument_children_keep_order_length_and_callback_short_circuit() {
        fn name(term: &StructuralTerm) -> &str {
            let StructuralTerm::Variable(name) = term else {
                panic!("the test application's children are variables");
            };
            name
        }
        let term = StructuralTerm::Application {
            result_type: TypeReferenceHandle::invalid(),
            target: symbols::SymbolHandle::invalid(),
            selections: Vec::new(),
            machine: "ordered".to_owned(),
            arguments: ["first", "second", "third"]
                .map(|name| StructuralTerm::Variable(name.to_owned()))
                .into(),
        };
        let mut children = term.children();
        assert_eq!(children.len(), 3);
        assert_eq!(name(children.next().unwrap()), "first");
        assert_eq!(children.len(), 2);
        assert_eq!(name(children.next().unwrap()), "second");
        assert_eq!(children.len(), 1);
        assert_eq!(name(children.next().unwrap()), "third");
        assert_eq!(children.len(), 0);
        assert!(children.next().is_none());
        let mut all = Vec::new();
        term.for_each_child(|child| all.push(name(child)));
        assert_eq!(all, ["first", "second", "third"]);
        let mut visited = Vec::new();
        assert!(term.any_child(|child| {
            visited.push(name(child));
            name(child) == "second"
        }));
        assert_eq!(visited, ["first", "second"]);
    }

    #[test]
    fn payload_comparison_keeps_authority_on_the_last_of_three_fields() {
        let identities = typed_trees::type_identity::TypeIdentityTable::default();
        let carrier = identities.intern("Carrier");
        let left = StructuralTerm::Constructor {
            identity: super::super::ConstructorIdentity::Nominal {
                carrier,
                reference: TypeReferenceHandle::from_arena_index(1),
                declaration: symbols::SymbolHandle::from_arena_index(1),
                case: None,
                member: None,
            },
            data: "Carrier".to_owned(),
            case: "Only".to_owned(),
            fields: ["first", "second", "third"]
                .map(|name| {
                    (
                        name.to_owned(),
                        StructuralTerm::ObservedValue {
                            observed_type: TypeReferenceHandle::from_arena_index(1),
                            value: Box::new(StructuralTerm::Integer(
                                numerics::bignum::BigInt::zero(),
                            )),
                            descriptor: None,
                        },
                    )
                })
                .into(),
        };
        let mut right = left.clone();
        let StructuralTerm::Constructor { fields, .. } = &mut right else {
            unreachable!();
        };
        let StructuralTerm::ObservedValue { observed_type, .. } = &mut fields[2].1 else {
            unreachable!();
        };
        *observed_type = TypeReferenceHandle::from_arena_index(2);
        assert_eq!(left, right, "semantic equality omits observation authority");
        assert!(left.same_resolution_payload(&left.clone()));
        assert!(!left.same_resolution_payload(&right));
        assert!(!right.same_resolution_payload(&left));
    }

    #[test]
    fn repeated_array_traversal_and_substitution_are_independent_of_extent() {
        let array = StructuralTerm::RepeatedArray {
            length: 1_000_000_000,
            element: Some(Box::new(StructuralTerm::Integer(
                numerics::bignum::BigInt::zero(),
            ))),
        };
        let mut visits = 0;
        array.for_each_child(|_| visits += 1);
        assert_eq!(visits, 1);
        let mut substitutions = 0;
        let changed = array.clone().map_children(|_| {
            substitutions += 1;
            StructuralTerm::Integer(numerics::bignum::BigInt::from_i64(1))
        });
        assert_eq!(substitutions, 1);
        assert_ne!(array, changed);
        assert!(array.same_resolution_payload(&array.clone()));
        assert!(!array.same_resolution_payload(&changed));
        let mut different_length = array.clone();
        if let StructuralTerm::RepeatedArray { length, .. } = &mut different_length {
            *length = 2;
        }
        assert_ne!(array, different_length);
        let empty = StructuralTerm::RepeatedArray {
            length: 0,
            element: None,
        };
        assert!(!empty.any_child(|_| true));
    }

    #[test]
    fn payload_comparison_keeps_hidden_descriptors_and_nested_authority() {
        let integer = |value| StructuralTerm::Integer(numerics::bignum::BigInt::from_i64(value));
        let observed = |reference, descriptor| StructuralTerm::ObservedValue {
            observed_type: TypeReferenceHandle::from_arena_index(reference),
            value: Box::new(integer(7)),
            descriptor,
        };
        let identities = typed_trees::type_identity::TypeIdentityTable::default();
        let carrier = identities.intern("Carrier");
        let nested = |value| StructuralTerm::Constructor {
            identity: super::super::ConstructorIdentity::Nominal {
                carrier,
                reference: TypeReferenceHandle::from_arena_index(1),
                declaration: symbols::SymbolHandle::from_arena_index(1),
                case: None,
                member: None,
            },
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
