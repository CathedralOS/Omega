//! Equality vs membership (frozen decision 11): `==`/`!=` is always VALUE
//! equality. A bare PAYLOAD-BEARING case name (`Command::Move`) denotes no
//! value -- only its domain -- so comparing against it is an error that
//! suggests `in`. The brace form (`Command::Move { dx: 1 }`) IS a constructed
//! value: when an `Equatable<Type>` implementation exists the compare is
//! legal and lowers through it (`crate::expressions::equatable`) in every
//! position. Payload-less cases are values too; their selected comparison
//! still owns the meaning. Mathematical identity uses explicit core::Identity.
//!
//! This check runs on the RESOLVED trees, BEFORE membership lowering: `in`
//! is still a distinct `Membership` node here, and transition case arms
//! desugar to membership at parse time, so every equality this pass sees is
//! user-written. That is what lets the check cover ALL positions --
//! statements, transition guard subjects/conditions, transition target
//! arguments, domain proof facts, and machine
//! contracts -- without re-flagging the tag compare that membership lowering
//! synthesizes afterwards (the guard tag clamp survives only as that
//! internal lowering of `in`).

use crate::tables::{resolved_expressions, resolved_statements};
use diagnostics::Diagnostic;
use resolved::SymbolResolvedTrees;
use resolved::data::{DataDefinition, DataMember};
use resolved::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use resolved::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use symbol_resolved_trees as resolved;

pub(crate) fn validate_equality_operands_in(
    program: &SymbolResolvedTrees,
    cohort: &crate::lowerer::TypingCohort<'_>,
) -> Result<(), Diagnostic> {
    for (ordinal, machine) in program.machines.iter().enumerate() {
        if ordinal < cohort.machines && !cohort.completes(machine.symbol) {
            continue;
        }
        for contract in program.signature_contracts(machine.contracts) {
            scan_proof_facts(program, contract.facts)?;
        }

        for state_handle in program.machine_state_handles(machine.states) {
            let state = program.machine_state(*state_handle);
            for statement in resolved_statements(program).statements(state.statement_nodes) {
                scan_statement(program, statement)?;
            }
        }
    }

    for domain in program
        .domain_definitions
        .iter()
        .skip(cohort.domain_definitions)
    {
        scan_proof_facts(program, domain.facts)?;
    }

    Ok(())
}

fn scan_proof_facts(
    program: &SymbolResolvedTrees,
    facts: arena::HandleSpan<resolved::domain::ProofFact>,
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
        StatementNode::Assembly(handle) => {
            let declarations = &program.tables.declarations;
            let graph = declarations.assembly_graphs.get(*handle);
            for block in declarations.assembly_blocks.span_or_empty(graph.blocks) {
                for expression in declarations
                    .assembly_expressions
                    .span_or_empty(block.requires)
                    .iter()
                    .chain(
                        declarations
                            .assembly_expressions
                            .span_or_empty(block.ensures),
                    )
                {
                    scan_expression(program, *expression)?;
                }
                for instruction in resolved_statements(program).statements(block.statement_nodes) {
                    scan_statement(program, instruction)?;
                }
            }
            Ok(())
        }
        StatementNode::RootBinding(binding) => {
            scan_expression(program, binding.receiver)?;
            if binding.implementation_operand.is_valid() {
                scan_expression(program, binding.implementation_operand)?;
            }
            Ok(())
        }
        StatementNode::AssemblyFact(fact) => scan_expression(program, fact.expression),
        StatementNode::Assignment(assignment) => {
            scan_expression(program, assignment.target)?;
            scan_expression(program, assignment.value)
        }
        StatementNode::Call(call) => {
            for argument in resolved_statements(program).expression_handles(call.arguments) {
                scan_expression(program, *argument)?;
            }
            Ok(())
        }
        StatementNode::ProofOutputBindingStatement(package) => {
            scan_expression(program, package.call)
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
            for argument in resolved_statements(program).expression_handles(*arguments) {
                scan_expression(program, *argument)?;
            }
            Ok(())
        }
        TransitionTargetNode::Value(expression) => scan_expression(program, *expression),
        TransitionTargetNode::Terminal => Ok(()),
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
        ExpressionNode::SavedSubject(_) => Ok(()),
        ExpressionNode::Match(dispatch) => {
            scan_expression(program, dispatch.subject)?;
            for arm in expressions.match_arms(dispatch.arms) {
                if let resolved::expression::MatchPattern::Value(pattern) = arm.pattern {
                    check_equality_operand(program, dispatch.subject)?;
                    check_equality_operand(program, pattern)?;
                    scan_expression(program, pattern)?;
                }
                scan_expression(program, arm.value)?;
            }
            Ok(())
        }
        ExpressionNode::Atomic(atomic) => scan_expression(program, atomic.value),
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
        ExpressionNode::Borrow(inner) => scan_expression(program, inner.target),
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
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::TypeExpression(_)
        | ExpressionNode::ZeroValue(_) => Ok(()),
    }
}

fn check_equality_operand(
    program: &SymbolResolvedTrees,
    operand: ExpressionHandle,
) -> Result<(), Diagnostic> {
    match program.tables.bodies.expressions.expression(operand) {
        ExpressionNode::Name(path) => {
            let members = resolved_expressions(program).name_path_members(path.members);
            let [type_name, case_name] = members else {
                return Ok(());
            };
            let Some((data, case_name)) = payload_bearing_case(
                program,
                type_name.as_str(),
                case_name.as_str(),
                program.tables.bodies.expressions.source_span(operand),
            ) else {
                return Ok(());
            };
            let data_name = data.name.as_str();
            Err(Diagnostic::error(format!(
                "`{data_name}::{case_name}` carries a payload, so the bare case name is not a value -- use `in` to test the case (`value in {data_name}::{case_name}`), or compare against a constructed value"
            )))
        }
        ExpressionNode::StructLiteral(literal) => {
            let Some(case_name) = literal.case_name.as_ref() else {
                return Ok(());
            };
            let Some((data, case_name)) = payload_bearing_case(
                program,
                literal.type_name.as_str(),
                case_name.as_str(),
                program.tables.bodies.expressions.source_span(operand),
            ) else {
                return Ok(());
            };
            let data_name = data.name.as_str();
            // An `Equatable<Type>` implementation makes the constructed-case
            // compare legal; typed lowering selects it and expands or calls
            // its `equals` row (`expression::table::structural_equality`).
            if !matches!(
                crate::expressions::equatable::equality_selection(program, data),
                crate::expressions::equatable::EqualitySelection::Missing
            ) {
                return Ok(());
            }
            Err(Diagnostic::error(format!(
                "cannot compare against payload-bearing case `{data_name}::{case_name}` with `==`: structural equality is synthesized only through a named implementation -- match on the case in a transition to bind its payload, or declare `{}` to synthesize structural equality, or use `core::Identity` with complete values and their carrier for mathematical identity",
                crate::expressions::equatable::suggested_equality_declaration(data_name)
            )))
        }
        _ => Ok(()),
    }
}

fn payload_bearing_case<'program>(
    program: &'program SymbolResolvedTrees,
    type_name: &str,
    case_name: &str,
    reference: source::SourceSpan,
) -> Option<(&'program DataDefinition, String)> {
    let data_definition =
        crate::expressions::equatable::data_definition_by_name_from(program, type_name, reference)?;

    program
        .data_members(data_definition.members)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant)
                if variant.name.as_str() == case_name && variant.payload.count() > 0 =>
            {
                Some((data_definition, case_name.to_owned()))
            }
            _ => None,
        })
}
