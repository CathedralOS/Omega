//! Exact proof-fact identity after value and static substitution.
//!
//! This is the single structural owner shared by theorem-schema verification
//! and quotient precondition correspondence. Rendered source labels alone are
//! never identity: the parallel symbol trace retains resolved call/member/type
//! identities and the complete static application.

use super::{RepresentativeStaticApplication, RepresentativeStaticBinding};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProofValueSubstitution {
    pub(super) symbol: SymbolHandle,
    rendered: String,
    trace: String,
}

impl ProofValueSubstitution {
    pub(super) fn symbolic(symbol: SymbolHandle, identity: String) -> Self {
        Self {
            symbol,
            rendered: identity.clone(),
            trace: identity,
        }
    }

    pub(super) fn boolean(symbol: SymbolHandle, value: bool) -> Self {
        Self {
            symbol,
            rendered: value.to_string(),
            trace: boolean_trace(value),
        }
    }

    pub(super) fn integer(
        symbol: SymbolHandle,
        spelling: &str,
        landing: numerics::literals::IntegerLanding,
    ) -> Self {
        Self {
            symbol,
            rendered: spelling.to_owned(),
            trace: integer_trace(spelling, landing),
        }
    }

    pub(super) fn float(
        symbol: SymbolHandle,
        spelling: &str,
        landing: numerics::literals::FloatFormat,
    ) -> Self {
        Self {
            symbol,
            rendered: spelling.to_owned(),
            trace: float_trace(spelling, Some(landing)),
        }
    }

    pub(super) fn byte_string(symbol: SymbolHandle, bytes: &[u8]) -> Self {
        Self {
            symbol,
            rendered: format!("{bytes:?}"),
            trace: byte_string_trace(bytes),
        }
    }

    pub(super) fn fixed_byte_array(symbol: SymbolHandle, bytes: &[u8]) -> Self {
        Self {
            symbol,
            rendered: format!("{bytes:?}"),
            trace: array_trace(bytes.iter().map(|byte| format!("integer:{byte}:unlanded"))),
        }
    }

    pub(super) fn boolean_array(symbol: SymbolHandle, values: &[bool]) -> Self {
        Self {
            symbol,
            rendered: format!("{values:?}"),
            trace: array_trace(values.iter().map(|value| boolean_trace(*value))),
        }
    }

    pub(super) fn nested_fixed_byte_array(
        symbol: SymbolHandle,
        rows: &[std::sync::Arc<[u8]>],
    ) -> Self {
        Self {
            symbol,
            rendered: format!(
                "[{}]",
                rows.iter()
                    .map(|row| format!("{:?}", row.as_ref()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            trace: array_trace(
                rows.iter().map(|row| {
                    array_trace(row.iter().map(|byte| format!("integer:{byte}:unlanded")))
                }),
            ),
        }
    }

    pub(super) fn nested_boolean_array(
        symbol: SymbolHandle,
        rows: &[std::sync::Arc<[bool]>],
    ) -> Self {
        Self {
            symbol,
            rendered: format!(
                "[{}]",
                rows.iter()
                    .map(|row| format!("{:?}", row.as_ref()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            trace: array_trace(
                rows.iter()
                    .map(|row| array_trace(row.iter().map(|value| boolean_trace(*value)))),
            ),
        }
    }

    pub(super) fn boolean_tensor3(
        symbol: SymbolHandle,
        planes: &[std::sync::Arc<[std::sync::Arc<[bool]>]>],
    ) -> Self {
        Self {
            symbol,
            rendered: format!(
                "[{}]",
                planes
                    .iter()
                    .map(|plane| format!(
                        "[{}]",
                        plane
                            .iter()
                            .map(|row| format!("{:?}", row.as_ref()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            trace: array_trace(planes.iter().map(|plane| {
                array_trace(
                    plane
                        .iter()
                        .map(|row| array_trace(row.iter().map(|value| boolean_trace(*value)))),
                )
            })),
        }
    }

    pub(super) fn recursive_primitive_array(
        symbol: SymbolHandle,
        elements: &[super::runtime_correspondence::ClosedRecursiveArrayElement],
    ) -> Self {
        Self {
            symbol,
            rendered: render_recursive_array(elements),
            trace: array_trace(elements.iter().map(recursive_array_element_trace)),
        }
    }

    pub(super) fn integer_array(
        symbol: SymbolHandle,
        elements: impl IntoIterator<Item = (String, numerics::literals::IntegerLanding)>,
    ) -> Self {
        let elements = elements.into_iter().collect::<Vec<_>>();
        Self {
            symbol,
            rendered: format!(
                "[{}]",
                elements
                    .iter()
                    .map(|(spelling, _)| spelling.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            trace: array_trace(
                elements
                    .iter()
                    .map(|(spelling, landing)| integer_trace(spelling, *landing)),
            ),
        }
    }

    pub(super) fn nested_integer_array(
        symbol: SymbolHandle,
        rows: &[std::sync::Arc<[super::runtime_correspondence::ClosedIntegerArrayElement]>],
    ) -> Self {
        Self {
            symbol,
            rendered: format!(
                "[{}]",
                rows.iter()
                    .map(|row| format!(
                        "[{}]",
                        row.iter()
                            .map(|element| element.spelling.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            trace: array_trace(rows.iter().map(|row| {
                array_trace(
                    row.iter()
                        .map(|element| integer_trace(&element.spelling, element.landing)),
                )
            })),
        }
    }

    pub(super) fn float_array(
        symbol: SymbolHandle,
        elements: impl IntoIterator<Item = (String, numerics::literals::FloatFormat)>,
    ) -> Self {
        let elements = elements.into_iter().collect::<Vec<_>>();
        Self {
            symbol,
            rendered: format!(
                "[{}]",
                elements
                    .iter()
                    .map(|(spelling, _)| spelling.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            trace: array_trace(
                elements
                    .iter()
                    .map(|(spelling, landing)| float_trace(spelling, Some(*landing))),
            ),
        }
    }

    pub(super) fn nested_float_array(
        symbol: SymbolHandle,
        rows: &[std::sync::Arc<[super::runtime_correspondence::ClosedFloatArrayElement]>],
    ) -> Self {
        Self {
            symbol,
            rendered: format!(
                "[{}]",
                rows.iter()
                    .map(|row| format!(
                        "[{}]",
                        row.iter()
                            .map(|element| element.spelling.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            trace: array_trace(rows.iter().map(|row| {
                array_trace(
                    row.iter()
                        .map(|element| float_trace(&element.spelling, Some(element.landing))),
                )
            })),
        }
    }

    pub(super) fn rebound(&self, symbol: SymbolHandle) -> Self {
        Self {
            symbol,
            rendered: self.rendered.clone(),
            trace: self.trace.clone(),
        }
    }
}

fn render_recursive_array(
    elements: &[super::runtime_correspondence::ClosedRecursiveArrayElement],
) -> String {
    format!(
        "[{}]",
        elements
            .iter()
            .map(render_recursive_array_element)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn render_recursive_array_element(
    element: &super::runtime_correspondence::ClosedRecursiveArrayElement,
) -> String {
    use super::runtime_correspondence::ClosedRecursiveArrayElement;
    match element {
        ClosedRecursiveArrayElement::Boolean(value) => value.to_string(),
        ClosedRecursiveArrayElement::Byte(value) => value.to_string(),
        ClosedRecursiveArrayElement::Integer(value) => value.spelling.clone(),
        ClosedRecursiveArrayElement::Float(value) => value.spelling.clone(),
        ClosedRecursiveArrayElement::Array(elements) => render_recursive_array(elements),
    }
}

fn recursive_array_element_trace(
    element: &super::runtime_correspondence::ClosedRecursiveArrayElement,
) -> String {
    use super::runtime_correspondence::ClosedRecursiveArrayElement;
    match element {
        ClosedRecursiveArrayElement::Boolean(value) => boolean_trace(*value),
        ClosedRecursiveArrayElement::Byte(value) => format!("integer:{value}:unlanded"),
        ClosedRecursiveArrayElement::Integer(value) => {
            integer_trace(&value.spelling, value.landing)
        }
        ClosedRecursiveArrayElement::Float(value) => {
            float_trace(&value.spelling, Some(value.landing))
        }
        ClosedRecursiveArrayElement::Array(elements) => {
            array_trace(elements.iter().map(recursive_array_element_trace))
        }
    }
}

pub(super) struct ProofFactIdentityContext<'a> {
    pub(super) values: &'a [ProofValueSubstitution],
    pub(super) static_bindings: &'a [RepresentativeStaticBinding],
}

pub(super) fn proof_facts_match(
    program: &TypedTrees,
    left: &ProofFact,
    right: &ProofFact,
    left_context: ProofFactIdentityContext<'_>,
    right_context: ProofFactIdentityContext<'_>,
) -> bool {
    let expression_matches = |left, right| {
        proof_expression_identity(program, left, left_context.values)
            == proof_expression_identity(program, right, right_context.values)
    };
    match (left, right) {
        (ProofFact::Expression(left), ProofFact::Expression(right)) => {
            expression_matches(*left, *right)
        }
        (ProofFact::Membership(left), ProofFact::Membership(right)) => {
            left.domain_symbol.is_valid()
                && left.domain_symbol == right.domain_symbol
                && expression_matches(left.value, right.value)
        }
        (ProofFact::Proposition(left), ProofFact::Proposition(right)) => {
            left.proposition.is_valid()
                && left.proposition == right.proposition
                && left.binder_arguments.len() == right.binder_arguments.len()
                && left
                    .binder_arguments
                    .iter()
                    .zip(&right.binder_arguments)
                    .all(|(left, right)| {
                        left.kind == right.kind
                            && binder_argument_identity(left, left_context.static_bindings)
                                == binder_argument_identity(right, right_context.static_bindings)
                    })
                && {
                    let left_arguments =
                        program.expression_table.expression_handles(left.arguments);
                    let right_arguments =
                        program.expression_table.expression_handles(right.arguments);
                    left_arguments.len() == right_arguments.len()
                        && left_arguments
                            .iter()
                            .zip(right_arguments)
                            .all(|(left, right)| expression_matches(*left, *right))
                }
        }
        _ => false,
    }
}

fn proof_expression_identity(
    program: &TypedTrees,
    expression: ExpressionHandle,
    values: &[ProofValueSubstitution],
) -> (String, String) {
    let rendered_values = values
        .iter()
        .map(|value| (value.symbol, value.rendered.clone()))
        .collect::<Vec<_>>();
    (
        program.render_proof_expression_with_symbols(expression, &rendered_values),
        expression_symbol_trace(program, expression, values),
    )
}

fn expression_symbol_trace(
    program: &TypedTrees,
    expression: ExpressionHandle,
    values: &[ProofValueSubstitution],
) -> String {
    let trace = |expression| expression_symbol_trace(program, expression, values);
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if members.len() == 1
                && let Some(value) = values
                    .iter()
                    .find(|value| value.symbol == path.symbol || value.symbol == path.head_symbol)
            {
                return value.trace.clone();
            }
            format!(
                "name:{:?}:{:?}:[{}]",
                path.head_symbol,
                path.symbol,
                members
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::")
            )
        }
        ExpressionNode::Call(call) => format!(
            "call:{:?}:{}:{:?}:{:?}:{:?}:{:?}:[{}]:({})",
            call.target_symbol,
            call.machine_arguments
                .iter()
                .map(static_argument_identity)
                .collect::<Vec<_>>()
                .join(","),
            call.quotient_operation,
            call.private_layout_operation,
            call.evidence_arguments,
            call.operational_acknowledgement,
            if call.receiver.is_valid() {
                trace(call.receiver)
            } else {
                Default::default()
            },
            program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| trace(*argument))
                .collect::<Vec<_>>()
                .join(","),
        ),
        ExpressionNode::ArrayLiteral(values_span) => array_trace(
            program
                .expression_table
                .expression_handles(*values_span)
                .iter()
                .map(|value| trace(*value)),
        ),
        ExpressionNode::Atomic(value) => trace(value.value),
        ExpressionNode::Binary(value) => format!("{}|{}", trace(value.left), trace(value.right)),
        ExpressionNode::Cast(value) => format!(
            "{}:{:?}:{}",
            trace(value.value),
            value.semantic_domain_symbol,
            program.normalized_type_identity(value.target_type)
        ),
        ExpressionNode::Indexed(value) => {
            format!("{}|{}", trace(value.collection), trace(value.index))
        }
        ExpressionNode::Member(value) => {
            format!("{}:{:?}", trace(value.receiver), value.member_symbol)
        }
        ExpressionNode::Borrow(value) => trace(value.target),
        ExpressionNode::Range(value) => format!("{}|{}", trace(value.start), trace(value.end)),
        ExpressionNode::StructLiteral(value) => format!(
            "struct:{:?}:{:?}:[{}]",
            value.type_symbol,
            value.case_symbol,
            program
                .expression_table
                .struct_fields(value.fields)
                .iter()
                .map(|field| format!("{:?}={}", field.field_symbol, trace(field.value)))
                .collect::<Vec<_>>()
                .join(",")
        ),
        ExpressionNode::Unary(value) => trace(value.operand),
        ExpressionNode::ZeroValue(value) => {
            format!("zero:{}", program.normalized_type_identity(*value))
        }
        ExpressionNode::Boolean(value) => boolean_trace(*value),
        ExpressionNode::Integer(value) => value.landing().map_or_else(
            || format!("integer:{}:unlanded", value.text()),
            |landing| integer_trace(value.text(), landing),
        ),
        ExpressionNode::Float(value) => float_trace(value.text(), value.landing()),
        ExpressionNode::String(value) => byte_string_trace(value),
    }
}

fn array_trace(elements: impl IntoIterator<Item = String>) -> String {
    format!(
        "array:[{}]",
        elements.into_iter().collect::<Vec<_>>().join(",")
    )
}

fn boolean_trace(value: bool) -> String {
    format!("boolean:{value}")
}

fn integer_trace(spelling: &str, landing: numerics::literals::IntegerLanding) -> String {
    format!(
        "integer:{spelling}:{}:{}",
        landing.landed_type.name(),
        landing.domain.name(),
    )
}

fn float_trace(spelling: &str, landing: Option<numerics::literals::FloatFormat>) -> String {
    landing.map_or_else(
        || format!("float:{spelling}:unlanded"),
        |landing| format!("float:{spelling}:{}", landing.name()),
    )
}

fn byte_string_trace(bytes: &[u8]) -> String {
    format!("byte-string:{bytes:?}")
}

pub(super) fn static_type_identities(
    application: &RepresentativeStaticApplication,
) -> Vec<(SymbolHandle, String)> {
    application
        .bindings
        .iter()
        .map(|binding| {
            (
                binding.parameter,
                static_argument_identity(&binding.argument),
            )
        })
        .collect()
}

fn binder_argument_identity(
    argument: &typed_trees::proposition::PropositionBinderArgument,
    substitutions: &[RepresentativeStaticBinding],
) -> String {
    substitutions
        .iter()
        .find(|binding| binding.parameter == argument.symbol)
        .map(|binding| static_argument_identity(&binding.argument))
        .unwrap_or_else(|| {
            argument.const_literal.as_ref().map_or_else(
                || format!("symbol:{:?}", argument.symbol),
                |value| format!("const:{value}"),
            )
        })
}

pub(super) fn static_arguments_match(
    left: &StaticMachineArgument,
    right: &StaticMachineArgument,
) -> bool {
    static_argument_identity(left) == static_argument_identity(right)
}

fn static_argument_identity(argument: &StaticMachineArgument) -> String {
    let head = if argument.symbol.is_valid() {
        format!("symbol:{:?}", argument.symbol)
    } else {
        format!(
            "path:{}",
            argument
                .path
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::")
        )
    };
    let application = argument
        .application
        .as_ref()
        .map_or_else(String::new, |application| {
            format!(
                "<{};{}>",
                application
                    .lifetime_arguments
                    .iter()
                    .map(|lifetime| lifetime.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
                application
                    .arguments
                    .iter()
                    .map(static_argument_identity)
                    .collect::<Vec<_>>()
                    .join(",")
            )
        });
    format!(
        "{head}{application}:const={:?}:evidence={:?}",
        argument.const_literal, argument.evidence_projection
    )
}
