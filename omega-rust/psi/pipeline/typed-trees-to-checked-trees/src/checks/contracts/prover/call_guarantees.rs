//! A call guarantee and a local result binding have separate lifetimes. Join
//! the surviving guarantee to surviving AssignedValue provenance, then compare
//! predicates under exact declaration/call substitution. Never replay a local
//! initializer or identify repeated calls by their printed arguments.
//! Machine entries and requirement signatures share this substitution, but
//! `callable` retains their distinct declaration-owned parameter/result scopes.
//! A public requirement guarantee is not a proof of its provider's body.
//!
//! This consumes the existing flow evidence rather than copying predicates at
//! each assignment. Storage dependencies retire changed inputs; assignment
//! provenance retires changed results. Invocation operands must be places or
//! literals (including constructor fields) and preserve their read footprints:
//! expression-root dependencies alone do
//! not establish that a computed actual still denotes its captured value.

use crate::flow::CanonicalPlace;
use crate::semantic_calls::CallSite;
use checked_trees::{CheckedOperatorFacts, FlowCallFact, FlowStateFact};
use facts::{FactPayload, FactPlace, FactPlan, PlaceRoot};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;

pub(in crate::checks) mod arithmetic;
pub(in crate::checks) mod callable;
use callable::Callable;
mod availability;
pub(in crate::checks) use availability::{AvailableGuarantee, available};
#[cfg(test)]
mod tests;

struct Invocation<'program> {
    site: CallSite<'program>,
    callable: Callable<'program>,
    caller_state: SymbolHandle,
    statement: usize,
    ordinal: usize,
}

pub(super) fn proves(
    program: &TypedTrees,
    facts: &checked_trees::CheckFacts,
    caller: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[facts::FactContextHandle],
    expression: ExpressionHandle,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let operators = &facts.operators;
    let semantic = &facts.semantic;
    let Some(required) = invocation(program, caller, call.statement_index, call.call_ordinal)
    else {
        return false;
    };
    let Some(frames) = frames else {
        return false;
    };
    if !stable_arguments(program, &required)
        || !builtin_predicate(program, operators, &required, expression)
    {
        return false;
    }
    let mut arithmetic_hypotheses = Vec::new();
    let exact = available(
        program,
        facts,
        caller,
        call.statement_index,
        contexts,
        frames,
    )
    .into_iter()
    .any(|guarantee| {
        if let Some(proposition) = arithmetic::at_call(
            program,
            facts,
            caller,
            &guarantee.invocation,
            guarantee.expression,
            frames,
        ) {
            arithmetic_hypotheses.push(validation::ScopedArithmeticHypothesis {
                proposition,
                holds: true,
            });
        }
        predicates_match(
            program,
            semantic,
            contexts,
            &required,
            expression,
            &guarantee.invocation,
            guarantee.expression,
            &mut Vec::new(),
        )
    });
    exact
        || arithmetic::proves(
            program,
            facts,
            caller,
            contexts,
            &required,
            expression,
            arithmetic_hypotheses,
            frames,
        )
}

fn capture_preserved(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    caller: &Machine,
    supplied: &Invocation<'_>,
    produced: ExpressionHandle,
    guarantee: ExpressionHandle,
    frames: &validation::CallFrameResolver<'_>,
) -> bool {
    let frame = frames.expression_write_frame(caller, produced);
    let Some(mut writes) = crate::flow::frame_storage_writes(
        program,
        caller.symbol,
        supplied.caller_state,
        supplied.statement,
        &frame,
        Some(frames),
    ) else {
        return false;
    };
    let Some(state) = borrow.states.iter().map(|(_, state)| state).find(|state| {
        state.machine_symbol == caller.symbol && state.state_symbol == supplied.caller_state
    }) else {
        return false;
    };
    let Some(call) = borrow.calls.span_or_empty(state.calls).iter().find(|call| {
        call.statement_index == supplied.statement
            && call.call_ordinal == supplied.ordinal
            && (call.target_symbol == supplied.callable.target_symbol()
                || call.target_symbol == supplied.callable.owner_symbol())
    }) else {
        return false;
    };
    // Ordinary body frames do not yet summarize every selected operator.
    // Reuse the independent signature/authority ceiling instead of trusting
    // an omitted dispatch or introducing another transitive body scanner.
    let Some(ceiling) = crate::flow::signature_ceiling_places(
        program,
        caller.symbol,
        supplied.caller_state,
        call,
        Some(frames),
    ) else {
        return false;
    };
    writes.extend(ceiling);
    let mut occurrences = Vec::new();
    crate::facts::contract_occurrences::append_expression_occurrences(
        program,
        guarantee,
        &mut occurrences,
    );
    occurrences.into_iter().all(|occurrence| {
        if validation::reserved_result_place(program, occurrence)
            .is_some_and(|result| result.machine_symbol == supplied.callable.owner_symbol())
            || bound_literal(program, supplied, occurrence).is_some()
        {
            return true;
        }
        let Some(place) = bound_place(program, supplied, occurrence) else {
            return false;
        };
        let Some(place) = crate::flow::rebase_exact_local_place(
            program,
            supplied.caller_state,
            supplied.statement,
            place,
            Some(frames),
        ) else {
            return false;
        };
        !writes.iter().any(|write| {
            crate::flow::normalized_event_place_root(program, write.root)
                == crate::flow::normalized_event_place_root(program, place.root)
                && crate::flow::canonical_place_segments_may_overlap(
                    program,
                    &write.segments,
                    &place.segments,
                )
        })
    })
}

fn invocation<'program>(
    program: &'program TypedTrees,
    caller: &FlowStateFact,
    statement: usize,
    ordinal: usize,
) -> Option<Invocation<'program>> {
    let site = crate::semantic_calls::find_call_site(
        program,
        caller.machine_symbol,
        caller.state_symbol,
        statement,
        ordinal,
    )?;
    let target = match &site {
        CallSite::Expression { call, .. } => call.target_symbol,
        CallSite::Statement(call) => call.target_symbol,
        CallSite::TransitionNamed { .. } => return None,
    };
    let callable = Callable::resolve(program, target)?;
    Some(Invocation {
        site,
        callable,
        caller_state: caller.state_symbol,
        statement,
        ordinal,
    })
}

fn stable_arguments(program: &TypedTrees, invocation: &Invocation<'_>) -> bool {
    let parameters = invocation.callable.parameters(program);
    let arguments =
        crate::semantic_calls::call_site_argument_expressions(program, &invocation.site);
    // Receiver, static and evidence substitution retain their own owners.
    let ordinary = match &invocation.site {
        CallSite::Expression { call, .. } => {
            call.machine_arguments.is_empty()
                && call.evidence_arguments.is_empty()
                && call.static_requirement_dispatch.is_none()
        }
        CallSite::Statement(call) => {
            call.machine_arguments.is_empty()
                && call.evidence_arguments.is_empty()
                && call.static_requirement_dispatch.is_none()
        }
        _ => false,
    };
    ordinary
        && parameters.len() == arguments.len()
        && parameters
            .iter()
            .all(|parameter| !parameter.is_self && !parameter.is_const)
        && arguments
            .iter()
            .all(|argument| stable_value(program, *argument, &mut Vec::new()))
}

fn stable_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    pending: &mut Vec<ExpressionHandle>,
) -> bool {
    if pending.contains(&expression) {
        return false;
    }
    pending.push(expression);
    let stable = if let ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
    {
        program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .all(|field| stable_value(program, field.value, pending))
    } else {
        super::scalars::literal(program, expression).is_some()
            || direct_place(program, expression)
                .is_some_and(|place| matches!(place.root, PlaceRoot::Symbol(_)))
    };
    pending.pop();
    stable
}

pub(in crate::checks) fn direct_place(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    let mut current = expression;
    let mut visited = Vec::new();
    loop {
        if visited.contains(&current) {
            return None;
        }
        visited.push(current);
        current = match program.expression_table.expression(current) {
            ExpressionNode::Name(_) => break,
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Borrow(borrow) => borrow.target,
            // Indexing needs its own selected-operation and selector-capture
            // evidence. Canonical storage spelling alone supplies neither.
            _ => return None,
        };
    }
    super::field_actuals::checked_place(program, expression)
}

fn builtin_predicate(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
) -> bool {
    super::has_builtin_operators(program, operators, expression)
        && invocation.callable.builtin_meaning(program, expression)
}

fn predicates_match(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &[facts::FactContextHandle],
    required: &Invocation<'_>,
    left: ExpressionHandle,
    supplied: &Invocation<'_>,
    right: ExpressionHandle,
    pending: &mut Vec<(ExpressionHandle, ExpressionHandle)>,
) -> bool {
    if pending.contains(&(left, right)) {
        return false;
    }
    pending.push((left, right));
    let matched = match (
        program.expression_table.expression(left),
        program.expression_table.expression(right),
    ) {
        (ExpressionNode::Binary(left_binary), ExpressionNode::Binary(right_binary)) => {
            left_binary.operator == right_binary.operator
                && same_scalar_type(
                    program,
                    required,
                    left_binary.left,
                    supplied,
                    right_binary.left,
                )
                && same_scalar_type(
                    program,
                    required,
                    left_binary.right,
                    supplied,
                    right_binary.right,
                )
                && predicates_match(
                    program,
                    semantic,
                    contexts,
                    required,
                    left_binary.left,
                    supplied,
                    right_binary.left,
                    pending,
                )
                && predicates_match(
                    program,
                    semantic,
                    contexts,
                    required,
                    left_binary.right,
                    supplied,
                    right_binary.right,
                    pending,
                )
        }
        (ExpressionNode::Unary(left_unary), ExpressionNode::Unary(right_unary)) => {
            left_unary.operator == right_unary.operator
                && same_scalar_type(
                    program,
                    required,
                    left_unary.operand,
                    supplied,
                    right_unary.operand,
                )
                && predicates_match(
                    program,
                    semantic,
                    contexts,
                    required,
                    left_unary.operand,
                    supplied,
                    right_unary.operand,
                    pending,
                )
        }
        (ExpressionNode::Cast(left_cast), ExpressionNode::Cast(right_cast)) => {
            same_scalar_type(program, required, left, supplied, right)
                && predicates_match(
                    program,
                    semantic,
                    contexts,
                    required,
                    left_cast.value,
                    supplied,
                    right_cast.value,
                    pending,
                )
        }
        _ => {
            match (
                bound_literal(program, required, left),
                bound_literal(program, supplied, right),
            ) {
                (Some(left), Some(right)) => left == right,
                _ => bound_place(program, required, left)
                    .zip(bound_place(program, supplied, right))
                    .is_some_and(|(left, right)| {
                        captured_place(program, semantic, contexts, left)
                            .zip(captured_place(program, semantic, contexts, right))
                            .is_some_and(|(left, right)| left == right)
                    }),
            }
        }
    };
    pending.pop();
    matched
}

fn same_scalar_type(
    program: &TypedTrees,
    left_owner: &Invocation<'_>,
    left: ExpressionHandle,
    right_owner: &Invocation<'_>,
    right: ExpressionHandle,
) -> bool {
    let reference =
        |owner: &Invocation<'_>, expression| owner.callable.scalar_reference(program, expression);
    match (reference(left_owner, left), reference(right_owner, right)) {
        (Some(left), Some(right)) => {
            program.primitive_type_reference(left).is_some()
                && program.primitive_type_reference(left) == program.primitive_type_reference(right)
                && program.arithmetic_domain_for_type_reference(left)
                    == program.arithmetic_domain_for_type_reference(right)
        }
        _ => false,
    }
}

fn actual_projection(
    program: &TypedTrees,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, Vec<facts::PlaceSegment>)> {
    direct_place(program, expression)?;
    crate::semantic_places::call_contract_argument_projection(
        program,
        invocation.callable.parameters(program),
        crate::semantic_calls::call_site_argument_expressions(program, &invocation.site),
        expression,
    )
}

fn bound_literal(
    program: &TypedTrees,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
) -> Option<facts::ScalarValue> {
    super::scalars::literal(program, expression).or_else(|| {
        let (value, remaining) = actual_projection(program, invocation, expression)?;
        if !remaining.is_empty() {
            return None;
        }
        super::scalars::literal(program, value)
    })
}

fn bound_place(
    program: &TypedTrees,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    if let Some(result) = validation::reserved_result_place(program, expression) {
        if result.machine_symbol != invocation.callable.owner_symbol() {
            return None;
        }
        let CallSite::Expression { expression, .. } = invocation.site else {
            return None;
        };
        return Some(CanonicalPlace {
            root: PlaceRoot::Expression(expression),
            segments: result.segments,
        });
    }
    let (actual, remaining) = actual_projection(program, invocation, expression)?;
    let mut place = crate::flow::canonical_place_from_expression_in_state(
        program,
        invocation.caller_state,
        invocation.statement,
        actual,
    )?;
    place.segments.extend(remaining);
    Some(place)
}

fn captured_place(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &[facts::FactContextHandle],
    mut place: CanonicalPlace,
) -> Option<CanonicalPlace> {
    // Flow copies this provenance with a whole-value copy and removes it on
    // overlapping writes. Its current statement need not be the original call.
    let mut source = None;
    let mut non_call_value = false;
    for fact in contexts.iter().flat_map(|context| {
        semantic
            .context_view(semantic.contexts.get(*context))
            .facts()
    }) {
        if !matches!(
            fact.payload,
            FactPayload::AssignedValue { .. } | FactPayload::AssignedScalarValue { .. }
        ) {
            continue;
        }
        let FactPlace::Place(destination) = fact.place else {
            continue;
        };
        let destination = semantic.places.get(destination);
        if destination.root != place.root || !destination.segments.is_empty() {
            continue;
        }
        match fact.payload {
            FactPayload::AssignedValue { value }
                if matches!(
                    program.expression_table.expression(value),
                    ExpressionNode::Call(_)
                ) =>
            {
                if source.is_some_and(|prior| prior != value) {
                    return None;
                }
                source = Some(value);
            }
            _ => non_call_value = true,
        }
    }
    if let Some(source) = source {
        if non_call_value {
            return None;
        }
        place.root = PlaceRoot::Expression(source);
    }
    Some(place)
}

#[cfg(test)]
mod prerequisite_roster_probes {
    //! Adversarial probes for the prerequisite roster `proves` runs before a
    //! call guarantee may stand in for a proven expression: the call must
    //! resolve to a real expression or statement site (`invocation`), every
    //! actual must be a stable place or literal (`stable_arguments` /
    //! `stable_value`), and only a direct place spelling earns the
    //! `PlaceRoot::Symbol` capture a bound-place join can consume
    //! (`direct_place`). Each pin names the roster member it exercises.

    use super::{Invocation, direct_place, invocation, stable_arguments, stable_value};
    use crate::semantic_calls::{CallSite, call_site_argument_expressions};
    use checked_trees::FlowStateFact;
    use facts::PlaceRoot;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::state::State;
    use typed_trees::statement::StatementNode;

    const SOURCE: &str = r#"
        data Pair {
            left: u64;
            right: u64;
        }

        data Main {
            slot: u64;
            cells: [u64; 4];
            flag: bool;
        }

        machine produce(v: u64) -> u64 {
            transition {
                _ -> v
            }
        }

        machine consume(p: Pair) -> u64 {
            transition {
                _ -> p.left
            }
        }

        machine Main::main(&mut self, k: u64) -> u64 {
            let own: Pair = Pair { left: k, right: 7 };
            let a: u64 = produce(k);
            let b: u64 = produce(k + 1);
            let c: u64 = produce(7);
            let d: u64 = produce(own.left);
            let e: u64 = produce(self.cells[0]);
            let f: u64 = consume(Pair { left: k, right: 7 });
            let g: u64 = consume(Pair { left: produce(k), right: 7 });
            let r: &u64 = &k;
            transition {
                _ -> self.done(a)
            }
            state done(&mut self, v: u64) -> u64 {
                transition {
                    _ -> v
                }
            }
        }
    "#;

    fn program() -> typed_trees::TypedTrees {
        let tokens = source_files_to_tokens::Lexer::new(SOURCE)
            .tokenize()
            .expect("tokenize");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("resolve");
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
    }

    /// Machine names keep their qualified diagnostic spelling
    /// (`Main::main`); a leaf name matches its final member.
    fn machine_named(program: &typed_trees::TypedTrees, name: &str) -> SymbolHandle {
        program
            .machines()
            .iter()
            .find(|machine| {
                let spelled = machine.name.as_str();
                spelled == name || spelled.ends_with(&format!("::{name}"))
            })
            .unwrap_or_else(|| panic!("machine {name}"))
            .symbol
    }

    fn machine_state<'program>(
        program: &'program typed_trees::TypedTrees,
        machine_name: &str,
        state_name: &str,
    ) -> (SymbolHandle, &'program State) {
        let machine = program
            .machines()
            .iter()
            .find(|machine| {
                let spelled = machine.name.as_str();
                spelled == machine_name || spelled.ends_with(&format!("::{machine_name}"))
            })
            .unwrap_or_else(|| panic!("machine {machine_name}"));
        let state = program
            .machine_states(machine)
            .iter()
            .find(|state| {
                let spelled = state.name.as_str();
                spelled == state_name || spelled.ends_with(&format!("::{state_name}"))
            })
            .unwrap_or_else(|| panic!("state {state_name}"));
        (machine.symbol, state)
    }

    fn invocation_at<'program>(
        program: &'program typed_trees::TypedTrees,
        statement: usize,
        ordinal: usize,
    ) -> Option<Invocation<'program>> {
        let (machine_symbol, state) = machine_state(program, "main", "main");
        let caller = FlowStateFact {
            machine_symbol,
            state_symbol: state.symbol,
            ..Default::default()
        };
        invocation(program, &caller, statement, ordinal)
    }

    fn statement(program: &typed_trees::TypedTrees, index: usize) -> &StatementNode {
        let (_, state) = machine_state(program, "main", "main");
        &program.statement_table.statements(state.statement_nodes)[index]
    }

    fn initializer(program: &typed_trees::TypedTrees, index: usize) -> ExpressionHandle {
        let StatementNode::LocalData(local) = statement(program, index) else {
            panic!("statement {index} is not a local binding");
        };
        local.initial_value
    }

    /// The sole actual of the expression call at `statement`/`ordinal`.
    fn only_argument(
        program: &typed_trees::TypedTrees,
        statement: usize,
        ordinal: usize,
    ) -> ExpressionHandle {
        let invocation = invocation_at(program, statement, ordinal).expect("call site");
        let arguments = call_site_argument_expressions(program, &invocation.site);
        assert_eq!(arguments.len(), 1, "fixture calls carry one actual");
        arguments[0]
    }

    #[test]
    fn invocation_resolves_only_ordinary_call_sites() {
        let program = program();
        let resolved = invocation_at(&program, 1, 0).expect("`produce(k)` site");
        assert!(matches!(resolved.site, CallSite::Expression { .. }));
        let produce = machine_named(&program, "produce");
        assert_eq!(
            resolved.callable.target_symbol(),
            program.machine_states(
                program
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == produce)
                    .expect("produce machine")
            )[0]
            .symbol
        );
        // `let r: &u64 = &k` is call-free: there is no site to bind.
        assert!(invocation_at(&program, 8, 0).is_none());
        // `_ -> self.done(a)` resolves to a named-transition site, which the
        // roster deliberately refuses: its parameters substitute positionally
        // through the transition edge, not through this ordinary binding.
        assert!(invocation_at(&program, 9, 0).is_none());
    }

    #[test]
    fn stable_arguments_admits_place_literal_and_member_actuals() {
        let program = program();
        for (statement, what) in [
            (1usize, "place actual `k`"),
            (3, "literal actual `7`"),
            (4, "member actual `own.left`"),
        ] {
            let invocation = invocation_at(&program, statement, 0).expect("call site");
            assert!(
                stable_arguments(&program, &invocation),
                "{what} must be admitted as stable"
            );
        }
    }

    #[test]
    fn stable_arguments_retires_computed_and_indexed_actuals() {
        let program = program();
        for (statement, what) in [
            (2usize, "computed actual `k + 1`"),
            (5, "indexed actual `self.cells[0]`"),
        ] {
            let invocation = invocation_at(&program, statement, 0).expect("call site");
            assert!(
                !stable_arguments(&program, &invocation),
                "{what} must not be admitted as stable"
            );
        }
    }

    #[test]
    fn stable_arguments_inspects_every_struct_literal_field() {
        let program = program();
        let stable = invocation_at(&program, 6, 0).expect("`consume(Pair{k, 7})` site");
        assert!(
            stable_arguments(&program, &stable),
            "a literal of stable fields must be admitted"
        );
        let nested_call =
            invocation_at(&program, 7, 0).expect("`consume(Pair{produce(k), 7})` site");
        assert!(
            !stable_arguments(&program, &nested_call),
            "a struct literal holding a call actual must retire"
        );
    }

    #[test]
    fn stable_value_admits_borrows_and_composites_of_stable_operands() {
        let program = program();
        // `let r: &u64 = &k` — a borrow of a direct place stays a place.
        let borrow = initializer(&program, 8);
        assert!(
            matches!(
                program.expression_table.expression(borrow),
                ExpressionNode::Borrow(_)
            ),
            "fixture keeps the borrow spelling"
        );
        assert!(stable_value(&program, borrow, &mut Vec::new()));
        // `Pair { left: k, right: 7 }` — every field stable.
        let literal = only_argument(&program, 6, 0);
        assert!(matches!(
            program.expression_table.expression(literal),
            ExpressionNode::StructLiteral(_)
        ));
        assert!(stable_value(&program, literal, &mut Vec::new()));
    }

    #[test]
    fn stable_value_retires_calls_and_computed_expressions() {
        let program = program();
        // The call result itself is a value, never a place or literal.
        let call = initializer(&program, 1);
        assert!(!stable_value(&program, call, &mut Vec::new()));
        // `k + 1` computes; the roster admits no evaluated operand.
        let computed = only_argument(&program, 2, 0);
        assert!(matches!(
            program.expression_table.expression(computed),
            ExpressionNode::Binary(_)
        ));
        assert!(!stable_value(&program, computed, &mut Vec::new()));
        // A struct literal carrying a call field retires wholesale.
        let mixed = only_argument(&program, 7, 0);
        assert!(matches!(
            program.expression_table.expression(mixed),
            ExpressionNode::StructLiteral(_)
        ));
        assert!(!stable_value(&program, mixed, &mut Vec::new()));
    }

    #[test]
    fn direct_place_names_only_member_and_borrow_roots() {
        let program = program();
        for (statement, what) in [(1usize, "plain name `k`"), (4, "member chain `own.left`")] {
            let place = direct_place(&program, only_argument(&program, statement, 0))
                .unwrap_or_else(|| panic!("{what} must resolve to a place"));
            assert!(
                matches!(place.root, PlaceRoot::Symbol(_)),
                "{what} must keep a symbol root"
            );
        }
        let borrow = initializer(&program, 8);
        let place = direct_place(&program, borrow).expect("`&k` unwraps to the borrowed place");
        assert!(matches!(place.root, PlaceRoot::Symbol(_)));
        for (statement, what) in [
            (2usize, "computed actual `k + 1`"),
            (5, "indexed actual `self.cells[0]`"),
        ] {
            assert!(
                direct_place(&program, only_argument(&program, statement, 0)).is_none(),
                "{what} must not resolve to a direct place"
            );
        }
        let call = initializer(&program, 1);
        assert!(direct_place(&program, call).is_none());
    }
}
