//! Equality vs membership (frozen decision 11): `==`/`!=` is always VALUE
//! equality. A bare PAYLOAD-BEARING case name (`Command::Move`) denotes no
//! value -- only its domain -- so comparing against it is an error that
//! suggests `in`. The brace form (`Command::Move { dx: 1 }`) IS a constructed
//! value: when an `Equatable<Type>` implementation exists the compare is
//! legal and lowers through it (`crate::expressions::equatable`). Logical fact
//! positions instead elaborate data identity without executable equality.
//! Payload-less cases stay legal `==` operands everywhere (tag identity is
//! the only thing equality could mean for them).
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

use diagnostics::Diagnostic;
use resolved::SymbolResolvedTrees;
use resolved::data::{DataDefinition, DataMember};
use resolved::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use resolved::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use symbol_resolved_trees as resolved;

pub(crate) fn validate_equality_operands(program: &SymbolResolvedTrees) -> Result<(), Diagnostic> {
    for machine in &program.machines {
        for contract in program.signature_contracts(machine.contracts) {
            scan_proof_facts(program, contract.facts)?;
        }

        for state_handle in program.machine_state_handles(machine.states) {
            let state = program.machine_state(*state_handle);
            for statement in program
                .tables
                .bodies
                .statements
                .statements(state.statement_nodes)
            {
                scan_statement(program, statement)?;
            }
        }
    }

    for domain in &program.domain_definitions {
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
                scan_expression(program, *expression, true)?;
            }
            resolved::domain::ProofFact::Membership(membership) => {
                scan_expression(program, membership.value, true)?;
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
                    scan_expression(program, *expression, true)?;
                }
                for instruction in program
                    .tables
                    .bodies
                    .statements
                    .statements(block.statement_nodes)
                {
                    scan_statement(program, instruction)?;
                }
            }
            Ok(())
        }
        StatementNode::RootBinding(binding) => {
            scan_expression(program, binding.receiver, false)?;
            if binding.implementation_operand.is_valid() {
                scan_expression(program, binding.implementation_operand, false)?;
            }
            Ok(())
        }
        StatementNode::AssemblyFact(fact) => scan_expression(program, fact.expression, true),
        StatementNode::Assignment(assignment) => {
            scan_expression(program, assignment.target, false)?;
            scan_expression(program, assignment.value, false)
        }
        StatementNode::Call(call) => {
            for argument in program
                .tables
                .bodies
                .statements
                .expression_handles(call.arguments)
            {
                scan_expression(program, *argument, false)?;
            }
            Ok(())
        }
        StatementNode::ProofOutputBindingStatement(package) => {
            scan_expression(program, package.call, false)
        }
        StatementNode::Expression(expression) => scan_expression(program, *expression, false),
        StatementNode::LocalData(local_data) => {
            scan_expression(program, local_data.initial_value, false)
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                scan_expression(program, guard, false)?;
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
                scan_expression(program, *argument, false)?;
            }
            Ok(())
        }
        TransitionTargetNode::Value(expression) => scan_expression(program, *expression, false),
        TransitionTargetNode::Terminal => Ok(()),
    }
}

fn scan_expression(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
    fact_position: bool,
) -> Result<(), Diagnostic> {
    if !expression.is_valid() {
        return Ok(());
    }

    let expressions = &program.tables.bodies.expressions;
    match expressions.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            scan_expression(program, dispatch.subject, false)?;
            for arm in expressions.match_arms(dispatch.arms) {
                if let resolved::expression::MatchPattern::Value(pattern) = arm.pattern {
                    check_equality_operand(program, dispatch.subject, false)?;
                    check_equality_operand(program, pattern, false)?;
                    scan_expression(program, pattern, false)?;
                }
                scan_expression(program, arm.value, fact_position)?;
            }
            Ok(())
        }
        ExpressionNode::Atomic(atomic) => scan_expression(program, atomic.value, false),
        ExpressionNode::Binary(binary) => {
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) {
                for operand in [binary.left, binary.right] {
                    check_equality_operand(program, operand, fact_position)?;
                }
            }
            let children_are_facts = fact_position
                && matches!(
                    binary.operator,
                    BinaryOperator::And
                        | BinaryOperator::Or
                        | BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                );
            scan_expression(program, binary.left, children_are_facts)?;
            scan_expression(program, binary.right, children_are_facts)
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in expressions.expression_handles(*elements) {
                scan_expression(program, *element, false)?;
            }
            Ok(())
        }
        ExpressionNode::Cast(cast) => scan_expression(program, cast.value, false),
        ExpressionNode::Call(call) => {
            scan_expression(program, call.receiver, false)?;
            for argument in expressions.expression_handles(call.arguments) {
                scan_expression(program, *argument, false)?;
            }
            Ok(())
        }
        ExpressionNode::Indexed(indexed) => {
            scan_expression(program, indexed.collection, false)?;
            scan_expression(program, indexed.index, false)
        }
        ExpressionNode::Member(member) => scan_expression(program, member.receiver, false),
        // The membership DOMAIN is a name path, not a value: a bare case
        // name is exactly what `in` is for, so only the subject is scanned.
        ExpressionNode::Membership(membership) => scan_expression(program, membership.value, false),
        ExpressionNode::Borrow(inner) => scan_expression(program, inner.target, false),
        ExpressionNode::Range(range) => {
            scan_expression(program, range.start, false)?;
            scan_expression(program, range.end, false)
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in expressions.struct_fields(literal.fields) {
                scan_expression(program, field.value, false)?;
            }
            Ok(())
        }
        ExpressionNode::Unary(unary) => scan_expression(
            program,
            unary.operand,
            fact_position && unary.operator == resolved::expression::UnaryOperator::LogicalNot,
        ),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => Ok(()),
    }
}

fn check_equality_operand(
    program: &SymbolResolvedTrees,
    operand: ExpressionHandle,
    fact_position: bool,
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
            // Logical identity applies to every data carrier. Typed formation
            // still checks complete operand types and the proof owner checks
            // all common/payload fields; this is never a tag-only comparison.
            if fact_position {
                return Ok(());
            }
            Err(Diagnostic::error(format!(
                "cannot compare against payload-bearing case `{data_name}::{case_name}` with `==`: structural equality is synthesized only through a named implementation -- match on the case in a transition to bind its payload, or declare `{}` to synthesize structural equality",
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
