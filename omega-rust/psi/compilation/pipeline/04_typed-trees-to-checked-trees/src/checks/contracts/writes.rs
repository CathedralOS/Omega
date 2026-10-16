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
//!
//! Successful membership judgments retain their exact operand occurrence in
//! `QualificationFacts::value_formations`. Constructor operands advance the
//! live contexts in authored order, so an earlier snapshot keeps its proof
//! even when a later operand withdraws membership from the original storage.
//! The record describes source membership only, not permission to copy or
//! transfer its carrier, and supplies no independent Terminal certificate.

use checked_trees::{CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;

use crate::labels::{machine_name, symbol_name};

mod domain_implications;
mod member_domains;
pub(super) use domain_implications::value_proves_qualification_in_contexts;
use domain_implications::{value_proves_domain, value_proves_qualification};
use member_domains::initializer_satisfies_predicate_domain;
pub(super) use member_domains::value_satisfies_domain_predicates_in_contexts;

/// Receiving obligations of fresh structural constructors, indexed once for
/// the state's authored expression walk. A row requests a judgment; it never
/// supplies membership. Successful judgments retain their operand's epoch.
struct ConstructionQualification {
    statement_index: usize,
    expression: ExpressionHandle,
    domain: SymbolHandle,
    semantic_domain: language_semantics::SemanticDomainId,
}

impl ConstructionQualification {
    fn site(&self) -> (usize, u32, u32) {
        (
            self.statement_index,
            self.expression.arena_index(),
            self.expression.generation(),
        )
    }
}

pub(super) fn check_domain_field_writes(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    frames: Option<&validation::CallFrameResolver<'_>>,
    decisions: &mut arena::Arena<checked_trees::CheckedValueQualification>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((machine, state)) = validation::machine_state_by_symbols(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    ) else {
        return;
    };

    // Synthesized zero constructors already retain their independently checked
    // zero-value memberships. Their diagnostic expression is the containing
    // literal, not an authored operand to prove again here.
    let mut construction_qualifications = Vec::new();
    let result_domains = program
        .machine_states(machine)
        .first()
        .map(|entry| {
            validation::field_domains::domain_constraint_identities(program, entry.return_type)
        })
        .unwrap_or_default();
    for (_, value) in facts.values.structural_values.nodes.iter() {
        if value.machine != state_flow.machine_symbol
            || value.state != state_flow.state_symbol
            || !matches!(
                value.kind,
                checked_trees::CheckedStructuralValueKind::ScalarArray { .. }
                    | checked_trees::CheckedStructuralValueKind::FixedArray { .. }
                    | checked_trees::CheckedStructuralValueKind::Record { .. }
                    | checked_trees::CheckedStructuralValueKind::Case(_)
            )
        {
            continue;
        }
        for (domain, semantic_domain) in
            validation::field_domains::domain_constraint_identities(program, value.type_reference)
        {
            // The checked result boundary introduces routed provenance only
            // after proving the returned root's predicates and custody. Requiring
            // that provenance on its fresh constructor would reject the issuer
            // before establishment. Defer only this exact returned root, without
            // recording a construction membership; children and locals still
            // owe their existing evidence at their own completion epochs.
            let authorized_result = result_domains.contains(&(domain, semantic_domain))
                && program.domain_definition(domain).is_some_and(|_| {
                    // The result contract above names the closed application;
                    // a family declaration's template ID is not that instance.
                    validation::field_domains::domain_requires_provenance(program, domain)
                        && crate::facts::qualification_evidence::machine_has_checked_domain_establishment(
                            program, machine, domain,
                        )
                })
                && facts.flow.control.exits.span_or_empty(state_flow.exits).iter().any(|exit| {
                    exit.machine_symbol == value.machine
                        && exit.state_symbol == value.state
                        && exit.statement_index == value.statement_index as usize
                        && super::return_values::exit_return_expression(program, exit)
                            == value.expression
                        && (!facts.flow.control.operands.is_valid(exit.returned_value)
                            || {
                                let operand = facts.flow.control.operands.get(exit.returned_value);
                                operand.expression == value.expression
                                    && operand.state_symbol == value.state
                                    && operand.statement_index == exit.statement_index
                            })
                });
            if authorized_result {
                continue;
            }
            construction_qualifications.push(ConstructionQualification {
                statement_index: value.statement_index as usize,
                expression: value.expression,
                domain,
                semantic_domain,
            });
        }
    }
    construction_qualifications.sort_by_key(ConstructionQualification::site);
    for (statement_index, statement) in program
        .statement_table
        .indexed_statements(state.statement_nodes)
    {
        // A local annotation is an obligation on its initializer, not an
        // establishment route. Check before the local's own statement facts
        // can be consulted, retaining exact indexed qualification identity.
        if let Some(local) = statement.as_local_data()
            && local.initial_value.is_valid()
        {
            for (domain_symbol, semantic_domain) in
                validation::field_domains::domain_constraint_identities(
                    program,
                    local.type_reference,
                )
            {
                let requires_provenance =
                    validation::field_domains::domain_requires_provenance(program, domain_symbol);
                // A `requires` predicate is equally an obligation on the
                // initializer: `self in T::Case` and `self.f` membership are
                // decided by the construction or by live evidence at its
                // place, never minted by the annotation itself.
                if validation::field_domains::domain_is_vacuous(
                    program,
                    domain_symbol,
                    &mut Vec::new(),
                ) {
                    // A predicate-free, route-free domain has a successful
                    // membership judgment too. Retain that exact initializer
                    // occurrence; lowering must not infer it from its receiver.
                    record_qualification(
                        decisions,
                        state_flow,
                        statement_index,
                        local.initial_value,
                        semantic_domain,
                    );
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
                        frames,
                    ))
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "cannot prove initializer of `{}` in {} is in domain `{}`; an annotation cannot establish routed qualification",
                        local.name, machine_name(program, state_flow.machine_symbol), symbol_name(program, domain_symbol),
                    )));
                } else {
                    record_qualification(
                        decisions,
                        state_flow,
                        statement_index,
                        local.initial_value,
                        semantic_domain,
                    );
                }
            }
        }
        // (1) Assignment into a domain-refined field, parameter, or local.
        if let Some(assignment) = statement.as_assignment() {
            for domain in validation::field_domains::assignment_target_domain_constraints(
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
                    validation::field_domains::domain_requires_provenance(program, domain_symbol);
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
                            frames,
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
                } else {
                    record_qualification(
                        decisions,
                        state_flow,
                        statement_index,
                        assignment.value,
                        domain.semantic_id,
                    );
                }
            }
        }

        // (1c) A concatenation establishes owned bytes in fresh activation
        // storage, which may replace a bounded owner or field but never
        // becomes a borrowed view (wiki/spec/terminal-psi/structural_access.md,
        // `EstablishByteSequenceConcatenation`). A `&[u8]` destination has no
        // capacity to bound the operands and no storage the result could live
        // in, so storing one there names bytes no declaration owns.
        let view_destination = match statement {
            StatementNode::Assignment(assignment) => {
                validation::field_domains::assignment_target_type_reference(
                    program,
                    machine,
                    state,
                    assignment.target,
                )
                .map(|target| {
                    (
                        program.expression_table.display_name(assignment.target),
                        target,
                        assignment.value,
                    )
                })
            }
            StatementNode::LocalData(local) if local.initial_value.is_valid() => Some((
                local.name.to_string(),
                local.type_reference,
                local.initial_value,
            )),
            _ => None,
        };
        if let Some((target_label, target, value)) = view_destination
            && program.is_borrowed_byte_slice(target)
            && matches!(
                program.expression_table.expression(value),
                ExpressionNode::Binary(binary)
                    if binary.operator == typed_trees::expression::BinaryOperator::Add
            )
            && crate::values::operator_is_builtin(&facts.operators, value)
        {
            diagnostics.push(Diagnostic::error(format!(
                "the concatenation stored into `{target_label}` in {} establishes owned bytes, \
                 but `{target_label}` is a borrowed `&[u8]` view that cannot hold them; store \
                 the concatenation into a bounded `[u8; N]` carrier whose capacity bounds both \
                 operands",
                machine_name(program, state_flow.machine_symbol),
            )));
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
        if let Some(transition) = statement.as_transition() {
            if let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                scan_construction_field_domains(
                    program,
                    facts,
                    state_flow,
                    &construction_qualifications,
                    statement_index,
                    guard,
                    &mut contexts,
                    decisions,
                    diagnostics,
                    frames,
                );
            }
            for target in transition.targets() {
                let mut branch_contexts = contexts.clone();
                let point = facts::ProgramPoint::TransitionArm {
                    machine_symbol: state_flow.machine_symbol,
                    state_symbol: state_flow.state_symbol,
                    statement_index,
                    transition_target: target,
                };
                // The fact plan retains each exact arm's contexts in append
                // order. Use that chain instead of scanning unrelated states
                // and arms for every construction check.
                branch_contexts.extend(facts.semantic.context_handles_at_point(point));
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
                        &construction_qualifications,
                        statement_index,
                        expression,
                        &mut branch_contexts,
                        decisions,
                        diagnostics,
                        frames,
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
                &construction_qualifications,
                statement_index,
                expression,
                &mut contexts,
                decisions,
                diagnostics,
                frames,
            );
        }
    }
}

/// The root expressions a statement carries (where a `T { f: X }` construction
/// may appear, possibly nested). The exhaustive set keeps the construction walk
/// in step with the statement shapes.
fn statement_root_expressions(
    program: &dyn ::validation::CheckingInput,
    statement: &StatementNode,
) -> Vec<ExpressionHandle> {
    match statement {
        StatementNode::RootBinding(binding) => [binding.receiver, binding.implementation_operand]
            .into_iter()
            .filter(|expression| expression.is_valid())
            .collect(),
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => Vec::new(),
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
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    construction_qualifications: &[ConstructionQualification],
    statement_index: usize,
    expression: ExpressionHandle,
    contexts: &mut Vec<facts::FactContextHandle>,
    decisions: &mut arena::Arena<checked_trees::CheckedValueQualification>,
    diagnostics: &mut Vec<Diagnostic>,
    frames: Option<&validation::CallFrameResolver<'_>>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => scan_construction_field_domains(
            program,
            facts,
            state_flow,
            construction_qualifications,
            statement_index,
            atomic.value,
            contexts,
            decisions,
            diagnostics,
            frames,
        ),
        ExpressionNode::Match(dispatch) => {
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                construction_qualifications,
                statement_index,
                dispatch.subject,
                contexts,
                decisions,
                diagnostics,
                frames,
            );
            let mut joined_contexts: Option<Vec<facts::FactContextHandle>> = None;
            for arm in program.expression_table.match_arms(dispatch.arms) {
                let mut arm_contexts = contexts.clone();
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    scan_construction_field_domains(
                        program,
                        facts,
                        state_flow,
                        construction_qualifications,
                        statement_index,
                        pattern,
                        &mut arm_contexts,
                        decisions,
                        diagnostics,
                        frames,
                    );
                }
                scan_construction_field_domains(
                    program,
                    facts,
                    state_flow,
                    construction_qualifications,
                    statement_index,
                    arm.value,
                    &mut arm_contexts,
                    decisions,
                    diagnostics,
                    frames,
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
                    construction_qualifications,
                    statement_index,
                    field.value,
                    contexts,
                    decisions,
                    diagnostics,
                    frames,
                );
                for (domain_symbol, semantic_domain) in construction_field_domain_identities(
                    program,
                    literal.type_symbol,
                    case_name.as_ref().map(|name| name.as_str()),
                    field.name.as_str(),
                ) {
                    // (a) The constructed value must be established in the domain.
                    if !value_proves_qualification_in_contexts(
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
                    } else {
                        record_qualification(
                            decisions,
                            state_flow,
                            statement_index,
                            field.value,
                            semantic_domain,
                        );
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
                    construction_qualifications,
                    statement_index,
                    *element,
                    contexts,
                    decisions,
                    diagnostics,
                    frames,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                construction_qualifications,
                statement_index,
                binary.left,
                contexts,
                decisions,
                diagnostics,
                frames,
            );
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                construction_qualifications,
                statement_index,
                binary.right,
                contexts,
                decisions,
                diagnostics,
                frames,
            );
        }
        ExpressionNode::Cast(cast) => {
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                construction_qualifications,
                statement_index,
                cast.value,
                contexts,
                decisions,
                diagnostics,
                frames,
            );
            judge_value_qualification_cast(
                program,
                facts,
                state_flow,
                statement_index,
                expression,
                cast,
                contexts,
                decisions,
                diagnostics,
                frames,
            );
        }
        ExpressionNode::Call(call) => {
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                construction_qualifications,
                statement_index,
                call.receiver,
                contexts,
                decisions,
                diagnostics,
                frames,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                scan_construction_field_domains(
                    program,
                    facts,
                    state_flow,
                    construction_qualifications,
                    statement_index,
                    *argument,
                    contexts,
                    decisions,
                    diagnostics,
                    frames,
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
                construction_qualifications,
                statement_index,
                indexed.collection,
                contexts,
                decisions,
                diagnostics,
                frames,
            );
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                construction_qualifications,
                statement_index,
                indexed.index,
                contexts,
                decisions,
                diagnostics,
                frames,
            );
        }
        ExpressionNode::Member(member) => scan_construction_field_domains(
            program,
            facts,
            state_flow,
            construction_qualifications,
            statement_index,
            member.receiver,
            contexts,
            decisions,
            diagnostics,
            frames,
        ),
        ExpressionNode::Borrow(inner) => scan_construction_field_domains(
            program,
            facts,
            state_flow,
            construction_qualifications,
            statement_index,
            inner.target,
            contexts,
            decisions,
            diagnostics,
            frames,
        ),
        ExpressionNode::Range(range) => {
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                construction_qualifications,
                statement_index,
                range.start,
                contexts,
                decisions,
                diagnostics,
                frames,
            );
            scan_construction_field_domains(
                program,
                facts,
                state_flow,
                construction_qualifications,
                statement_index,
                range.end,
                contexts,
                decisions,
                diagnostics,
                frames,
            );
        }
        ExpressionNode::Unary(unary) => scan_construction_field_domains(
            program,
            facts,
            state_flow,
            construction_qualifications,
            statement_index,
            unary.operand,
            contexts,
            decisions,
            diagnostics,
            frames,
        ),
        typed_trees::literal_expression_pattern!() => {}
    }
    // Children complete before the next operand may mutate their source.
    // Reuse this walk's exact live contexts, just as record-field admission does.
    let site = (
        statement_index,
        expression.arena_index(),
        expression.generation(),
    );
    let first = construction_qualifications.partition_point(|row| row.site() < site);
    let rows = &construction_qualifications[first..];
    for row in rows.iter().take_while(|row| row.site() == site) {
        let mut captures = facts
            .flow
            .control
            .operands
            .iter()
            .map(|(_, operand)| operand)
            .filter(|operand| {
                operand.expression == expression
                    && operand.state_symbol == state_flow.state_symbol
                    && operand.statement_index == statement_index
            });
        let capture = captures.next().filter(|_| captures.next().is_none());
        if value_proves_qualification_in_contexts(
            program,
            facts,
            state_flow,
            statement_index,
            expression,
            row.domain,
            row.semantic_domain,
            contexts,
        ) || (capture.is_some()
            && !validation::field_domains::domain_requires_provenance(program, row.domain)
            && value_satisfies_domain_predicates_in_contexts(
                program,
                facts,
                state_flow,
                statement_index,
                expression,
                row.domain,
                contexts,
                &mut Vec::new(),
                capture,
                frames,
            ))
        {
            // A completed constructor can establish a predicate from its actual
            // fields. This uses its operand epoch and never introduces routed
            // provenance or assumes the promised annotation as evidence.
            record_qualification(
                decisions,
                state_flow,
                statement_index,
                expression,
                row.semantic_domain,
            );
        } else {
            diagnostics.push(Diagnostic::error(format!(
                "construction value is not proven in domain `{}`",
                symbol_name(program, row.domain),
            )));
        }
    }
}

/// The declared predicate-domain symbols of a constructed field: a case literal's
/// PAYLOAD field (for the named variant) or a record/common struct field whose
/// declared type carries one or more predicate-bearing domain constraints.
/// `validation::field_domains::construction_field_type` plus domain
/// extraction.
fn construction_field_domain_identities(
    program: &dyn ::validation::CheckingInput,
    type_symbol: SymbolHandle,
    case_name: Option<&str>,
    field_name: &str,
) -> Vec<(SymbolHandle, language_semantics::SemanticDomainId)> {
    let Some(data_definition) = program.data_definition_by_symbol(type_symbol) else {
        return Vec::new();
    };
    if data_definition.type_parameters.count() > 0 {
        return Vec::new();
    }

    let Some(field_type) = validation::field_domains::construction_field_type(
        program,
        data_definition,
        case_name,
        field_name,
    ) else {
        return Vec::new();
    };
    validation::field_domains::domain_constraint_identities(program, field_type)
        .into_iter()
        .filter(|(symbol, _)| {
            validation::field_domains::domain_requires_provenance(program, *symbol)
                || program
                    .domain_definitions()
                    .iter()
                    .any(|domain| domain.symbol == *symbol && domain.predicate_body.is_present())
        })
        .collect()
}

/// Retain the successful source judgment, not the field annotation. The walk
/// calls this before advancing through the next operand's mutation contexts.
/// `value as T in D` over a structural carrier owes `D`'s predicates at the
/// cast's own occurrence ("Predicate qualification: prove the target
/// predicates", domains.md, exact coercion). The operand's live contexts decide
/// them exactly as they decide a qualified initializer: constructed fields,
/// assigned values, guards and carried memberships. Scalar carriers keep the
/// value-interval judgment in `validation`. Routed qualifications, scalar or
/// structural, instead retain the operand's exact live membership evidence;
/// predicate proofs alone cannot introduce their provenance.
#[allow(clippy::too_many_arguments)]
fn judge_value_qualification_cast(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    statement_index: usize,
    expression: ExpressionHandle,
    cast: &typed_trees::expression::TableCastExpression,
    contexts: &[facts::FactContextHandle],
    decisions: &mut arena::Arena<checked_trees::CheckedValueQualification>,
    diagnostics: &mut Vec<Diagnostic>,
    frames: Option<&validation::CallFrameResolver<'_>>,
) {
    let domain_symbol = cast.semantic_domain_symbol;
    let Some(domain) = program.domain_definition(domain_symbol) else {
        return;
    };
    let requires_provenance =
        validation::field_domains::domain_requires_provenance(program, domain_symbol);
    if cast.semantic_domain.is_empty()
        || cast.form.is_recast()
        || (!requires_provenance
            && program
                .primitive_type_reference(domain.target_type)
                .is_some())
        || validation::field_domains::domain_is_vacuous(program, domain_symbol, &mut Vec::new())
    {
        return;
    }
    let semantic_domain = if cast.semantic_domain_id.is_valid() {
        cast.semantic_domain_id
    } else {
        domain.semantic_id
    };
    if value_proves_qualification_in_contexts(
        program,
        facts,
        state_flow,
        statement_index,
        cast.value,
        domain_symbol,
        semantic_domain,
        contexts,
    ) || (!requires_provenance
        && value_satisfies_domain_predicates_in_contexts(
            program,
            facts,
            state_flow,
            statement_index,
            cast.value,
            domain_symbol,
            contexts,
            &mut Vec::new(),
            None,
            frames,
        ))
    {
        record_qualification(
            decisions,
            state_flow,
            statement_index,
            expression,
            semantic_domain,
        );
        return;
    }
    diagnostics.push(qualification_cast_refusal(
        program,
        domain_symbol,
        requires_provenance,
    ));
}

fn qualification_cast_refusal(
    program: &dyn ::validation::CheckingInput,
    domain: SymbolHandle,
    requires_provenance: bool,
) -> Diagnostic {
    let reason = if requires_provenance {
        "cannot establish a non-vacuous domain; every predicate must be proved and routed provenance must come from an exact requirement authorized by the domain"
    } else {
        "cannot prove every domain predicate from the checked operand value; constrain that value with a guarantee or guard"
    };
    Diagnostic::error(format!(
        "`as ... in {}` {reason}",
        symbol_name(program, domain)
    ))
}

/// A routed cast outside an executable value flow has no checked operand
/// provenance. Moving its judgment here must not admit unvisited constants or
/// declaration expressions merely because their asserted target is routed.
pub(super) fn check_routed_cast_coverage(
    program: &dyn ::validation::CheckingInput,
    decisions: &arena::Arena<checked_trees::CheckedValueQualification>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (expression, node) in program.expression_table.expression_entries() {
        let ExpressionNode::Cast(cast) = node else {
            continue;
        };
        if !cast.semantic_domain.is_empty()
            && validation::field_domains::domain_requires_provenance(
                program,
                cast.semantic_domain_symbol,
            )
            && !decisions
                .iter()
                .any(|(_, decision)| decision.expression == expression)
        {
            diagnostics.push(qualification_cast_refusal(
                program,
                cast.semantic_domain_symbol,
                true,
            ));
        }
    }
}

fn record_qualification(
    decisions: &mut arena::Arena<checked_trees::CheckedValueQualification>,
    state: &FlowStateFact,
    statement_index: usize,
    expression: ExpressionHandle,
    semantic_domain: language_semantics::SemanticDomainId,
) {
    let Ok(statement_index) = u32::try_from(statement_index) else {
        return;
    };
    decisions.append(checked_trees::CheckedValueQualification {
        machine: state.machine_symbol,
        state: state.state_symbol,
        statement_index,
        expression,
        semantic_domain,
    });
}
