//! Struct-literal field validation: every named field of a brace construction
//! must be a declared member of the constructed shape. Covers current-shape
//! record literals (`Counter { count: 0 }`), historical-shape literals
//! (`Counter::v1 { counter: 3 }` -- lowered upstream to record literals of the
//! version's root-level shape definition), and case-payload literals
//! (`Command::Say { text: ... }`). Literals whose head type is not a data
//! definition in this program (or is generic, where member types depend on
//! instantiation) are left to later layers.

use crate::arithmetic_domains::{ValueEnv, validate_arithmetic_domains};
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::{DataDefinition, DataMember};
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

pub(crate) fn validate_struct_literal_fields(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Assignment(assignment) => {
                        scan_expression(program, machine, state,assignment.target, diagnostics);
                        scan_expression(program, machine, state,assignment.value, diagnostics);
                    }
                    StatementNode::Call(call) => {
                        for argument in program.statement_table.expression_handles(call.arguments) {
                            scan_expression(program, machine, state,*argument, diagnostics);
                        }
                    }
                    StatementNode::Expression(expression) => {
                        scan_expression(program, machine, state,*expression, diagnostics);
                    }
                    StatementNode::LocalData(local_data) => {
                        scan_expression(program, machine, state,local_data.initial_value, diagnostics);
                    }
                    StatementNode::Transition(transition) => {
                        if let TransitionGuardNode::When(guard) = &transition.guard {
                            scan_expression(program, machine, state,*guard, diagnostics);
                        }
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            scan_transition_target(
                                program,
                                machine,
                                state,
                                program.statement_table.transition_target(target),
                                diagnostics,
                            );
                        }
                    }
                }
            }
        }
    }
}

fn scan_transition_target(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    target: &TransitionTargetNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match target {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                scan_expression(program, machine, state,*argument, diagnostics);
            }
        }
        TransitionTargetNode::Value(expression) => {
            scan_expression(program, machine, state,*expression, diagnostics);
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}

fn scan_expression(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::StructLiteral(literal) => {
            validate_literal_field_names(program, &literal, diagnostics);
            enforce_construction_field_ranges(program, machine, state, &literal, diagnostics);
            for field in program.expression_table.struct_fields(literal.fields) {
                scan_expression(program, machine, state,field.value, diagnostics);
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                scan_expression(program, machine, state,*element, diagnostics);
            }
        }
        ExpressionNode::Binary(binary) => {
            scan_expression(program, machine, state,binary.left, diagnostics);
            scan_expression(program, machine, state,binary.right, diagnostics);
        }
        ExpressionNode::Cast(cast) => scan_expression(program, machine, state,cast.value, diagnostics),
        ExpressionNode::Call(call) => {
            scan_expression(program, machine, state,call.receiver, diagnostics);
            for argument in program.expression_table.expression_handles(call.arguments) {
                scan_expression(program, machine, state,*argument, diagnostics);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            scan_expression(program, machine, state,indexed.collection, diagnostics);
            scan_expression(program, machine, state,indexed.index, diagnostics);
        }
        ExpressionNode::Member(member) => scan_expression(program, machine, state,member.receiver, diagnostics),
        ExpressionNode::Mutable(inner) => scan_expression(program, machine, state,*inner, diagnostics),
        ExpressionNode::Range(range) => {
            scan_expression(program, machine, state,range.start, diagnostics);
            scan_expression(program, machine, state,range.end, diagnostics);
        }
        ExpressionNode::Unary(unary) => scan_expression(program, machine, state,unary.operand, diagnostics),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

/// Check one literal's named fields against the constructed shape's declared
/// members: record literals (current or historical shape) construct FIELD
/// members; case literals construct the named variant's PAYLOAD fields.
fn validate_literal_field_names(
    program: &TypedTrees,
    literal: &TableStructLiteral,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let type_name = literal.type_name.as_str();
    let Some(data_definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == type_name)
    else {
        return;
    };
    if data_definition.type_parameters.count() > 0 {
        return;
    }

    match &literal.case_name {
        None => {
            // A case-bearing type (sum or mixed) has no record-form literal:
            // construction always names the case, which pins the tag. The
            // zero case with named common fields is `Type::ZeroCase { ... }`.
            if program
                .data_members(data_definition)
                .iter()
                .any(|member| matches!(member, DataMember::Variant(_)))
            {
                diagnostics.push(Diagnostic::error(format!(
                    "data `{type_name}` is case-bearing; construct a case (`{type_name}::Case {{ ... }}`) instead of a record literal"
                )));
                return;
            }
            for field in program.expression_table.struct_fields(literal.fields) {
                if !data_declares_field(program, data_definition, field.name.as_str()) {
                    diagnostics.push(Diagnostic::error(format!(
                        "data `{type_name}` has no field `{}`",
                        field.name.as_str()
                    )));
                }
            }
        }
        Some(case_name) => {
            let Some(variant) = program
                .data_members(data_definition)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.name.as_str() == case_name.as_str() => {
                        Some(variant)
                    }
                    _ => None,
                })
            else {
                return;
            };
            // A case literal names the case's PAYLOAD fields, and -- for mixed
            // shapes -- may name COMMON fields alongside them (unnamed common
            // fields zero-initialize; frozen decision 7's construction rule).
            for field in program.expression_table.struct_fields(literal.fields) {
                let declared = program
                    .data_payload_fields(variant)
                    .iter()
                    .any(|payload_field| payload_field.name.as_str() == field.name.as_str())
                    || data_declares_field(program, data_definition, field.name.as_str());
                if !declared {
                    diagnostics.push(Diagnostic::error(format!(
                        "case `{type_name}::{}` has no payload field `{}`",
                        case_name.as_str(),
                        field.name.as_str()
                    )));
                }
            }
        }
    }
}

pub(crate) fn data_declares_field(
    program: &TypedTrees,
    data_definition: &DataDefinition,
    field_name: &str,
) -> bool {
    program.data_members(data_definition).iter().any(
        |member| matches!(member, DataMember::Field(field) if field.name.as_str() == field_name),
    )
}

/// Decision-17 / fact-catalog soundness: a field declared with a range
/// refinement (`index: i32 [0..=15]`) must be CONSTRUCTED with a value provably
/// in that range -- otherwise a destructure / field read that trusts the range
/// (S4 narrowing in `places::declared_place_type_raw`) would rest on an
/// unenforced bound. An integer literal is checked exactly; a non-literal value
/// is accepted when its PROVEN interval (type ranges + the flow facts visible
/// here) is within the field range -- e.g. copying a same-range field -- and
/// rejected otherwise. Without this, the field-fact narrowing is unsound.
fn enforce_construction_field_ranges(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    literal: &TableStructLiteral,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let type_name = literal.type_name.as_str();
    let Some(data_definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == type_name)
    else {
        return;
    };
    if data_definition.type_parameters.count() > 0 {
        return;
    }

    for field in program.expression_table.struct_fields(literal.fields) {
        let Some(field_type) = construction_field_type(
            program,
            data_definition,
            literal.case_name.as_ref().map(|name| name.as_str()),
            field.name.as_str(),
        ) else {
            continue;
        };
        let Some(range) = crate::arithmetic_domains::range_constraint_interval(program, field_type)
        else {
            continue;
        };
        let bounds = format!(
            "{}..={}",
            range.low().map(|low| low.to_string()).unwrap_or_default(),
            range.high().map(|high| high.to_string()).unwrap_or_default(),
        );
        match construction_field_literal(program, field.value) {
            Some(value) => {
                let below = range.low().is_some_and(|low| value < low);
                let above = range.high().is_some_and(|high| value > high);
                if below || above {
                    diagnostics.push(Diagnostic::error(format!(
                        "construction of `{type_name}` field `{}`: value {value} is outside its declared range `{bounds}`",
                        field.name.as_str()
                    )));
                }
            }
            None => {
                // Non-literal value: accept when its PROVEN interval is within
                // the field range (a same-range field copy, a guarded value),
                // else reject. The value's own arithmetic obligation is reported
                // by the normal statement walk, so its diagnostics go to a
                // throwaway buffer here -- we only add the range-violation.
                let owner = format!(
                    "construction of `{type_name}` field `{}`",
                    field.name.as_str()
                );
                let mut throwaway = Vec::new();
                let interval = validate_arithmetic_domains(
                    program,
                    machine,
                    Some(state),
                    field.value,
                    &ValueEnv::new(),
                    None,
                    &owner,
                    &mut throwaway,
                );
                let provably_in_range = range
                    .low()
                    .is_none_or(|low| interval.low().is_some_and(|value_low| value_low >= low))
                    && range
                        .high()
                        .is_none_or(|high| interval.high().is_some_and(|value_high| value_high <= high));
                if !provably_in_range {
                    diagnostics.push(Diagnostic::error(format!(
                        "construction of `{type_name}` field `{}` cannot be proven within its declared range `{bounds}`; constrain the value, construct with a literal in range, or widen the field",
                        field.name.as_str()
                    )));
                }
            }
        }
    }
}

/// The declared type of a constructed field: a case literal's PAYLOAD field (for
/// the named variant) or a record/common struct field.
fn construction_field_type(
    program: &TypedTrees,
    data_definition: &DataDefinition,
    case_name: Option<&str>,
    field_name: &str,
) -> Option<omega_typed_trees::types::TypeReferenceHandle> {
    if let Some(case_name) = case_name
        && let Some(variant) =
            program
                .data_members(data_definition)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.name.as_str() == case_name => {
                        Some(variant)
                    }
                    _ => None,
                })
    {
        for payload_field in program.data_payload_fields(variant) {
            if payload_field.name.as_str() == field_name {
                return payload_field
                    .type_reference
                    .is_valid()
                    .then_some(payload_field.type_reference);
            }
        }
    }
    program
        .data_members(data_definition)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == field_name => field
                .type_reference
                .is_valid()
                .then_some(field.type_reference),
            _ => None,
        })
}

/// Read an integer-literal construction value (the parser folds a negative
/// `-5` into `Integer(-5)`, so the bare `Integer` case suffices). `None` for any
/// non-literal value -- those are conservatively rejected by the caller.
fn construction_field_literal(program: &TypedTrees, value: ExpressionHandle) -> Option<i64> {
    match program.expression_table.expression(value) {
        ExpressionNode::Integer(literal) => Some(*literal),
        _ => None,
    }
}
