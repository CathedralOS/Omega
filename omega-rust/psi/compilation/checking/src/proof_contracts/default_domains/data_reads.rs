//! Scanning statement and expression reads and validating each data read
//! against established places and range gates.
//!
//! An input parameter arrives as a value, not a fresh zero representation.
//! Nominal input checking proves its declared domains at the call boundary;
//! entry provisioning independently refuses a non-ZII-valid context. Neither
//! judgment depends on the callee's qualified name. Retain resolved parameter
//! identity through a projected read so qualification cannot reclassify that
//! incoming value as vacant storage. Subsequent invariant windows still police
//! every affected read, independently of entry validity.

use crate::proof_contracts::default_domains::place_queries::{
    data_definition_for_expression, declared_root_established, expression_root_symbol,
    place_spelling_covers, self_place_spelling, transition_evaluated_expressions,
    write_place_spelling, write_target_index_expressions,
};
use crate::proof_contracts::default_domains::{InvariantWindow, TrackedPlace};
use diagnostics::Diagnostic;
use language_core::place_label;
use symbols::JoinedPath;
use typed_trees::data::DataDefinition;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

/// Scan value-position reads against construction establishment and open
/// invariant windows, including facts transported from predecessor states.
pub(crate) fn scan_statement_reads(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    call_established: &[String],
    inherited_windows: &[InvariantWindow],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut reads: Vec<ExpressionHandle> = Vec::new();
    match statement {
        // An assembly fact is itself a checked consumption point. Its proof
        // checker handles establishment; this runtime-read scan must not
        // interpret it as an executed expression.
        StatementNode::RootBinding(binding) => {
            reads.extend(
                [binding.receiver, binding.implementation_operand]
                    .into_iter()
                    .filter(|expression| expression.is_valid()),
            );
        }
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => reads.push(assignment.value),
        StatementNode::Expression(expression) => reads.push(*expression),
        StatementNode::LocalData(local) => {
            if local.initial_value.is_valid() {
                reads.push(local.initial_value);
            }
        }
        StatementNode::Call(call) => {
            let receiver = program.statement_table.name_path_members(call.receiver);
            let data_binding = program
                .state_data_binding(machine, state)
                .map(|name| name.as_str());
            if receiver.len() > 1
                && data_binding.is_some_and(|root| receiver[0].as_str() == root)
                && let Some(definition) = machine
                    .attached_data
                    .as_ref()
                    .and_then(|attached| program.data_definition_by_name(attached))
                && crate::value_custody::data::data_requires_establishment(program, definition)
            {
                validate_data_read(
                    program,
                    state,
                    definition,
                    receiver[0].as_str(),
                    receiver[1].as_str(),
                    call.receiver_root_symbol,
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
            // A STATEMENT call's argument handles live in the statement
            // table's arena, not the expression table's. Resolving them
            // through `expression_table` returns whatever occupies those
            // indices there -- foreign expressions from other declarations --
            // which this scan then reports as reads of places the machine
            // does not have.
            reads.extend(
                program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied(),
            );
        }
        StatementNode::Transition(transition) => {
            // Guards, named-target arguments, and value targets are all
            // evaluated reads at the transition point.
            reads.extend(transition_evaluated_expressions(program, transition));
        }
    }
    if let Some(assignment) = statement.as_assignment() {
        // The place spine of the target is the write path, but every index
        // expression inside it is an ordinary evaluated read.
        reads.extend(write_target_index_expressions(program, assignment.target));
    }
    for read in reads {
        scan_expression_reads(
            program,
            machine,
            state,
            read,
            tracked,
            entry_established,
            call_established,
            inherited_windows,
            diagnostics,
        );
    }
}

fn scan_expression_reads(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    call_established: &[String],
    inherited_windows: &[InvariantWindow],
    diagnostics: &mut Vec<Diagnostic>,
) {
    scan_expression_use(
        program,
        machine,
        state,
        expression,
        tracked,
        entry_established,
        call_established,
        inherited_windows,
        true,
        diagnostics,
    );
}

/// A projection's address spine does not consume each intermediate record.
/// Its final value does: taking `sample.child` observes that whole child,
/// whereas reading `sample.child.value` may use the raw scalar during repair.
fn scan_expression_use(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    call_established: &[String],
    inherited_windows: &[InvariantWindow],
    consumes_value: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    validate_qualified_place_read(
        program,
        machine,
        state,
        expression,
        tracked,
        entry_established,
        call_established,
        inherited_windows,
        diagnostics,
    );
    if consumes_value && let Some(spelling) = write_place_spelling(program, expression) {
        // Dynamic selectors overlap every possible element. A nominal value
        // also consumes a covering window; a raw scalar projection does not.
        let consumes_record =
            data_definition_for_expression(program, machine, Some(state), expression).is_some();
        if matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Name(_)
        ) && let Some(definition) =
            data_definition_for_expression(program, machine, Some(state), expression)
            && crate::value_custody::data::data_requires_establishment(program, definition)
        {
            // Passing or borrowing the whole nominal value cannot bypass the
            // same establishment gate a projected observation must satisfy.
            validate_data_read(
                program,
                state,
                definition,
                &spelling,
                "<whole value>",
                expression_root_symbol(program, expression),
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
        }
        let overlaps_consumed_value = |place: &str| {
            place_spelling_covers(&spelling, place)
                || (consumes_record && place_spelling_covers(place, &spelling))
        };
        if let Some(place) = tracked
            .iter()
            .find(|place| place.any_window_open() && overlaps_consumed_value(&place.spelling))
        {
            diagnostics.push(Diagnostic::error(format!(
                "the next whole-value read of `{spelling}` occurs inside an OPEN invariant \
                 window: a prior write left data `{}`'s default domain FALSE -- \
                 restore the facts before copying or exposing the value (ch11)",
                place.definition.name.as_str()
            )));
        } else if let Some((_, data_name, _)) = inherited_windows
            .iter()
            .find(|(place, _, _)| overlaps_consumed_value(place))
        {
            diagnostics.push(Diagnostic::error(format!(
                "the next whole-value read of `{spelling}` occurs inside an OPEN invariant \
                 window carried from a predecessor state for data `{data_name}`"
            )));
        }
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if consumes_value && members.len() > 1 {
                let receiver_spelling = members[..members.len() - 1].joined_path_with(".");
                if let Some(place) = tracked
                    .iter()
                    .find(|place| place_spelling_covers(&place.spelling, &receiver_spelling))
                {
                    validate_data_read(
                        program,
                        state,
                        place.definition,
                        &receiver_spelling,
                        members.last().expect("non-empty path").as_str(),
                        path.head_symbol,
                        tracked,
                        entry_established,
                        call_established,
                        inherited_windows,
                        diagnostics,
                    );
                }
            }
            // A machine call such as `self.console.exit_process(..)` stores
            // `self.console` as the call receiver Name path. It still consumes
            // the attached `self` value, so it must not bypass establishment
            // merely because no standalone Member node was built.
            if consumes_value
                && members.len() > 1
                && members[0].is_self_receiver()
                && let Some(definition) = machine
                    .attached_data
                    .as_ref()
                    .and_then(|attached| program.data_definition_by_name(attached))
            {
                validate_data_read(
                    program,
                    state,
                    definition,
                    "self",
                    members[1].as_str(),
                    path.head_symbol,
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Member(member) => {
            if let Some(receiver_spelling) = self_place_spelling(program, member.receiver) {
                // A covering ancestor carries its own schema, not the
                // projected receiver's instantiated child schema.
                let definition =
                    data_definition_for_expression(program, machine, Some(state), member.receiver)
                        .or_else(|| {
                            tracked
                                .iter()
                                .find(|place| place.spelling == receiver_spelling)
                                .map(|place| place.definition)
                        });
                let established_child =
                    self_place_spelling(program, expression).is_some_and(|child| {
                        direct_place_established(
                            &child,
                            tracked,
                            entry_established,
                            call_established,
                        )
                    });
                if consumes_value
                    && !established_child
                    && let Some(definition) = definition
                    && crate::value_custody::data::data_requires_establishment(program, definition)
                {
                    validate_data_read(
                        program,
                        state,
                        definition,
                        &receiver_spelling,
                        member.member.as_str(),
                        expression_root_symbol(program, member.receiver),
                        tracked,
                        entry_established,
                        call_established,
                        inherited_windows,
                        diagnostics,
                    );
                }
            }
            // Traverse every receiver, including a Name, to retain its exact
            // qualification gate. This address step does not consume the
            // incomplete containing value or replay its whole-domain check.
            scan_expression_use(
                program,
                machine,
                state,
                member.receiver,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                false,
                diagnostics,
            );
        }

        ExpressionNode::Indexed(indexed) => {
            scan_expression_use(
                program,
                machine,
                state,
                indexed.collection,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                false,
                diagnostics,
            );
            scan_expression_reads(
                program,
                machine,
                state,
                indexed.index,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
        }
        ExpressionNode::Binary(binary) => {
            scan_expression_reads(
                program,
                machine,
                state,
                binary.left,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
            scan_expression_reads(
                program,
                machine,
                state,
                binary.right,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
        }
        ExpressionNode::Borrow(inner) => {
            if inner.access == language_core::ReferenceAccess::WriteOnly {
                // An establishment loan exposes storage, not a readable value.
                // Its formation/forwarding checker owns exact write authority.
                // Runtime selectors still observe their ordinary operands.
                for index in write_target_index_expressions(program, inner.target) {
                    scan_expression_reads(
                        program,
                        machine,
                        state,
                        index,
                        tracked,
                        entry_established,
                        call_established,
                        inherited_windows,
                        diagnostics,
                    );
                }
                return;
            }
            if let ExpressionNode::Member(member) =
                program.expression_table.expression(inner.target)
                && let Some(receiver_spelling) = self_place_spelling(program, member.receiver)
                && let Some(place) = tracked.iter().find(|place| {
                    place_spelling_covers(&place.spelling, &receiver_spelling)
                        && place.any_window_open()
                })
            {
                validate_data_read(
                    program,
                    state,
                    place.definition,
                    &receiver_spelling,
                    member.member.as_str(),
                    expression_root_symbol(program, member.receiver),
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
            scan_expression_reads(
                program,
                machine,
                state,
                inner.target,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
        }
        ExpressionNode::Call(call) => {
            scan_expression_reads(
                program,
                machine,
                state,
                call.receiver,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                scan_expression_reads(
                    program,
                    machine,
                    state,
                    *argument,
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Cast(cast) => {
            scan_expression_reads(
                program,
                machine,
                state,
                cast.value,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
        }
        ExpressionNode::Unary(unary) => {
            scan_expression_reads(
                program,
                machine,
                state,
                unary.operand,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
        }
        ExpressionNode::Atomic(atomic) => {
            for operand in [atomic.value, atomic.result] {
                scan_expression_reads(
                    program,
                    machine,
                    state,
                    operand,
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Range(range) => {
            for bound in [range.start, range.end] {
                scan_expression_reads(
                    program,
                    machine,
                    state,
                    bound,
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program
                .expression_table
                .expression_handles(*elements)
                .iter()
                .copied()
            {
                scan_expression_reads(
                    program,
                    machine,
                    state,
                    element,
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                scan_expression_reads(
                    program,
                    machine,
                    state,
                    field.value,
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Match(matched) => {
            scan_expression_reads(
                program,
                machine,
                state,
                matched.subject,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
            for arm in program.expression_table.match_arms(matched.arms) {
                scan_expression_reads(
                    program,
                    machine,
                    state,
                    arm.value,
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    scan_expression_reads(
                        program,
                        machine,
                        state,
                        pattern,
                        tracked,
                        entry_established,
                        call_established,
                        inherited_windows,
                        diagnostics,
                    );
                }
            }
        }
        _ => {}
    }
}

/// Address traversal need not consume an incomplete containing record, but
/// it still crosses the exact declared qualification of each selected child.
/// Structural field composition cannot manufacture predicate or routed-domain
/// membership after that qualification was peeled for field lookup.
fn validate_qualified_place_read(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    call_established: &[String],
    inherited_windows: &[InvariantWindow],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(reference) = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        expression,
    ) else {
        return;
    };
    if !matches!(
        program.type_reference_table.type_reference(reference),
        typed_trees::types::TypeReferenceNode::Constrained { .. }
    ) || !crate::value_custody::data::type_requires_establishment(program, reference)
    {
        return;
    }
    let Some(spelling) = self_place_spelling(program, expression) else {
        return;
    };
    // The nearest current observation wins over an older established ancestor.
    // An accepted direct field store also proves its own qualification while
    // unrelated siblings or the parent's coupling may still be unfinished.
    let covering = tracked
        .iter()
        .filter(|place| place_spelling_covers(&place.spelling, &spelling))
        .max_by_key(|place| place.spelling.len());
    let established = if let Some(place) = covering {
        !place.any_window_open()
            && (place.established
                || program
                    .data_members(place.definition)
                    .iter()
                    .filter_map(|member| member.as_field())
                    .any(|field| {
                        place.established_fields.contains(&field.symbol)
                            && place_label(&place.spelling, field.name.as_str()) == spelling
                    }))
    } else {
        entry_established
            .iter()
            .any(|place| place_spelling_covers(place, &spelling))
            || declared_root_established(
                program,
                state,
                expression_root_symbol(program, expression),
            )
    };
    // A checked exact child store remains established even while an ancestor
    // is only reserved storage. It cannot waive the parent's coupled domain:
    // every overlapping open window below still fences the observation.
    let established = established
        || call_established
            .iter()
            .any(|place| place_spelling_covers(place, &spelling));
    let open = tracked.iter().any(|place| {
        place.any_window_open()
            && (place_spelling_covers(&place.spelling, &spelling)
                || place_spelling_covers(&spelling, &place.spelling))
    }) || inherited_windows.iter().any(|(place, _, _)| {
        place_spelling_covers(place, &spelling) || place_spelling_covers(&spelling, place)
    });
    if !established || open {
        diagnostics.push(Diagnostic::error(format!(
            "reading `{spelling}` crosses an open default-domain invariant window before its declared qualification is established: the zeroed representation is not yet a value of `{}`",
            program.display_type_reference_with_constraints(reference),
        )));
    }
}

fn validate_data_read(
    program: &dyn crate::CheckingInput,
    state: &State,
    definition: &DataDefinition,
    receiver_spelling: &str,
    member_name: &str,
    receiver_root: symbols::SymbolHandle,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    call_established: &[String],
    inherited_windows: &[InvariantWindow],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let place = tracked
        .iter()
        .find(|place| place_spelling_covers(&place.spelling, receiver_spelling));
    let established = place.map(|place| place.established).unwrap_or_else(|| {
        // Declared inputs are established by their caller, including an input
        // whose type also qualifies the machine's name. Qualification grants
        // no storage and cannot turn the same parameter into a zeroed owner.
        // The existing construction walk still owns fresh storage; tracked
        // mutations take precedence and retain their window checks below.
        entry_established
            .iter()
            .any(|established| place_spelling_covers(established, receiver_spelling))
            || call_established
                .iter()
                .any(|established| place_spelling_covers(established, receiver_spelling))
            || declared_root_established(program, state, receiver_root)
    });
    let established = established
        || composed_value_established(
            program,
            definition,
            receiver_spelling,
            tracked,
            entry_established,
            call_established,
            &mut Vec::new(),
        );
    if !established {
        diagnostics.push(Diagnostic::error(format!(
            "reading `{receiver_spelling}.{member_name}` crosses an open default-domain \
             invariant window before data `{}` is established: the zeroed representation \
             is not yet a `{}` (ch12's access gate) -- construct it on every path first \
             (the cross-state must-analysis carries establishment)",
            definition.name.as_str(),
            definition.name.as_str()
        )));
    }
    // Any covering place with an open window polices the read -- a wildcard
    // place's window still covers a closed concrete sibling it subsumes.
    if tracked.iter().any(|place| {
        place_spelling_covers(&place.spelling, receiver_spelling) && place.any_window_open()
    }) {
        diagnostics.push(Diagnostic::error(format!(
            "reading `{receiver_spelling}.{member_name}` inside an OPEN invariant window: \
             a prior write left data `{}`'s default domain FALSE -- restore the facts \
             before this consumption point (ch11)",
            definition.name.as_str()
        )));
    }
    if place.is_none()
        && inherited_windows
            .iter()
            .any(|(spelling, _, _)| place_spelling_covers(spelling, receiver_spelling))
    {
        diagnostics.push(Diagnostic::error(format!(
            "reading `{receiver_spelling}.{member_name}` inside an OPEN invariant window \
             carried from a predecessor state: data `{}`'s default domain is FALSE -- \
             restore the facts before this consumption point (ch11 window transport)",
            definition.name.as_str()
        )));
    }
}

/// A nested gated field gates its containing machine value, but establishing
/// that child must in turn establish the parent once every gated child is
/// ready. This is the establishment analogue of structural ZII composition:
/// the parent carries no independent ceremony when its own authored default
/// domain accepts zero.
pub(crate) fn attached_value_established(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    call_established: &[String],
) -> bool {
    // No data binding means this machine owns no record here, so there is no
    // whole-value establishment to find.
    let Some(data_binding) = program
        .state_data_binding(machine, state)
        .map(|name| name.as_str())
    else {
        return false;
    };
    if direct_place_established(data_binding, tracked, entry_established, call_established) {
        return true;
    }
    let Some(attached) = machine.attached_data.as_ref() else {
        return false;
    };
    let Some(definition) = program.data_definition_by_name(attached) else {
        return false;
    };
    composed_value_established(
        program,
        definition,
        data_binding,
        tracked,
        entry_established,
        call_established,
        &mut Vec::new(),
    )
}

/// Recompose the instantiated default domain from exact established children
/// and the parent's separately proved predicates. Neither a valid child nor
/// an address-spine traversal establishes an unrelated sibling or coupling.
fn composed_value_established(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    spelling: &str,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    call_established: &[String],
    active: &mut Vec<symbols::SymbolHandle>,
) -> bool {
    if tracked.iter().any(|place| {
        place.any_window_open()
            && (place_spelling_covers(&place.spelling, spelling)
                || place_spelling_covers(spelling, &place.spelling))
    }) {
        return false;
    }
    if direct_place_established(spelling, tracked, entry_established, call_established) {
        return true;
    }
    if active.contains(&definition.symbol) {
        return false;
    }
    let root = tracked.iter().find(|place| place.spelling == spelling);
    let fields = root.map(|place| place.fields.as_slice()).unwrap_or(&[]);
    let valuation = fields
        .iter()
        .map(|(name, value)| (name.as_str(), *value))
        .collect::<Vec<_>>();
    for fact in program.proof_facts.span_or_empty(definition.where_facts) {
        if !super::place_queries::fact_mentions_any_field(program, definition, fact) {
            continue;
        }
        let Some(expression) = fact.as_expression() else {
            return false;
        };
        if !super::symbolic_values::fold(
            program,
            definition,
            &valuation,
            root.map(|place| place.symbols.as_slice()).unwrap_or(&[]),
            root.map(|place| place.measures.as_slice()).unwrap_or(&[]),
            root.is_some_and(|place| place.born_zero),
            *expression,
        )
        .is_some_and(|value| value != 0)
        {
            return false;
        }
    }
    // A case-bearing whole value additionally requires its selected payload
    // and case constraints. Its existing direct establishment owns that
    // selection; common children alone cannot manufacture a case witness.
    if program
        .data_members(definition)
        .iter()
        .any(|member| member.as_variant().is_some())
    {
        return false;
    }
    active.push(definition.symbol);
    let established = program
        .data_members(definition)
        .iter()
        .filter_map(|member| member.as_field())
        .all(|field| {
            if !crate::value_custody::data::type_requires_establishment(
                program,
                field.type_reference,
            ) {
                return true;
            }
            if root.is_some_and(|place| place.established_fields.contains(&field.symbol)) {
                return true;
            }
            let child = place_label(spelling, field.name.as_str());
            if direct_place_established(&child, tracked, entry_established, call_established) {
                return true;
            }
            if let Some(definition) =
                super::place_queries::data_definition_for_type(program, field.type_reference)
            {
                // A type shell's extra qualification remains its own gate;
                // only a whole accepted store can establish that refinement.
                if matches!(
                    program
                        .type_reference_table
                        .type_reference(field.type_reference),
                    typed_trees::types::TypeReferenceNode::Constrained { .. }
                ) {
                    return false;
                }
                return composed_value_established(
                    program,
                    definition,
                    &child,
                    tracked,
                    entry_established,
                    call_established,
                    active,
                );
            }
            let Some(interval) =
                crate::proof_contracts::arithmetic_domains::range_constraint_interval(
                    program,
                    field.type_reference,
                )
            else {
                return false;
            };
            let Some(value) = fields
                .iter()
                .find(|(name, _)| name == field.name.as_str())
                .and_then(|(_, value)| *value)
                .and_then(super::symbolic_values::LiteralValue::integer)
            else {
                return false;
            };
            interval.low().is_none_or(|low| value >= i128::from(low))
                && interval.high().is_none_or(|high| value <= i128::from(high))
        });
    active.pop();
    established
}

fn direct_place_established(
    spelling: &str,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    call_established: &[String],
) -> bool {
    if let Some(place) = tracked.iter().find(|place| place.spelling == spelling) {
        return place.established && !place.any_window_open();
    }
    entry_established.iter().any(|place| place == spelling)
        || call_established.iter().any(|place| place == spelling)
}

/// Implicit range/containment gates hold only after every zero-excluding
/// common field has a known established value. Scalar ranges can be discharged
/// by the ordinary literal valuation. Nested data and arrays are established
/// by whole-value construction for now; a scalar field write cannot fabricate
/// evidence for them.
pub(crate) fn range_gates_hold(
    program: &dyn crate::CheckingInput,
    place: &TrackedPlace<'_>,
) -> bool {
    for member in program.data_members(place.definition) {
        let Some(field) = member.as_field() else {
            continue;
        };
        if !crate::value_custody::data::type_requires_establishment(program, field.type_reference) {
            continue;
        }
        if place.established_fields.contains(&field.symbol) {
            continue;
        }
        let Some(interval) = crate::proof_contracts::arithmetic_domains::range_constraint_interval(
            program,
            field.type_reference,
        ) else {
            // Nested records and arrays require whole-value establishment in
            // this first slice.
            return false;
        };
        let Some(value) = place
            .fields
            .iter()
            .find(|(name, _)| name == field.name.as_str())
            .and_then(|(_, value)| *value)
            .and_then(super::symbolic_values::LiteralValue::integer)
        else {
            return false;
        };
        if interval.low().is_some_and(|low| value < i128::from(low))
            || interval.high().is_some_and(|high| value > i128::from(high))
        {
            return false;
        }
    }
    true
}
