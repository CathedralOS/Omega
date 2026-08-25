use crate::lowerer::PendingAuthoredExpression;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind as Kind,
    AuthoredDeclarationSelectionLateBinding as LateBinding,
    AuthoredDeclarationSelectionRecordError,
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

    Ok(())
}

fn expression_candidates(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
) -> Vec<Candidate> {
    let expressions = &program.tables.bodies.expressions;
    let expression_span = expressions.source_span(expression);
    let mut candidates = Vec::new();

    match expressions.expression(expression) {
        ExpressionNode::Binary(_) | ExpressionNode::Unary(_) => candidates.push(Candidate {
            expression,
            source_span: expression_span,
            kind: Kind::Operator,
            target: CandidateTarget::LateBound(LateBinding::CheckedOperator),
        }),
        ExpressionNode::Call(call)
            if call.operational_acknowledgement.origin
                == psi_language_semantics::CallOperationalAcknowledgementOrigin::Source =>
        {
            candidates.push(Candidate {
                expression,
                source_span: call.target.source_span(),
                kind: Kind::Call,
                target: resolved_or_late(call.target_symbol, LateBinding::CheckedCall),
            });
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

    candidates
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
