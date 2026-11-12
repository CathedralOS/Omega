/*
We type the suffixed policy operators (`a +% b`, `a +| b`, `a +! b`): an
arithmetic or shift operator whose suffix chooses Wrapping, Saturating or
Trapping for that one operation while its operands and result stay plain
integers (wiki/spec/language/numeric_values.md#policy-operators).

The specification defines a suffixed operator as exactly what the bare
operator means for two operands qualified with that policy. So this pass
writes that definition down instead of adding a second arithmetic: once the
operand carrier `T` is known, `a +% b` becomes

    ((a as T in Wrapping) + (b as T in Wrapping)) as T

and checking, lowering, interpretation and native realization see only the
policy-qualified arithmetic they already implement. An anonymous operand is
left bare so it lands to the carrier like the literal in `counter + 1`; a
shift's count keeps its own type.
*/

use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use symbols::BuiltinTypeAtom;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableCastExpression,
};
use typed_trees::name::Identifier;
use typed_trees::references::DeclarationSelection;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// What one operand contributes to the operation's carrier.
#[derive(Clone, Copy)]
enum Operand {
    /// A bare fixed-width integer type.
    Carrier(TypeReferenceHandle),
    /// An untyped number that lands to the other operand's carrier.
    Anonymous,
}

struct Elaboration {
    expression: ExpressionHandle,
    policy: ArithmeticDomain,
    carrier: TypeReferenceHandle,
    cast_left: bool,
    cast_right: bool,
}

pub(crate) fn elaborate(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    // Most programs write no suffixed operator; one pass over the nodes
    // settles that before any statement is walked.
    if !program
        .expression_table
        .iter_backing_expressions()
        .any(|(_, node)| suffixed(node).is_some())
    {
        return Ok(());
    }
    // Typing stage 03 admits a suffixed operator only in executable
    // statements, so walking those reaches every one this program can
    // evaluate. The node carries its own policy, so a clone or a projection
    // of this program keeps it, and a failed pass leaves it in place for
    // checking to reject; nothing here can turn it into the bare operator.
    let values =
        validation::collect_executable_value_origins(program, &DeclarationSelection::all());
    let mut sites = Vec::new();
    let mut expressions = Vec::new();
    for (_, value) in values.values.iter() {
        let checked_trees::CheckedValueOrigin::StateStatement {
            machine_symbol,
            state_symbol,
            ..
        } = value.origin
        else {
            continue;
        };
        expressions.clear();
        validation::expression_traversal::collect_expression_tree(
            program,
            value.expression,
            &mut expressions,
        );
        sites.extend(
            expressions
                .iter()
                .filter(|expression| {
                    suffixed(program.expression_table.expression(**expression)).is_some()
                })
                .map(|expression| (*expression, machine_symbol, state_symbol)),
        );
    }
    sites.sort_by_key(|(expression, _, _)| expression.arena_index());
    sites.dedup_by_key(|(expression, _, _)| expression.arena_index());

    let mut elaborations = Vec::with_capacity(sites.len());
    let mut diagnostics = Vec::new();
    for (expression, machine_symbol, state_symbol) in sites {
        let Some((machine, state)) = program
            .machine_by_symbol(machine_symbol)
            .zip(program.state_by_symbol(state_symbol))
        else {
            continue;
        };
        match plan(program, machine, state, expression) {
            Ok(elaboration) => elaborations.push(elaboration),
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    for elaboration in elaborations {
        rewrite(program, elaboration);
    }
    Ok(())
}

/// The operation and policy of a node written with a policy suffix.
fn suffixed(node: &ExpressionNode) -> Option<(&TableBinaryExpression, ArithmeticDomain)> {
    match node {
        ExpressionNode::Binary(binary) if binary.policy != ArithmeticDomain::Exact => {
            Some((binary, binary.policy))
        }
        _ => None,
    }
}

fn plan(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> Result<Elaboration, Diagnostic> {
    let span = program.expression_table.source_span(expression);
    let Some((binary, policy)) = suffixed(program.expression_table.expression(expression)) else {
        return Err(
            Diagnostic::error("a policy operator lost its operation before typing")
                .with_source_span(span),
        );
    };
    let spelling = format!(
        "{}{}",
        binary.operator.display_name(),
        policy.operator_suffix()
    );
    let is_shift = matches!(
        binary.operator,
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
    );
    let left = operand(program, machine, state, binary.left, &spelling, "left")
        .map_err(|diagnostic| diagnostic.with_source_span(span))?;
    if is_shift {
        // The count keeps its own type and its policy row; only the shifted
        // value names the carrier.
        let Operand::Carrier(carrier) = left else {
            return Err(Diagnostic::error(format!(
                "`{spelling}` needs an integer-typed value to shift: an anonymous number has no carrier to wrap, clamp or trap at"
            ))
            .with_source_span(span));
        };
        // Both operands of a builtin operator are unqualified, and a count
        // that carries a domain would also change which count law applies.
        match operand(program, machine, state, binary.right, &spelling, "count") {
            Ok(Operand::Carrier(_) | Operand::Anonymous) => {}
            Err(diagnostic) => return Err(diagnostic.with_source_span(span)),
        }
        return Ok(Elaboration {
            expression,
            policy,
            carrier,
            cast_left: true,
            cast_right: false,
        });
    }
    let right = operand(program, machine, state, binary.right, &spelling, "right")
        .map_err(|diagnostic| diagnostic.with_source_span(span))?;
    let (carrier, cast_left, cast_right) = match (left, right) {
        (Operand::Carrier(left), Operand::Carrier(right)) => {
            // `type_symbol` answers the invalid handle for every primitive
            // name, so it cannot tell `u32` from `u64`; the named symbol can.
            let named = |reference| {
                program
                    .type_reference_table
                    .type_reference(reference)
                    .as_named_symbol()
            };
            if named(left) != named(right) {
                return Err(Diagnostic::error(format!(
                    "`{spelling}` needs both operands in one fixed-width integer carrier, found `{}` and `{}`; convert one operand explicitly",
                    program.display_type_reference(left),
                    program.display_type_reference(right),
                ))
                .with_source_span(span));
            }
            (left, true, true)
        }
        (Operand::Carrier(carrier), Operand::Anonymous) => (carrier, true, false),
        (Operand::Anonymous, Operand::Carrier(carrier)) => (carrier, false, true),
        (Operand::Anonymous, Operand::Anonymous) => {
            return Err(Diagnostic::error(format!(
                "`{spelling}` needs at least one integer-typed operand: a wholly anonymous calculation has no carrier to wrap, clamp or trap at"
            ))
            .with_source_span(span));
        }
    };
    Ok(Elaboration {
        expression,
        policy,
        carrier,
        cast_left,
        cast_right,
    })
}

fn operand(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    spelling: &str,
    side: &str,
) -> Result<Operand, Diagnostic> {
    let Some(reference) =
        validation::expression_result_type_reference(program, machine, state, expression)
    else {
        if validation::has_anonymous_numeric_results(program, expression) {
            return Ok(Operand::Anonymous);
        }
        return Err(Diagnostic::error(format!(
            "`{spelling}` could not determine the type of its {side} operand; bind it to an integer-typed local first"
        )));
    };
    let table = &program.type_reference_table;
    match table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. }
            if matches!(
                program.symbols.builtin_type_atom(*symbol),
                Some(
                    BuiltinTypeAtom::I8
                        | BuiltinTypeAtom::I16
                        | BuiltinTypeAtom::I32
                        | BuiltinTypeAtom::I64
                        | BuiltinTypeAtom::U8
                        | BuiltinTypeAtom::U16
                        | BuiltinTypeAtom::U32
                        | BuiltinTypeAtom::U64
                )
            ) =>
        {
            Ok(Operand::Carrier(reference))
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            // Spell the type the way source writes it: `u32 in Km & Wrapping`.
            let domains = table
                .constraints(*constraints)
                .iter()
                .map(|constraint| {
                    let name = constraint.display_name();
                    name.strip_prefix("in ").unwrap_or(&name).to_owned()
                })
                .collect::<Vec<_>>()
                .join(" & ");
            Err(Diagnostic::error(format!(
                "`{spelling}` takes unqualified integer operands, but its {side} operand has type `{} in {domains}`; a suffixed operator neither reads nor removes a domain, so cast the domain away first",
                program.display_type_reference(*base_type),
            )))
        }
        _ => Err(Diagnostic::error(format!(
            "`{spelling}` is a fixed-width integer operation, but its {side} operand has type `{}`",
            program.display_type_reference(reference),
        ))),
    }
}

fn rewrite(program: &mut TypedTrees, elaboration: Elaboration) {
    let Elaboration {
        expression,
        policy,
        carrier,
        cast_left,
        cast_right,
    } = elaboration;
    let Some(binary) = program.expression_table.binary_node(expression).copied() else {
        return;
    };
    retain_policy_type(program, carrier, policy);
    let span = program.expression_table.source_span(expression);
    let label = program.display_type_reference(carrier);
    let cast = |program: &mut TypedTrees, value: ExpressionHandle, domain| {
        let mut target_label = arena::HandleSpan::empty();
        program
            .expression_table
            .push_name_path_member(&mut target_label, Identifier::generated(label.clone()));
        TableCastExpression {
            value,
            target_type: carrier,
            result_type: TypeReferenceHandle::invalid(),
            target_label,
            domain,
            semantic_domain: arena::HandleSpan::empty(),
            semantic_domain_arguments: arena::HandleSpan::empty(),
            semantic_domain_symbol: symbols::SymbolHandle::invalid(),
            semantic_domain_id: language_semantics::SemanticDomainId::NULL,
            form: language_core::cast_form::CastForm::Value,
        }
    };
    let qualify = |program: &mut TypedTrees, value: ExpressionHandle, needed: bool| {
        if !needed {
            return value;
        }
        let node = ExpressionNode::Cast(cast(program, value, policy));
        let qualified = program.expression_table.insert(node);
        program.expression_table.set_source_span(qualified, span);
        qualified
    };
    let left = qualify(program, binary.left, cast_left);
    let right = qualify(program, binary.right, cast_right);
    let operation =
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator: binary.operator,
                right,
                // The casts now say what the suffix said.
                policy: ArithmeticDomain::Exact,
            }));
    program.expression_table.set_source_span(operation, span);
    // The authored operator occurrence follows the operation it selected.
    program
        .expression_table
        .move_authored_selection_occurrences(expression, operation);
    let plain = cast(program, operation, ArithmeticDomain::Exact);
    *program.expression_table.expression_mut(expression) = ExpressionNode::Cast(plain);
}

/// The checker finds a policy-qualified result type by lookup, so the
/// `T in Policy` shell has to exist before the casts that name it.
fn retain_policy_type(
    program: &mut TypedTrees,
    carrier: TypeReferenceHandle,
    policy: ArithmeticDomain,
) {
    if program
        .type_reference_table
        .type_reference(carrier)
        .as_named_symbol()
        .is_some_and(|symbol| {
            program
                .type_reference_table
                .find_arithmetic_result_type_reference(symbol, policy)
                .is_some()
        })
    {
        return;
    }
    let constraints = program
        .type_reference_table
        .insert_constraints([TypeConstraintNode::ArithmeticDomain(policy)]);
    program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: carrier,
            constraints,
        });
}
