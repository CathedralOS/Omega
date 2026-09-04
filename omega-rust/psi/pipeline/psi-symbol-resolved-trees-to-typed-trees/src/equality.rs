//! Equality vs membership (frozen decision 11): `==`/`!=` is always VALUE
//! equality. A bare PAYLOAD-BEARING case name (`Command::Move`) denotes no
//! value -- only its domain -- so comparing against it is an error that
//! suggests `in`. The brace form (`Command::Move { dx: 1 }`) IS a constructed
//! value: when the sum declares `Type satisfies Equatable;` the compare is
//! legal and expands to synthesized structural equality (`crate::equatable`);
//! without the conformance it stays an error that now suggests declaring it.
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

use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees as resolved;
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
    facts: psi_arena::HandleSpan<resolved::domain::ProofFact>,
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
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => Ok(()),
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
        ExpressionNode::Atomic(atomic) => scan_expression(program, atomic.value, fact_position),
        ExpressionNode::Binary(binary) => {
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) {
                for operand in [binary.left, binary.right] {
                    check_equality_operand(program, operand, fact_position)?;
                }
            }
            scan_expression(program, binary.left, fact_position)?;
            scan_expression(program, binary.right, fact_position)
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in expressions.expression_handles(*elements) {
                scan_expression(program, *element, fact_position)?;
            }
            Ok(())
        }
        ExpressionNode::Cast(cast) => scan_expression(program, cast.value, fact_position),
        ExpressionNode::Call(call) => {
            scan_expression(program, call.receiver, fact_position)?;
            for argument in expressions.expression_handles(call.arguments) {
                scan_expression(program, *argument, fact_position)?;
            }
            Ok(())
        }
        ExpressionNode::Indexed(indexed) => {
            scan_expression(program, indexed.collection, fact_position)?;
            scan_expression(program, indexed.index, fact_position)
        }
        ExpressionNode::Member(member) => scan_expression(program, member.receiver, fact_position),
        // The membership DOMAIN is a name path, not a value: a bare case
        // name is exactly what `in` is for, so only the subject is scanned.
        ExpressionNode::Membership(membership) => {
            scan_expression(program, membership.value, fact_position)
        }
        ExpressionNode::Borrow(inner) => scan_expression(program, inner.target, fact_position),
        ExpressionNode::Range(range) => {
            scan_expression(program, range.start, fact_position)?;
            scan_expression(program, range.end, fact_position)
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in expressions.struct_fields(literal.fields) {
                scan_expression(program, field.value, fact_position)?;
            }
            Ok(())
        }
        ExpressionNode::Unary(unary) => scan_expression(program, unary.operand, fact_position),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
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
            // A declared `Type satisfies Equatable;` synthesizes structural
            // equality, so the constructed-case compare is legal and lowers
            // to the inline expansion (`expression::table::structural_equality`).
            if crate::equatable::equatable_conformance_declared(program, &data_name) {
                return Ok(());
            }
            // FACT position over RECURSIVE (hence proof-only) data: no
            // runtime lowering exists to synthesize -- the structural
            // entailment judge owns this equality (math roster N3;
            // injectivity/disjointness over constructed cases). Runtime
            // positions keep the fence.
            if fact_position && data_is_directly_recursive(program, &data_name) {
                return Ok(());
            }
            Err(Diagnostic::error(format!(
                "cannot compare against payload-bearing case `{data_name}::{case_name}` with `==`: structural equality is not synthesized for non-conforming types -- match on the case in a transition to bind its payload, or declare `{data_name} satisfies Equatable;` to synthesize structural equality"
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

/// Whether `type_name`'s definition contains itself INLINE through any field
/// or case payload (Nat's `prev: Nat`, Seq's `tail: Seq<T>`) -- the seed of
/// the typed layer's proof-only classification, recomputed here at the
/// resolved level because this pass runs before typing. Direct recursion
/// only: mutually recursive types stay conservatively fenced in fact
/// position until a consumer needs them.
pub(crate) fn data_is_directly_recursive(program: &SymbolResolvedTrees, type_name: &str) -> bool {
    let Some(definition) = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == type_name)
    else {
        return false;
    };
    program
        .data_members(definition.members)
        .iter()
        .any(|member| {
            let fields = match member {
                DataMember::Field(field) => std::slice::from_ref(field),
                DataMember::Variant(variant) => program.data_payload_fields(variant.payload),
            };
            fields
                .iter()
                .any(|field| type_reference_names(&field.type_reference, type_name))
        })
}

/// Whether a declaration-storage type reference names `goal` inline (named
/// or generic-base forms -- the shapes the roster's recursive payloads use;
/// references and slices are indirection and never count, mirroring the
/// proof-only containment rule).
fn type_reference_names(reference: &resolved::types::TypeReference, goal: &str) -> bool {
    use resolved::types::TypeReference;
    match reference {
        TypeReference::Named { name, .. } => name.as_str() == goal,
        TypeReference::Generic(generic) => generic.base_name.as_str() == goal,
        _ => false,
    }
}
