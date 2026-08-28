use crate::lowerer::{PendingAuthoredExpression, PendingAuthoredProofMembership};
use psi_diagnostics::Diagnostic;
use psi_language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind as Kind,
    AuthoredDeclarationSelectionLateBinding as LateBinding,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError,
};
use psi_source::SourceSpan;
use psi_symbol_resolved_trees::{
    SymbolResolvedTrees,
    expression::{ExpressionHandle, ExpressionNode},
};
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Copy)]
enum CandidateTarget {
    Resolved(SymbolHandle),
    LateBound(LateBinding),
}

#[derive(Debug, Clone, Copy)]
struct Candidate {
    expression: ExpressionHandle,
    source_span: SourceSpan,
    kind: Kind,
    target: CandidateTarget,
}

pub(crate) fn finalize_authored_expression_selections(
    program: &mut SymbolResolvedTrees,
    pending: &[PendingAuthoredExpression],
    pending_proof_memberships: &[PendingAuthoredProofMembership],
) -> Result<(), Diagnostic> {
    let mut seen = Vec::new();

    for pending_expression in pending {
        if seen.contains(&pending_expression.expression) {
            continue;
        }
        seen.push(pending_expression.expression);

        let candidates = expression_candidates(program, pending_expression.expression);
        for candidate in candidates {
            let occurrence = match candidate.target {
                CandidateTarget::Resolved(symbol) => program
                    .record_resolved_authored_declaration_selection(
                        candidate.source_span,
                        pending_expression.exposure,
                        candidate.kind,
                        symbol,
                    ),
                CandidateTarget::LateBound(binding) => program
                    .record_late_bound_authored_declaration_selection(
                        candidate.source_span,
                        pending_expression.exposure,
                        candidate.kind,
                        binding,
                    ),
            }
            .map_err(record_diagnostic)?;

            program
                .tables
                .bodies
                .expressions
                .attach_authored_selection_occurrences(candidate.expression, [occurrence]);
        }
    }

    for pending in pending_proof_memberships {
        let psi_symbol_resolved_trees::domain::ProofFact::Membership(membership) =
            program.tables.declarations.proof_facts.get(pending.fact)
        else {
            return Err(Diagnostic::error(
                "authored proof-membership custody no longer identifies a membership fact",
            ));
        };
        let members = program
            .tables
            .declarations
            .domain_path_members
            .span_or_empty(membership.domain);
        let source_span = path_span(members, SourceSpan::default());
        let target = resolved_or_late(
            membership.domain_symbol,
            LateBinding::CheckedDomainMembership,
        );
        match target {
            CandidateTarget::Resolved(symbol) => program
                .record_resolved_authored_declaration_selection(
                    source_span,
                    pending.exposure,
                    Kind::DomainMembership,
                    symbol,
                ),
            CandidateTarget::LateBound(binding) => program
                .record_late_bound_authored_declaration_selection(
                    source_span,
                    pending.exposure,
                    Kind::DomainMembership,
                    binding,
                ),
        }
        .map_err(record_diagnostic)?;
    }

    for candidate in conformance_bound_candidates(program) {
        record_unattached_candidate(program, candidate)?;
    }

    finalize_authored_statement_call_selections(program)?;

    for candidate in statement_candidates(program) {
        record_unattached_candidate(program, candidate)?;
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum AuthoredStatementCallSite {
    Statement {
        statements: psi_arena::HandleSpan<psi_symbol_resolved_trees::statement::Statement>,
        offset: usize,
    },
    TransitionTarget {
        statements: psi_arena::HandleSpan<psi_symbol_resolved_trees::statement::Statement>,
        offset: usize,
        continuation: bool,
    },
}

#[derive(Debug, Clone, Copy)]
struct AuthoredStatementCallCandidate {
    site: AuthoredStatementCallSite,
    source_span: SourceSpan,
    target: CandidateTarget,
}

fn finalize_authored_statement_call_selections(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    use psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as Exposure;
    use psi_symbol_resolved_trees::statement::Statement;

    let mut candidates = Vec::new();
    for machine in program.machines.iter() {
        for state_handle in program.machine_state_handles(machine.states) {
            let state = program.machine_state(*state_handle);
            for (offset, statement) in program
                .state_statements(state.statements)
                .iter()
                .enumerate()
            {
                match statement {
                    Statement::Call(call)
                        if call.operational_acknowledgement.origin
                            == psi_language_semantics::CallOperationalAcknowledgementOrigin::Source
                            && nonempty(call.target.source_span()) =>
                    {
                        candidates.push(AuthoredStatementCallCandidate {
                            site: AuthoredStatementCallSite::Statement {
                                statements: state.statements,
                                offset,
                            },
                            source_span: call.target.source_span(),
                            target: resolved_or_late(call.target_symbol, LateBinding::CheckedCall),
                        });
                    }
                    Statement::Transition(transition) => {
                        collect_authored_transition_call_candidate(
                            state.statements,
                            offset,
                            false,
                            &transition.target,
                            &mut candidates,
                        );
                        if let Some(continuation) = &transition.continuation {
                            collect_authored_transition_call_candidate(
                                state.statements,
                                offset,
                                true,
                                continuation,
                                &mut candidates,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    for candidate in candidates {
        let occurrence = match candidate.target {
            CandidateTarget::Resolved(symbol) => program
                .record_resolved_authored_declaration_selection(
                    candidate.source_span,
                    Exposure::PrivateImplementation,
                    Kind::Call,
                    symbol,
                ),
            CandidateTarget::LateBound(binding) => program
                .record_late_bound_authored_declaration_selection(
                    candidate.source_span,
                    Exposure::PrivateImplementation,
                    Kind::Call,
                    binding,
                ),
        }
        .map_err(record_diagnostic)?;
        attach_statement_call_occurrence(program, candidate.site, occurrence)?;
    }
    Ok(())
}

fn collect_authored_transition_call_candidate(
    statements: psi_arena::HandleSpan<psi_symbol_resolved_trees::statement::Statement>,
    offset: usize,
    continuation: bool,
    target: &psi_symbol_resolved_trees::statement::TransitionTarget,
    candidates: &mut Vec<AuthoredStatementCallCandidate>,
) {
    let psi_symbol_resolved_trees::statement::TransitionTarget::Named(target) = target else {
        return;
    };
    if nonempty(target.source_span) {
        candidates.push(AuthoredStatementCallCandidate {
            site: AuthoredStatementCallSite::TransitionTarget {
                statements,
                offset,
                continuation,
            },
            source_span: target.source_span,
            target: resolved_or_late(target.symbol, LateBinding::CheckedCall),
        });
    }
}

fn attach_statement_call_occurrence(
    program: &mut SymbolResolvedTrees,
    site: AuthoredStatementCallSite,
    occurrence: AuthoredDeclarationSelectionOccurrenceId,
) -> Result<(), Diagnostic> {
    use psi_symbol_resolved_trees::statement::{Statement, TransitionTarget};

    let (statements, offset) = match site {
        AuthoredStatementCallSite::Statement { statements, offset }
        | AuthoredStatementCallSite::TransitionTarget {
            statements, offset, ..
        } => (statements, offset),
    };
    let Some(statement) = program
        .tables
        .declarations
        .state_statements
        .span_mut_or_empty(statements)
        .get_mut(offset)
    else {
        return Err(Diagnostic::error(
            "authored call-selection site no longer identifies a statement",
        ));
    };
    match (site, statement) {
        (AuthoredStatementCallSite::Statement { .. }, Statement::Call(call)) => {
            call.authored_call_selection = Some(occurrence);
        }
        (
            AuthoredStatementCallSite::TransitionTarget { continuation, .. },
            Statement::Transition(transition),
        ) => {
            let target = if continuation {
                transition.continuation.as_mut()
            } else {
                Some(&mut transition.target)
            };
            let Some(TransitionTarget::Named(target)) = target else {
                return Err(Diagnostic::error(
                    "authored call-selection site no longer identifies a named transition",
                ));
            };
            target.authored_call_selection = Some(occurrence);
        }
        _ => {
            return Err(Diagnostic::error(
                "authored call-selection site changed kind during symbol resolution",
            ));
        }
    }
    Ok(())
}

fn nonempty(source_span: SourceSpan) -> bool {
    source_span.span.start < source_span.span.end
}

/// Retain explicit realization-machine references only after closed
/// conformance normalization has resolved each target to one exact machine.
/// The referenced member remains private implementation even when the complete
/// conformance itself is public.
pub(crate) fn finalize_conformance_reference_selections(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    use psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as Exposure;

    let selections = program
        .conformances
        .iter()
        .flat_map(|conformance| match &conformance.implementation {
            psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed {
                rows,
            } => rows
                .iter()
                .filter_map(|row| {
                    row.authored_realization_source_span
                        .map(|source_span| (source_span, row.realization_machine))
                })
                .collect::<Vec<_>>(),
            psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::AttachedRequirementMachines => {
                Vec::new()
            }
        })
        .collect::<Vec<_>>();
    for (source_span, realization_machine) in selections {
        if !realization_machine.is_valid() {
            return Err(Diagnostic::error(
                "explicit conformance realization remained unresolved after normalization",
            )
            .with_source_span(source_span));
        }
        program
            .record_resolved_authored_declaration_selection(
                source_span,
                Exposure::PrivateImplementation,
                Kind::StaticPathSegment,
                realization_machine,
            )
            .map_err(record_diagnostic)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct UnattachedCandidate {
    source_span: SourceSpan,
    exposure: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    kind: Kind,
    target: CandidateTarget,
}

fn record_unattached_candidate(
    program: &mut SymbolResolvedTrees,
    candidate: UnattachedCandidate,
) -> Result<(), Diagnostic> {
    match candidate.target {
        CandidateTarget::Resolved(symbol) => program
            .record_resolved_authored_declaration_selection(
                candidate.source_span,
                candidate.exposure,
                candidate.kind,
                symbol,
            ),
        CandidateTarget::LateBound(binding) => program
            .record_late_bound_authored_declaration_selection(
                candidate.source_span,
                candidate.exposure,
                candidate.kind,
                binding,
            ),
    }
    .map(|_| ())
    .map_err(record_diagnostic)
}

fn conformance_bound_candidates(program: &SymbolResolvedTrees) -> Vec<UnattachedCandidate> {
    use psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as Exposure;

    let mut candidates = Vec::new();
    for machine in program.machines.iter() {
        collect_conformance_bound_candidates(
            program,
            &machine.conformance_bounds,
            if machine.is_public {
                Exposure::PublicInterface
            } else {
                Exposure::PrivateImplementation
            },
            &mut candidates,
        );
    }
    for trait_definition in program.traits.iter() {
        collect_conformance_bound_candidates(
            program,
            &trait_definition.conformance_bounds,
            if trait_definition.is_public {
                Exposure::PublicInterface
            } else {
                Exposure::PrivateImplementation
            },
            &mut candidates,
        );
    }
    for data_definition in program.data_definitions.iter() {
        let Some(selection) = data_definition
            .quotient
            .as_ref()
            .and_then(|quotient| quotient.equivalence.as_ref())
        else {
            continue;
        };
        if selection.conformance_name.is_source_backed() {
            candidates.push(UnattachedCandidate {
                source_span: selection.conformance_name.source_span(),
                // The selected proof implementation licenses formation but is
                // not quotient API identity.
                exposure: Exposure::PrivateImplementation,
                kind: Kind::Conformance,
                target: resolved_or_late(
                    selection.conformance_symbol,
                    LateBinding::CheckedConformance,
                ),
            });
        }
    }
    candidates
}

fn collect_conformance_bound_candidates(
    program: &SymbolResolvedTrees,
    bounds: &[psi_symbol_resolved_trees::machine::GenericConformanceBound],
    exposure: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    candidates: &mut Vec<UnattachedCandidate>,
) {
    for bound in bounds {
        if bound.carrier_name.is_source_backed() {
            candidates.push(UnattachedCandidate {
                source_span: bound.carrier_name.source_span(),
                exposure,
                kind: Kind::TypeReference,
                target: resolved_or_late(bound.carrier, LateBinding::CheckedStaticPathSegment),
            });
        }
        if let Some(selected) = &bound.selected_conformance {
            let fallback_span = bound.carrier_name.source_span();
            let source_span = path_span(&selected.path, fallback_span);
            candidates.push(UnattachedCandidate {
                source_span,
                exposure,
                kind: Kind::Conformance,
                target: resolved_or_late(selected.symbol, LateBinding::CheckedConformance),
            });
            if let Some(application) = &selected.application {
                collect_bound_static_argument_candidates(
                    program,
                    &application.arguments,
                    exposure,
                    source_span,
                    candidates,
                );
            }
        }
    }
}

fn collect_bound_static_argument_candidates(
    program: &SymbolResolvedTrees,
    arguments: &[psi_symbol_resolved_trees::expression::StaticMachineArgument],
    exposure: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    fallback_span: SourceSpan,
    candidates: &mut Vec<UnattachedCandidate>,
) {
    for argument in arguments {
        if argument.path.iter().any(|member| member.is_source_backed())
            && (!argument.symbol.is_valid()
                || is_selectable_declaration_symbol(program, argument.symbol))
        {
            candidates.push(UnattachedCandidate {
                source_span: path_span(&argument.path, fallback_span),
                exposure,
                kind: static_argument_kind(program, argument.symbol),
                target: resolved_or_late(argument.symbol, LateBinding::CheckedStaticArgument),
            });
        }
        if let Some(application) = &argument.application {
            collect_bound_static_argument_candidates(
                program,
                &application.arguments,
                exposure,
                fallback_span,
                candidates,
            );
        }
    }
}

fn statement_candidates(program: &SymbolResolvedTrees) -> Vec<UnattachedCandidate> {
    let mut candidates = Vec::new();
    for machine in program.machines.iter() {
        for state_handle in program.machine_state_handles(machine.states) {
            let state = program.machine_state(*state_handle);
            for statement in program
                .tables
                .declarations
                .state_statements
                .span_or_empty(state.statements)
            {
                let psi_symbol_resolved_trees::statement::Statement::Call(call) = statement else {
                    continue;
                };
                collect_statement_static_argument_candidates(
                    program,
                    &call.machine_arguments,
                    call.target.source_span(),
                    &mut candidates,
                );
            }
        }
    }
    candidates
        .retain(|candidate| candidate.source_span.span.start < candidate.source_span.span.end);
    candidates
}

fn collect_statement_static_argument_candidates(
    program: &SymbolResolvedTrees,
    arguments: &[psi_symbol_resolved_trees::expression::StaticMachineArgument],
    fallback_span: SourceSpan,
    candidates: &mut Vec<UnattachedCandidate>,
) {
    for argument in arguments {
        if argument.path.iter().any(|member| member.is_source_backed())
            && (!argument.symbol.is_valid()
                || is_selectable_declaration_symbol(program, argument.symbol))
        {
            candidates.push(UnattachedCandidate {
                source_span: path_span(&argument.path, fallback_span),
                exposure: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
                kind: static_argument_kind(program, argument.symbol),
                target: resolved_or_late(argument.symbol, LateBinding::CheckedStaticArgument),
            });
        }
        if let Some(application) = &argument.application {
            collect_statement_static_argument_candidates(
                program,
                &application.arguments,
                fallback_span,
                candidates,
            );
        }
    }
}

fn expression_candidates(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
) -> Vec<Candidate> {
    let expressions = &program.tables.bodies.expressions;
    let expression_span = expressions.source_span(expression);
    let mut candidates = Vec::new();

    match expressions.expression(expression) {
        ExpressionNode::Binary(_) | ExpressionNode::Indexed(_) | ExpressionNode::Unary(_) => {
            candidates.push(Candidate {
                expression,
                source_span: expression_span,
                kind: Kind::Operator,
                target: CandidateTarget::LateBound(LateBinding::CheckedOperator),
            })
        }
        ExpressionNode::Call(call) => {
            if call.operational_acknowledgement.origin
                == psi_language_semantics::CallOperationalAcknowledgementOrigin::Source
            {
                candidates.push(Candidate {
                    expression,
                    source_span: call.target.source_span(),
                    kind: Kind::Call,
                    target: resolved_or_late(call.target_symbol, LateBinding::CheckedCall),
                });
            }
            collect_static_argument_candidates(
                program,
                expression,
                &call.machine_arguments,
                &mut candidates,
            );
        }
        ExpressionNode::Member(member) => candidates.push(Candidate {
            expression,
            source_span: member.member.source_span(),
            kind: Kind::MemberAccess,
            target: resolved_or_late(member.member_symbol, LateBinding::CheckedMember),
        }),
        ExpressionNode::Membership(membership) => {
            let members = expressions.name_path_members(membership.domain);
            if membership.domain_symbol.is_valid() {
                candidates.push(Candidate {
                    expression,
                    source_span: path_span(members, expression_span),
                    kind: Kind::DomainMembership,
                    target: CandidateTarget::Resolved(membership.domain_symbol),
                });
            } else {
                if membership.case_type_symbol.is_valid() {
                    candidates.push(Candidate {
                        expression,
                        source_span: members
                            .first()
                            .map_or(expression_span, |member| member.source_span()),
                        kind: Kind::CaseReference,
                        target: CandidateTarget::Resolved(membership.case_type_symbol),
                    });
                }
                candidates.push(Candidate {
                    expression,
                    source_span: members
                        .last()
                        .map_or(expression_span, |member| member.source_span()),
                    kind: Kind::CaseMembership,
                    target: resolved_or_late(
                        membership.case_symbol,
                        LateBinding::CheckedCaseMembership,
                    ),
                });
            }
        }
        ExpressionNode::Name(path) if !path.is_self_value => {
            let members = expressions.name_path_members(path.members);
            let symbols = expressions.name_path_member_symbols(path.member_symbols);
            for (offset, member) in members.iter().enumerate() {
                let symbol = symbols
                    .get(offset)
                    .copied()
                    .unwrap_or_else(SymbolHandle::invalid);
                // A bare unresolved root is a lexical value place (parameter
                // or local), not a declaration selection. Resolved lexical
                // binders are excluded for the same reason. Later segments
                // remain authored field/case/declaration selections and may
                // still require checked contextual resolution.
                if (offset == 0 && !symbol.is_valid())
                    || (symbol.is_valid() && !is_selectable_declaration_symbol(program, symbol))
                {
                    continue;
                }
                candidates.push(Candidate {
                    expression,
                    source_span: member.source_span(),
                    kind: Kind::StaticPathSegment,
                    target: resolved_or_late(symbol, LateBinding::CheckedStaticPathSegment),
                });
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            candidates.push(Candidate {
                expression,
                source_span: literal.type_name.source_span(),
                kind: Kind::StructLiteralType,
                target: resolved_or_late(
                    literal.type_symbol,
                    LateBinding::CheckedStructLiteralType,
                ),
            });
            if let Some(case_name) = &literal.case_name {
                candidates.push(Candidate {
                    expression,
                    source_span: case_name.source_span(),
                    kind: Kind::StructLiteralCase,
                    target: resolved_or_late(
                        literal.case_symbol.unwrap_or_else(SymbolHandle::invalid),
                        LateBinding::CheckedStructLiteralCase,
                    ),
                });
            }
            candidates.extend(
                expressions
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| Candidate {
                        expression,
                        source_span: field.name.source_span(),
                        kind: Kind::StructLiteralField,
                        target: resolved_or_late(
                            field.field_symbol,
                            LateBinding::CheckedStructLiteralField,
                        ),
                    }),
            );
        }
        _ => {}
    }

    // Parser/lowering-generated helpers carry no token span. They may appear
    // beneath an authored expression, but only a candidate with an exact
    // source token is itself an authored declaration selection.
    candidates
        .retain(|candidate| candidate.source_span.span.start < candidate.source_span.span.end);
    candidates
}

fn is_selectable_declaration_symbol(program: &SymbolResolvedTrees, symbol: SymbolHandle) -> bool {
    !matches!(
        program.symbols.get(symbol).kind,
        psi_symbols::SymbolKind::Unknown
            | psi_symbols::SymbolKind::Root
            | psi_symbols::SymbolKind::Local
            | psi_symbols::SymbolKind::Parameter
            | psi_symbols::SymbolKind::TypeParameter
            | psi_symbols::SymbolKind::ConformanceParameter
            | psi_symbols::SymbolKind::MachineParameter
            | psi_symbols::SymbolKind::PropositionParameter
            | psi_symbols::SymbolKind::PropositionMachineParameter
    )
}

fn collect_static_argument_candidates(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
    arguments: &[psi_symbol_resolved_trees::expression::StaticMachineArgument],
    candidates: &mut Vec<Candidate>,
) {
    for argument in arguments {
        if argument.path.iter().any(|member| member.is_source_backed())
            && (!argument.symbol.is_valid()
                || is_selectable_declaration_symbol(program, argument.symbol))
        {
            candidates.push(Candidate {
                expression,
                source_span: path_span(
                    &argument.path,
                    program.tables.bodies.expressions.source_span(expression),
                ),
                kind: static_argument_kind(program, argument.symbol),
                target: resolved_or_late(argument.symbol, LateBinding::CheckedStaticArgument),
            });
        }
        if let Some(application) = &argument.application {
            collect_static_argument_candidates(
                program,
                expression,
                &application.arguments,
                candidates,
            );
        }
    }
}

fn static_argument_kind(program: &SymbolResolvedTrees, symbol: SymbolHandle) -> Kind {
    if symbol.is_valid()
        && matches!(
            program.symbols.get(symbol).kind,
            psi_symbols::SymbolKind::Conformance | psi_symbols::SymbolKind::ConformanceParameter
        )
    {
        Kind::Conformance
    } else {
        Kind::StaticArgument
    }
}

fn resolved_or_late(symbol: SymbolHandle, late: LateBinding) -> CandidateTarget {
    if symbol.is_valid() {
        CandidateTarget::Resolved(symbol)
    } else {
        CandidateTarget::LateBound(late)
    }
}

fn path_span(
    members: &[psi_symbol_resolved_trees::name::DiagnosticName],
    fallback: SourceSpan,
) -> SourceSpan {
    let Some(first) = members.first() else {
        return fallback;
    };
    let Some(last) = members.last() else {
        return fallback;
    };
    if first.source_span().source_id == last.source_span().source_id {
        SourceSpan::new(
            first.source_span().source_id,
            psi_source::Span::new(first.source_span().span.start, last.source_span().span.end),
        )
    } else {
        fallback
    }
}

fn record_diagnostic(error: AuthoredDeclarationSelectionRecordError) -> Diagnostic {
    Diagnostic::error(format!(
        "failed to retain authored declaration selection: {error:?}"
    ))
}
