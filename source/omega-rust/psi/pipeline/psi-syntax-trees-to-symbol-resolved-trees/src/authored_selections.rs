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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug)]
struct CandidateGroup {
    source_span: SourceSpan,
    exposure: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    kind: Kind,
    compiler_partition:
        Option<psi_language_semantics::declaration_selection::CompilerDerivedSelectionPartition>,
    target: CandidateTarget,
    expressions: Vec<ExpressionHandle>,
}

fn reconcile_copy_targets(
    program: &SymbolResolvedTrees,
    source_span: SourceSpan,
    kind: Kind,
    retained: CandidateTarget,
    candidate: CandidateTarget,
) -> Result<CandidateTarget, Diagnostic> {
    match (retained, candidate) {
        (CandidateTarget::Resolved(left), CandidateTarget::Resolved(right)) if left == right => {
            Ok(retained)
        }
        (CandidateTarget::LateBound(left), CandidateTarget::LateBound(right)) if left == right => {
            Ok(retained)
        }
        (CandidateTarget::Resolved(_), CandidateTarget::LateBound(_)) => Ok(retained),
        (CandidateTarget::LateBound(_), CandidateTarget::Resolved(_)) => Ok(candidate),
        // Const/type specialization copies declaration symbols as well as the
        // authored expression. Equal source-backed declaration coordinates
        // and symbol kinds therefore identify one authored declaration even
        // when the executable copy has a distinct arena handle. Retain the
        // earliest symbol, which is the unspecialized authored declaration.
        (CandidateTarget::Resolved(left), CandidateTarget::Resolved(right))
            if resolved_symbols_share_authored_declaration(program, left, right) =>
        {
            Ok(CandidateTarget::Resolved(earlier_symbol(left, right)))
        }
        (CandidateTarget::Resolved(left), CandidateTarget::Resolved(right)) => {
            Err(Diagnostic::error(format!(
                "compiler-derived copies of one authored {kind:?} selection resolved inconsistently: `{}` versus `{}`",
                program.symbols.display_path(left, "::"),
                program.symbols.display_path(right, "::"),
            ))
            .with_source_span(source_span))
        }
        _ => Err(Diagnostic::error(format!(
            "compiler-derived copies of one authored {kind:?} selection resolved inconsistently"
        ))
        .with_source_span(source_span)),
    }
}

fn earlier_symbol(left: SymbolHandle, right: SymbolHandle) -> SymbolHandle {
    if left.arena_index() <= right.arena_index() {
        left
    } else {
        right
    }
}

fn resolved_symbols_share_authored_declaration(
    program: &SymbolResolvedTrees,
    left: SymbolHandle,
    right: SymbolHandle,
) -> bool {
    program.symbols.get(left).kind == program.symbols.get(right).kind
        && program.symbols.symbol_source_span(left).is_some()
        && program.symbols.symbol_source_span(left) == program.symbols.symbol_source_span(right)
}

pub(crate) fn finalize_authored_expression_selections(
    program: &mut SymbolResolvedTrees,
    pending: &[PendingAuthoredExpression],
    pending_proof_memberships: &[PendingAuthoredProofMembership],
) -> Result<(), Diagnostic> {
    for pending_expression in pending {
        if program
            .tables
            .bodies
            .expressions
            .authored_expression_exposure(pending_expression.expression)
            != Some(pending_expression.exposure)
        {
            return Err(Diagnostic::error(
                "pending authored expression lost its declared exposure before selection finalization",
            ));
        }
    }

    // Compiler rewrites may copy an authored expression before declaration
    // selections are finalized. The expression table carries that authored
    // provenance explicitly, so enumerate every retained copy and bind all of
    // them to the one occurrence minted for the exact source token. This keeps
    // later lowering free to select whichever semantically equivalent copy it
    // owns without orphaning the source occurrence.
    let authored_expressions = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(expression, _)| {
            if program
                .tables
                .bodies
                .expressions
                .authored_selection_occurrences(expression)
                .len()
                != 0
            {
                return None;
            }
            program
                .tables
                .bodies
                .expressions
                .authored_expression_exposure(expression)
                .map(|exposure| (expression, exposure))
        })
        .collect::<Vec<_>>();

    let mut groups: Vec<CandidateGroup> = Vec::new();
    for (expression, exposure) in authored_expressions {
        let compiler_partition = program
            .tables
            .bodies
            .expressions
            .compiler_selection_partition(expression);
        for candidate in expression_candidates(program, expression) {
            if let Some(group) = groups.iter_mut().find(|group| {
                group.source_span == candidate.source_span
                    && group.exposure == exposure
                    && group.kind == candidate.kind
                    && group.compiler_partition == compiler_partition
            }) {
                group.target = reconcile_copy_targets(
                    program,
                    candidate.source_span,
                    candidate.kind,
                    group.target,
                    candidate.target,
                )?;
                if !group.expressions.contains(&candidate.expression) {
                    group.expressions.push(candidate.expression);
                }
            } else {
                groups.push(CandidateGroup {
                    source_span: candidate.source_span,
                    exposure,
                    kind: candidate.kind,
                    compiler_partition,
                    target: candidate.target,
                    expressions: vec![candidate.expression],
                });
            }
        }
    }

    for group in groups {
        let occurrence = match group.target {
            CandidateTarget::Resolved(symbol) => program
                .record_resolved_authored_declaration_selection_in_partition(
                    group.source_span,
                    group.exposure,
                    group.kind,
                    group.compiler_partition,
                    symbol,
                ),
            CandidateTarget::LateBound(binding) => program
                .record_late_bound_authored_declaration_selection_in_partition(
                    group.source_span,
                    group.exposure,
                    group.kind,
                    group.compiler_partition,
                    binding,
                ),
        }
        .map_err(record_diagnostic)?;
        for expression in group.expressions {
            program
                .tables
                .bodies
                .expressions
                .attach_authored_selection_occurrences(expression, [occurrence]);
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
        let membership = *membership;
        if membership.authored_domain_selection.is_some() {
            continue;
        }
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
        let occurrence = match target {
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
        let psi_symbol_resolved_trees::domain::ProofFact::Membership(membership) = program
            .tables
            .declarations
            .proof_facts
            .get_mut(pending.fact)
        else {
            unreachable!("validated authored proof-membership fact changed variant")
        };
        membership.authored_domain_selection = Some(occurrence);
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
    compiler_partition:
        Option<psi_language_semantics::declaration_selection::CompilerDerivedSelectionPartition>,
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
                            && call.authored_call_selection.is_none()
                            && nonempty(call.target.source_span()) =>
                    {
                        candidates.push(AuthoredStatementCallCandidate {
                            site: AuthoredStatementCallSite::Statement {
                                statements: state.statements,
                                offset,
                            },
                            source_span: call.target.source_span(),
                            compiler_partition: machine.compiler_selection_partition,
                            target: resolved_or_late(call.target_symbol, LateBinding::CheckedCall),
                        });
                    }
                    Statement::Transition(transition) => {
                        collect_authored_transition_call_candidate(
                            state.statements,
                            offset,
                            false,
                            &transition.target,
                            machine.compiler_selection_partition,
                            &mut candidates,
                        );
                        if let Some(continuation) = &transition.continuation {
                            collect_authored_transition_call_candidate(
                                state.statements,
                                offset,
                                true,
                                continuation,
                                machine.compiler_selection_partition,
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
        let occurrence =
            if let Some(occurrence) = existing_statement_call_occurrence(program, candidate)? {
                occurrence
            } else {
                match candidate.target {
                    CandidateTarget::Resolved(symbol) => program
                        .record_resolved_authored_declaration_selection_in_partition(
                            candidate.source_span,
                            Exposure::PrivateImplementation,
                            Kind::Call,
                            candidate.compiler_partition,
                            symbol,
                        ),
                    CandidateTarget::LateBound(binding) => program
                        .record_late_bound_authored_declaration_selection_in_partition(
                            candidate.source_span,
                            Exposure::PrivateImplementation,
                            Kind::Call,
                            candidate.compiler_partition,
                            binding,
                        ),
                }
                .map_err(record_diagnostic)?
            };
        attach_statement_call_occurrence(program, candidate.site, occurrence)?;
    }
    Ok(())
}

fn existing_statement_call_occurrence(
    program: &SymbolResolvedTrees,
    candidate: AuthoredStatementCallCandidate,
) -> Result<Option<AuthoredDeclarationSelectionOccurrenceId>, Diagnostic> {
    use psi_language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure,
        AuthoredDeclarationSelectionTarget as Target,
    };

    let mut retained = None;
    for selection in program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.source_span() == candidate.source_span
                && selection.exposure() == Exposure::PrivateImplementation
                && selection.kind() == Kind::Call
                && selection.compiler_partition() == candidate.compiler_partition
        })
    {
        let existing_target = match selection.target() {
            Target::Resolved(existing) => CandidateTarget::Resolved(existing.selected_symbol()),
            Target::LateBound(binding) => CandidateTarget::LateBound(binding),
            Target::Intrinsic(_) => {
                return Err(Diagnostic::error(
                    "authored statement call reclassification found an intrinsic selection for the same source token",
                )
                .with_source_span(candidate.source_span));
            }
        };
        reconcile_copy_targets(
            program,
            candidate.source_span,
            Kind::Call,
            existing_target,
            candidate.target,
        )?;
        if retained.is_some_and(|occurrence| occurrence != selection.occurrence_id()) {
            return Err(Diagnostic::error(
                "authored statement call reclassification found duplicate selections for the same source token",
            )
            .with_source_span(candidate.source_span));
        }
        retained = Some(selection.occurrence_id());
    }
    Ok(retained)
}

fn collect_authored_transition_call_candidate(
    statements: psi_arena::HandleSpan<psi_symbol_resolved_trees::statement::Statement>,
    offset: usize,
    continuation: bool,
    target: &psi_symbol_resolved_trees::statement::TransitionTarget,
    compiler_partition: Option<
        psi_language_semantics::declaration_selection::CompilerDerivedSelectionPartition,
    >,
    candidates: &mut Vec<AuthoredStatementCallCandidate>,
) {
    let psi_symbol_resolved_trees::statement::TransitionTarget::Named(target) = target else {
        return;
    };
    if target.authored_call_selection.is_none() && nonempty(target.source_span) {
        candidates.push(AuthoredStatementCallCandidate {
            site: AuthoredStatementCallSite::TransitionTarget {
                statements,
                offset,
                continuation,
            },
            source_span: target.source_span,
            compiler_partition,
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
        let candidate = UnattachedCandidate {
            source_span,
            exposure: Exposure::PrivateImplementation,
            kind: Kind::StaticPathSegment,
            target: CandidateTarget::Resolved(realization_machine),
        };
        record_unattached_candidate(program, candidate)?;
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
    if selection_already_recorded(program, candidate) {
        return Ok(());
    }
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

fn selection_already_recorded(
    program: &SymbolResolvedTrees,
    candidate: UnattachedCandidate,
) -> bool {
    use psi_symbol_resolved_trees::AuthoredDeclarationSelectionTarget;

    program.authored_declaration_selections().iter().any(|row| {
        row.source_span() == candidate.source_span
            && row.exposure() == candidate.exposure
            && row.kind() == candidate.kind
            && row.compiler_partition().is_none()
            && match (row.target(), candidate.target) {
                (
                    AuthoredDeclarationSelectionTarget::Resolved(existing),
                    CandidateTarget::Resolved(candidate),
                ) => existing.selected_symbol() == candidate,
                (
                    AuthoredDeclarationSelectionTarget::LateBound(existing),
                    CandidateTarget::LateBound(candidate),
                ) => existing == candidate,
                _ => false,
            }
    })
}

fn conformance_bound_candidates(program: &SymbolResolvedTrees) -> Vec<UnattachedCandidate> {
    use psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as Exposure;

    let mut candidates = Vec::new();
    for machine in program.machines.iter() {
        collect_conformance_bound_candidates(
            program,
            &machine.conformance_bounds,
            if machine.is_public
                || matches!(
                    machine.supply_mode,
                    psi_language_semantics::MachineSupplyMode::Boundary
                        | psi_language_semantics::MachineSupplyMode::AdmissionClaim
                )
            {
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
        ExpressionNode::Cast(cast) if !cast.semantic_domain.is_empty() => {
            let members = expressions.name_path_members(cast.semantic_domain);
            candidates.push(Candidate {
                expression,
                source_span: path_span(members, expression_span),
                kind: Kind::DomainMembership,
                target: resolved_or_late(
                    cast.semantic_domain_symbol,
                    LateBinding::CheckedDomainMembership,
                ),
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
