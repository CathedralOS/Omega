//! Equality vs membership (frozen decision 11): `==`/`!=` is always VALUE
//! equality. A bare PAYLOAD-BEARING case name (`Command::Move`) denotes no
//! value -- only its domain -- so comparing against it is an error that
//! suggests `in`. The brace form (`Command::Move { dx: 1 }`) IS a constructed
//! value, but structural equality is not synthesized yet, so it keeps the
//! interim error. Payload-less cases stay legal `==` operands everywhere
//! (tag identity is the only thing equality could mean for them).
//!
//! This check runs on the RESOLVED trees, BEFORE membership lowering: `in`
//! is still a distinct `Membership` node here, and transition case arms
//! desugar to membership at parse time, so every equality this pass sees is
//! user-written. That is what lets the check cover ALL positions --
//! statements, transition guard subjects/conditions, transition target
//! arguments, domain `when` classifiers and proof facts, and machine
//! contracts -- without re-flagging the tag compare that membership lowering
//! synthesizes afterwards (the guard tag clamp survives only as that
//! internal lowering of `in`).

use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use resolved::SymbolResolvedTrees;
use resolved::data::DataMember;
use resolved::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use resolved::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

pub(crate) fn validate_equality_operands(program: &SymbolResolvedTrees) -> Result<(), Diagnostic> {
    for machine in &program.machines {
        for contract in program.signature_contracts(machine.contracts) {
            scan_proof_facts(program, contract.facts)?;
        }

        for state_handle in program.machine_state_handles(machine.states) {
            let state = program.machine_state(*state_handle);
            for statement in program.tables.bodies.statements.statements(state.statement_nodes) {
                scan_statement(program, statement)?;
            }
        }
    }

    for domain in &program.domain_definitions {
        if domain.classifier.is_valid() {
            scan_expression(program, domain.classifier)?;
        }
        scan_proof_facts(program, domain.facts)?;
    }

    Ok(())
}

fn scan_proof_facts(
    program: &SymbolResolvedTrees,
    facts: omega_core::arena::HandleSpan<resolved::domain::ProofFact>,
) -> Result<(), Diagnostic> {
    for fact in program.proof_facts(facts) {
        match fact {
            resolved::domain::ProofFact::Expression(expression) => {
                scan_expression(program, *expression)?;
            }
            resolved::domain::ProofFact::Membership(membership) => {
                scan_expression(program, membership.value)?;
            }
        }
    }
    Ok(())
}

fn scan_statement(
    program: &SymbolResolvedTrees,
    statement: &StatementNode,
) -> Result<(), Diagnostic> {
    match statement {
        StatementNode::Assignment(assignment) => {
            scan_expression(program, assignment.target)?;
            scan_expression(program, assignment.value)
        }
        StatementNode::Call(call) => {
            for argument in program
                .tables
                .bodies
                .statements
                .expression_handles(call.arguments)
            {
                scan_expression(program, *argument)?;
            }
            Ok(())
        }
        StatementNode::Expression(expression) => scan_expression(program, *expression),
        StatementNode::LocalData(local_data) => scan_expression(program, local_data.initial_value),
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                scan_expression(program, guard)?;
            }
            scan_transition_target(program, transition.target)?;
            scan_transition_target(program, transition.continuation)
        }
    }
}

fn scan_transition_target(
    program: &SymbolResolvedTrees,
    target: resolved::statement::TransitionTargetHandle,
) -> Result<(), Diagnostic> {
    if !target.is_valid() {
        return Ok(());
    }
    match program.tables.bodies.statements.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program
                .tables
                .bodies
                .statements
                .expression_handles(*arguments)
            {
                scan_expression(program, *argument)?;
            }
            Ok(())
        }
        TransitionTargetNode::Value(expression) => scan_expression(program, *expression),
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => Ok(()),
    }
}

fn scan_expression(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
) -> Result<(), Diagnostic> {
    if !expression.is_valid() {
        return Ok(());
    }

    let expressions = &program.tables.bodies.expressions;
    match expressions.expression(expression) {
        ExpressionNode::Binary(binary) => {
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) {
                for operand in [binary.left, binary.right] {
                    check_equality_operand(program, operand)?;
                }
            }
            scan_expression(program, binary.left)?;
            scan_expression(program, binary.right)
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in expressions.expression_handles(*elements) {
                scan_expression(program, *element)?;
            }
            Ok(())
        }
        ExpressionNode::Cast(cast) => scan_expression(program, cast.value),
        ExpressionNode::Call(call) => {
            scan_expression(program, call.receiver)?;
            for argument in expressions.expression_handles(call.arguments) {
                scan_expression(program, *argument)?;
            }
            Ok(())
        }
        ExpressionNode::Indexed(indexed) => {
            scan_expression(program, indexed.collection)?;
            scan_expression(program, indexed.index)
        }
        ExpressionNode::Member(member) => scan_expression(program, member.receiver),
        // The membership DOMAIN is a name path, not a value: a bare case
        // name is exactly what `in` is for, so only the subject is scanned.
        ExpressionNode::Membership(membership) => scan_expression(program, membership.value),
        ExpressionNode::Mutable(inner) => scan_expression(program, *inner),
        ExpressionNode::Range(range) => {
            scan_expression(program, range.start)?;
            scan_expression(program, range.end)
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in expressions.struct_fields(literal.fields) {
                scan_expression(program, field.value)?;
            }
            Ok(())
        }
        ExpressionNode::Unary(unary) => scan_expression(program, unary.operand),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => Ok(()),
    }
}

fn check_equality_operand(
    program: &SymbolResolvedTrees,
    operand: ExpressionHandle,
) -> Result<(), Diagnostic> {
    match program.tables.bodies.expressions.expression(operand) {
        ExpressionNode::Name(path) => {
            let members = program
                .tables
                .bodies
                .expressions
                .name_path_members(path.members);
            let [type_name, case_name] = members else {
                return Ok(());
            };
            let Some((data_name, case_name)) =
                payload_bearing_case(program, type_name.as_str(), case_name.as_str())
            else {
                return Ok(());
            };
            Err(Diagnostic::error(format!(
                "`{data_name}::{case_name}` carries a payload, so the bare case name is not a value -- use `in` to test the case (`value in {data_name}::{case_name}`), or compare against a constructed value"
            )))
        }
        ExpressionNode::StructLiteral(literal) => {
            let Some(case_name) = literal.case_name.as_ref() else {
                return Ok(());
            };
            let Some((data_name, case_name)) =
                payload_bearing_case(program, literal.type_name.as_str(), case_name.as_str())
            else {
                return Ok(());
            };
            Err(Diagnostic::error(format!(
                "cannot compare against payload-bearing case `{data_name}::{case_name}` with `==`: structural equality is not synthesized yet -- match on the case in a transition to bind its payload"
            )))
        }
        _ => Ok(()),
    }
}

fn payload_bearing_case(
    program: &SymbolResolvedTrees,
    type_name: &str,
    case_name: &str,
) -> Option<(String, String)> {
    let data_definition = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == type_name)?;

    program
        .data_members(data_definition.members)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant)
                if variant.name.as_str() == case_name && variant.payload.count() > 0 =>
            {
                Some((type_name.to_owned(), case_name.to_owned()))
            }
            _ => None,
        })
}
