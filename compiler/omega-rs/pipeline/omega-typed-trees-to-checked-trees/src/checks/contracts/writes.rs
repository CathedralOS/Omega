//! #66 write-enforcement for encoding-domain-refined fields.
//!
//! A field declared `out: &[u8] in Utf8` carries a DOMAIN refinement. Read-
//! narrowing (the synthesized `requires self.out in Utf8` machine contract)
//! trusts that refinement at every read -- which is sound ONLY if every WRITE to
//! the field is proven in-domain. This check is that enforcement: an assignment
//! `self.f = X` (and a constructed `T { f: X }`, handled in omega-validation for
//! the literal case) whose target field declares a domain `D` must establish
//! `X in D`, exactly as a `requires <arg> in D` call argument must. The discharge
//! reuses the call-requires machinery: a value proven in `D` by the entry-context
//! facts (a domained param/field copy), or a string literal whose comptime bytes
//! satisfy `D`'s classifier (`super::grants`), is accepted; anything else (a raw
//! `&[u8]` with no domain fact) is rejected. This is the encoding-domain analog
//! of the #63 assignment range-check; without it the field-read narrowing would
//! rest on an unenforced refinement (the #40 trap).

use omega_checked_trees::{CheckFacts, FlowStateFact};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_facts::FactPayload;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::statement::StatementNode;
use omega_typed_trees::types::TypeReferenceHandle;

use crate::labels::{canonical_place_label, machine_name, symbol_name};

pub(super) fn check_domain_field_writes(
    program: &omega_typed_trees::TypedTrees,
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
    let Some(state) =
        crate::find_state_in_machine(program, state_flow.machine_symbol, state_flow.state_symbol)
    else {
        return;
    };

    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        // (1) Assignment `self.f = X` into a domain-refined field.
        if let StatementNode::Assignment(assignment) = statement
            && let Some(domain_symbol) =
                crate::field_domain::target_field_domain_symbol(program, machine, assignment.target)
            && !value_proves_domain(
                program,
                facts,
                state_flow,
                statement_index,
                assignment.value,
                domain_symbol,
            )
        {
            let target_label = program.expression_table.display_name(assignment.target);
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove the value assigned to `{target_label}` in {} is in domain `{}`; \
                 a field declared `in {}` requires every write to be established in that domain \
                 (pass a value already proven in the domain, or a literal the domain's classifier \
                 accepts)",
                machine_name(program, state_flow.machine_symbol),
                symbol_name(program, domain_symbol),
                symbol_name(program, domain_symbol),
            )));
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
            && crate::field_domain::target_field_domain_symbol(program, machine, assignment.target)
                .is_some()
            && let Some(field_type) =
                crate::field_domain::attached_data_field_type(program, machine, assignment.target)
            && let Some(capacity) =
                crate::field_domain::type_reference_fixed_array_capacity(program, field_type)
        {
            let target_label = program.expression_table.display_name(assignment.target);
            match static_max_byte_length(program, machine, assignment.value) {
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
        for expression in statement_root_expressions(program, statement) {
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                statement_index,
                expression,
                diagnostics,
            );
        }
    }
}

/// The root expressions a statement carries (where a `T { f: X }` construction
/// may appear, possibly nested). The exhaustive set keeps the construction walk
/// in step with the statement shapes.
fn statement_root_expressions(
    program: &omega_typed_trees::TypedTrees,
    statement: &StatementNode,
) -> Vec<ExpressionHandle> {
    match statement {
        StatementNode::Assignment(assignment) => vec![assignment.target, assignment.value],
        StatementNode::Call(call) => program
            .statement_table
            .expression_handles(call.arguments)
            .to_vec(),
        StatementNode::Expression(expression) => vec![*expression],
        StatementNode::LocalData(local_data) => vec![local_data.initial_value],
        StatementNode::Transition(transition) => {
            let mut roots = Vec::new();
            if let omega_typed_trees::statement::TransitionGuardNode::When(guard) = &transition.guard
            {
                roots.push(*guard);
            }
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target) {
                    omega_typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
                        roots.extend(
                            program
                                .statement_table
                                .expression_handles(*arguments)
                                .iter()
                                .copied(),
                        );
                    }
                    omega_typed_trees::statement::TransitionTargetNode::Value(expression) => {
                        roots.push(*expression);
                    }
                    omega_typed_trees::statement::TransitionTargetNode::SelfTarget
                    | omega_typed_trees::statement::TransitionTargetNode::Terminal => {}
                }
            }
            roots
        }
    }
}

/// The maximum byte length the runtime value of `expression` can take when that
/// bound is statically known, or `None` when it cannot be bounded. Underwrites
/// the length-fits check on writes into a bounded `[u8; N]` text carrier:
///   * a string literal contributes its exact byte length;
///   * a concatenation `a + b` contributes the sum of its operands' bounds;
///   * a `self.field` read contributes its declared `[u8; N]` carrier capacity
///     (an owned bounded source).
/// Anything else -- a `&[u8]` view source (no inline capacity), a runtime call
/// result, a local -- is unbounded, yielding `None` (conservatively rejected by
/// the caller). Notably an in-place append `self.buf + "x"` into a `[u8; N]`
/// buffer bounds to `N + 1 > N` and is correctly rejected: proving it fits needs
/// the buffer's flow-sensitive running length, not this static bound.
fn static_max_byte_length(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    expression: ExpressionHandle,
) -> Option<usize> {
    match program.expression_table.expression(expression) {
        ExpressionNode::String(literal) => Some(literal.as_bytes().len()),
        ExpressionNode::Binary(binary)
            if binary.operator == omega_typed_trees::expression::BinaryOperator::Add =>
        {
            let left = static_max_byte_length(program, machine, binary.left)?;
            let right = static_max_byte_length(program, machine, binary.right)?;
            Some(left.saturating_add(right))
        }
        _ => {
            let field_type =
                crate::field_domain::attached_data_field_type(program, machine, expression)?;
            crate::field_domain::type_reference_fixed_array_capacity(program, field_type)
        }
    }
}

/// Walk `expression` for `StructLiteral` constructions; for each constructed
/// field whose declared type carries a domain, require the field value provably
/// in that domain (reusing the assignment discharge `value_proves_domain`).
fn scan_construction_field_domains(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    statement_index: usize,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::StructLiteral(literal) => {
            let type_name = literal.type_name.clone();
            let case_name = literal.case_name.clone();
            for field in program.expression_table.struct_fields(literal.fields) {
                if let Some(domain_symbol) = construction_field_domain_symbol(
                    program,
                    type_name.as_str(),
                    case_name.as_ref().map(|name| name.as_str()),
                    field.name.as_str(),
                ) && !value_proves_domain(
                    program,
                    facts,
                    state_flow,
                    statement_index,
                    field.value,
                    domain_symbol,
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                        "construction of `{}` field `{}` is not proven in domain `{}`; \
                         a field declared `in {}` requires every construction value to be \
                         established in that domain (construct with a literal the domain's \
                         classifier accepts, or a value already proven in the domain)",
                        type_name.as_str(),
                        field.name.as_str(),
                        symbol_name(program, domain_symbol),
                        symbol_name(program, domain_symbol),
                    )));
                }
                scan_construction_field_domains(
                    program,
                    facts,
                    state_flow,
                    statement_index,
                    field.value,
                    diagnostics,
                );
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
                diagnostics,
            );
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                statement_index,
                binary.right,
                diagnostics,
            );
        }
        ExpressionNode::Cast(cast) => scan_construction_field_domains(
            program,
            facts,
            state_flow,
            statement_index,
            cast.value,
            diagnostics,
        ),
        ExpressionNode::Call(call) => {
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                statement_index,
                call.receiver,
                diagnostics,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                scan_construction_field_domains(
                    program,
                    facts,
                    state_flow,
                    statement_index,
                    *argument,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Indexed(indexed) => {
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                statement_index,
                indexed.collection,
                diagnostics,
            );
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                statement_index,
                indexed.index,
                diagnostics,
            );
        }
        ExpressionNode::Member(member) => scan_construction_field_domains(
            program,
            facts,
            state_flow,
            statement_index,
            member.receiver,
            diagnostics,
        ),
        ExpressionNode::Mutable(inner) => scan_construction_field_domains(
            program,
            facts,
            state_flow,
            statement_index,
            *inner,
            diagnostics,
        ),
        ExpressionNode::Range(range) => {
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                statement_index,
                range.start,
                diagnostics,
            );
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                statement_index,
                range.end,
                diagnostics,
            );
        }
        ExpressionNode::Unary(unary) => scan_construction_field_domains(
            program,
            facts,
            state_flow,
            statement_index,
            unary.operand,
            diagnostics,
        ),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

/// The declared encoding-domain symbol of a constructed field: a case literal's
/// PAYLOAD field (for the named variant) or a record/common struct field whose
/// declared type carries a `TypeConstraintNode::Domain`. `None` otherwise.
/// Mirrors omega-validation `struct_literals::construction_field_type` + domain
/// extraction.
fn construction_field_domain_symbol(
    program: &omega_typed_trees::TypedTrees,
    type_name: &str,
    case_name: Option<&str>,
    field_name: &str,
) -> Option<SymbolHandle> {
    let data_definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == type_name)?;
    if data_definition.type_parameters.count() > 0 {
        return None;
    }

    let field_type = construction_field_type(program, data_definition, case_name, field_name)?;
    let domain_name = crate::field_domain::domain_constraint_name(program, field_type)?;
    crate::field_domain::resolve_domain_symbol(program, &domain_name)
}

/// The declared type of a constructed field (a case PAYLOAD field for the named
/// variant, else a record/common FIELD member).
fn construction_field_type(
    program: &omega_typed_trees::TypedTrees,
    data_definition: &omega_typed_trees::data::DataDefinition,
    case_name: Option<&str>,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    if let Some(case_name) = case_name
        && let Some(variant) =
            program
                .data_members(data_definition)
                .iter()
                .find_map(|member| match member {
                    omega_typed_trees::data::DataMember::Variant(variant)
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
            omega_typed_trees::data::DataMember::Field(field)
                if field.name.as_str() == field_name =>
            {
                field
                    .type_reference
                    .is_valid()
                    .then_some(field.type_reference)
            }
            _ => None,
        })
}

/// Whether the assigned `value` is provably in `domain_symbol` at this statement:
/// a string literal the domain's classifier accepts (the construction-grant),
/// or a value carried in (a domained param/field) whose entry-context domain
/// fact implies `domain_symbol`. Mirrors the call-requires discharge.
fn value_proves_domain(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    if crate::field_domain::string_literal_expression_grants_domain(program, value, domain_symbol) {
        return true;
    }

    // Concat preserves a classifier-backed domain: a `left + right` whose two
    // operands are each provably in `domain_symbol` is itself in the domain, for
    // the recognized concat-preserving byte-predicates (valid_utf8/no_nul/
    // ascii_only/non_empty). This is the DOMAIN half of the rung-2 growth bound;
    // the capacity half -- that the result fits an owned `[u8; N]` carrier -- is
    // the separate length-fits check at the write site, so admitting the domain
    // here is not on its own a license to overflow the target.
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(value)
        && binary.operator == omega_typed_trees::expression::BinaryOperator::Add
        && crate::field_domain::domain_is_concat_preserving(program, domain_symbol)
    {
        let (left, right) = (binary.left, binary.right);
        if value_proves_domain(program, facts, state_flow, statement_index, left, domain_symbol)
            && value_proves_domain(program, facts, state_flow, statement_index, right, domain_symbol)
        {
            return true;
        }
    }

    // A value-position call whose target's DECLARED return type carries a domain
    // implying `domain_symbol` is accepted on the same trust basis as a declared
    // param domain at a call site: the signature's domain is trusted at use
    // sites, with its establishment enforced separately (params at call sites;
    // return bodies are the deferred returns-domain check). This is how a
    // `-> &[u8] in Utf8` value-call result flows into a `&[u8] in Utf8` field.
    if value_call_return_domain_implies(program, value, domain_symbol) {
        return true;
    }

    let value_label = program.expression_table.display_name(value);
    let entry_constraints = facts
        .flow
        .state_statement(state_flow, statement_index)
        .map(|statement| statement.entry_constraints)
        .unwrap_or(state_flow.entry_constraints);

    facts
        .flow
        .semantic_constraint_contexts(entry_constraints)
        .any(|context_handle| {
            let context = facts.semantic.contexts.get(context_handle);
            facts.semantic.context_view(context).facts().any(|fact| {
                let fact_domain = match fact.payload {
                    FactPayload::DomainMembership { domain_symbol, .. }
                    | FactPayload::ContractDomainMembership { domain_symbol, .. } => domain_symbol,
                    _ => return false,
                };
                if !facts.semantic.domain_implies(fact_domain, domain_symbol) {
                    return false;
                }
                // Match the fact's subject against the assigned value, by the
                // fact's place label and (for a contract membership) its declared
                // `value` expression label -- the same two-pronged match the
                // statement-transfer propagation uses.
                let omega_facts::FactPlace::Place(fact_place) = fact.place else {
                    return false;
                };
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
                        value.is_valid()
                            && program.expression_table.display_name(value) == value_label
                    }
                    _ => false,
                }
            })
        })
}

/// Whether `value` is a value-position call whose resolved target state declares
/// a return type carrying a domain that implies `domain_symbol`.
fn value_call_return_domain_implies(
    program: &omega_typed_trees::TypedTrees,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return false;
    };
    let Some(target) = crate::find_state(program, call.target_symbol) else {
        return false;
    };
    if !target.return_type.is_valid() {
        return false;
    }
    let Some(return_domain) = crate::field_domain::domain_constraint_name(program, target.return_type)
        .and_then(|name| crate::field_domain::resolve_domain_symbol(program, &name))
    else {
        return false;
    };
    program_domain_implies(program, return_domain, domain_symbol)
}

/// Whether `source_domain` is or imports `target_domain` (a declared
/// domain-membership chain). The `FactPlan::domain_implies` analog over the typed
/// program, for resolving signature-declared domains without a fact context.
fn program_domain_implies(
    program: &omega_typed_trees::TypedTrees,
    source_domain: SymbolHandle,
    target_domain: SymbolHandle,
) -> bool {
    if source_domain == target_domain {
        return true;
    }
    let Some(domain) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == source_domain)
    else {
        return false;
    };
    program.proof_facts(domain).iter().any(|fact| match fact {
        omega_typed_trees::domain::ProofFact::Membership(membership) => {
            program_domain_implies(program, membership.domain_symbol, target_domain)
        }
        omega_typed_trees::domain::ProofFact::Expression(_) => false,
    })
}

