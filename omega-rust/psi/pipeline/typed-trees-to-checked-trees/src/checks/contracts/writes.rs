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
                let requires_provenance =
                    crate::facts::field_domain::domain_requires_provenance(program, domain_symbol);
                // A `requires` predicate is equally an obligation on the
                // initializer: `self in T::Case` and `self.f` membership are
                // decided by the construction or by live evidence at its
                // place, never minted by the annotation itself.
                let predicate_domain = program.domain_definitions().iter().any(|domain| {
                    domain.symbol == domain_symbol && domain.predicate_body.is_present()
                });
                if !(requires_provenance || predicate_domain) {
                    continue;
                }
                if !value_proves_qualification(
                    program,
                    facts,
                    state_flow,
                    statement_index,
                    local.initial_value,
                    domain_symbol,
                    semantic_domain,
                ) && !(!requires_provenance
                    && initializer_satisfies_predicate_domain(
                        program,
                        facts,
                        state_flow,
                        statement_index,
                        local.initial_value,
                        domain_symbol,
                        &mut Vec::new(),
                    ))
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

/// Discharge a `requires` predicate domain against the initializer itself.
/// `self in T::Case` lowers to `self == T::Case` (unions become `||`); the
/// fact is decided by the construction's own selected case or by live
/// `AssignedCase`/guard evidence at the value's place. Member comparisons
/// (`self.f == 0`, `self.flag`) read the literal's field value or the place's
/// scalar snapshot; `self.f in D` recurses into the field initializer or the
/// nested place. Mutation invalidation retires both evidence forms, so stale
/// establishment never survives a rewrite, and a foreign case or owner fails
/// the exact variant-symbol comparison.
fn initializer_satisfies_predicate_domain(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
    active: &mut Vec<SymbolHandle>,
) -> bool {
    if !domain_symbol.is_valid() || active.contains(&domain_symbol) {
        return false;
    }
    // An explicit `x as T in D` mint declares the qualification directly;
    // the staged mint fence at validation owns judging its legality.
    if let ExpressionNode::Cast(cast) = program.expression_table.expression(value)
        && cast.semantic_domain_symbol == domain_symbol
    {
        return true;
    }
    // A domain-owned operator is the sanctioned producer for its domain: the
    // selected candidate's domain qualifies the result, matching the trust
    // basis of a declared `-> T in D` call signature.
    if facts
        .operators
        .resolved_uses()
        .filter(|operator_use| operator_use.expression == value)
        .filter_map(|operator_use| facts.operators.selected_candidate(operator_use))
        .any(|candidate| candidate.domain_symbol == domain_symbol)
    {
        return true;
    }
    let Some(domain) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
    else {
        return false;
    };
    if !domain.predicate_body.is_present() {
        return false;
    }
    let type_symbol =
        crate::lookup::machine_symbol_from_type_reference_handle(program, domain.target_type);
    let type_symbol = type_symbol.is_valid().then_some(type_symbol);
    let entry_constraints = facts
        .flow
        .state_statement(state, statement_index)
        .map(|statement| statement.entry_constraints)
        .unwrap_or(state.entry_constraints);
    let contexts = facts
        .flow
        .semantic_constraint_contexts(entry_constraints)
        .collect::<Vec<_>>();
    active.push(domain_symbol);
    let satisfied = program.proof_facts(domain).iter().all(|fact| match fact {
        typed_trees::domain::ProofFact::Expression(expression) => {
            predicate_expression_holds_on_value(
                program,
                facts,
                state,
                statement_index,
                &contexts,
                value,
                *expression,
                type_symbol,
            )
        }
        typed_trees::domain::ProofFact::Membership(membership) => {
            membership.domain_arguments.is_empty()
                && member_subject_satisfies_domain(
                    program,
                    facts,
                    state,
                    statement_index,
                    &contexts,
                    value,
                    membership.value,
                    membership.domain_symbol,
                    type_symbol,
                    active,
                )
        }
        typed_trees::domain::ProofFact::Proposition(_) => false,
    });
    active.pop();
    satisfied
}

/// Evaluate one `requires` expression with `self` bound to `value`: `&&`/`||`
/// compose, `self == T::Case` compares the subject's selected case, and
/// `self.f` predicates compare a member's scalar. Anything else is not
/// decidable here and falls back to the membership proof.
fn predicate_expression_holds_on_value(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    value: ExpressionHandle,
    expression: ExpressionHandle,
    type_symbol: Option<SymbolHandle>,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary)
            if binary.operator == typed_trees::expression::BinaryOperator::And =>
        {
            predicate_expression_holds_on_value(
                program,
                facts,
                state,
                statement_index,
                contexts,
                value,
                binary.left,
                type_symbol,
            ) && predicate_expression_holds_on_value(
                program,
                facts,
                state,
                statement_index,
                contexts,
                value,
                binary.right,
                type_symbol,
            )
        }
        ExpressionNode::Binary(binary)
            if binary.operator == typed_trees::expression::BinaryOperator::Or =>
        {
            predicate_expression_holds_on_value(
                program,
                facts,
                state,
                statement_index,
                contexts,
                value,
                binary.left,
                type_symbol,
            ) || predicate_expression_holds_on_value(
                program,
                facts,
                state,
                statement_index,
                contexts,
                value,
                binary.right,
                type_symbol,
            )
        }
        ExpressionNode::Binary(binary) => {
            if let Some(selected) = case_membership_decision(
                program,
                facts,
                state,
                statement_index,
                contexts,
                value,
                binary.left,
                binary.right,
                type_symbol,
            ) {
                return match binary.operator {
                    typed_trees::expression::BinaryOperator::Equal => selected,
                    typed_trees::expression::BinaryOperator::NotEqual => !selected,
                    _ => false,
                };
            }
            member_comparison_holds_on_value(
                program,
                facts,
                state,
                statement_index,
                contexts,
                value,
                binary.operator,
                binary.left,
                binary.right,
                type_symbol,
            )
        }
        ExpressionNode::Member(_) | ExpressionNode::Name(_) => {
            member_scalar_at_subject(
                program,
                facts,
                state,
                statement_index,
                contexts,
                value,
                expression,
                type_symbol,
            ) == Some(facts::ScalarValue::Boolean(true))
        }
        _ => false,
    }
}

/// When a comparison's operands split into a `self`-rooted path and a name
/// resolving to a variant symbol, decide whether the subject's selected case
/// is that variant. Returns `None` when the operands are not that shape or no
/// case evidence exists for the subject.
#[allow(clippy::too_many_arguments)]
fn case_membership_decision(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    value: ExpressionHandle,
    left: ExpressionHandle,
    right: ExpressionHandle,
    type_symbol: Option<SymbolHandle>,
) -> Option<bool> {
    for (subject, case_expression) in [(left, right), (right, left)] {
        let ExpressionNode::Name(path) = program.expression_table.expression(case_expression)
        else {
            continue;
        };
        let case_symbol = path.symbol;
        if !case_symbol.is_valid()
            || program.symbols.get(case_symbol).kind != symbols::SymbolKind::Variant
        {
            continue;
        }
        let segments =
            crate::flow::relative_place_segments_from_expression(program, subject, type_symbol)?;
        return selected_case_is(
            program,
            facts,
            state,
            statement_index,
            contexts,
            value,
            &segments,
            case_symbol,
        );
    }
    None
}

/// Whether the subject `value` followed by `segments` currently selects
/// `case_symbol`. A place answers from live case evidence; a construction
/// literal answers from its own selected variant and member initializers.
fn selected_case_is(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    value: ExpressionHandle,
    segments: &[facts::PlaceSegment],
    case_symbol: SymbolHandle,
) -> Option<bool> {
    if let Some(selected) = literal_member_value(program, value, segments)
        .and_then(|member| construction_selected_case(program, member))
    {
        return Some(selected == case_symbol);
    }
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement_index,
        value,
    )?;
    if !matches!(place.root, facts::PlaceRoot::Symbol(_)) {
        return None;
    }
    let mut subject = place.clone();
    subject.extend_segments(segments);
    Some(crate::flow::place_case_has_value(
        program,
        &facts.semantic,
        contexts,
        state.machine_symbol,
        state.state_symbol,
        statement_index,
        &subject,
        case_symbol,
        true,
    ))
}

/// The selected variant of a construction expression: a cased struct literal,
/// a `T::Case(...)` call, or a bare variant name. Record literals and
/// non-construction expressions have no selected case.
fn construction_selected_case(
    program: &typed_trees::TypedTrees,
    value: ExpressionHandle,
) -> Option<SymbolHandle> {
    match program.expression_table.expression(value) {
        ExpressionNode::StructLiteral(literal) => literal.case_symbol,
        ExpressionNode::Call(call) => {
            let ExpressionNode::Name(receiver) = program.expression_table.expression(call.receiver)
            else {
                return None;
            };
            let data_definition = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == receiver.symbol)?;
            program
                .data_members(data_definition)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Variant(variant)
                        if variant.name.as_str() == call.target.as_str() =>
                    {
                        Some(variant.symbol)
                    }
                    _ => None,
                })
        }
        ExpressionNode::Name(path)
            if path.symbol.is_valid()
                && program.symbols.get(path.symbol).kind == symbols::SymbolKind::Variant =>
        {
            Some(path.symbol)
        }
        _ => None,
    }
}

/// Follow `self`-relative field segments into a struct literal's member
/// initializers. An empty segment list is the literal itself; a non-field
/// segment or a non-literal member value cannot be descended into.
fn literal_member_value(
    program: &typed_trees::TypedTrees,
    value: ExpressionHandle,
    segments: &[facts::PlaceSegment],
) -> Option<ExpressionHandle> {
    let mut current = value;
    for segment in segments {
        let facts::PlaceSegment::Field { symbol } = segment else {
            return None;
        };
        let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(current)
        else {
            return None;
        };
        current = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .find(|field| field.field_symbol == *symbol)
            .map(|field| field.value)?;
    }
    Some(current)
}

/// A `self`-rooted member path's current scalar: live assignment evidence at
/// the place's member segments, or the literal field's own constant.
#[allow(clippy::too_many_arguments)]
fn member_scalar_at_subject(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    value: ExpressionHandle,
    member_path: ExpressionHandle,
    type_symbol: Option<SymbolHandle>,
) -> Option<facts::ScalarValue> {
    let segments =
        crate::flow::relative_place_segments_from_expression(program, member_path, type_symbol)?;
    if let Some(scalar) = literal_member_value(program, value, &segments)
        .and_then(|member| expression_constant_scalar(program, member))
    {
        return Some(scalar);
    }
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement_index,
        value,
    )?;
    if !matches!(place.root, facts::PlaceRoot::Symbol(_)) {
        return None;
    }
    let mut subject = place.clone();
    subject.extend_segments(&segments);
    crate::values::scalar_value_at_place(
        program,
        &facts.semantic,
        contexts
            .iter()
            .map(|context| facts.semantic.contexts.get(*context)),
        &subject,
    )
}

fn expression_constant_scalar(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<facts::ScalarValue> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Some(facts::ScalarValue::Boolean(*value)),
        _ => program
            .expression_table
            .constant_integer_value(expression)
            .map(|value| facts::ScalarValue::Integer(numerics::bignum::BigInt::from_i64(value))),
    }
}

/// A comparison where one operand is a `self`-rooted member path and the
/// other is a constant: `self.a == 0`, `0 < self.b`, `self.flag`. Compares
/// the member's live scalar or literal value against the constant; when the
/// constant leads, the operator is mirrored so the member stays the subject.
#[allow(clippy::too_many_arguments)]
fn member_comparison_holds_on_value(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    value: ExpressionHandle,
    operator: typed_trees::expression::BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    type_symbol: Option<SymbolHandle>,
) -> bool {
    use typed_trees::expression::BinaryOperator;
    for (member, constant, operator) in [
        (left, right, operator),
        (
            right,
            left,
            match operator {
                BinaryOperator::Less => BinaryOperator::Greater,
                BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
                BinaryOperator::Greater => BinaryOperator::Less,
                BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
                other => other,
            },
        ),
    ] {
        if crate::flow::relative_place_segments_from_expression(program, member, type_symbol)
            .is_none()
            || expression_constant_scalar(program, constant).is_none()
        {
            continue;
        }
        let Some(actual) = member_scalar_at_subject(
            program,
            facts,
            state,
            statement_index,
            contexts,
            value,
            member,
            type_symbol,
        ) else {
            continue;
        };
        let expected = expression_constant_scalar(program, constant).expect("checked above");
        return match (&actual, &expected) {
            (facts::ScalarValue::Integer(actual), facts::ScalarValue::Integer(expected)) => {
                match operator {
                    BinaryOperator::Equal => actual == expected,
                    BinaryOperator::NotEqual => actual != expected,
                    BinaryOperator::Less => actual < expected,
                    BinaryOperator::LessOrEqual => actual <= expected,
                    BinaryOperator::Greater => actual > expected,
                    BinaryOperator::GreaterOrEqual => actual >= expected,
                    _ => false,
                }
            }
            (facts::ScalarValue::Boolean(actual), facts::ScalarValue::Boolean(expected)) => {
                match operator {
                    BinaryOperator::Equal => actual == expected,
                    BinaryOperator::NotEqual => actual != expected,
                    _ => false,
                }
            }
            _ => false,
        };
    }
    false
}

/// `self.f in D` on the initializer: a literal descends into the field's own
/// initializer and re-discharge; a place proves the nested place's domain the
/// ordinary way.
#[allow(clippy::too_many_arguments)]
fn member_subject_satisfies_domain(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    value: ExpressionHandle,
    member: ExpressionHandle,
    domain_symbol: SymbolHandle,
    type_symbol: Option<SymbolHandle>,
    active: &mut Vec<SymbolHandle>,
) -> bool {
    let Some(segments) =
        crate::flow::relative_place_segments_from_expression(program, member, type_symbol)
    else {
        return false;
    };
    if let Some(member_value) = literal_member_value(program, value, &segments) {
        return initializer_satisfies_predicate_domain(
            program,
            facts,
            state,
            statement_index,
            member_value,
            domain_symbol,
            active,
        ) || value_proves_domain_in_contexts(
            program,
            facts,
            state,
            statement_index,
            member_value,
            domain_symbol,
            contexts,
        );
    }
    let Some(place) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement_index,
        value,
    ) else {
        return false;
    };
    let mut subject = place.clone();
    subject.extend_segments(&segments);
    super::prover::prove_domain_at_place(
        program,
        &facts.semantic,
        contexts,
        &subject,
        domain_symbol,
    )
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

    // An admitted recast view `&x as &T` re-reads the SOURCE place's bytes
    // through the let's stated type, so an `in D` annotation on that type is an
    // obligation on the viewed place `x`, not on the reference-typed
    // initializer (which owns no scalar place of its own). Validation's recast
    // judgment already proved the source's declared representation facts imply
    // the cast's stated shape before admitting the view; the residual question
    // here is the source's own establishment in `domain_symbol`: live
    // membership evidence at its place, or a declared leaf domain implying
    // `domain_symbol` that is an invariant of the place — caller- or
    // binder-established for parameters and initialized locals, and gated on
    // the zero-initialized value satisfying it for machine storage and
    // zero-initialized locals (the same soundness gate the field-domain fact
    // seeding applies, so the annotation still cannot mint qualification).
    if let Some(source) = recast_view_source_expression(program, value)
        && (recast_source_proves_domain(
            program,
            facts,
            state_flow,
            statement_index,
            source,
            contexts,
            domain_symbol,
        ) || recast_source_declared_domain_implies(program, state_flow, source, domain_symbol))
    {
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

/// The viewed place expression behind an admitted recast view: `&x as &T` is a
/// `Cast` whose `form` is a borrow re-view spelling (`RecastShared` or
/// `RecastMutable`), and the `&mut x as &mut T` pun wraps that cast in the
/// unary `&mut`. Borrow shells peel in both directions so either spelling
/// reduces to the source place `x`. Validation only admits a recast in the
/// blessed `let` initializer position, so a non-recast value -- a conversion
/// `x as T`, a place read, or any other shape -- yields `None` and keeps the
/// ordinary discharge paths.
fn recast_view_source_expression(
    program: &typed_trees::TypedTrees,
    value: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let mut current = value;
    loop {
        match program.expression_table.expression(current) {
            ExpressionNode::Borrow(inner) => current = inner.target,
            ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                let mut source = cast.value;
                while let ExpressionNode::Borrow(inner) =
                    program.expression_table.expression(source)
                {
                    source = inner.target;
                }
                return Some(source);
            }
            _ => return None,
        }
    }
}

/// Whether the recast source place carries LIVE membership evidence in
/// `domain_symbol` -- an assigned value whose scalar satisfies the predicate,
/// or a carried `DomainMembership`/`ContractDomainMembership` fact at the
/// place (a minted local, a contract-proven parameter, a checked prior write).
fn recast_source_proves_domain(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    statement_index: usize,
    source: ExpressionHandle,
    contexts: &[facts::FactContextHandle],
    domain_symbol: SymbolHandle,
) -> bool {
    let Some(subject) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_flow.state_symbol,
        statement_index,
        source,
    ) else {
        return false;
    };
    super::prover::prove_domain_at_place(
        program,
        &facts.semantic,
        contexts,
        &subject,
        domain_symbol,
    )
}

/// Whether the recast source's DECLARED leaf type carries a predicate domain
/// that implies `domain_symbol`. The declared domain is an invariant of the
/// place this pass enforces on every write, so the residual obligation is the
/// domain's own establishment at the place's initial value: a state parameter
/// is caller-established and an initialized local was proven at its `let`,
/// while machine storage and zero-initialized locals additionally require the
/// zero value to satisfy the domain -- the same soundness gate the
/// field-domain fact seeding applies (`semantic/field_domains.rs`). A domain
/// the zero value violates stays a write-time obligation rather than an
/// invariant, so its reads must still follow a proven write.
fn recast_source_declared_domain_implies(
    program: &typed_trees::TypedTrees,
    state_flow: &FlowStateFact,
    source: ExpressionHandle,
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
    // `self`-rooted storage is always zero-initialized, so the declared leaf
    // domain is a valid invariant only when the zero value provably satisfies
    // it.
    if let Some(leaf_type) =
        crate::facts::field_domain::attached_data_field_type(program, machine, source)
    {
        return crate::facts::field_domain::predicate_domain_constraint_symbols(program, leaf_type)
            .into_iter()
            .any(|source_domain| {
                domain_admits_zero_value(program, source_domain)
                    && crate::facts::field_domain::domain_membership_implies(
                        program,
                        source_domain,
                        domain_symbol,
                    )
            });
    }
    let Some(leaf_type) =
        crate::facts::field_domain::direct_state_place_type_reference(program, state, source)
    else {
        return false;
    };
    let established = state_place_is_established(program, state, source);
    crate::facts::field_domain::predicate_domain_constraint_symbols(program, leaf_type)
        .into_iter()
        .any(|source_domain| {
            (established || domain_admits_zero_value(program, source_domain))
                && crate::facts::field_domain::domain_membership_implies(
                    program,
                    source_domain,
                    domain_symbol,
                )
        })
}

/// Whether a non-`self` state place's declared domain was established before
/// the current statement: a parameter's declared domain is caller-established
/// (and `&`/`&mut`/`&write` re-establishment is enforced on writes), and an
/// initialized local's annotation was discharged at its own `let`. A
/// zero-initialized local declared `in D` has no such evidence -- the same
/// storage case as a machine field -- and any other root conservatively
/// reports unestablished.
fn state_place_is_established(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    source: ExpressionHandle,
) -> bool {
    let Some(symbol) = state_place_root_symbol(program, source) else {
        return false;
    };
    if program
        .state_parameters(state)
        .iter()
        .any(|parameter| parameter.symbol == symbol)
    {
        return true;
    }
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .any(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol => {
                local.initial_value.is_valid()
            }
            _ => false,
        })
}

/// The root symbol of a state place expression, peeling borrow shells and
/// member projections. `self` is never a state-place root (the caller resolves
/// it through `attached_data_field_type` first), so a `self`-headed name is
/// not special-cased here.
fn state_place_root_symbol(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => state_place_root_symbol(program, inner.target),
        ExpressionNode::Member(member) => state_place_root_symbol(program, member.receiver),
        ExpressionNode::Indexed(indexed) => state_place_root_symbol(program, indexed.collection),
        ExpressionNode::Name(path) => path
            .head_symbol
            .is_valid()
            .then_some(path.head_symbol)
            .or_else(|| path.symbol.is_valid().then_some(path.symbol)),
        _ => None,
    }
}

/// Whether `domain_symbol`'s `self`-facts provably hold on the ZERO value of
/// its declared carrier -- the scalar-carrier half of the ZII soundness gate
/// `domain_admits_empty_byte_sequence` covers for recognized byte predicates.
/// An integer leaf's ZII is `0` and a `bool` leaf's is `false`; `self`-relative
/// membership facts recurse because a member's ZII is again the all-zero value
/// of its own declared type. A leaf the evaluator cannot decide -- a float,
/// byte string, record, runtime projection, or a non-`self` membership subject
/// -- conservatively refuses, so an unproven domain stays a write-time
/// obligation rather than an invariant.
fn domain_admits_zero_value(
    program: &typed_trees::TypedTrees,
    domain_symbol: SymbolHandle,
) -> bool {
    domain_admits_zero_value_inner(program, domain_symbol, &mut Vec::new())
}

fn domain_admits_zero_value_inner(
    program: &typed_trees::TypedTrees,
    domain_symbol: SymbolHandle,
    active: &mut Vec<SymbolHandle>,
) -> bool {
    if crate::facts::field_domain::domain_admits_empty_byte_sequence(program, domain_symbol) {
        return true;
    }
    if active.contains(&domain_symbol) {
        return false;
    }
    let Some(domain) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
    else {
        return false;
    };
    // Generic or indexed domain instances can bind their zero question to the
    // instance arguments; only closed symbol-only theories are answered here.
    if !domain.type_parameters.is_empty() || !domain.index_arguments.is_empty() {
        return false;
    }
    let facts = program.proof_facts.span_or_empty(domain.facts);
    if facts.is_empty() {
        return false;
    }
    active.push(domain_symbol);
    let admitted = facts.iter().all(|fact| match fact {
        typed_trees::domain::ProofFact::Expression(expression) => matches!(
            super::prover::evaluate_scalar(program, *expression, &mut |leaf| {
                zero_value_scalar_leaf(program, domain, leaf)
            }),
            Some(super::prover::ScalarValue::Boolean(true))
        ),
        typed_trees::domain::ProofFact::Membership(membership) => {
            expression_is_self_relative(program, membership.value)
                && domain_admits_zero_value_inner(program, membership.domain_symbol, active)
        }
        typed_trees::domain::ProofFact::Proposition(_) => false,
    });
    active.pop();
    admitted
}

/// The ZII scalar of a `self`-relative leaf in a domain fact: the leaf's
/// declared type resolved through its retained position, or through the
/// domain target's member path when the position is opaque, then the zero of
/// that primitive -- `0` for an integer leaf, `false` for a `bool` leaf.
/// Floats, byte carriers, records, and runtime projections have no decidable
/// scalar here, so the leaf stays unresolved and the enclosing evaluation
/// fails closed.
fn zero_value_scalar_leaf(
    program: &typed_trees::TypedTrees,
    domain: &typed_trees::domain::DomainDefinition,
    leaf: ExpressionHandle,
) -> Option<super::prover::ScalarValue> {
    let leaf_type =
        crate::flow::expression_place_type_reference(program, leaf, &[]).or_else(|| {
            let self_type = crate::lookup::machine_symbol_from_type_reference_handle(
                program,
                domain.target_type,
            );
            let segments = crate::flow::relative_place_segments_from_expression(
                program,
                leaf,
                self_type.is_valid().then_some(self_type),
            )?;
            crate::flow::project_type_reference_from_segments(
                program,
                domain.target_type,
                &segments,
            )
        })?;
    match program.type_reference_table.primitive_type(leaf_type) {
        Some(typed_trees::types::PrimitiveType::Bool) => {
            Some(super::prover::ScalarValue::Boolean(false))
        }
        Some(primitive) if primitive.accepts_integer_literal() => Some(
            super::prover::ScalarValue::Integer(numerics::bignum::BigInt::from_i64(0)),
        ),
        _ => None,
    }
}

/// Whether an expression is a `self`-rooted member path in a domain fact --
/// `self`, `self.f`, or a deeper projection -- where `self` binds the domain's
/// target value. A membership fact over any other subject (a constant, an
/// unrelated place) is not evidence about the domain's own zero value.
fn expression_is_self_relative(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => expression_is_self_relative(program, inner.target),
        ExpressionNode::Member(member) => expression_is_self_relative(program, member.receiver),
        ExpressionNode::Indexed(indexed) => {
            expression_is_self_relative(program, indexed.collection)
        }
        ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .first()
            .is_some_and(|member| member.as_str() == "self"),
        _ => false,
    }
}
