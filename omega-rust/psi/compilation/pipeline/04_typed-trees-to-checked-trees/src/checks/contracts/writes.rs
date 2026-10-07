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
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceHandle;

use crate::labels::{machine_name, symbol_name};

mod byte_lengths;
mod domain_implications;
mod member_domains;
use byte_lengths::{known_byte_lengths_before, static_max_byte_length};
use domain_implications::{
    construction_value_proves_qualification, value_proves_domain, value_proves_qualification,
};
use member_domains::initializer_satisfies_predicate_domain;

pub(super) fn check_domain_field_writes(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(machine) = crate::lookup::machine_by_symbol(program, state_flow.machine_symbol) else {
        return;
    };
    let Some(state) = crate::semantic::calls::find_state_in_machine(
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
            for domain in crate::facts::field_domain::assignment_target_domain_constraints(
                program,
                machine,
                state,
                assignment.target,
            ) {
                let domain_symbol = domain.symbol;
                // A write establishes a PREDICATE-ONLY domain by satisfying its
                // predicates, the same route a `let` initializer already takes
                // above. `domains.md`: "Predicates alone establish
                // predicate-only membership"; a routed domain still needs its
                // provenance and so keeps the plain rejection.
                let requires_provenance =
                    crate::facts::field_domain::domain_requires_provenance(program, domain_symbol);
                // A domain whose membership is exactly an interval is the
                // target's integer range: the bounded-assignment obligation
                // proves every write inside it, from guards and operand
                // ranges alike, as it did for the bracketed range.
                let exact_interval =
                    validation::exact_declared_domain_interval(program, domain).is_some();
                if !exact_interval
                    && !value_proves_domain(
                        program,
                        facts,
                        state_flow,
                        statement_index,
                        assignment.value,
                        domain_symbol,
                    )
                    && !(!requires_provenance
                        && (initializer_satisfies_predicate_domain(
                            program,
                            facts,
                            state_flow,
                            statement_index,
                            assignment.value,
                            domain_symbol,
                            &mut Vec::new(),
                        )))
                {
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
/// field whose declared type carries a domain, require the field value provably
/// in that domain (reusing the assignment discharge `value_proves_domain`).
/// Walk `expression` for `StructLiteral` constructions; for each constructed
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
                    if let Some(machine) =
                        crate::lookup::machine_by_symbol(program, state_flow.machine_symbol)
                        && let Some(state) = crate::semantic::calls::find_state_in_machine(
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
        | ExpressionNode::UndeterminedNaN(_)
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
