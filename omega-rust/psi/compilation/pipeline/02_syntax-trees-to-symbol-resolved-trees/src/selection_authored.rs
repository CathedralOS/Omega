/*
    The authored-selection ledger: one exact declaration occurrence per
    authored source token.

    Compiler rewrites may copy an authored expression before selection; every
    retained copy binds to the occurrence minted for its source token.
    Expression, conformance-reference, and constant-initializer selections
    finalize here once their declarations have symbols.
*/

use crate::resolution::lowerer::{PendingAuthoredExpression, PendingAuthoredProofMembership};
use crate::tables::body_expressions;
use crate::tables::resolved_declarations;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind as Kind,
    AuthoredDeclarationSelectionLateBinding as LateBinding,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError,
    BuildOperation, CompilerDerivedSelectionPartition, ResolvedAuthoredCallBinding,
};
use source::SourceSpan;
/* Handle-keyed maps take the integer hasher; none is iterated for output order. */
type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;
use crate::symbols::qualified_payload_free_case_type;
use crate::symbols::type_predicates::type_operand_nodes;
use std::collections::HashSet;
use symbol_resolved_trees::domain::ProofFact;
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::{
    SymbolResolvedTrees,
    expression::{ExpressionHandle, ExpressionNode},
};
use symbols::{JoinedPath, SymbolHandle, SymbolLookup};

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
    exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    kind: Kind,
    compiler_partition:
        Option<language_semantics::declaration_selection::CompilerDerivedSelectionPartition>,
    target: CandidateTarget,
    expressions: Vec<ExpressionHandle>,
    top_level_static_argument_position: Option<usize>,
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
        (CandidateTarget::Resolved(left), CandidateTarget::Resolved(right)) => {
            let correspondence = resolved_symbols_share_authored_declaration(program, left, right);
            if matches!(correspondence, Ok(true)) {
                return Ok(CandidateTarget::Resolved(earlier_symbol(left, right)));
            }
            let reason = correspondence
                .err()
                .map_or_else(String::new, |reason| format!("; {reason}"));
            // Preserve the refusing coordinates so equal printed names do not hide
            // a missing declaration or a different evidence owner.
            let coordinate = |symbol| {
                let row = program.symbols.get(symbol);
                let parent = row.parent;
                let parent_kind = parent.is_valid().then(|| program.symbols.get(parent).kind);
                (
                    symbol,
                    row.kind,
                    parent,
                    parent_kind,
                    program.symbols.symbol_source_span(symbol),
                    program.symbols.symbol_provenance_source_span(symbol),
                )
            };
            Err(Diagnostic::error(format!(
                "compiler-derived copies of one authored {kind:?} selection resolved inconsistently: `{}` versus `{}`{reason}; left coordinate={:?}; right coordinate={:?}",
                program.symbols.display_path(left, "::"),
                program.symbols.display_path(right, "::"),
                coordinate(left), coordinate(right),
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

/*
    Freeze the original call declaration after this owner has reconciled copied
    expressions. A current executable copy need not have the original handle;
    consumers receive this correspondence instead of repeating that judgment.
*/
pub(crate) fn original_call_target(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
) -> Result<Option<SymbolHandle>, Diagnostic> {
    let table = &program.tables.bodies.expressions;
    let ExpressionNode::Call(call) = table.expression(expression) else {
        return Ok(None);
    };
    let mut original = None;
    for occurrence in table.authored_selection_occurrences(expression) {
        let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
            return Err(Diagnostic::error(
                "original call correspondence lost its occurrence",
            ));
        };
        if selection.kind() != Kind::Call || selection.source_span() != call.target.source_span() {
            continue;
        }
        let Some(target) = selection
            .target()
            .as_resolved()
            .map(|selected| selected.selected_symbol())
        else {
            continue;
        };
        reconcile_copy_targets(
            program,
            selection.source_span(),
            Kind::Call,
            CandidateTarget::Resolved(target),
            CandidateTarget::Resolved(call.target_symbol),
        )?;
        let binding = (target, selection.exposure(), selection.compiler_partition());
        if original.is_some_and(|previous| previous != binding) {
            return Err(Diagnostic::error(
                "original call correspondence has conflicting occurrence owners",
            )
            .with_source_span(selection.source_span()));
        }
        original = Some(binding);
    }
    Ok(original.map(|(target, _, _)| target))
}

fn resolved_symbols_share_authored_declaration(
    program: &SymbolResolvedTrees,
    left: SymbolHandle,
    right: SymbolHandle,
) -> Result<bool, String> {
    if program.symbols.get(left).kind != program.symbols.get(right).kind {
        return Ok(false);
    }
    // Generated entries and evidence children have no declaration token of their
    // own. Use their authenticated declaration for every copied selection; the
    // executable call or static operand still retains its exact selected handle.
    let left_custody = super::call_identity::selected_declaration_source(program, left);
    let right_custody = super::call_identity::selected_declaration_source(program, right);
    if matches!((&left_custody, &right_custody), (Ok(left), Ok(right)) if left == right) {
        return Ok(true);
    }
    Err(format!(
        "declaration custody left={left_custody:?} right={right_custody:?}",
    ))
}

pub(crate) fn finalize_authored_expression_selections(
    program: &mut SymbolResolvedTrees,
    work: &[ExpressionHandle],
    pending: &[PendingAuthoredExpression],
    pending_proof_memberships: &[PendingAuthoredProofMembership],
) -> Result<(), Diagnostic> {
    for pending_expression in pending {
        if body_expressions(program).authored_expression_exposure(pending_expression.expression)
            != Some(pending_expression.exposure)
        {
            return Err(Diagnostic::error(
                "pending authored expression lost its declared exposure before selection finalization",
            ));
        }
    }

    let authored_expressions = work
        .iter()
        .copied()
        .filter_map(|expression| {
            if body_expressions(program)
                .authored_selection_occurrences(expression)
                .len()
                != 0
            {
                return None;
            }
            body_expressions(program)
                .authored_expression_exposure(expression)
                .map(|exposure| (expression, exposure))
        })
        .collect::<Vec<_>>();

    finalize_expression_groups(program, authored_expressions)?;

    for pending in pending_proof_memberships {
        if let ProofFact::Expression(expression) =
            program.tables.declarations.proof_facts.get(pending.fact)
            && matches!(program.tables.bodies.expressions.expression(*expression),
                ExpressionNode::Membership(membership)
                    if !membership.domain_symbol.is_valid()
                        && membership.case_type_symbol.is_valid()
                        && membership.case_symbol.is_valid())
        {
            continue;
        }
        let ProofFact::Membership(membership) =
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
        let members = resolved_declarations(program)
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
        let ProofFact::Membership(membership) = program
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

    finalize_statement_static_argument_selections(program)?;

    Ok(())
}

pub(super) fn normalize_case_membership_facts(
    program: &mut SymbolResolvedTrees,
    pending: &[PendingAuthoredProofMembership],
) -> Result<(), Diagnostic> {
    for pending in pending {
        let ProofFact::Membership(membership) =
            program.tables.declarations.proof_facts.get(pending.fact)
        else {
            continue;
        };
        let membership = *membership;
        let members = program.domain_path_members(membership.domain);
        let source_span = path_span(members, SourceSpan::default());
        let name = members.joined_path();
        let selected =
            crate::symbols::membership_selection(&program.symbols, &name, source_span)
                .map_err(|message| Diagnostic::error(message).with_source_span(source_span))?;
        let Some(crate::symbols::MembershipSelection::Case {
            owner: case_type_symbol,
            case: case_symbol,
        }) = selected
        else {
            continue;
        };
        if membership.domain_symbol.is_valid() || membership.authored_domain_selection.is_some() {
            continue;
        }
        let members = members.to_vec();
        let fact_span = program
            .proof_fact_source_span(pending.fact)
            .unwrap_or(source_span);
        let expressions = &mut program.tables.bodies.expressions;
        let partition = expressions.compiler_selection_partition(membership.value);
        let mut domain = arena::HandleSpan::empty();
        for member in members {
            expressions.push_name_path_member(&mut domain, member);
        }
        let expression = expressions.insert(ExpressionNode::Membership(
            symbol_resolved_trees::expression::TableMembershipExpression {
                is_pattern: false,
                value: membership.value,
                domain,
                domain_symbol: SymbolHandle::invalid(),
                case_type_symbol,
                case_symbol,
            },
        ));
        expressions.set_source_span(expression, fact_span);
        expressions.set_authored_expression_exposure(expression, pending.exposure);
        if let Some(partition) = partition {
            expressions.set_compiler_selection_partition(expression, partition);
        }
        *program
            .tables
            .declarations
            .proof_facts
            .get_mut(pending.fact) = ProofFact::Expression(expression);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum AuthoredStatementCallSite {
    Statement {
        statements: arena::HandleSpan<symbol_resolved_trees::statement::Statement>,
        offset: usize,
    },
    TransitionTarget {
        statements: arena::HandleSpan<symbol_resolved_trees::statement::Statement>,
        offset: usize,
        continuation: bool,
    },
}

#[derive(Debug, Clone, Copy)]
struct AuthoredStatementCallCandidate {
    site: AuthoredStatementCallSite,
    source_span: SourceSpan,
    compiler_partition:
        Option<language_semantics::declaration_selection::CompilerDerivedSelectionPartition>,
    target: CandidateTarget,
}

fn collect_authored_statement_call_candidates(
    program: &SymbolResolvedTrees,
    statements: arena::HandleSpan<symbol_resolved_trees::statement::Statement>,
    partition: Option<CompilerDerivedSelectionPartition>,
    candidates: &mut Vec<AuthoredStatementCallCandidate>,
) {
    use symbol_resolved_trees::statement::Statement;
    for (offset, statement) in program.state_statements(statements).iter().enumerate() {
        match statement {
            Statement::Call(call)
                if call.operational_acknowledgement.origin
                    == language_semantics::CallOperationalAcknowledgementOrigin::Source
                    && !(call.target.as_str() == "code_layout"
                        && !call.target_symbol.is_valid()
                        && call.receiver.is_empty())
                    && call.authored_call_selection.is_none()
                    && nonempty(call.target.source_span()) =>
            {
                candidates.push(AuthoredStatementCallCandidate {
                    site: AuthoredStatementCallSite::Statement { statements, offset },
                    source_span: call.target.source_span(),
                    compiler_partition: partition,
                    target: resolved_or_late(call.target_symbol, LateBinding::CheckedCall),
                });
            }
            Statement::Transition(transition) => {
                collect_authored_transition_call_candidate(
                    program,
                    statements,
                    offset,
                    false,
                    &transition.target,
                    partition,
                    candidates,
                );
                if let Some(continuation) = &transition.continuation {
                    collect_authored_transition_call_candidate(
                        program,
                        statements,
                        offset,
                        true,
                        continuation,
                        partition,
                        candidates,
                    );
                }
            }
            Statement::Assembly(handle) => {
                let declarations = &program.tables.declarations;
                let graph = declarations.assembly_graphs.get(*handle);
                for block in declarations.assembly_blocks.span_or_empty(graph.blocks) {
                    collect_authored_statement_call_candidates(
                        program,
                        block.instructions,
                        partition,
                        candidates,
                    );
                }
            }
            _ => {}
        }
    }
}

fn finalize_authored_statement_call_selections(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as Exposure;
    let mut candidates = Vec::new();
    for machine in program.machines.iter() {
        for state_handle in program.machine_state_handles(machine.states) {
            let state = program.machine_state(*state_handle);
            collect_authored_statement_call_candidates(
                program,
                state.statements,
                machine.compiler_selection_partition,
                &mut candidates,
            );
        }
    }

    let mut call_selection_index = call_selection_index(program);
    for candidate in candidates {
        let occurrence = if let Some(occurrence) = existing_call_occurrence(
            program,
            candidate.source_span,
            Exposure::PrivateImplementation,
            candidate.compiler_partition,
            candidate.target,
            &call_selection_index,
        )? {
            occurrence
        } else {
            let occurrence = match candidate.target {
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
            .map_err(record_diagnostic)?;
            index_latest_call_selection(program, &mut call_selection_index);
            occurrence
        };
        attach_statement_call_occurrence(program, candidate.site, occurrence)?;
    }
    Ok(())
}

type CallSelectionKey = (
    usize,
    usize,
    usize,
    Option<CompilerDerivedSelectionPartition>,
);

fn call_selection_index(program: &SymbolResolvedTrees) -> HashMap<CallSelectionKey, Vec<usize>> {
    let mut index = HashMap::default();
    for (row, selection) in program.authored_declaration_selections().iter().enumerate() {
        if selection.kind() != Kind::Call {
            continue;
        }
        let span = selection.source_span();
        index
            .entry((
                span.source_id.0,
                span.span.start,
                span.span.end,
                selection.compiler_partition(),
            ))
            .or_insert_with(Vec::new)
            .push(row);
    }
    index
}

fn index_latest_call_selection(
    program: &SymbolResolvedTrees,
    index: &mut HashMap<CallSelectionKey, Vec<usize>>,
) {
    let Some(selection) = program.authored_declaration_selections().as_slice().last() else {
        return;
    };
    if selection.kind() != Kind::Call {
        return;
    }
    let span = selection.source_span();
    index
        .entry((
            span.source_id.0,
            span.span.start,
            span.span.end,
            selection.compiler_partition(),
        ))
        .or_default()
        .push(program.authored_declaration_selections().len() - 1);
}

fn existing_call_occurrence(
    program: &SymbolResolvedTrees,
    source_span: SourceSpan,
    exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    compiler_partition: Option<CompilerDerivedSelectionPartition>,
    target: CandidateTarget,
    index: &HashMap<CallSelectionKey, Vec<usize>>,
) -> Result<Option<AuthoredDeclarationSelectionOccurrenceId>, Diagnostic> {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget as Target;

    let Some(rows) = index.get(&(
        source_span.source_id.0,
        source_span.span.start,
        source_span.span.end,
        compiler_partition,
    )) else {
        return Ok(None);
    };
    let mut retained = None;
    for &row in rows {
        let selection = &program.authored_declaration_selections().as_slice()[row];
        if selection.exposure() != exposure {
            continue;
        }
        let existing_target = match selection.target() {
            Target::Resolved(existing) => CandidateTarget::Resolved(existing.selected_symbol()),
            Target::LateBound(binding) => CandidateTarget::LateBound(binding),
            Target::Intrinsic(intrinsic) => {
                return Err(Diagnostic::error(format!(
                    "authored call reclassification found an intrinsic selection for the same source token: at {source_span:?}, retained {:?} {intrinsic:?}, candidate {target:?}, exposure {exposure:?}, partition {compiler_partition:?}", selection.occurrence_id(),
                ))
                .with_source_span(source_span));
            }
        };
        reconcile_copy_targets(program, source_span, Kind::Call, existing_target, target)?;
        if retained.is_some_and(|occurrence| occurrence != selection.occurrence_id()) {
            return Err(Diagnostic::error(format!(
                "authored call reclassification found duplicate selections for the same source token: at {source_span:?}, earlier {retained:?}, repeated {:?} {:?}, candidate {target:?}, exposure {exposure:?}, partition {compiler_partition:?}", selection.occurrence_id(), selection.target(),
            ))
            .with_source_span(source_span));
        }
        retained = Some(selection.occurrence_id());
    }
    Ok(retained)
}

fn collect_authored_transition_call_candidate(
    program: &SymbolResolvedTrees,
    statements: arena::HandleSpan<symbol_resolved_trees::statement::Statement>,
    offset: usize,
    continuation: bool,
    target: &symbol_resolved_trees::statement::TransitionTarget,
    compiler_partition: Option<
        language_semantics::declaration_selection::CompilerDerivedSelectionPartition,
    >,
    candidates: &mut Vec<AuthoredStatementCallCandidate>,
) {
    let symbol_resolved_trees::statement::TransitionTarget::Named(target) = target else {
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
            target: resolved_or_late(
                program
                    .machines
                    .iter()
                    .find(|machine| machine.symbol == target.symbol)
                    .and_then(|machine| program.machine_state_handles(machine.states).first())
                    .map(|state| program.machine_state(*state).symbol)
                    .unwrap_or(target.symbol),
                LateBinding::CheckedCall,
            ),
        });
    }
}

fn attach_statement_call_occurrence(
    program: &mut SymbolResolvedTrees,
    site: AuthoredStatementCallSite,
    occurrence: AuthoredDeclarationSelectionOccurrenceId,
) -> Result<(), Diagnostic> {
    use symbol_resolved_trees::statement::{Statement, TransitionTarget};

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

/*
    Complete the original operand occurrences after normalized map selection.
    Attachments retain copy identity; a span-only lookup cannot issue custody.
*/
pub(super) fn finalize_normalized_static_operands(
    program: &mut SymbolResolvedTrees,
    occurrences: &[AuthoredDeclarationSelectionOccurrenceId],
    exposure: Option<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
    partition: Option<CompilerDerivedSelectionPartition>,
    operands: &[super::static_arguments::ResolvedStaticOperand],
) -> Result<(), Diagnostic> {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget as Target;
    for operand in operands {
        let mut candidates = occurrences
            .iter()
            .filter_map(|occurrence| program.authored_declaration_selections().get(*occurrence))
            .filter(|row| {
                row.source_span() == operand.source
                    && row.kind() == Kind::StaticArgument
                    && row.exposure() == exposure.unwrap_or(language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation)
                    && (exposure.is_none() || row.compiler_partition() == partition)
            });
        let original = candidates.next().copied().ok_or_else(|| {
            Diagnostic::error("normalized member has no attached static selection occurrence")
                .with_source_span(operand.source)
        })?;
        if candidates.next().is_some() {
            return Err(Diagnostic::error(
                "normalized member has ambiguous attached static selection occurrences",
            )
            .with_source_span(operand.source));
        }
        match original.target() {
            Target::LateBound(LateBinding::CheckedStaticArgument) => program
                .finalize_authored_declaration_selection(
                    original.occurrence_id(),
                    LateBinding::CheckedStaticArgument,
                    operand.entry,
                )
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "normalized static selection finalization failed: {reason:?}"
                    ))
                    .with_source_span(operand.source)
                })?,
            Target::Resolved(target) if target.selected_symbol() == operand.entry => {}
            _ => {
                return Err(Diagnostic::error(
                    "normalized member conflicts with its original static selection",
                )
                .with_source_span(operand.source));
            }
        }
    }
    Ok(())
}

/*
    Retain explicit realization-machine references only after closed
    conformance normalization has resolved each target to one exact machine.
    The referenced member remains private implementation even when the complete
    conformance itself is public.
*/
pub(crate) fn finalize_conformance_reference_selections(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as Exposure;

    let selections = program
        .conformances
        .iter()
        .flat_map(|conformance| {
            conformance.rows.iter().filter_map(|row| {
                row.authored_realization_source_span
                    .map(|source_span| (source_span, row.realization_machine))
            })
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
    exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
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
    use symbol_resolved_trees::AuthoredDeclarationSelectionTarget;

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
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as Exposure;

    let mut candidates = Vec::new();
    for machine in program.machines.iter() {
        collect_conformance_bound_candidates(
            &machine.conformance_bounds,
            machine.interface_exposure,
            &mut candidates,
        );
    }
    for trait_definition in program.traits.iter() {
        collect_conformance_bound_candidates(
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
    bounds: &[symbol_resolved_trees::machine::GenericConformanceBound],
    exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    candidates: &mut Vec<UnattachedCandidate>,
) {
    for bound in bounds {
        if bound.trait_name.is_source_backed() {
            candidates.push(UnattachedCandidate {
                source_span: bound.trait_name.source_span(),
                exposure,
                kind: Kind::TypeReference,
                target: resolved_or_late(bound.trait_symbol, LateBinding::CheckedStaticPathSegment),
            });
        }
    }
}

fn finalize_statement_static_argument_selections(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as Exposure;
    use symbol_resolved_trees::statement::Statement;
    let mut sites = Vec::new();
    for machine in program.machines.iter() {
        for state in program.machine_state_handles(machine.states) {
            let statements = program.machine_state(*state).statements;
            collect_statement_static_sites(
                program,
                statements,
                machine.compiler_selection_partition,
                &mut sites,
            );
        }
    }
    for (statements, offset, partition, candidates) in sites {
        let mut occurrences = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let existing = program
                .authored_declaration_selections()
                .iter()
                .filter(|selection| {
                    selection.source_span() == candidate.source_span
                        && selection.exposure() == Exposure::PrivateImplementation
                        && selection.kind() == candidate.kind
                        && selection.compiler_partition() == partition
                })
                .map(|selection| selection.occurrence_id())
                .collect::<Vec<_>>();
            let occurrence = match existing.as_slice() {
                [occurrence] => {
                    let retained = program
                        .authored_declaration_selections()
                        .get(*occurrence)
                        .expect("indexed existing statement source selection");
                    let target = match retained.target() {
                        language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target) => CandidateTarget::Resolved(target.selected_symbol()),
                        language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::LateBound(binding) => CandidateTarget::LateBound(binding),
                        _ => return Err(Diagnostic::error("statement static input cannot reuse an intrinsic source selection").with_source_span(candidate.source_span)),
                    };
                    reconcile_copy_targets(
                        program,
                        candidate.source_span,
                        candidate.kind,
                        target,
                        candidate.target,
                    )?;
                    *occurrence
                }
                [] => match candidate.target {
                    CandidateTarget::Resolved(symbol) => program
                        .record_resolved_authored_declaration_selection_in_partition(
                            candidate.source_span,
                            Exposure::PrivateImplementation,
                            candidate.kind,
                            partition,
                            symbol,
                        ),
                    CandidateTarget::LateBound(binding) => program
                        .record_late_bound_authored_declaration_selection_in_partition(
                            candidate.source_span,
                            Exposure::PrivateImplementation,
                            candidate.kind,
                            partition,
                            binding,
                        ),
                }
                .map_err(record_diagnostic)?,
                _ => {
                    return Err(Diagnostic::error(
                        "original statement static input has ambiguous source custody",
                    )
                    .with_source_span(candidate.source_span));
                }
            };
            if !occurrences.contains(&occurrence) {
                occurrences.push(occurrence);
            }
        }
        let Some(Statement::Call(call)) = program
            .tables
            .declarations
            .state_statements
            .span_mut_or_empty(statements)
            .get_mut(offset)
        else {
            return Err(Diagnostic::error(
                "original static-input site no longer identifies a statement call",
            ));
        };
        call.authored_static_argument_selections = occurrences.into_boxed_slice();
    }
    Ok(())
}

type StatementStaticSite = (
    arena::HandleSpan<symbol_resolved_trees::statement::Statement>,
    usize,
    Option<CompilerDerivedSelectionPartition>,
    Vec<UnattachedCandidate>,
);
fn collect_statement_static_sites(
    program: &SymbolResolvedTrees,
    statements: arena::HandleSpan<symbol_resolved_trees::statement::Statement>,
    partition: Option<CompilerDerivedSelectionPartition>,
    sites: &mut Vec<StatementStaticSite>,
) {
    use symbol_resolved_trees::statement::Statement;
    for (offset, statement) in program.state_statements(statements).iter().enumerate() {
        match statement {
            Statement::Call(call) => {
                let mut candidates = Vec::new();
                collect_statement_static_argument_candidates(
                    program,
                    &call.machine_arguments,
                    call.target.source_span(),
                    BuildOperation::from_call_target(call.target.as_str())
                        .is_some_and(BuildOperation::selects_product_declarations)
                        && !call.target_symbol.is_valid(),
                    &mut candidates,
                );
                sites.push((statements, offset, partition, candidates));
            }
            Statement::Assembly(handle) => {
                let declarations = &program.tables.declarations;
                for block in declarations
                    .assembly_blocks
                    .span_or_empty(declarations.assembly_graphs.get(*handle).blocks)
                {
                    collect_statement_static_sites(program, block.instructions, partition, sites);
                }
            }
            _ => {}
        }
    }
}

fn collect_statement_static_argument_candidates(
    program: &SymbolResolvedTrees,
    arguments: &[symbol_resolved_trees::expression::StaticMachineArgument],
    fallback_span: SourceSpan,
    defer: bool,
    candidates: &mut Vec<UnattachedCandidate>,
) {
    for argument in arguments {
        if has_authored_static_declaration(program, argument) {
            candidates.push(UnattachedCandidate { source_span: path_span(&argument.path, fallback_span), exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation, kind: static_argument_kind(program, argument.symbol), target: if defer { CandidateTarget::LateBound(LateBinding::CheckedStaticArgument) } else { resolved_or_late(argument.symbol, LateBinding::CheckedStaticArgument) } });
        }
        if let Some(application) = &argument.application {
            collect_statement_static_argument_candidates(
                program,
                &application.arguments,
                fallback_span,
                defer,
                candidates,
            );
            if application.member.is_valid()
                && let Some(member) = body_expressions(program).static_member(application.member)
            {
                collect_statement_static_argument_candidates(
                    program,
                    std::slice::from_ref(member),
                    fallback_span,
                    defer,
                    candidates,
                );
            }
        }
    }
}

/*
    Every expression handle inside a signature-contract clause (requires,
    ensures, and membership facts), including nested operands. A call token
    inside one of these clauses may name an uninterpreted proof application
    rather than a package declaration, so declaration-selection recording
    cannot assume a callee symbol exists for it.
*/
fn contract_clause_expression_handles(
    program: &SymbolResolvedTrees,
) -> HashSet<ExpressionHandle, symbols::BuildSymbolHasher> {
    let mut handles = HashSet::default();
    for (_, contract) in program.tables.declarations.signature_contracts.iter() {
        for fact in resolved_declarations(program)
            .proof_facts
            .span_or_empty(contract.facts)
        {
            let root = match fact {
                ProofFact::Expression(expression) => *expression,
                ProofFact::Membership(membership) => membership.value,
            };
            collect_expression_subtree(program, root, &mut handles);
        }
    }
    handles
}

fn collect_expression_subtree(
    program: &SymbolResolvedTrees,
    root: ExpressionHandle,
    handles: &mut HashSet<ExpressionHandle, symbols::BuildSymbolHasher>,
) {
    if !root.is_valid() || !handles.insert(root) {
        return;
    }
    let expressions = &program.tables.bodies.expressions;
    match expressions.expression(root) {
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                collect_expression_subtree(program, call.receiver, handles);
            }
            for argument in expressions.expression_handles(call.arguments) {
                collect_expression_subtree(program, *argument, handles);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_subtree(program, binary.left, handles);
            collect_expression_subtree(program, binary.right, handles);
        }
        ExpressionNode::Unary(unary) => collect_expression_subtree(program, unary.operand, handles),
        ExpressionNode::Cast(cast) => collect_expression_subtree(program, cast.value, handles),
        ExpressionNode::Member(member) => {
            collect_expression_subtree(program, member.receiver, handles)
        }
        ExpressionNode::Borrow(borrow) => {
            collect_expression_subtree(program, borrow.target, handles)
        }
        ExpressionNode::Indexed(indexed) => {
            collect_expression_subtree(program, indexed.collection, handles);
            collect_expression_subtree(program, indexed.index, handles);
        }
        ExpressionNode::Membership(membership) => {
            collect_expression_subtree(program, membership.value, handles)
        }
        ExpressionNode::Atomic(atomic) => {
            collect_expression_subtree(program, atomic.value, handles);
            collect_expression_subtree(program, atomic.result, handles);
        }
        ExpressionNode::Range(range) => {
            collect_expression_subtree(program, range.start, handles);
            collect_expression_subtree(program, range.end, handles);
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in expressions.expression_handles(*elements) {
                collect_expression_subtree(program, *element, handles);
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in expressions.struct_fields(literal.fields) {
                collect_expression_subtree(program, field.value, handles);
            }
        }
        ExpressionNode::Match(dispatch) => {
            collect_expression_subtree(program, dispatch.subject, handles);
            for arm in expressions.match_arms(dispatch.arms) {
                collect_expression_subtree(program, arm.value, handles);
                for pattern in expressions.match_pattern_dependencies(arm.pattern) {
                    collect_expression_subtree(program, pattern, handles);
                }
            }
        }
        ExpressionNode::SavedSubject(saved) => {
            collect_expression_subtree(
                program,
                expressions.match_subject(saved.capture).subject,
                handles,
            );
            collect_expression_subtree(program, saved.pattern_head, handles);
        }
        ExpressionNode::Name(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::String(_)
        | ExpressionNode::TypeExpression(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

/*
    Package names direct lookup to a dependency; they need no declaration
    symbols of their own. Recognize only the complete loader-certified prefix.
    Real modules and members after it still supply their ordinary selections.
*/
fn package_qualifier_member_count(
    program: &SymbolResolvedTrees,
    members: &[DiagnosticName],
    reference: SourceSpan,
) -> usize {
    let mut prefix = String::new();
    let mut package_members = 0;
    for (offset, member) in members.iter().enumerate() {
        if offset != 0 {
            prefix.push_str("::");
        }
        prefix.push_str(member.as_str());
        if program
            .symbols
            .is_source_package_qualifier(reference, &prefix)
        {
            package_members = offset + 1;
        }
    }
    package_members
}

fn expression_candidates(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
    type_operands: &HashSet<ExpressionHandle, symbols::BuildSymbolHasher>,
    contract_clause_expressions: &HashSet<ExpressionHandle, symbols::BuildSymbolHasher>,
) -> Result<Vec<Candidate>, Diagnostic> {
    let expressions = &program.tables.bodies.expressions;
    let expression_span = expressions.source_span(expression);
    let mut candidates = Vec::new();

    match expressions.expression(expression) {
        ExpressionNode::Binary(_)
            if crate::symbols::type_predicates::is_type_predicate(program, expression) => {}
        ExpressionNode::Indexed(_) if type_operands.contains(&expression) => {}
        ExpressionNode::Binary(_) | ExpressionNode::Indexed(_) | ExpressionNode::Unary(_) => {
            candidates.push(Candidate {
                expression,
                source_span: expression_span,
                kind: Kind::Operator,
                target: CandidateTarget::LateBound(LateBinding::CheckedOperator),
            })
        }
        ExpressionNode::Call(call) => {
            let unbound_contract_call = !call.target_symbol.is_valid()
                && !call.receiver.is_valid()
                && contract_clause_expressions.contains(&expression)
                && matches!(
                    program
                        .symbols
                        .lookup_top_level_by_name_and_kinds_from_source_matching(
                            call.target.as_str(),
                            super::ANY_DECLARATION_LOOKUP_KINDS,
                            call.target.source_span(),
                            |_| true,
                        ),
                    SymbolLookup::NotFound
                );
            let data_layout_query =
                language_core::data_layout_queries::is_data_layout_query_spelling(
                    call.target.as_str(),
                    call.target_symbol.is_valid(),
                    call.receiver.is_valid(),
                );
            if call.operational_acknowledgement.origin
                == language_semantics::CallOperationalAcknowledgementOrigin::Source
                && !unbound_contract_call
                && !data_layout_query
                && !(call.target.as_str() == "code_layout"
                    && !call.target_symbol.is_valid()
                    && !call.receiver.is_valid())
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
                BuildOperation::from_call_target(call.target.as_str())
                    .is_some_and(BuildOperation::selects_product_declarations)
                    && !call.target_symbol.is_valid(),
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
            if !membership.domain_symbol.is_valid() && !membership.case_type_symbol.is_valid() {
                let name = members.joined_path();
                let reference = path_span(members, expression_span);
                let _ = crate::symbols::membership_selection(&program.symbols, &name, reference)
                    .map_err(|message| Diagnostic::error(message).with_source_span(reference))?;
            }
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
                        source_span: path_span(
                            &members[..members.len().saturating_sub(1)],
                            expression_span,
                        ),
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
        ExpressionNode::Name(path) if path.static_application.is_valid() => {
            let selected = expressions
                .static_member(path.static_application)
                .ok_or_else(|| {
                    Diagnostic::error("declaration selection lost its owning static application")
                        .with_source_span(expression_span)
                })?;
            collect_static_argument_candidates(
                program,
                expression,
                std::slice::from_ref(selected),
                false,
                &mut candidates,
            );
        }
        ExpressionNode::Name(path) => {
            let members = expressions.name_path_members(path.members);
            if !path.head_symbol.is_valid() && members.len() > 1 {
                let name = members.joined_path();
                let reference = path_span(members, expression_span);
                let _ = qualified_payload_free_case_type(&program.symbols, &name, reference)
                    .map_err(|message| Diagnostic::error(message).with_source_span(reference))?;
            }
            let symbols = expressions.name_path_member_symbols(path.member_symbols);
            let package_members = if path.head_symbol.is_valid() || members.len() < 2 {
                0
            } else {
                package_qualifier_member_count(
                    program,
                    members,
                    path_span(members, expression_span),
                )
            };
            for (offset, member) in members.iter().enumerate() {
                let symbol = symbols
                    .get(offset)
                    .copied()
                    .unwrap_or_else(SymbolHandle::invalid);
                if ((offset == 0 || offset < package_members) && !symbol.is_valid())
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
            if !literal.type_symbol.is_valid() {
                let _ = crate::symbols::constructor_type(
                    &program.symbols,
                    literal.type_name.as_str(),
                    literal.type_name.source_span(),
                )
                .map_err(|message| {
                    Diagnostic::error(message).with_source_span(literal.type_name.source_span())
                })?;
            }
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
        .retain(|candidate| candidate.source_span.span.start < candidate.source_span.span.end);
    Ok(candidates)
}

fn is_selectable_declaration_symbol(program: &SymbolResolvedTrees, symbol: SymbolHandle) -> bool {
    !matches!(
        program.symbols.get(symbol).kind,
        symbols::SymbolKind::Unknown
            | symbols::SymbolKind::Root
            | symbols::SymbolKind::Local
            | symbols::SymbolKind::Parameter
            | symbols::SymbolKind::TypeParameter
            | symbols::SymbolKind::ConformanceParameter
            | symbols::SymbolKind::MachineParameter
            | symbols::SymbolKind::PropositionParameter
            | symbols::SymbolKind::PropositionMachineParameter
    )
}

fn collect_static_argument_candidates(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
    arguments: &[symbol_resolved_trees::expression::StaticMachineArgument],
    defer_call_arguments: bool,
    candidates: &mut Vec<Candidate>,
) {
    for argument in arguments {
        if has_authored_static_declaration(program, argument) {
            candidates.push(Candidate {
                expression,
                source_span: path_span(
                    &argument.path,
                    program.tables.bodies.expressions.source_span(expression),
                ),
                kind: static_argument_kind(program, argument.symbol),
                target: if defer_call_arguments {
                    CandidateTarget::LateBound(LateBinding::CheckedStaticArgument)
                } else {
                    resolved_or_late(argument.symbol, LateBinding::CheckedStaticArgument)
                },
            });
        }
        if let Some(application) = &argument.application {
            collect_static_argument_candidates(
                program,
                expression,
                &application.arguments,
                defer_call_arguments,
                candidates,
            );
            if application.member.is_valid()
                && let Some(member) = body_expressions(program).static_member(application.member)
            {
                collect_static_argument_candidates(
                    program,
                    expression,
                    std::slice::from_ref(member),
                    defer_call_arguments,
                    candidates,
                );
            }
        }
    }
}

fn has_authored_static_declaration(
    program: &SymbolResolvedTrees,
    argument: &symbol_resolved_trees::expression::StaticMachineArgument,
) -> bool {
    if !argument.symbol.is_valid()
        && argument.application.is_none()
        && argument.evidence_projection.is_none()
        && let [name] = argument.path.as_ref()
        && language_semantics::const_value::boolean_literal_spelling(name.as_str()).is_some()
    {
        return false;
    }
    argument.path.iter().any(|member| member.is_source_backed())
        && (!argument.symbol.is_valid()
            || is_selectable_declaration_symbol(program, argument.symbol))
}

fn static_argument_kind(program: &SymbolResolvedTrees, symbol: SymbolHandle) -> Kind {
    if symbol.is_valid()
        && matches!(
            program.symbols.get(symbol).kind,
            symbols::SymbolKind::Conformance | symbols::SymbolKind::ConformanceParameter
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

fn path_span(members: &[DiagnosticName], fallback: SourceSpan) -> SourceSpan {
    let Some(first) = members.first() else {
        return fallback;
    };
    let Some(last) = members.last() else {
        return fallback;
    };
    if first.source_span().source_id == last.source_span().source_id {
        SourceSpan::new(
            first.source_span().source_id,
            source::Span::new(first.source_span().span.start, last.source_span().span.end),
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

/*
    Mint the initializer's own constructor/field occurrences before substitution.
    A live use later carries these declaration-side occurrences alongside its
    separate constant selection, without inventing consumer-side constructor use.
*/
pub(crate) fn finalize_constant_expression_selections(
    program: &mut SymbolResolvedTrees,
    initializers: impl IntoIterator<Item = ExpressionHandle>,
) -> Result<(), Diagnostic> {
    let expressions = initializers
        .into_iter()
        .filter_map(|expression| {
            body_expressions(program)
                .authored_expression_exposure(expression)
                .map(|exposure| (expression, exposure))
        })
        .collect();
    finalize_expression_groups(program, expressions)
}

fn finalize_expression_groups(
    program: &mut SymbolResolvedTrees,
    authored_expressions: Vec<(
        ExpressionHandle,
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    )>,
) -> Result<(), Diagnostic> {
    let contract_clause_expressions = contract_clause_expression_handles(program);
    let mut operand_nodes = Vec::new();
    for &(expression, _) in &authored_expressions {
        if crate::symbols::type_predicates::is_type_predicate(program, expression)
            && let ExpressionNode::Binary(binary) =
                program.tables.bodies.expressions.expression(expression)
        {
            type_operand_nodes(program, binary.left, &mut operand_nodes);
            type_operand_nodes(program, binary.right, &mut operand_nodes);
        }
    }
    let type_operands = operand_nodes.into_iter().collect();
    let mut groups: Vec<CandidateGroup> = Vec::new();
    let mut group_index: HashMap<_, usize> = HashMap::default();
    for (expression, exposure) in authored_expressions {
        let compiler_partition = body_expressions(program).compiler_selection_partition(expression);
        for candidate in expression_candidates(
            program,
            expression,
            &type_operands,
            &contract_clause_expressions,
        )? {
            let top_level_static_argument_position =
                if matches!(candidate.kind, Kind::StaticArgument | Kind::Conformance) {
                    match program.tables.bodies.expressions.expression(expression) {
                        ExpressionNode::Call(call) => top_level_static_argument_position(
                            &call.machine_arguments,
                            candidate.source_span,
                        ),
                        _ => None,
                    }
                } else {
                    None
                };
            let key = (
                candidate.source_span,
                exposure,
                candidate.kind,
                compiler_partition,
            );
            if let Some(group) = group_index.get(&key).map(|&index| &mut groups[index]) {
                if group.top_level_static_argument_position != top_level_static_argument_position {
                    return Err(Diagnostic::error("compiler-derived copies of a static operand disagree on its authored call position")
                        .with_source_span(candidate.source_span));
                }
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
                group_index.insert(key, groups.len());
                groups.push(CandidateGroup {
                    source_span: candidate.source_span,
                    exposure,
                    kind: candidate.kind,
                    compiler_partition,
                    target: candidate.target,
                    expressions: vec![candidate.expression],
                    top_level_static_argument_position,
                });
            }
        }
    }

    let mut call_selection_index = call_selection_index(program);
    for group in groups {
        let retained = if group.kind == Kind::Call {
            existing_call_occurrence(
                program,
                group.source_span,
                group.exposure,
                group.compiler_partition,
                group.target,
                &call_selection_index,
            )?
        } else {
            None
        };
        let occurrence = if let Some(occurrence) = retained {
            occurrence
        } else {
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
            index_latest_call_selection(program, &mut call_selection_index);
            occurrence
        };
        if let Some(position) = group.top_level_static_argument_position {
            program
                .retain_top_level_static_argument_position(occurrence, position)
                .map_err(|error| {
                    Diagnostic::error(format!(
                        "failed to retain static operand position: {error:?}"
                    ))
                    .with_source_span(group.source_span)
                })?;
        }
        let group_call_target = if group.kind == Kind::Call {
            program
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or_else(|| Diagnostic::error("reconciled call group lost its occurrence"))?
                .target()
                .as_resolved()
                .map(|selected| selected.selected_symbol())
        } else {
            None
        };
        for expression in group.expressions {
            program
                .tables
                .bodies
                .expressions
                .attach_authored_selection_occurrences(expression, [occurrence]);
            if group.kind == Kind::Call {
                if let (Some(selected), ExpressionNode::Call(call)) = (
                    group_call_target,
                    program.tables.bodies.expressions.expression_mut(expression),
                ) && !call.target_symbol.is_valid()
                {
                    call.target_symbol = selected;
                }
                let original = original_call_target(program, expression)?;
                if let (Some(original), ExpressionNode::Call(call)) = (
                    original,
                    program.tables.bodies.expressions.expression_mut(expression),
                ) {
                    call.authored_call_binding = Some(ResolvedAuthoredCallBinding {
                        selected: call.target_symbol,
                        original,
                    });
                }
            }
        }
    }

    Ok(())
}

/*
    Match the authored operand token before typed lowering discards its source
    coordinates. Nested applications retain their own occurrences, not slots in
    the enclosing call; resolved declarations and omitted literal occurrences
    never change the positions of the call's remaining operands.
*/
fn top_level_static_argument_position(
    arguments: &[symbol_resolved_trees::expression::StaticMachineArgument],
    occurrence_span: SourceSpan,
) -> Option<usize> {
    arguments.iter().position(|argument| {
        !argument.path.is_empty()
            && path_span(&argument.path, SourceSpan::default()) == occurrence_span
    })
}

#[cfg(test)]
mod copied_call_tests {
    use super::{CandidateTarget, Kind, reconcile_copy_targets};
    use crate::{ResolutionRequest, resolve};
    use std::sync::Arc;
    use symbol_resolved_trees::expression::ExpressionNode;
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

    #[test]
    fn copied_calls_authenticate_evidence_methods_without_erasing_their_bindings() {
        let text = r#"
            trait Predicate {
                machine valid(value: u64) -> bool;
                machine other(value: u64) -> bool;
            }
            machine first<Model satisfies Predicate>(value: u64) -> bool {
                Model::valid(value)
            }
            machine second<Model satisfies Predicate>(value: u64) -> bool {
                Model::valid(value)
            }
            machine different<Model satisfies Predicate>(value: u64) -> bool {
                Model::other(value)
            }
        "#;
        let mut sources = source::SourceMap::default();
        let source = sources
            .add("calls/evidence.omg".into(), text.into())
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap();
        let mut program = resolve(ResolutionRequest {
            syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
        .unwrap();
        let calls = program
            .tables
            .bodies
            .expressions
            .iter_expressions()
            .filter_map(|(_, expression)| {
                let ExpressionNode::Call(call) = expression else {
                    return None;
                };
                let target = program.symbols.get(call.target_symbol);
                (target.kind == SymbolKind::State
                    && program.symbols.get(target.parent).kind == SymbolKind::ConformanceParameter)
                    .then_some((
                        call.target_symbol,
                        call.target.source_span(),
                        call.authored_call_binding,
                    ))
            })
            .collect::<Vec<_>>();
        let valid = calls
            .iter()
            .filter(|(symbol, _, _)| program.symbols.name(*symbol) == "valid")
            .collect::<Vec<_>>();
        assert_eq!(valid.len(), 2);
        let left = valid[0].0;
        let right = valid[1].0;
        assert_ne!(left, right);
        assert_ne!(
            program.symbols.get(left).parent,
            program.symbols.get(right).parent
        );
        assert!(program.symbols.symbol_source_span(left).is_none());
        assert!(program.symbols.symbol_source_span(right).is_none());
        assert_eq!(
            reconcile_copy_targets(
                &program,
                valid[0].1,
                Kind::Call,
                CandidateTarget::Resolved(left),
                CandidateTarget::Resolved(right)
            )
            .unwrap(),
            CandidateTarget::Resolved(super::earlier_symbol(left, right))
        );
        for (selected, _, binding) in &calls {
            assert_eq!(
                binding
                    .expect("each authored call retains its binding")
                    .selected,
                *selected
            );
        }
        let other = calls
            .iter()
            .find(|(symbol, _, _)| program.symbols.name(*symbol) == "other")
            .unwrap()
            .0;
        assert!(
            reconcile_copy_targets(
                &program,
                valid[0].1,
                Kind::Call,
                CandidateTarget::Resolved(left),
                CandidateTarget::Resolved(other)
            )
            .is_err()
        );

        let malformed = program.symbols.append_children(
            program.symbols.get(left).parent,
            [(SymbolKind::State, SymbolNameRef::Static("valid"))],
        );
        let malformed = SymbolTableBuilder::child_handles(malformed).next().unwrap();
        assert_eq!(
            program.symbols.display_path(left, "::"),
            program.symbols.display_path(malformed, "::")
        );
        assert!(
            reconcile_copy_targets(
                &program,
                calls[0].1,
                Kind::Call,
                CandidateTarget::Resolved(left),
                CandidateTarget::Resolved(malformed)
            )
            .is_err()
        );
    }
    #[test]
    fn projected_evidence_metadata_survives_reactivation_without_restoring_lookup() {
        let text = r#"
            trait Parent<Value> { machine valid(value: Value) -> bool; }
            trait Model<Value>: Parent<Value> {}
            trait Base<Value> { machine check(value: Value) -> Value; }
            trait Refined<Value, Policy satisfies Model<Value>> = Base<Value> {
                machine Base::check
                ensures Policy::valid(result);
            }
        "#;
        let mut sources = source::SourceMap::default();
        let source = sources
            .add("calls/activation.omg".into(), text.into())
            .source_id;
        let parse = || {
            let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap()
        };
        let sources = Arc::new(sources);
        let mut projected = resolve(ResolutionRequest {
            syntax: parse(),
            sources: Some(Arc::clone(&sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
        .unwrap();
        let original = projected
            .tables
            .bodies
            .expressions
            .iter_expressions()
            .find_map(|(_, expression)| {
                let ExpressionNode::Call(call) = expression else {
                    return None;
                };
                (projected.symbols.name(call.target_symbol) == "valid").then(|| {
                    call.authored_call_binding
                        .expect("original contract call binding")
                        .original
                })
            })
            .expect("the applied refinement carries its original call");
        let old_binder = projected.symbols.get(original).parent;
        let old_owner = projected.symbols.get(old_binder).parent;
        assert_eq!(projected.symbols.name(old_owner), "Refined");
        let bound = projected
            .traits
            .iter()
            .find(|declaration| declaration.symbol == old_owner)
            .unwrap()
            .conformance_bounds
            .iter()
            .find(|bound| bound.binder == old_binder)
            .unwrap();
        let arguments = bound.arguments;
        let argument_values = projected.child_type_references(arguments).to_vec();
        assert_eq!(argument_values.len(), 1);
        let signature =
            crate::symbols::requirement_namespaces::evidence_requirement(&projected, original)
                .unwrap()
                .unwrap()
                .clone();
        let declared =
            super::super::call_identity::selected_declaration_source(&projected, original).unwrap();
        projected.roots.traits.retain(|_| false);
        let mut inactive = sources.as_ref().clone();
        inactive.retain_selected_sources(|_| false);
        projected.symbols = projected
            .symbols
            .clone()
            .begin_source_activation(Arc::new(inactive), Vec::new())
            .unwrap()
            .finish()
            .begin_source_activation(Arc::clone(&sources), Vec::new())
            .unwrap()
            .finish();
        let mut projected = projected.clone();
        projected.rebuild_tables();
        assert!(projected.traits.is_empty());
        assert_eq!(projected.child_type_references(arguments), argument_values);
        assert!(!projected.symbols.symbol_source_is_selected(old_owner));
        let reference = source::SourceSpan::new(source, source::Span::new(0, 1));
        assert!(
            projected
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    "Refined",
                    &[SymbolKind::Trait],
                    reference
                )
                .is_none()
        );
        assert_eq!(
            crate::symbols::requirement_namespaces::evidence_requirement(&projected, original)
                .unwrap(),
            Some(&signature)
        );
        assert_eq!(
            super::super::call_identity::selected_declaration_source(&projected, original).unwrap(),
            declared
        );
        assert!(!projected.symbols.symbol_source_is_selected(old_owner));
        assert!(
            projected
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    "Refined",
                    &[SymbolKind::Trait],
                    reference
                )
                .is_none()
        );

        let resumed = crate::resolve_extension(crate::ExtensionRequest {
            base: Arc::new(projected),
            syntax: parse(),
            sources: Arc::clone(&sources),
            top_level_bindings: Vec::new(),
        })
        .unwrap();
        let mut resumed = resumed.trees().clone();
        assert_eq!(resumed.child_type_references(arguments), argument_values);
        let owners = resumed
            .traits
            .iter()
            .filter(|declaration| declaration.name.as_str() == "Refined")
            .map(|declaration| declaration.symbol)
            .collect::<Vec<_>>();
        assert_eq!(owners.len(), 1);
        assert_ne!(owners[0], old_owner);
        assert!(!resumed.symbols.symbol_source_is_selected(old_owner));
        assert!(resumed.symbols.symbol_source_is_selected(owners[0]));
        let active_calls = resumed
            .traits
            .iter()
            .find(|declaration| declaration.symbol == owners[0])
            .unwrap()
            .refinement_clauses
            .iter()
            .flat_map(|clause| resumed.signature_contracts(clause.signature.contracts))
            .flat_map(|contract| resumed.proof_facts(contract.facts))
            .filter_map(|fact| match fact {
                symbol_resolved_trees::domain::ProofFact::Expression(expression) => {
                    Some(*expression)
                }
                symbol_resolved_trees::domain::ProofFact::Membership(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(active_calls.len(), 1);
        let mut selected = Vec::new();
        let mut active_count = 0;
        let mut retained_original_count = 0;
        for (expression, node) in resumed.tables.bodies.expressions.iter_expressions() {
            let ExpressionNode::Call(call) = node else {
                continue;
            };
            if resumed.symbols.name(call.target_symbol) != "valid" {
                continue;
            }
            let binding = call.authored_call_binding.expect("contract call binding");
            assert_eq!(binding.original, original);
            assert_eq!(binding.selected, call.target_symbol);
            assert_eq!(
                super::super::call_identity::selected_declaration_source(
                    &resumed,
                    binding.original
                )
                .unwrap(),
                declared
            );
            assert_eq!(
                super::super::call_identity::selected_declaration_source(
                    &resumed,
                    binding.selected
                )
                .unwrap(),
                declared
            );
            if active_calls.contains(&expression) {
                active_count += 1;
                assert_ne!(binding.selected, binding.original);
                let binder = resumed.symbols.get(binding.selected).parent;
                assert_ne!(binder, old_binder);
                assert_eq!(resumed.symbols.get(binder).parent, owners[0]);
                selected.push(binding.selected);
            } else if binding.selected == binding.original {
                retained_original_count += 1;
            }
        }
        assert_eq!(active_count, 1);
        assert!(retained_original_count > 0);
        assert_eq!(
            super::super::call_identity::selected_declaration_source(&resumed, original).unwrap(),
            declared
        );
        let malformed = resumed.symbols.append_children(
            old_binder,
            [(SymbolKind::State, SymbolNameRef::Static("valid"))],
        );
        let malformed = SymbolTableBuilder::child_handles(malformed).next().unwrap();
        assert!(
            super::super::call_identity::selected_declaration_source(&resumed, malformed).is_err()
        );
        assert!(
            super::super::call_identity::selected_declaration_source(&resumed, original).is_err()
        );
        assert_eq!(
            super::super::call_identity::selected_declaration_source(&resumed, selected[0])
                .unwrap(),
            declared
        );
        assert!(!resumed.symbols.symbol_source_is_selected(old_owner));
    }
    #[test]
    fn copied_static_callable_entries_keep_their_selected_applications() {
        let text = r#"
            machine identity<Value>(value: Value) -> Value { value }
            machine invoke<Value, machine Function>(value: Value) -> Value
            requires machine Function(value: Value) -> Value;
            { Function(value) }
            machine relay<Value>(value: Value) -> Value {
                invoke<Value, identity<Value>>(value)
            }
        "#;
        let mut sources = source::SourceMap::default();
        let source = sources
            .add("calls/static.omg".into(), text.into())
            .source_id;
        let parse = || {
            let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap()
        };
        let sources = Arc::new(sources);
        let mut projected = resolve(ResolutionRequest {
            syntax: parse(),
            sources: Some(Arc::clone(&sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
        .unwrap();
        let machine = projected
            .machines
            .iter()
            .find(|m| m.name.as_str() == "identity")
            .unwrap();
        let original_owner = machine.symbol;
        let original = projected
            .machine_state(projected.machine_state_handles(machine.states)[0])
            .symbol;
        assert!(projected.symbols.symbol_source_span(original).is_none());
        let declared =
            super::super::call_identity::selected_declaration_source(&projected, original).unwrap();
        let occurrence = projected
            .authored_declaration_selections()
            .iter()
            .find(|selection| {
                selection.kind() == Kind::StaticArgument
                    && selection
                        .target()
                        .as_resolved()
                        .is_some_and(|target| target.selected_symbol() == original)
            })
            .expect("the generic callable is selected as a static argument")
            .source_span();
        projected.roots.machines.retain(|_| false);
        let mut inactive = sources.as_ref().clone();
        inactive.retain_selected_sources(|_| false);
        projected.symbols = projected
            .symbols
            .clone()
            .begin_source_activation(Arc::new(inactive), Vec::new())
            .unwrap()
            .finish()
            .begin_source_activation(Arc::clone(&sources), Vec::new())
            .unwrap()
            .finish();
        projected.rebuild_tables();
        assert!(!projected.symbols.symbol_source_is_selected(original_owner));
        let resumed = crate::resolve_extension(crate::ExtensionRequest {
            base: Arc::new(projected),
            syntax: parse(),
            sources: Arc::clone(&sources),
            top_level_bindings: Vec::new(),
        })
        .unwrap();
        let mut resumed = resumed.trees().clone();
        let machine = resumed
            .machines
            .iter()
            .find(|m| m.name.as_str() == "identity")
            .unwrap();
        let current_owner = machine.symbol;
        let current = resumed
            .machine_state(resumed.machine_state_handles(machine.states)[0])
            .symbol;
        assert_ne!(current_owner, original_owner);
        assert_ne!(current, original);
        assert!(!resumed.symbols.symbol_source_is_selected(original_owner));
        assert!(resumed.symbols.symbol_source_is_selected(current_owner));
        assert_eq!(
            reconcile_copy_targets(
                &resumed,
                occurrence,
                Kind::StaticArgument,
                CandidateTarget::Resolved(original),
                CandidateTarget::Resolved(current)
            )
            .unwrap(),
            CandidateTarget::Resolved(original)
        );
        assert!(
            resumed
                .authored_declaration_selections()
                .iter()
                .any(|selection| {
                    selection.kind() == Kind::StaticArgument
                        && selection.source_span() == occurrence
                        && selection
                            .target()
                            .as_resolved()
                            .is_some_and(|target| target.selected_symbol() == original)
                })
        );
        let relay = resumed
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == "relay")
            .unwrap();
        let relay_value = resumed
            .machine_type_parameters(relay)
            .iter()
            .find(|parameter| parameter.name.as_str() == "Value")
            .unwrap()
            .symbol;
        let current_arguments = resumed
            .tables
            .bodies
            .expressions
            .iter_expressions()
            .filter_map(|(_, expression)| {
                let ExpressionNode::Call(call) = expression else {
                    return None;
                };
                call.machine_arguments
                    .iter()
                    .find(|argument| argument.symbol == current)
            })
            .collect::<Vec<_>>();
        assert!(!current_arguments.is_empty());
        for argument in current_arguments {
            let application = argument
                .application
                .as_ref()
                .expect("selected generic application");
            assert_eq!(application.arguments.len(), 1);
            assert_eq!(application.arguments[0].symbol, relay_value);
            assert_eq!(
                resumed.symbols.get(application.arguments[0].symbol).parent,
                relay.symbol
            );
            assert_eq!(argument.symbol, current);
        }
        assert_eq!(
            super::super::call_identity::selected_declaration_source(&resumed, current).unwrap(),
            declared
        );
        let other_machine = resumed
            .machines
            .iter()
            .find(|m| m.name.as_str() == "relay")
            .unwrap();
        let other = resumed
            .machine_state(resumed.machine_state_handles(other_machine.states)[0])
            .symbol;
        assert!(
            reconcile_copy_targets(
                &resumed,
                occurrence,
                Kind::StaticArgument,
                CandidateTarget::Resolved(original),
                CandidateTarget::Resolved(other)
            )
            .is_err()
        );
        let extra = resumed.symbols.append_children(
            current_owner,
            [(SymbolKind::State, SymbolNameRef::Static("not_entry"))],
        );
        let extra = SymbolTableBuilder::child_handles(extra).next().unwrap();
        assert!(
            reconcile_copy_targets(
                &resumed,
                occurrence,
                Kind::StaticArgument,
                CandidateTarget::Resolved(current),
                CandidateTarget::Resolved(extra)
            )
            .is_err()
        );
    }
}
