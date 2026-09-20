//! #66 write-enforcement for encoding-domain-refined fields.
//!
//! A field declared `out: &[u8] in Utf8` carries a DOMAIN refinement. Read-
//! narrowing (the synthesized `requires self.out in Utf8` machine contract)
//! trusts that refinement at every read -- which is sound ONLY if every WRITE to
//! the field is proven in-domain. This check is that enforcement: an assignment
//! `self.f = X` (and a constructed `T { f: X }`, handled in validation for
//! the literal case) whose target field declares a domain `D` must establish
//! `X in D`, exactly as a `requires <arg> in D` call argument must. The discharge
//! reuses the call-requires machinery: a value proven in `D` by the entry-context
//! facts (a domained param/field copy), or a string literal whose comptime bytes
//! satisfy `D`'s byte-predicate fact (`super::grants`), is accepted; anything else (a raw
//! `&[u8]` with no domain fact) is rejected. This is the encoding-domain analog
//! of the #63 assignment range-check; without it the field-read narrowing would
//! rest on an unenforced refinement (the #40 trap).

use checked_trees::{CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;
use facts::FactPayload;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceHandle;

use crate::labels::{canonical_place_label, machine_name, symbol_name};

pub(super) fn check_domain_field_writes(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == state_flow.machine_symbol)
    else {
        return;
    };
    let Some(state) = crate::semantic_calls::find_state_in_machine(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    ) else {
        return;
    };

    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        // A local annotation is an obligation on its initializer, not an
        // establishment route. Check before the local's own statement facts
        // can be consulted, retaining exact indexed qualification identity.
        if let StatementNode::LocalData(local) = statement
            && local.initial_value.is_valid()
        {
            for (domain_symbol, semantic_domain) in
                crate::facts::field_domain::domain_constraint_identities(
                    program,
                    local.type_reference,
                )
            {
                if crate::facts::field_domain::domain_requires_provenance(program, domain_symbol)
                    && !value_proves_qualification(
                        program,
                        facts,
                        state_flow,
                        statement_index,
                        local.initial_value,
                        domain_symbol,
                        semantic_domain,
                    )
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "cannot prove initializer of `{}` in {} is in domain `{}`; an annotation cannot establish routed qualification",
                        local.name, machine_name(program, state_flow.machine_symbol), symbol_name(program, domain_symbol),
                    )));
                }
            }
        }
        // (1) Assignment into a domain-refined field, parameter, or local.
        if let StatementNode::Assignment(assignment) = statement {
            for domain_symbol in crate::facts::field_domain::assignment_target_domain_symbols(
                program,
                machine,
                state,
                assignment.target,
            ) {
                if !value_proves_domain(
                    program,
                    facts,
                    state_flow,
                    statement_index,
                    assignment.value,
                    domain_symbol,
                ) {
                    let target_label = program.expression_table.display_name(assignment.target);
                    diagnostics.push(Diagnostic::error(format!(
                        "cannot prove the value assigned to `{target_label}` in {} is in domain `{}`; \
                         a place declared `in {}` requires every write to be established in that domain \
                         (pass a value already proven in the domain, or a literal its byte-predicate \
                         fact accepts)",
                        machine_name(program, state_flow.machine_symbol),
                        symbol_name(program, domain_symbol),
                        symbol_name(program, domain_symbol),
                    )));
                }
            }
        }

        // (1b) Length-fits for a write into an OWNED bounded text carrier
        // `[u8; N] in D`: the assigned value's maximum byte length must provably
        // be <= N. This is the capacity half of the rung-2 growth bound and the
        // dual of the concat-domain law -- it is what makes admitting `a + b`
        // into the domain sound, by proving the materialized result cannot
        // overflow the N-byte inline storage ("overflow should never happen").
        // Gated on the field carrying a domain, so it touches only the rung-2
        // text carrier and not unrelated `[u8; N]` byte buffers. A view carrier
        // `&[u8] in D` owns no inline storage (capacity is None) and is skipped.
        // A value whose maximum length cannot be bounded (an unbounded view
        // source, a runtime call result) is conservatively rejected.
        if let StatementNode::Assignment(assignment) = statement
            && !crate::facts::field_domain::assignment_target_domain_symbols(
                program,
                machine,
                state,
                assignment.target,
            )
            .is_empty()
            && let Some(field_type) = crate::facts::field_domain::assignment_target_type_reference(
                program,
                machine,
                state,
                assignment.target,
            )
            && let Some(capacity) =
                crate::facts::field_domain::type_reference_fixed_array_capacity(program, field_type)
        {
            let target_label = program.expression_table.display_name(assignment.target);
            let known_lengths = known_byte_lengths_before(program, machine, state, statement_index);
            match static_max_byte_length(
                program,
                machine,
                state,
                statement_index,
                assignment.value,
                &known_lengths,
            ) {
                Some(max_length) if max_length <= capacity => {}
                Some(max_length) => diagnostics.push(Diagnostic::error(format!(
                    "the value assigned to `{target_label}` in {} can be up to {max_length} \
                     byte(s), exceeding the {capacity}-byte capacity of its `[u8; {capacity}]` \
                     carrier; a bounded text carrier requires every write to provably fit",
                    machine_name(program, state_flow.machine_symbol),
                ))),
                None => diagnostics.push(Diagnostic::error(format!(
                    "cannot bound the maximum byte length of the value assigned to \
                     `{target_label}` in {}; a bounded `[u8; {capacity}]` text carrier requires a \
                     write whose length is statically bounded (a literal, a concatenation of \
                     bounded operands, or another bounded carrier) so it provably fits",
                    machine_name(program, state_flow.machine_symbol),
                ))),
            }
        }

        // (2) Brace CONSTRUCTION `T { f: X }` of a domain-refined field (the
        // #60-1c parallel for domains): every constructed domain field must be
        // established too, else a later read trusting the field is unsound.
        let mut contexts = facts
            .flow
            .contexts
            .semantic_context_refs
            .span_or_empty(
                facts
                    .flow
                    .state_statement(state_flow, statement_index)
                    .map_or(state_flow.entry_semantic_contexts, |statement| {
                        statement.entry_semantic_contexts
                    }),
            )
            .iter()
            .map(|reference| reference.context)
            .collect::<Vec<_>>();
        if let StatementNode::Transition(transition) = statement {
            if let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                scan_construction_field_domains(
                    program,
                    facts,
                    state_flow,
                    statement_index,
                    guard,
                    &mut contexts,
                    diagnostics,
                );
            }
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                let mut branch_contexts = contexts.clone();
                let point = facts::ProgramPoint::TransitionArm {
                    machine_symbol: state_flow.machine_symbol,
                    state_symbol: state_flow.state_symbol,
                    statement_index,
                    transition_target: target,
                };
                branch_contexts.extend(
                    facts
                        .semantic
                        .contexts
                        .iter()
                        .filter_map(|(handle, context)| (context.point == point).then_some(handle)),
                );
                let expressions = match program.statement_table.transition_target(target) {
                    typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
                        program
                            .statement_table
                            .expression_handles(*arguments)
                            .to_vec()
                    }
                    typed_trees::statement::TransitionTargetNode::Value(expression) => {
                        vec![*expression]
                    }
                    _ => Vec::new(),
                };
                for expression in expressions {
                    scan_construction_field_domains(
                        program,
                        facts,
                        state_flow,
                        statement_index,
                        expression,
                        &mut branch_contexts,
                        diagnostics,
                    );
                }
            }
            continue;
        }
        for expression in statement_root_expressions(program, statement) {
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                statement_index,
                expression,
                &mut contexts,
                diagnostics,
            );
        }
    }
}

/// The root expressions a statement carries (where a `T { f: X }` construction
/// may appear, possibly nested). The exhaustive set keeps the construction walk
/// in step with the statement shapes.
fn statement_root_expressions(
    program: &typed_trees::TypedTrees,
    statement: &StatementNode,
) -> Vec<ExpressionHandle> {
    match statement {
        StatementNode::RootBinding(binding) => [binding.receiver, binding.implementation_operand]
            .into_iter()
            .filter(|expression| expression.is_valid())
            .collect(),
        StatementNode::AssemblyFact(_) => Vec::new(),
        StatementNode::Assignment(assignment) => vec![assignment.target, assignment.value],
        StatementNode::Call(call) => program
            .statement_table
            .expression_handles(call.arguments)
            .to_vec(),
        StatementNode::Expression(expression) => vec![*expression],
        StatementNode::LocalData(local_data) => vec![local_data.initial_value],
        // Guard and selected targets have separate evaluation contexts.
        StatementNode::Transition(_) => Vec::new(),
    }
}

/// The maximum byte length the runtime value of `expression` can take when that
/// bound is statically known, or `None` when it cannot be bounded. Underwrites
/// the length-fits check on writes into a bounded `[u8; N]` text carrier:
///   * a string literal contributes its exact byte length;
///   * a concatenation `a + b` contributes the sum of its operands' bounds;
///   * a place with a straight-line reaching write contributes that write's
///     running bound; otherwise an owned carrier read contributes its declared
///     `[u8; N]` capacity;
///   * a value call contributes its declared bounded-carrier return capacity.
///
/// Anything else -- a `&[u8]` view source (no inline capacity) or an unresolved
/// local -- is unbounded, yielding `None` (conservatively rejected by the
/// caller). The `known_lengths` input is a conservative straight-line
/// reaching-definition summary: calls and other opaque statements clear it,
/// and every write invalidates overlapping places before publishing its new
/// bound. Thus in-place append uses the proven current length when one reaches
/// the statement, never merely the storage capacity.
fn static_max_byte_length(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    known_lengths: &[KnownByteLength],
) -> Option<usize> {
    if !expression.is_valid() {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::String(literal) => Some(literal.len()),
        ExpressionNode::Binary(binary)
            if binary.operator == typed_trees::expression::BinaryOperator::Add =>
        {
            let left = static_max_byte_length(
                program,
                machine,
                state,
                statement_index,
                binary.left,
                known_lengths,
            )?;
            let right = static_max_byte_length(
                program,
                machine,
                state,
                statement_index,
                binary.right,
                known_lengths,
            )?;
            Some(left.saturating_add(right))
        }
        ExpressionNode::Borrow(inner) => static_max_byte_length(
            program,
            machine,
            state,
            statement_index,
            inner.target,
            known_lengths,
        ),
        ExpressionNode::Call(call) => {
            let target = crate::semantic_calls::find_state(program, call.target_symbol)?;
            crate::facts::field_domain::type_reference_fixed_array_capacity(
                program,
                target.return_type,
            )
        }
        _ => {
            if let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index,
                expression,
            ) && let Some(known) = known_lengths
                .iter()
                .rev()
                .find(|known| known.place == place)
            {
                return Some(known.max_length);
            }
            let field_type =
                crate::facts::field_domain::attached_data_field_type(program, machine, expression)
                    .or_else(|| {
                        crate::facts::field_domain::direct_state_place_type_reference(
                            program, state, expression,
                        )
                    })?;
            crate::facts::field_domain::type_reference_fixed_array_capacity(program, field_type)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KnownByteLength {
    place: crate::flow::CanonicalPlace,
    max_length: usize,
}

/// Compute the straight-line byte-length facts reaching `statement_index`.
/// This is deliberately born-conservative: a call/assembly/transition or a
/// value-position call clears the summary because it may mutate an aliased
/// place. Ordinary assignments retain disjoint facts, invalidate overlapping
/// places, and publish the assigned value's new maximum length.
fn known_byte_lengths_before(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
) -> Vec<KnownByteLength> {
    let mut known = Vec::new();
    for (index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .enumerate()
    {
        match statement {
            StatementNode::Assignment(assignment) => {
                if expression_contains_value_call(program, assignment.value) {
                    known.clear();
                }
                let max_length = static_max_byte_length(
                    program,
                    machine,
                    state,
                    index,
                    assignment.value,
                    &known,
                );
                if let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    index,
                    assignment.target,
                ) {
                    forget_overlapping_lengths(program, &mut known, &place);
                    if let Some(max_length) = max_length {
                        known.push(KnownByteLength { place, max_length });
                    }
                } else {
                    known.clear();
                }
            }
            StatementNode::LocalData(local) => {
                if expression_contains_value_call(program, local.initial_value) {
                    known.clear();
                }
                let max_length = static_max_byte_length(
                    program,
                    machine,
                    state,
                    index,
                    local.initial_value,
                    &known,
                );
                if let Some(place) = crate::flow::canonical_place_from_symbol(local.symbol)
                    && let Some(max_length) = max_length
                {
                    forget_overlapping_lengths(program, &mut known, &place);
                    known.push(KnownByteLength { place, max_length });
                }
            }
            StatementNode::RootBinding(_)
            | StatementNode::AssemblyFact(_)
            | StatementNode::Call(_)
            | StatementNode::Expression(_)
            | StatementNode::Transition(_) => known.clear(),
        }
    }
    known
}

fn forget_overlapping_lengths(
    program: &typed_trees::TypedTrees,
    known: &mut Vec<KnownByteLength>,
    written: &crate::flow::CanonicalPlace,
) {
    known.retain(|candidate| {
        candidate.place.root != written.root
            || !crate::flow::canonical_place_segments_may_overlap(
                program,
                &candidate.place.segments,
                &written.segments,
            )
    });
}

fn expression_contains_value_call(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(_) => true,
        ExpressionNode::Match(dispatch) => {
            expression_contains_value_call(program, dispatch.subject)
                || program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .any(|arm| {
                        matches!(arm.pattern, typed_trees::expression::MatchPattern::Value(pattern)
                        if expression_contains_value_call(program, pattern))
                            || expression_contains_value_call(program, arm.value)
                    })
        }
        ExpressionNode::Atomic(atomic) => expression_contains_value_call(program, atomic.value),
        ExpressionNode::Binary(binary) => {
            expression_contains_value_call(program, binary.left)
                || expression_contains_value_call(program, binary.right)
        }
        ExpressionNode::Cast(cast) => expression_contains_value_call(program, cast.value),
        ExpressionNode::Indexed(indexed) => {
            expression_contains_value_call(program, indexed.collection)
                || expression_contains_value_call(program, indexed.index)
        }
        ExpressionNode::Member(member) => expression_contains_value_call(program, member.receiver),
        ExpressionNode::Borrow(inner) => expression_contains_value_call(program, inner.target),
        ExpressionNode::Unary(unary) => expression_contains_value_call(program, unary.operand),
        ExpressionNode::Range(range) => {
            expression_contains_value_call(program, range.start)
                || expression_contains_value_call(program, range.end)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| expression_contains_value_call(program, field.value)),
        ExpressionNode::ArrayLiteral(elements) => program
            .expression_table
            .expression_handles(*elements)
            .iter()
            .any(|element| expression_contains_value_call(program, *element)),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

/// Walk `expression` for `StructLiteral` constructions; for each constructed
/// field whose declared type carries a domain, require the field value provably
/// in that domain (reusing the assignment discharge `value_proves_domain`).
fn scan_construction_field_domains(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    statement_index: usize,
    expression: ExpressionHandle,
    contexts: &mut Vec<facts::FactContextHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => scan_construction_field_domains(
            program,
            facts,
            state_flow,
            statement_index,
            atomic.value,
            contexts,
            diagnostics,
        ),
        ExpressionNode::Match(dispatch) => {
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                statement_index,
                dispatch.subject,
                contexts,
                diagnostics,
            );
            let mut joined_contexts: Option<Vec<facts::FactContextHandle>> = None;
            for arm in program.expression_table.match_arms(dispatch.arms) {
                let mut arm_contexts = contexts.clone();
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    scan_construction_field_domains(
                        program,
                        facts,
                        state_flow,
                        statement_index,
                        pattern,
                        &mut arm_contexts,
                        diagnostics,
                    );
                }
                scan_construction_field_domains(
                    program,
                    facts,
                    state_flow,
                    statement_index,
                    arm.value,
                    &mut arm_contexts,
                    diagnostics,
                );
                match &mut joined_contexts {
                    Some(joined) => joined.retain(|context| arm_contexts.contains(context)),
                    None => joined_contexts = Some(arm_contexts),
                }
            }
            // Only contexts surviving every arm remain usable afterwards.
            *contexts = joined_contexts.unwrap_or_default();
        }
        ExpressionNode::StructLiteral(literal) => {
            let type_name = literal.type_name.clone();
            let case_name = literal.case_name.clone();
            for field in program.expression_table.struct_fields(literal.fields) {
                scan_construction_field_domains(
                    program,
                    facts,
                    state_flow,
                    statement_index,
                    field.value,
                    contexts,
                    diagnostics,
                );
                for (domain_symbol, semantic_domain) in construction_field_domain_identities(
                    program,
                    literal.type_symbol,
                    case_name.as_ref().map(|name| name.as_str()),
                    field.name.as_str(),
                ) {
                    // (a) The constructed value must be established in the domain.
                    if !construction_value_proves_qualification(
                        program,
                        facts,
                        state_flow,
                        statement_index,
                        field.value,
                        domain_symbol,
                        semantic_domain,
                        contexts,
                    ) {
                        diagnostics.push(Diagnostic::error(format!(
                            "construction of `{}` field `{}` is not proven in domain `{}`; \
                             a field declared `in {}` requires every construction value to be \
                             established in that domain (construct with a literal its byte-predicate \
                             fact accepts, or a value already proven in the domain)",
                            type_name.as_str(),
                            field.name.as_str(),
                            symbol_name(program, domain_symbol),
                            symbol_name(program, domain_symbol),
                        )));
                    }
                    // (b) CAPACITY: a bounded `[u8; N]` text carrier field must not be
                    // constructed with a value longer than N -- the construction parallel
                    // of the assignment length-fits check (1b). A too-long literal would
                    // otherwise overflow the field's inline storage. Only fires for a
                    // domain-carrying `[u8; N]` field (view carriers have no capacity).
                    if let Some(machine) = program
                        .machines()
                        .iter()
                        .find(|machine| machine.symbol == state_flow.machine_symbol)
                        && let Some(state) = crate::semantic_calls::find_state_in_machine(
                            program,
                            state_flow.machine_symbol,
                            state_flow.state_symbol,
                        )
                        && let Some(field_type) = construction_field_type_by_symbol(
                            program,
                            literal.type_symbol,
                            case_name.as_ref().map(|name| name.as_str()),
                            field.name.as_str(),
                        )
                        && let Some(capacity) =
                            crate::facts::field_domain::type_reference_fixed_array_capacity(
                                program, field_type,
                            )
                    {
                        let known_lengths =
                            known_byte_lengths_before(program, machine, state, statement_index);
                        match static_max_byte_length(
                            program,
                            machine,
                            state,
                            statement_index,
                            field.value,
                            &known_lengths,
                        ) {
                            Some(max_length) if max_length <= capacity => {}
                            Some(max_length) => diagnostics.push(Diagnostic::error(format!(
                                "construction of `{}` field `{}` supplies a value up to \
                                 {max_length} byte(s), exceeding the {capacity}-byte capacity of \
                                 its `[u8; {capacity}]` carrier; a bounded text carrier requires \
                                 every write to provably fit",
                                type_name.as_str(),
                                field.name.as_str(),
                            ))),
                            None => diagnostics.push(Diagnostic::error(format!(
                                "cannot bound the maximum byte length of the value constructing \
                                 `{}` field `{}`; a bounded `[u8; {capacity}]` text carrier \
                                 requires a write whose length is statically bounded (a literal, \
                                 a concatenation of bounded operands, or another bounded carrier) \
                                 so it provably fits",
                                type_name.as_str(),
                                field.name.as_str(),
                            ))),
                        }
                    }
                }
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                scan_construction_field_domains(
                    program,
                    facts,
                    state_flow,
                    statement_index,
                    *element,
                    contexts,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                statement_index,
                binary.left,
                contexts,
                diagnostics,
            );
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                statement_index,
                binary.right,
                contexts,
                diagnostics,
            );
        }
        ExpressionNode::Cast(cast) => scan_construction_field_domains(
            program,
            facts,
            state_flow,
            statement_index,
            cast.value,
            contexts,
            diagnostics,
        ),
        ExpressionNode::Call(call) => {
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                statement_index,
                call.receiver,
                contexts,
                diagnostics,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                scan_construction_field_domains(
                    program,
                    facts,
                    state_flow,
                    statement_index,
                    *argument,
                    contexts,
                    diagnostics,
                );
            }
            if let Some(call) = facts
                .flow
                .control
                .calls
                .span_or_empty(state_flow.calls)
                .iter()
                .find(|call| {
                    call.statement_index == statement_index
                        && call.authored_expression == expression
                })
            {
                *contexts = facts
                    .flow
                    .contexts
                    .semantic_context_refs
                    .span_or_empty(call.exit_semantic_contexts)
                    .iter()
                    .map(|reference| reference.context)
                    .collect();
            } else {
                contexts.clear();
            }
        }
        ExpressionNode::Indexed(indexed) => {
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                statement_index,
                indexed.collection,
                contexts,
                diagnostics,
            );
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                statement_index,
                indexed.index,
                contexts,
                diagnostics,
            );
        }
        ExpressionNode::Member(member) => scan_construction_field_domains(
            program,
            facts,
            state_flow,
            statement_index,
            member.receiver,
            contexts,
            diagnostics,
        ),
        ExpressionNode::Borrow(inner) => scan_construction_field_domains(
            program,
            facts,
            state_flow,
            statement_index,
            inner.target,
            contexts,
            diagnostics,
        ),
        ExpressionNode::Range(range) => {
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                statement_index,
                range.start,
                contexts,
                diagnostics,
            );
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                statement_index,
                range.end,
                contexts,
                diagnostics,
            );
        }
        ExpressionNode::Unary(unary) => scan_construction_field_domains(
            program,
            facts,
            state_flow,
            statement_index,
            unary.operand,
            contexts,
            diagnostics,
        ),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

/// The declared predicate-domain symbols of a constructed field: a case literal's
/// PAYLOAD field (for the named variant) or a record/common struct field whose
/// declared type carries one or more predicate-bearing domain constraints.
/// Mirrors validation `struct_literals::construction_field_type` + domain
/// extraction.
fn construction_field_domain_identities(
    program: &typed_trees::TypedTrees,
    type_symbol: SymbolHandle,
    case_name: Option<&str>,
    field_name: &str,
) -> Vec<(SymbolHandle, language_semantics::SemanticDomainId)> {
    let Some(data_definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == type_symbol)
    else {
        return Vec::new();
    };
    if data_definition.type_parameters.count() > 0 {
        return Vec::new();
    }

    let Some(field_type) = construction_field_type(program, data_definition, case_name, field_name)
    else {
        return Vec::new();
    };
    crate::facts::field_domain::domain_constraint_identities(program, field_type)
        .into_iter()
        .filter(|(symbol, _)| {
            crate::facts::field_domain::domain_requires_provenance(program, *symbol)
                || program
                    .domain_definitions()
                    .iter()
                    .any(|domain| domain.symbol == *symbol && domain.predicate_body.is_present())
        })
        .collect()
}

/// Routed theories require the exact live instance at the initializer's actual
/// place. Read the last completed operand-call context when calls occur in the
/// statement, otherwise its entry context; never read the destination binding's
/// post-statement facts or equate places by their display names.
fn value_proves_qualification(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    domain: SymbolHandle,
    identity: language_semantics::SemanticDomainId,
) -> bool {
    if !value_case_path_is_selected(program, facts, state, statement_index, value) {
        return false;
    }
    if !crate::facts::field_domain::domain_requires_provenance(program, domain) {
        return value_proves_domain(program, facts, state, statement_index, value, domain);
    }
    let Some(subject) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement_index,
        value,
    ) else {
        return false;
    };
    let contexts = facts
        .flow
        .control
        .calls
        .span_or_empty(state.calls)
        .iter()
        .rfind(|call| call.statement_index == statement_index)
        .map(|call| call.exit_semantic_contexts)
        .unwrap_or_else(|| {
            facts
                .flow
                .state_statement(state, statement_index)
                .map_or(state.entry_semantic_contexts, |statement| {
                    statement.entry_semantic_contexts
                })
        });
    let contexts = facts
        .flow
        .contexts
        .semantic_context_refs
        .span_or_empty(contexts)
        .iter()
        .map(|reference| reference.context)
        .collect::<Vec<_>>();
    super::exits::exact_scalar_membership(program, facts, &contexts, &subject, domain, identity)
}

/// Resolve the selected definition, not the authored spelling: a qualified
/// foreign construction must establish that exact owner's field contracts.
fn construction_field_type_by_symbol(
    program: &typed_trees::TypedTrees,
    type_symbol: SymbolHandle,
    case_name: Option<&str>,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    let data_definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == type_symbol)?;
    if data_definition.type_parameters.count() > 0 {
        return None;
    }
    construction_field_type(program, data_definition, case_name, field_name)
}

fn construction_field_type(
    program: &typed_trees::TypedTrees,
    data_definition: &typed_trees::data::DataDefinition,
    case_name: Option<&str>,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    if let Some(case_name) = case_name
        && let Some(variant) = program
            .data_members(data_definition)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == case_name =>
                {
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
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == field_name => {
                field
                    .type_reference
                    .is_valid()
                    .then_some(field.type_reference)
            }
            _ => None,
        })
}

/// An extracted payload needs its current tag before any qualification rule
/// can consume its field facts. Use the same completed-operand contexts as
/// routed membership, never an initializer or a post-assignment destination.
fn value_case_path_is_selected(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
) -> bool {
    let Some(subject) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement_index,
        value,
    ) else {
        return true;
    };
    if !subject
        .segments
        .iter()
        .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
    {
        return true;
    }
    let contexts = facts
        .flow
        .control
        .calls
        .span_or_empty(state.calls)
        .iter()
        .rfind(|call| call.statement_index == statement_index)
        .map(|call| call.exit_semantic_contexts)
        .unwrap_or_else(|| {
            facts
                .flow
                .state_statement(state, statement_index)
                .map_or(state.entry_semantic_contexts, |statement| {
                    statement.entry_semantic_contexts
                })
        });
    let contexts = facts
        .flow
        .contexts
        .semantic_context_refs
        .span_or_empty(contexts)
        .iter()
        .map(|reference| reference.context)
        .collect::<Vec<_>>();
    crate::flow::place_cases_are_selected(
        program,
        &facts.semantic,
        &contexts,
        state.machine_symbol,
        state.state_symbol,
        statement_index,
        &subject,
    )
}

/// Prove an assigned value from live membership or an independently checked
/// predicate. A declared field alone cannot license an inactive payload read.
fn value_proves_domain(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let entry_constraints = facts
        .flow
        .state_statement(state_flow, statement_index)
        .map(|statement| statement.entry_constraints)
        .unwrap_or(state_flow.entry_constraints);
    let contexts = facts
        .flow
        .semantic_constraint_contexts(entry_constraints)
        .collect::<Vec<_>>();
    if !value_case_path_is_selected(program, facts, state_flow, statement_index, value) {
        return false;
    }
    value_proves_domain_in_contexts(
        program,
        facts,
        state_flow,
        statement_index,
        value,
        domain_symbol,
        &contexts,
    )
}

fn construction_value_proves_qualification(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    domain: SymbolHandle,
    identity: language_semantics::SemanticDomainId,
    contexts: &[facts::FactContextHandle],
) -> bool {
    let subject = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement_index,
        value,
    );
    if subject.as_ref().is_some_and(|subject| {
        !crate::flow::place_cases_are_selected(
            program,
            &facts.semantic,
            contexts,
            state.machine_symbol,
            state.state_symbol,
            statement_index,
            subject,
        )
    }) {
        return false;
    }
    if crate::facts::field_domain::domain_requires_provenance(program, domain) {
        return subject.as_ref().is_some_and(|subject| {
            super::exits::exact_scalar_membership(
                program, facts, contexts, subject, domain, identity,
            )
        });
    }
    // Numeric field domains use the same contextual proof as nominal call
    // arguments; requiring only an existing membership token would reject a
    // construction whose scalar value already establishes the predicate.
    if typed_trees::domain::supports_symbol_only_proof(program, domain)
        && subject.as_ref().is_some_and(|subject| {
            super::prover::prove_domain_at_place(
                program,
                &facts.semantic,
                contexts,
                subject,
                domain,
            )
        })
    {
        return true;
    }
    value_proves_domain_in_contexts(
        program,
        facts,
        state,
        statement_index,
        value,
        domain,
        contexts,
    )
}

fn value_proves_domain_in_contexts(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
    contexts: &[facts::FactContextHandle],
) -> bool {
    if !typed_trees::domain::supports_symbol_only_proof(program, domain_symbol) {
        return false;
    }
    if crate::facts::field_domain::string_literal_expression_grants_domain(
        program,
        value,
        domain_symbol,
    ) {
        return true;
    }

    // Concat preserves a byte-predicate domain: a `left + right` whose two
    // operands are each provably in `domain_symbol` is itself in the domain, for
    // the recognized concat-preserving byte-predicates (valid_utf8/no_nul/
    // ascii_only/non_empty). This is the DOMAIN half of the rung-2 growth bound;
    // the capacity half -- that the result fits an owned `[u8; N]` carrier -- is
    // the separate length-fits check at the write site, so admitting the domain
    // here is not on its own a license to overflow the target.
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(value)
        && binary.operator == typed_trees::expression::BinaryOperator::Add
        && crate::facts::field_domain::domain_is_concat_preserving(program, domain_symbol)
    {
        let (left, right) = (binary.left, binary.right);
        if construction_value_proves_qualification(
            program,
            facts,
            state_flow,
            statement_index,
            left,
            domain_symbol,
            language_semantics::SemanticDomainId::NULL,
            contexts,
        ) && construction_value_proves_qualification(
            program,
            facts,
            state_flow,
            statement_index,
            right,
            domain_symbol,
            language_semantics::SemanticDomainId::NULL,
            contexts,
        ) {
            return true;
        }
    }

    // A value-position call whose target's DECLARED return type carries a domain
    // implying `domain_symbol` is accepted on the same trust basis as a declared
    // param domain at a call site: the signature's domain is trusted at use
    // sites, with its establishment enforced separately (params at call sites;
    // scalar results at ordinary exits in exits/result_domains.rs). This is how a
    // `-> &[u8] in Utf8` value-call result flows into a `&[u8] in Utf8` field.
    if value_call_return_domain_implies(program, value, domain_symbol) {
        return true;
    }

    // A place whose DECLARED leaf type carries a ZII-admitting domain implying
    // the target domain remains a valid source even when flow invalidation has
    // discarded its transient membership fact (for example, after the enclosing
    // record crosses a mutable out-parameter call). Its zero/default value is
    // in-domain and every later write is checked by this pass, so that restricted
    // declaration is an invariant of the place rather than merely an
    // entry-context fact. Empty-violating domains still require a flow proof.
    if declared_value_domain_implies(program, state_flow, value, domain_symbol) {
        return true;
    }

    let value_label = program.expression_table.display_name(value);
    let value_place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_flow.state_symbol,
        statement_index,
        value,
    );
    contexts.iter().any(|context_handle| {
        let context = facts.semantic.contexts.get(*context_handle);
        facts.semantic.context_view(context).facts().any(|fact| {
            let fact_domain = match fact.payload {
                FactPayload::DomainMembership { domain_symbol, .. }
                | FactPayload::ContractDomainMembership { domain_symbol, .. } => domain_symbol,
                _ => return false,
            };
            if !typed_trees::domain::supports_symbol_only_proof(program, fact_domain) {
                return false;
            }
            if !facts.semantic.domain_implies(fact_domain, domain_symbol)
                && !crate::facts::field_domain::domain_membership_implies(
                    program,
                    fact_domain,
                    domain_symbol,
                )
            {
                return false;
            }
            // Match the fact's subject against the assigned value, by the
            // fact's place label and (for a contract membership) its declared
            // `value` expression label -- the same two-pronged match the
            // statement-transfer propagation uses.
            let facts::FactPlace::Place(fact_place) = fact.place else {
                return false;
            };
            if value_place.as_ref().is_some_and(|value_place| {
                crate::flow::canonical_place_from_semantic_place(
                    program,
                    &facts.semantic,
                    facts.semantic.places.get(fact_place),
                )
                .is_some_and(|fact_place| fact_place == *value_place)
            }) {
                return true;
            }
            let place_label = canonical_place_label(
                program,
                &facts.semantic,
                facts.semantic.places.get(fact_place),
            );
            if place_label == value_label {
                return true;
            }
            match fact.payload {
                FactPayload::DomainMembership { value, .. }
                | FactPayload::ContractDomainMembership { value, .. } => {
                    value.is_valid() && program.expression_table.display_name(value) == value_label
                }
                _ => false,
            }
        })
    })
}

/// Whether `value` is a state place whose declared leaf type carries a
/// ZII-admitting domain that implies `domain_symbol`.
fn declared_value_domain_implies(
    program: &typed_trees::TypedTrees,
    state_flow: &FlowStateFact,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == state_flow.machine_symbol)
    else {
        return false;
    };
    let Some(state) = crate::semantic_calls::find_state_in_machine(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    ) else {
        return false;
    };
    let Some(value_type) = crate::facts::field_domain::assignment_target_type_reference(
        program, machine, state, value,
    ) else {
        return false;
    };
    crate::facts::field_domain::predicate_domain_constraint_symbols(program, value_type)
        .into_iter()
        .filter(|value_domain| {
            crate::facts::field_domain::domain_admits_empty_byte_sequence(program, *value_domain)
        })
        .any(|value_domain| {
            crate::facts::field_domain::domain_membership_implies(
                program,
                value_domain,
                domain_symbol,
            )
        })
}

/// Whether `value` is a value-position call whose resolved target state declares
/// a return type carrying a domain that implies `domain_symbol`.
fn value_call_return_domain_implies(
    program: &typed_trees::TypedTrees,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return false;
    };
    let Some(target) = crate::semantic_calls::find_state(program, call.target_symbol) else {
        return false;
    };
    if !target.return_type.is_valid() {
        return false;
    }
    crate::facts::field_domain::predicate_domain_constraint_symbols(program, target.return_type)
        .into_iter()
        .any(|return_domain| {
            crate::facts::field_domain::domain_membership_implies(
                program,
                return_domain,
                domain_symbol,
            )
        })
}
