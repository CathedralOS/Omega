//! Canonical semantic identity for proof obligations.
//!
//! `proof_obligation_key` renders one obligation as a canonical string. The
//! proof-search draft (`wiki/drafts/reference/proof_search_cache.md`) requires keys to
//! carry normalized semantic identity -- exact retained content plus
//! schema/checker compatibility -- while a rename that does not change
//! meaning must not invalidate a semantically identical obligation.
//!
//! What enters the key: the obligation variant; resolved declaration identity
//! of every retained `SymbolHandle` (package- or toolchain-qualified where
//! provenance exists, the canonical declaration path otherwise); the
//! normalized base-type identity; every retained `ProofConstraint`'s content
//! in declared order; and a structural encoding of the expression, guard,
//! and operand payloads, where name paths serialize through their resolved
//! symbols rather than their spellings.
//!
//! What stays out: `Identifier` display fields beside a resolved symbol,
//! source spans, positional statement indices, and checker seeds -- none of
//! them distinguish obligations semantically. Identifiers that ARE the
//! retained identity (receivers, evidence names, symbolic-constraint field
//! names, witness places) keep their authored text. `KEY_SCHEMA` versions
//! the encoding itself, so checker/schema changes can invalidate stale keys
//! deliberately instead of silently colliding.

use crate::obligations::plan::{
    BinaryValueOperands, IntegerRange, ProofConstraint, ProofObligation, ProofObligationOwner,
    ProofPlan,
};
use arena::HandleSpan;
use std::fmt;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, FloatLiteral, MatchPattern,
    PrivateLayoutOperationRequest, QuotientOperationRequest, StaticMachineArgument,
    StaticSymbolApplication, TableCallExpression, TableNamePath,
};
use typed_trees::statement::TransitionGuardNode;
use typed_trees::typed_trees::StaticRequirementDispatch;

/// Encoding version. Bump when the canonicalization itself changes so keys
/// computed under a previous schema never alias current ones.
pub const KEY_SCHEMA: u32 = 1;

/// Canonical semantic identity of one proof obligation. The string is the
/// complete normalized content; equal obligations produce equal keys even
/// when their display spellings or statement positions differ, and distinct
/// content never merges.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProofObligationKey(String);

impl ProofObligationKey {
    /// The canonical key text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ProofObligationKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl fmt::Display for ProofObligationKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Compute the canonical semantic identity of `obligation` in `proof_plan`.
pub fn proof_obligation_key(
    proof_plan: &ProofPlan<'_>,
    obligation: &ProofObligation,
) -> ProofObligationKey {
    let program = proof_plan.program;
    let body = match obligation {
        ProofObligation::BoundedAssignment(obligation) => compound(
            "bounded-assignment",
            [
                machine_state_identity(program, obligation.machine_symbol, obligation.state_symbol),
                atom(
                    "guard-source",
                    symbol_identity(program, obligation.state_guard_source),
                ),
                obligation
                    .state_guard
                    .as_ref()
                    .map(|guard| guard_identity(program, guard))
                    .unwrap_or_else(|| "unguarded".to_owned()),
                compound("target", [expression_identity(program, obligation.target)]),
                compound("value", [expression_identity(program, obligation.value)]),
                constraints_identity(
                    proof_plan,
                    obligation.value_constraints,
                    "value-constraints",
                ),
                atom(
                    "base",
                    program
                        .package_qualified_type_identity(obligation.base_type)
                        .as_str(),
                ),
                constraints_identity(proof_plan, obligation.constraints, "constraints"),
                binary_operands_identity(program, &obligation.binary_operands),
                compound(
                    "witness-bounds",
                    obligation
                        .ensures_witness_bounds
                        .iter()
                        .map(|(place, bound)| {
                            compound("witness", [atom("place", place), bound.to_string()])
                        }),
                ),
            ],
        ),
        ProofObligation::BoundedCallArgument(obligation) => compound(
            "bounded-call-argument",
            [
                machine_state_identity(program, obligation.machine_symbol, obligation.state_symbol),
                obligation
                    .receiver
                    .as_ref()
                    .map(|receiver| atom("receiver", receiver.as_str()))
                    .unwrap_or_else(|| "receiver:none".to_owned()),
                atom("target", symbol_identity(program, obligation.target_symbol)),
                atom(
                    "parameter",
                    symbol_identity(program, obligation.parameter_symbol),
                ),
                compound(
                    "argument",
                    [expression_identity(program, obligation.argument)],
                ),
                constraints_identity(
                    proof_plan,
                    obligation.argument_constraints,
                    "argument-constraints",
                ),
                atom(
                    "base",
                    program
                        .package_qualified_type_identity(obligation.base_type)
                        .as_str(),
                ),
                constraints_identity(proof_plan, obligation.constraints, "constraints"),
                compound(
                    "sibling",
                    [expression_identity(program, obligation.sibling_argument)],
                ),
            ],
        ),
        ProofObligation::BoundedInitializer(obligation) => compound(
            "bounded-initializer",
            [
                owner_identity(program, &obligation.owner),
                compound("value", [expression_identity(program, obligation.value)]),
                atom(
                    "base",
                    program
                        .package_qualified_type_identity(obligation.base_type)
                        .as_str(),
                ),
                constraints_identity(proof_plan, obligation.constraints, "constraints"),
            ],
        ),
        ProofObligation::BoundedStateReturn(obligation) => compound(
            "bounded-state-return",
            [
                machine_state_identity(program, obligation.machine_symbol, obligation.state_symbol),
                compound("value", [expression_identity(program, obligation.value)]),
                constraints_identity(
                    proof_plan,
                    obligation.value_constraints,
                    "value-constraints",
                ),
                atom(
                    "base",
                    program
                        .package_qualified_type_identity(obligation.base_type)
                        .as_str(),
                ),
                constraints_identity(proof_plan, obligation.constraints, "constraints"),
                binary_operands_identity(program, &obligation.binary_operands),
            ],
        ),
        ProofObligation::BoundedValue(obligation) => compound(
            "bounded-value",
            [
                owner_identity(program, &obligation.owner),
                atom(
                    "base",
                    program
                        .package_qualified_type_identity(obligation.base_type)
                        .as_str(),
                ),
                constraints_identity(proof_plan, obligation.constraints, "constraints"),
            ],
        ),
        ProofObligation::BoundedTransitionArgument(obligation) => compound(
            "bounded-transition-argument",
            [
                machine_state_identity(program, obligation.machine_symbol, obligation.state_symbol),
                atom(
                    "parameter",
                    symbol_identity(program, obligation.parameter_symbol),
                ),
                compound(
                    "argument",
                    [expression_identity(program, obligation.argument)],
                ),
                constraints_identity(
                    proof_plan,
                    obligation.argument_constraints,
                    "argument-constraints",
                ),
                atom(
                    "base",
                    program
                        .package_qualified_type_identity(obligation.base_type)
                        .as_str(),
                ),
                constraints_identity(proof_plan, obligation.constraints, "constraints"),
                guard_identity(program, &obligation.guard),
                compound(
                    "refuted",
                    obligation
                        .refuted_exit_guards
                        .iter()
                        .map(|guard| expression_identity(program, *guard)),
                ),
                compound(
                    "sibling",
                    [expression_identity(program, obligation.sibling_argument)],
                ),
            ],
        ),
        ProofObligation::GuardedTransition(obligation) => compound(
            "guarded-transition",
            [
                machine_state_identity(program, obligation.machine_symbol, obligation.state_symbol),
                guard_identity(program, &obligation.guard),
            ],
        ),
    };
    ProofObligationKey(compound(
        "obligation",
        [atom("schema", KEY_SCHEMA.to_string()), body],
    ))
}

fn machine_state_identity(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> String {
    compound(
        "in",
        [
            atom("machine", symbol_identity(program, machine)),
            atom("state", symbol_identity(program, state)),
        ],
    )
}

fn owner_identity(program: &TypedTrees, owner: &ProofObligationOwner) -> String {
    match owner {
        ProofObligationOwner::Unknown => "owner:unknown".to_owned(),
        ProofObligationOwner::MachineOwnedData {
            machine_symbol,
            data_symbol,
            ..
        } => compound(
            "owner",
            [
                atom(
                    "machine-data-machine",
                    symbol_identity(program, *machine_symbol),
                ),
                atom("machine-data", symbol_identity(program, *data_symbol)),
            ],
        ),
        ProofObligationOwner::StateParameter {
            machine_symbol,
            state_symbol,
            parameter_symbol,
            ..
        } => compound(
            "owner",
            [
                atom(
                    "state-param-machine",
                    symbol_identity(program, *machine_symbol),
                ),
                atom("state-param-state", symbol_identity(program, *state_symbol)),
                atom("state-param", symbol_identity(program, *parameter_symbol)),
            ],
        ),
        ProofObligationOwner::StateReturn {
            machine_symbol,
            state_symbol,
            ..
        } => compound(
            "owner",
            [
                atom(
                    "state-return-machine",
                    symbol_identity(program, *machine_symbol),
                ),
                atom(
                    "state-return-state",
                    symbol_identity(program, *state_symbol),
                ),
            ],
        ),
    }
}

fn constraints_identity(
    proof_plan: &ProofPlan<'_>,
    constraints: HandleSpan<ProofConstraint>,
    tag: &str,
) -> String {
    compound(
        tag,
        proof_plan
            .type_constraints
            .span_or_empty(constraints)
            .iter()
            .map(constraint_identity),
    )
}

fn constraint_identity(constraint: &ProofConstraint) -> String {
    match constraint {
        ProofConstraint::Named(name) => atom("named", name.as_str()),
        ProofConstraint::IntegerRange { minimum, maximum } => {
            compound("integer-range", [minimum.to_string(), maximum.to_string()])
        }
        ProofConstraint::IntegerRangeSymbolicMax {
            minimum,
            max_field,
            max_offset,
        } => compound(
            "symbolic-max",
            [
                minimum.to_string(),
                atom("field", max_field.as_str()),
                max_offset.to_string(),
            ],
        ),
        ProofConstraint::IntegerRangeSiblingLenMax {
            minimum,
            sibling,
            max_offset,
        } => compound(
            "sibling-len",
            [
                minimum.to_string(),
                atom("sibling", sibling.as_str()),
                max_offset.to_string(),
            ],
        ),
        ProofConstraint::FloatRange {
            minimum,
            maximum,
            maximum_inclusive,
        } => compound(
            "float-range",
            [
                float_identity(minimum),
                float_identity(maximum),
                maximum_inclusive.to_string(),
            ],
        ),
        ProofConstraint::ArithmeticDomain(domain) => atom("arithmetic-domain", domain.name()),
    }
}

fn float_identity(literal: &FloatLiteral) -> String {
    format!("{:016x}", literal.value_f64().to_bits())
}

fn guard_identity(program: &TypedTrees, guard: &TransitionGuardNode) -> String {
    match guard {
        TransitionGuardNode::Always => "guard:always".to_owned(),
        TransitionGuardNode::When(condition) => {
            compound("guard-when", [expression_identity(program, *condition)])
        }
    }
}

fn binary_operands_identity(
    program: &TypedTrees,
    operands: &Option<BinaryValueOperands>,
) -> String {
    match operands {
        None => "binary-operands:none".to_owned(),
        Some(operands) => compound(
            "binary-operands",
            [
                binary_operator_tag(operands.operator).to_owned(),
                expression_identity(program, operands.left),
                integer_range_identity(operands.left_range.as_ref()),
                expression_identity(program, operands.right),
                integer_range_identity(operands.right_range.as_ref()),
            ],
        ),
    }
}

fn integer_range_identity(range: Option<&IntegerRange>) -> String {
    match range {
        None => "unbounded".to_owned(),
        Some(range) => compound(
            "declared-range",
            [range.minimum.to_string(), range.maximum.to_string()],
        ),
    }
}

fn binary_operator_tag(operator: BinaryOperator) -> &'static str {
    match operator {
        BinaryOperator::Add => "add",
        BinaryOperator::And => "and",
        BinaryOperator::BitwiseAnd => "bit-and",
        BinaryOperator::BitwiseOr => "bit-or",
        BinaryOperator::BitwiseXor => "bit-xor",
        BinaryOperator::CaseMembership => "case-membership",
        BinaryOperator::Divide => "div",
        BinaryOperator::Equal => "eq",
        BinaryOperator::Greater => "gt",
        BinaryOperator::GreaterOrEqual => "ge",
        BinaryOperator::Less => "lt",
        BinaryOperator::LessOrEqual => "le",
        BinaryOperator::Modulo => "mod",
        BinaryOperator::Multiply => "mul",
        BinaryOperator::NotEqual => "ne",
        BinaryOperator::Or => "or",
        BinaryOperator::ShiftLeft => "shl",
        BinaryOperator::ShiftRight => "shr",
        BinaryOperator::Subtract => "sub",
    }
}

fn unary_operator_tag(operator: typed_trees::expression::UnaryOperator) -> &'static str {
    match operator {
        typed_trees::expression::UnaryOperator::BitwiseNot => "bit-not",
        typed_trees::expression::UnaryOperator::LogicalNot => "logical-not",
    }
}

/// Canonical identity of one resolved declaration: the hermetic package- or
/// toolchain-qualified identity where provenance exists, the canonical
/// declaration path otherwise, `unresolved` for an invalid handle.
fn symbol_identity(program: &TypedTrees, symbol: SymbolHandle) -> String {
    if !symbol.is_valid() {
        return "unresolved".to_owned();
    }
    if let Ok(identity) = program.normalized_hermetic_symbol_identity(symbol) {
        return identity;
    }
    program.symbols.display_path(symbol, "::")
}

fn name_path_identity(program: &TypedTrees, path: &TableNamePath) -> String {
    if path.symbol.is_valid() {
        return atom("name", symbol_identity(program, path.symbol));
    }
    let members = program.expression_table.name_path_members(path.members);
    let member_symbols = program
        .expression_table
        .name_path_member_symbols(path.member_symbols);
    // An unresolved path keeps its spelled members; resolved member symbols
    // take precedence over their spellings wherever they exist.
    let identity = members
        .iter()
        .enumerate()
        .map(|(index, member)| {
            member_symbols
                .get(index)
                .filter(|symbol| symbol.is_valid())
                .map(|symbol| symbol_identity(program, *symbol))
                .unwrap_or_else(|| member.as_str().to_owned())
        })
        .collect::<Vec<_>>()
        .join("::");
    atom("name-spelled", identity)
}

fn expression_identity(program: &TypedTrees, expression: ExpressionHandle) -> String {
    if !expression.is_valid() {
        return "expr:invalid".to_owned();
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(node) => compound(
            "match",
            [expression_identity(program, node.subject)]
                .into_iter()
                .chain(
                    program
                        .expression_table
                        .match_arms(node.arms)
                        .iter()
                        .map(|arm| {
                            compound(
                                "arm",
                                [
                                    match arm.pattern {
                                        MatchPattern::Value(pattern) => {
                                            expression_identity(program, pattern)
                                        }
                                        MatchPattern::Wildcard => "wildcard".to_owned(),
                                    },
                                    expression_identity(program, arm.value),
                                ],
                            )
                        }),
                ),
        ),
        ExpressionNode::ArrayLiteral(elements) => compound(
            "array",
            program
                .expression_table
                .expression_handles(*elements)
                .iter()
                .map(|element| expression_identity(program, *element)),
        ),
        ExpressionNode::Atomic(node) => compound(
            "atomic",
            [
                expression_identity(program, node.value),
                expression_identity(program, node.result),
                format!("{:?}", node.ordering),
                format!("{:?}", node.result_custody),
            ],
        ),
        ExpressionNode::Binary(node) => compound(
            binary_operator_tag(node.operator),
            [
                expression_identity(program, node.left),
                expression_identity(program, node.right),
            ],
        ),
        ExpressionNode::Boolean(value) => atom("bool", value.to_string()),
        ExpressionNode::Cast(node) => compound(
            "cast",
            [
                expression_identity(program, node.value),
                atom(
                    "target",
                    program
                        .package_qualified_type_identity(node.target_type)
                        .as_str(),
                ),
                atom("domain", node.domain.name()),
                atom(
                    "domain-symbol",
                    symbol_identity(program, node.semantic_domain_symbol),
                ),
                compound(
                    "domain-arguments",
                    program
                        .type_reference_table
                        .type_reference_handles(node.semantic_domain_arguments)
                        .iter()
                        .map(|type_reference| {
                            program
                                .package_qualified_type_identity(*type_reference)
                                .as_str()
                                .to_owned()
                        }),
                ),
                format!("{:?}", node.form),
            ],
        ),
        ExpressionNode::Call(node) => call_identity(program, node),
        ExpressionNode::Float(value) => atom("float", float_identity(value)),
        ExpressionNode::Indexed(node) => compound(
            "index",
            [
                expression_identity(program, node.collection),
                expression_identity(program, node.index),
            ],
        ),
        ExpressionNode::Integer(value) => match value.value_bignum() {
            Some(value) => atom("int", value.to_string()),
            None => "int:undecoded".to_owned(),
        },
        ExpressionNode::Member(node) => compound(
            "member",
            [
                expression_identity(program, node.receiver),
                if node.member_symbol.is_valid() {
                    atom("member", symbol_identity(program, node.member_symbol))
                } else {
                    atom("member-name", node.member.as_str())
                },
                node.case_variant
                    .as_ref()
                    .map(|variant| atom("case", variant.as_str()))
                    .unwrap_or_else(|| "case:none".to_owned()),
            ],
        ),
        ExpressionNode::Borrow(node) => compound(
            "borrow",
            [
                expression_identity(program, node.target),
                format!("{:?}", node.access),
            ],
        ),
        ExpressionNode::Name(path) => name_path_identity(program, path),
        ExpressionNode::Range(node) => compound(
            "range",
            [
                expression_identity(program, node.start),
                expression_identity(program, node.end),
                node.end_inclusive.to_string(),
            ],
        ),
        ExpressionNode::StructLiteral(node) => compound(
            "struct",
            [
                atom("type", symbol_identity(program, node.type_symbol)),
                node.case_symbol
                    .map(|case| atom("case", symbol_identity(program, case)))
                    .unwrap_or_else(|| "case:none".to_owned()),
            ]
            .into_iter()
            .chain(
                program
                    .expression_table
                    .struct_fields(node.fields)
                    .iter()
                    .map(|field| {
                        compound(
                            "field",
                            [
                                if field.field_symbol.is_valid() {
                                    atom("field", symbol_identity(program, field.field_symbol))
                                } else {
                                    atom("field-name", field.name.as_str())
                                },
                                expression_identity(program, field.value),
                            ],
                        )
                    }),
            ),
        ),
        ExpressionNode::String(bytes) => atom("str", hex(bytes)),
        ExpressionNode::Unary(node) => compound(
            unary_operator_tag(node.operator),
            [expression_identity(program, node.operand)],
        ),
        ExpressionNode::ZeroValue(type_reference) => atom(
            "zero",
            program
                .package_qualified_type_identity(*type_reference)
                .as_str(),
        ),
    }
}

fn call_identity(program: &TypedTrees, call: &TableCallExpression) -> String {
    compound(
        "call",
        [
            expression_identity(program, call.receiver),
            atom("target", symbol_identity(program, call.target_symbol)),
            atom(
                "static-parameter",
                symbol_identity(program, call.static_machine_parameter),
            ),
            requirement_dispatch_identity(program, call.static_requirement_dispatch.as_ref()),
            compound(
                "machine-arguments",
                call.machine_arguments
                    .iter()
                    .map(|argument| static_argument_identity(program, argument)),
            ),
            quotient_identity(program, call.quotient_operation.as_ref()),
            call.private_layout_operation
                .as_ref()
                .map(|request| private_layout_identity(program, request))
                .unwrap_or_else(|| "private-layout:none".to_owned()),
            compound(
                "arguments",
                program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|argument| expression_identity(program, *argument)),
            ),
            compound(
                "evidence-arguments",
                call.evidence_arguments
                    .iter()
                    .map(|name| atom("evidence", name.as_str())),
            ),
            format!("{:?}", call.operational_acknowledgement),
        ],
    )
}

fn requirement_dispatch_identity(
    program: &TypedTrees,
    dispatch: Option<&StaticRequirementDispatch>,
) -> String {
    match dispatch {
        None => "dispatch:none".to_owned(),
        Some(dispatch) => compound(
            "dispatch",
            [
                format!(
                    "commitment:{}",
                    hex(&dispatch.application_commitment.as_bytes())
                ),
                format!(
                    "fingerprint:{:016x}",
                    dispatch.application_report_fingerprint
                ),
                atom("trait", symbol_identity(program, dispatch.declaring_trait)),
                atom(
                    "requirement",
                    symbol_identity(program, dispatch.requirement),
                ),
                atom(
                    "machine",
                    symbol_identity(program, dispatch.realization_machine),
                ),
                atom(
                    "state",
                    symbol_identity(program, dispatch.realization_state),
                ),
            ],
        ),
    }
}

fn quotient_identity(program: &TypedTrees, request: Option<&QuotientOperationRequest>) -> String {
    match request {
        None => "quotient:none".to_owned(),
        Some(request) => compound(
            "quotient",
            [
                format!("{:?}", request.kind),
                static_argument_identity(program, &request.representative_operation),
                compound(
                    "theorems",
                    request.theorem_evidence.iter().map(|theorem| {
                        compound(
                            "theorem",
                            [
                                format!("{:?}", theorem.role),
                                static_argument_identity(program, &theorem.application),
                            ],
                        )
                    }),
                ),
            ],
        ),
    }
}

fn private_layout_identity(
    program: &TypedTrees,
    request: &PrivateLayoutOperationRequest,
) -> String {
    compound(
        "private-layout",
        [static_argument_identity(program, &request.selected_slot)],
    )
}

fn static_argument_identity(program: &TypedTrees, argument: &StaticMachineArgument) -> String {
    compound(
        "static-argument",
        [
            if argument.type_reference.is_valid() {
                atom(
                    "type",
                    program
                        .package_qualified_type_identity(argument.type_reference)
                        .as_str(),
                )
            } else {
                "type:none".to_owned()
            },
            atom(
                "path",
                argument
                    .path
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::"),
            ),
            argument
                .application
                .as_deref()
                .map(|application| static_symbol_application_identity(program, application))
                .unwrap_or_else(|| "application:none".to_owned()),
            argument
                .const_literal
                .as_ref()
                .and_then(|literal| literal.value_bignum())
                .map(|value| atom("const", value.to_string()))
                .unwrap_or_else(|| "const:none".to_owned()),
            argument
                .evidence_projection
                .as_ref()
                .map(|projection| {
                    atom(
                        "evidence-projection",
                        format!(
                            "{}.{}",
                            projection.term.as_str(),
                            projection.member.as_str()
                        ),
                    )
                })
                .unwrap_or_else(|| "evidence-projection:none".to_owned()),
            atom("symbol", symbol_identity(program, argument.symbol)),
        ],
    )
}

fn static_symbol_application_identity(
    program: &TypedTrees,
    application: &StaticSymbolApplication,
) -> String {
    compound(
        "application",
        [
            compound(
                "lifetimes",
                application
                    .lifetime_arguments
                    .iter()
                    .map(|name| atom("lifetime", name.as_str())),
            ),
            compound(
                "arguments",
                application
                    .arguments
                    .iter()
                    .map(|argument| static_argument_identity(program, argument)),
            ),
        ],
    )
}

/// `tag:value` leaf. Values are escaped so free text can never close a
/// compound early or forge separators.
fn atom(tag: &str, value: impl AsRef<str>) -> String {
    format!("{tag}:{}", escape(value.as_ref()))
}

fn compound(tag: &str, parts: impl IntoIterator<Item = String>) -> String {
    let mut body = String::with_capacity(32);
    for part in parts {
        if !body.is_empty() {
            body.push(',');
        }
        body.push_str(&part);
    }
    format!("{tag}({body})")
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\\' | '(' | ')' | ',' => {
                escaped.push('\\');
                escaped.push(character);
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = fmt::Write::write_fmt(&mut text, format_args!("{byte:02x}"));
    }
    text
}

#[cfg(test)]
mod tests;
