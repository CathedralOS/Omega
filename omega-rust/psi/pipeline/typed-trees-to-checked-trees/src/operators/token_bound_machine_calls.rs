//! Bind a selected token-bearing machine to its own checked body.
//!
//! The executable-supply contract in wiki/spec/language/expressions.md says an
//! ordinary direct `machine + Name(...) { ... }` is supplied by its own checked
//! body, through ordinary call machinery, with no operator interpreter and no
//! satisfier search. Operand-directed selection (`resolve_spelling_for_operands`
//! over the typed `machine_token_bindings` views) settles which declaration a
//! spelled use means; this pass turns that settled meaning into the ordinary
//! call the author could have written by hand -- `left + right` becomes a
//! `Call` on `Wrapped::add`'s entry state with the operands as its arguments,
//! in operand order, evaluated once each.
//!
//! It runs at the checked stage rather than at typing because operand types
//! are only settled by the checked value facts; typing rewrites `==` to a
//! written `equals` from declared data alone, which is not enough to choose
//! between overloaded token bindings. It runs before program validation so
//! ownership, effects, termination, contracts, and every executing consumer
//! (interpreter, lowering, Terminal, native) see a plain call edge to the
//! declaration -- the same shape build-time evaluation forms for selected
//! boundary providers (`build-time-evaluation/.../selected_operators.rs`). The
//! rewritten node keeps its authored `Operator` selection occurrence, which
//! finalization settles to the machine symbol
//! (`authored_selections/finalization.rs`), so the checked artifact retains
//! the authored token occurrence together with the exact declaration.
//!
//! Binary and indexed spellings at expression occurrences bind here. `[]`
//! (and `[..]` under the spec's range normalization -- an omitted start means
//! zero, and an inclusive end `..=end` means `end + 1` -- so `a[..b]` and
//! `a[c..=d]` bind exactly like `a[0..b]` and `a[c..d+1]`) supplies the
//! collection place as the call receiver when the entry state's first
//! parameter is `self`, matching the loan a named `collection.at(index)`
//! forms; a token binding whose first operand is an ordinary parameter keeps
//! the operands as ordinary arguments. Open-ended range uses (`a[start..]`,
//! `a[..]`), and any other resolved selection of a token-bearing machine -- a
//! `==` folded into match-arm equality, say -- reject at this pass: leaving
//! the selection fact without a body binding would let a later consumer treat
//! the operand primitives as builtin arithmetic, which the contract forbids.

use checked_trees::{
    CheckedOperatorOccurrence, CheckedOperatorResolutionStatus, CheckedValueOrigin,
};
use diagnostics::Diagnostic;
use numerics::literals::IntegerLiteral;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableCallExpression,
    TableRangeExpression,
};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

use super::expression_type_reference_for_origin;

/// Rewrite every resolved binary use of a token-bearing machine into an
/// ordinary call on that machine's entry state.
pub(crate) fn bind_token_bound_machine_calls(
    program: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    if program.machine_token_bindings().is_empty() {
        return Ok(());
    }
    let mut facts = crate::derive_pre_flow_operator_selections(program);
    // A domain-homed binding (`machine + Quantity::Additive::add`) is a
    // `DomainPending` candidate until binding-site selection settles it. That
    // selection reads only static operand qualifications, mints, and
    // signature `requires` -- never flow facts -- so it can run here, before
    // validation, and an unselected domain meaning keeps the builtin surface
    // exactly as a domain-homed `operator` declaration did.
    crate::operators::select_pending_domain_operator_meanings(program, &mut facts);
    let mut diagnostics = Vec::new();
    let mut bindings: Vec<(ExpressionHandle, SymbolHandle, CheckedValueOrigin)> = Vec::new();
    for operator_use in facts.uses_with_status(CheckedOperatorResolutionStatus::Resolved) {
        let selected = operator_use.selected_operator_symbol;
        if !program
            .machine_token_bindings()
            .iter()
            .any(|view| view.symbol == selected)
        {
            continue;
        }
        let expression = operator_use.expression;
        // One expression can surface under several value origins (a shared
        // template body, for example); its body binding happens once.
        if bindings.iter().any(|(bound, _, _)| *bound == expression) {
            continue;
        }
        let bindable = operator_use.occurrence == CheckedOperatorOccurrence::Expression
            && matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Binary(_) | ExpressionNode::Indexed(_)
            );
        if !bindable {
            diagnostics.push(Diagnostic::error(format!(
                "`{}` was selected for its fixed operator token `{}` in a position whose body supply is not implemented; only binary and indexed expression uses bind the declaration's own body",
                machine_name(program, selected),
                operator_use.spelling.symbol(),
            )));
            continue;
        }
        bindings.push((expression, selected, operator_use.origin));
    }

    for (expression, machine_symbol, origin) in bindings {
        // Copy the operand source handles before any endpoint normalization
        // mutates the expression table below.
        let operand_source = match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => OperandSource::Pair([binary.left, binary.right]),
            ExpressionNode::Indexed(indexed) => OperandSource::Indexed {
                collection: indexed.collection,
                index: indexed.index,
            },
            _ => unreachable!("binding candidates are binary or indexed expressions"),
        };
        let Some(machine) = crate::lookup::machine_by_symbol(program, machine_symbol) else {
            diagnostics.push(Diagnostic::error(
                "a selected token-bearing machine lost its typed declaration before body binding",
            ));
            continue;
        };
        let Some(entry) = program.machine_states(machine).first() else {
            diagnostics.push(Diagnostic::error(format!(
                "`{}` was selected for its fixed operator token but declares no entry state",
                machine.name
            )));
            continue;
        };
        let (entry_symbol, target) = (entry.symbol, machine.name.clone());
        // Copy the signature views the rewrite needs before endpoint
        // normalization mutates the expression table.
        let first_is_self = program
            .state_parameters(entry)
            .first()
            .is_some_and(|parameter| parameter.is_self);
        // `[..]` telescopes are exactly (collection, start, end); the range
        // endpoints supply operands one and two after the spec's
        // normalization.
        let start_parameter = program
            .state_parameters(entry)
            .get(1)
            .map(|parameter| parameter.type_reference);
        let operands = match operand_source {
            OperandSource::Pair(pair) => pair.to_vec(),
            OperandSource::Indexed { collection, index } => {
                let mut operands = vec![collection];
                let range = match program.expression_table.expression(index) {
                    ExpressionNode::Range(range) => Some(*range),
                    _ => None,
                };
                match range {
                    Some(range) => {
                        match range_operands(program, &target, range, start_parameter, origin) {
                            Ok((start, end)) => {
                                operands.push(start);
                                operands.push(end);
                            }
                            Err(diagnostic) => {
                                diagnostics.push(diagnostic);
                                continue;
                            }
                        }
                    }
                    None => operands.push(index),
                }
                operands
            }
        };
        // A `self` first operand is the receiver: the collection place takes
        // the same loan a named `collection.at(index)` call forms. Otherwise
        // operand zero is an ordinary argument like any binary operand.
        let (receiver, argument_handles) = if first_is_self {
            (operands[0], &operands[1..])
        } else {
            (ExpressionHandle::invalid(), &operands[..])
        };
        let arguments = program
            .expression_table
            .insert_expression_handles(argument_handles.iter().copied());
        *program.expression_table.expression_mut(expression) =
            ExpressionNode::Call(TableCallExpression {
                receiver,
                target_symbol: entry_symbol,
                static_machine_parameter: SymbolHandle::invalid(),
                target,
                static_requirement_dispatch: None,
                machine_arguments: Box::default(),
                quotient_operation: None,
                private_layout_operation: None,
                arguments,
                evidence_arguments: Box::default(),
                operational_acknowledgement: language_semantics::CallOperationalAcknowledgement {
                    origin: language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
                    ..Default::default()
                },
            });
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// The token-bearing machine whose entry state a compiler-synthesized call
/// targets, if the call is one bound by [`bind_token_bound_machine_calls`].
/// Finalization uses it to settle the retained `Operator` occurrence on the
/// rewritten node to the exact declaration.
pub(crate) fn token_bound_machine_call_target(
    program: &TypedTrees,
    call: &TableCallExpression,
) -> Option<SymbolHandle> {
    if call.operational_acknowledgement.origin
        != language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized
        || !call.target_symbol.is_valid()
    {
        return None;
    }
    program
        .machines()
        .iter()
        .filter(|machine| machine.spelling.is_some())
        .find(|machine| {
            program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == call.target_symbol)
        })
        .map(|machine| machine.symbol)
}

fn machine_name(program: &TypedTrees, symbol: SymbolHandle) -> String {
    crate::lookup::machine_by_symbol(program, symbol).map_or_else(
        || "<unknown machine>".to_owned(),
        |machine| machine.name.to_string(),
    )
}

/// Operand source positions of a token-bound expression, copied out before
/// range endpoint normalization mutates the expression table.
enum OperandSource {
    Pair([ExpressionHandle; 2]),
    Indexed {
        collection: ExpressionHandle,
        index: ExpressionHandle,
    },
}

/// The operand pair an authored range supplies to a `[..]` binding's start and
/// end parameters, after the normalization the spec assigns to `[..]`: an
/// omitted start means zero, and an inclusive end `start..=end` means
/// `start..(end + 1)`.
///
/// Both normalized forms are gated on integer operands. Literal zero needs a
/// declared integer start parameter to land on. The synthesized `end + 1` must
/// stay builtin arithmetic, and a bare integer primitive can never match a
/// direct token binding -- the semantic-home law keeps compiler primitives out
/// of every declared operator family -- so an integer end operand guarantees
/// the inserted `+` means the builtin one. An omitted end (`a[start..]`,
/// `a[..]`) would need the collection's length, which a `[..]` telescope has
/// no operand formation for, so those uses still reject.
fn range_operands(
    program: &mut TypedTrees,
    machine_name: &str,
    range: TableRangeExpression,
    start_parameter: Option<TypeReferenceHandle>,
    origin: CheckedValueOrigin,
) -> Result<(ExpressionHandle, ExpressionHandle), Diagnostic> {
    if !range.end.is_valid() {
        return Err(Diagnostic::error(format!(
            "`{machine_name}` was selected for its fixed operator token `[..]` but the range use omits its end; the omitted endpoint would be the collection's length, which a declared telescope cannot form -- closed `start..end`, `..end`, `start..=end`, and `..=end` uses bind the declaration's own body",
        )));
    }
    let start = if range.start.is_valid() {
        range.start
    } else {
        let start_accepts_zero = start_parameter
            .and_then(|type_reference| program.primitive_type_reference(type_reference))
            .is_some_and(PrimitiveType::accepts_integer_literal);
        if !start_accepts_zero {
            return Err(Diagnostic::error(format!(
                "`{machine_name}` was selected for its fixed operator token `[..]` but the range use omits its start; the omitted endpoint is zero, which only an integer start parameter forms -- write it explicitly as `0..`",
            )));
        }
        program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)))
    };
    if !range.end_inclusive {
        return Ok((start, range.end));
    }
    let end_is_integer = expression_type_reference_for_origin(program, range.end, origin)
        .and_then(|type_reference| validation::unwrapped_type_reference(program, type_reference))
        .and_then(|type_reference| program.primitive_type_reference(type_reference))
        .is_some_and(PrimitiveType::accepts_integer_literal);
    if !end_is_integer {
        return Err(Diagnostic::error(format!(
            "`{machine_name}` was selected for its fixed operator token `[..]` but the range use is inclusive (`..=`); the normalized `end + 1` form requires an integer end operand -- write the exclusive bound explicitly",
        )));
    }
    let one = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
    let end = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: range.end,
            operator: BinaryOperator::Add,
            right: one,
        }));
    Ok((start, end))
}

#[cfg(test)]
mod tests {
    use diagnostics::Diagnostic;
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
    use tokens_to_syntax_trees::parse_syntax_trees;
    use typed_trees::expression::{BinaryOperator, ExpressionNode, TableCallExpression};
    use typed_trees::statement::StatementNode;

    use crate::CheckingRequest;
    use crate::lower_typed_trees;

    fn check_source(source: &str) -> Result<checked_trees::CheckedTrees, Vec<Diagnostic>> {
        let tokens = Lexer::new(source).tokenize().unwrap();
        let syntax = parse_syntax_trees(&tokens).unwrap();
        let resolved = resolve(ResolutionRequest::new(&syntax)).unwrap();
        let typed = lower_symbol_resolved_trees(&resolved).unwrap();
        lower_typed_trees(typed, &CheckingRequest::settled())
    }

    fn source_with_use(index: &str) -> String {
        // `high`'s `requires` bound keeps the synthesized `end + 1` provably
        // exact -- the spec makes an overflowing endpoint calculation a proof
        // error, so the bound is part of the test's meaning, not decoration.
        format!(
            "data Buffer {{ value: u64; }} machine [..] Buffer::window(&self, start: u64, end: u64) -> u64 {{ end }} machine run(buffer: Buffer, low: u64, high: u64) -> u64 requires high <= 100 {{ buffer[{index}] }}"
        )
    }

    /// The `Call` `run`'s tail expression was rewritten into, plus the checked
    /// program the call's handles index.
    fn run_tail_call(source: &str) -> (checked_trees::CheckedTrees, TableCallExpression) {
        let checked = check_source(source).expect("the range use binds the machine body");
        let run = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "run")
            .expect("the source declares machine run");
        let entry = &checked.machine_states(run)[0];
        let StatementNode::Expression(handle) = *checked
            .statement_table
            .statements(entry.statement_nodes)
            .last()
            .expect("run's entry state ends in a statement")
        else {
            panic!("run's tail statement is an expression");
        };
        let ExpressionNode::Call(call) = checked.expression_table.expression(handle).clone() else {
            panic!("the rewritten tail expression is an ordinary call");
        };
        (checked, call)
    }

    fn integer_argument(
        checked: &checked_trees::CheckedTrees,
        handle: typed_trees::expression::ExpressionHandle,
    ) -> i64 {
        let ExpressionNode::Integer(literal) = checked.expression_table.expression(handle) else {
            panic!("the operand is the synthesized integer literal");
        };
        literal.value_i64().expect("the literal fits i64")
    }

    #[test]
    fn closed_range_binds_the_authored_bounds() {
        let (checked, call) = run_tail_call(&source_with_use("low..high"));
        assert!(call.receiver.is_valid());
        let arguments = checked.expression_table.expression_handles(call.arguments);
        assert_eq!(arguments.len(), 2);
        assert!(matches!(
            checked.expression_table.expression(arguments[0]),
            ExpressionNode::Name(_)
        ));
        assert!(matches!(
            checked.expression_table.expression(arguments[1]),
            ExpressionNode::Name(_)
        ));
    }

    #[test]
    fn omitted_range_start_supplies_zero() {
        let (checked, call) = run_tail_call(&source_with_use("..high"));
        let arguments = checked.expression_table.expression_handles(call.arguments);
        assert_eq!(arguments.len(), 2);
        assert_eq!(integer_argument(&checked, arguments[0]), 0);
        assert!(matches!(
            checked.expression_table.expression(arguments[1]),
            ExpressionNode::Name(_)
        ));
    }

    #[test]
    fn inclusive_range_end_supplies_end_plus_one() {
        for index in ["low..=high", "..=high"] {
            let (checked, call) = run_tail_call(&source_with_use(index));
            let arguments = checked.expression_table.expression_handles(call.arguments);
            assert_eq!(arguments.len(), 2);
            let ExpressionNode::Binary(end) = checked.expression_table.expression(arguments[1])
            else {
                panic!("an inclusive end supplies its `end + 1` normalization");
            };
            assert_eq!(end.operator, BinaryOperator::Add);
            assert!(matches!(
                checked.expression_table.expression(end.left),
                ExpressionNode::Name(_)
            ));
            assert_eq!(integer_argument(&checked, end.right), 1);
        }
    }

    #[test]
    fn omitted_range_start_and_inclusive_end_supply_zero_and_end_plus_one() {
        let (checked, call) = run_tail_call(&source_with_use("..=high"));
        let arguments = checked.expression_table.expression_handles(call.arguments);
        assert_eq!(arguments.len(), 2);
        assert_eq!(integer_argument(&checked, arguments[0]), 0);
        assert!(matches!(
            checked.expression_table.expression(arguments[1]),
            ExpressionNode::Binary(_)
        ));
    }

    #[test]
    fn open_ended_range_still_rejects() {
        for index in ["low..", ".."] {
            let diagnostics = match check_source(&source_with_use(index)) {
                Ok(_) => panic!(
                    "an omitted end needs the collection's length, which a `[..]` telescope cannot form"
                ),
                Err(diagnostics) => diagnostics,
            };
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("omits its end")),
                "{diagnostics:#?}"
            );
        }
    }

    #[test]
    fn omitted_start_on_a_non_integer_parameter_rejects() {
        let source = r#"
            data Buffer { value: u64; }
            data Index { raw: u64; }
            machine [..] Buffer::window(&self, start: Index, end: u64) -> u64 { end }
            machine run(buffer: Buffer, high: u64) -> u64 { buffer[..high] }
        "#;
        let diagnostics = match check_source(source) {
            Ok(_) => panic!("zero has no formation for a non-integer start parameter"),
            Err(diagnostics) => diagnostics,
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("omits its start")),
            "{diagnostics:#?}"
        );
    }

    #[test]
    fn inclusive_end_on_a_non_integer_operand_rejects() {
        let source = r#"
            data Buffer { value: u64; }
            data Index { raw: u64; }
            machine [..] Buffer::window(&self, start: u64, end: Index) -> u64 { start }
            machine run(buffer: Buffer, low: u64, high: Index) -> u64 { buffer[low..=high] }
        "#;
        let diagnostics = match check_source(source) {
            Ok(_) => panic!("`end + 1` has no integer formation for a data end operand"),
            Err(diagnostics) => diagnostics,
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("integer end operand")),
            "{diagnostics:#?}"
        );
    }
}
