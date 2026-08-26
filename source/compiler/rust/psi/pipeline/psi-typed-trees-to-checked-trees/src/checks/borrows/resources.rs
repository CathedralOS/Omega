use psi_checked_trees::{
    BorrowFacts, BorrowLoanFact, BorrowLoanLineage, CheckFacts, CheckedDirectBorrowLoanResource,
    CheckedDirectBorrowParentLifetime, CheckedDirectBorrowRestorationObligation,
    CheckedParentBorrowResource, CheckedReborrowLoanResource, CheckedReborrowParentEndStatus,
    CheckedReborrowParentSuspensionBoundary, CheckedReborrowRestorationObligation, FlowFacts,
    FlowInvalidationSource, ParentLexicalStatusAtChildEnd,
};
use psi_diagnostics::Diagnostic;

/// Populate the checked-only direct-root and direct-reborrow resource closures
/// before ordinary checked-fact replay.
pub(super) fn initialize_checked_direct_borrow_resources(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    replay_checked_direct_reborrow_lineage(program, &facts.borrow)?;
    let direct = reconstruct_direct_borrow_resources(&facts.borrow, &facts.flow)?;
    let reborrows = reconstruct_reborrow_resource_drafts(&facts.borrow, &facts.flow)?;
    let installation = plan_resource_installation(&direct, &reborrows)?;
    install_borrow_resources(&mut facts.borrow, direct, &reborrows, &installation);
    Ok(())
}

/// Independently replay every retained resource from the authoritative loan
/// and flow-lifetime ledgers, then transactionally rebuild both arenas with
/// remapped typed parent handles. The rows never participate in admission.
pub(super) fn replay_checked_direct_borrow_resources(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    replay_checked_direct_reborrow_lineage(program, &facts.borrow)?;
    let expected_direct = reconstruct_direct_borrow_resources(&facts.borrow, &facts.flow)?;
    let expected_reborrows = reconstruct_reborrow_resource_drafts(&facts.borrow, &facts.flow)?;
    let retained = facts
        .borrow
        .direct_loan_resources
        .iter()
        .map(|(_, resource)| resource.clone())
        .collect::<Vec<_>>();
    if retained != expected_direct {
        return Err(vec![Diagnostic::error(
            "checked direct-root borrow resource closure drifted from independent replay",
        )]);
    }
    validate_retained_reborrow_resources(&facts.borrow, &expected_reborrows)?;
    let installation = plan_resource_installation(&expected_direct, &expected_reborrows)?;

    install_borrow_resources(
        &mut facts.borrow,
        expected_direct,
        &expected_reborrows,
        &installation,
    );
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckedReborrowLoanResourceDraft {
    loan: psi_arena::Handle<BorrowLoanFact>,
    machine_symbol: psi_symbols::SymbolHandle,
    state_symbol: psi_symbols::SymbolHandle,
    owner_symbol: psi_symbols::SymbolHandle,
    owner_path: Vec<psi_checked_trees::BorrowLoanOwnerSegment>,
    captured_place: psi_checked_trees::CapturedPlace,
    access: psi_checked_trees::BorrowAccessKind,
    activation_source: psi_checked_trees::FlowInvalidationSource,
    weakening_source: psi_checked_trees::FlowInvalidationSource,
    weakening_reason: psi_checked_trees::FlowBorrowWeakeningReason,
    parent_loan: psi_arena::Handle<BorrowLoanFact>,
    child_activation: psi_arena::Handle<psi_checked_trees::FlowBorrowActivationFact>,
    parent_entry_constraint: psi_arena::Handle<psi_checked_trees::FlowConstraintRef>,
    child_weakening: psi_arena::Handle<psi_checked_trees::FlowBorrowWeakeningFact>,
    parent_weakening: psi_arena::Handle<psi_checked_trees::FlowBorrowWeakeningFact>,
    parent_lexical_status: ParentLexicalStatusAtChildEnd,
}

impl CheckedReborrowLoanResourceDraft {
    fn close(&self, parent_resource: CheckedParentBorrowResource) -> CheckedReborrowLoanResource {
        CheckedReborrowLoanResource {
            loan: self.loan,
            machine_symbol: self.machine_symbol,
            state_symbol: self.state_symbol,
            owner_symbol: self.owner_symbol,
            owner_path: self.owner_path.clone(),
            captured_place: self.captured_place.clone(),
            access: self.access.clone(),
            activation_source: self.activation_source,
            weakening_source: self.weakening_source,
            weakening_reason: self.weakening_reason,
            parent_loan: self.parent_loan,
            parent_resource: parent_resource.clone(),
            parent_suspension: CheckedReborrowParentSuspensionBoundary {
                child_loan: self.loan,
                parent_loan: self.parent_loan,
                parent_resource: parent_resource.clone(),
                child_activation: self.child_activation,
                parent_entry_constraint: self.parent_entry_constraint,
                source: self.activation_source,
            },
            parent_end_status: CheckedReborrowParentEndStatus {
                child_loan: self.loan,
                parent_loan: self.parent_loan,
                parent_resource: parent_resource.clone(),
                child_weakening: self.child_weakening,
                parent_weakening: self.parent_weakening,
                status: self.parent_lexical_status,
            },
            restoration: CheckedReborrowRestorationObligation {
                child_loan: self.loan,
                parent_loan: self.parent_loan,
                parent_resource,
                child_weakening_source: self.weakening_source,
                child_weakening_reason: self.weakening_reason,
            },
        }
    }
}

fn validate_retained_reborrow_resources(
    borrow: &BorrowFacts,
    expected: &[CheckedReborrowLoanResourceDraft],
) -> Result<(), Vec<Diagnostic>> {
    let retained = borrow.reborrow_loan_resources.iter().collect::<Vec<_>>();
    if retained.len() != expected.len() {
        return Err(reborrow_resource_drift());
    }

    let mut prior_reborrows = Vec::new();
    for ((resource_handle, retained), draft) in retained.into_iter().zip(expected) {
        let parent_resource = retained_parent_resource(borrow, draft.parent_loan, &prior_reborrows)
            .ok_or_else(reborrow_resource_drift)?;
        if retained != &draft.close(parent_resource) {
            return Err(reborrow_resource_drift());
        }
        prior_reborrows.push((draft.loan, resource_handle));
    }
    Ok(())
}

fn retained_parent_resource(
    borrow: &BorrowFacts,
    parent_loan: psi_arena::Handle<BorrowLoanFact>,
    prior_reborrows: &[(
        psi_arena::Handle<BorrowLoanFact>,
        psi_arena::Handle<CheckedReborrowLoanResource>,
    )],
) -> Option<CheckedParentBorrowResource> {
    match &borrow.loans.get(parent_loan).lineage {
        BorrowLoanLineage::DirectRoot => {
            let mut matches = borrow
                .direct_loan_resources
                .iter()
                .filter(|(_, resource)| resource.loan == parent_loan);
            let handle = matches.next()?.0;
            matches
                .next()
                .is_none()
                .then_some(CheckedParentBorrowResource::DirectRoot { resource: handle })
        }
        BorrowLoanLineage::Reborrow { .. } => {
            prior_reborrows.iter().find_map(|(loan, resource)| {
                (*loan == parent_loan).then_some(CheckedParentBorrowResource::Reborrow {
                    resource: *resource,
                })
            })
        }
        BorrowLoanLineage::UnretainedDerived => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParentResourceIndex {
    Direct(usize),
    Reborrow(usize),
}

/// Resolve the entire parent graph before either retained arena is reset.
/// Installation is therefore a purely indexed, infallible rewrite.
fn plan_resource_installation(
    direct: &[CheckedDirectBorrowLoanResource],
    reborrows: &[CheckedReborrowLoanResourceDraft],
) -> Result<Vec<ParentResourceIndex>, Vec<Diagnostic>> {
    let mut plan = Vec::with_capacity(reborrows.len());
    for (child_index, child) in reborrows.iter().enumerate() {
        let direct_matches = direct
            .iter()
            .enumerate()
            .filter(|(_, resource)| resource.loan == child.parent_loan)
            .map(|(index, _)| ParentResourceIndex::Direct(index));
        let reborrow_matches = reborrows[..child_index]
            .iter()
            .enumerate()
            .filter(|(_, resource)| resource.loan == child.parent_loan)
            .map(|(index, _)| ParentResourceIndex::Reborrow(index));
        let mut matches = direct_matches.chain(reborrow_matches);
        let Some(parent) = matches.next() else {
            return Err(reborrow_resource_drift());
        };
        if matches.next().is_some() {
            return Err(reborrow_resource_drift());
        }
        plan.push(parent);
    }
    Ok(plan)
}

fn install_borrow_resources(
    borrow: &mut BorrowFacts,
    direct: Vec<CheckedDirectBorrowLoanResource>,
    reborrows: &[CheckedReborrowLoanResourceDraft],
    installation: &[ParentResourceIndex],
) {
    borrow.direct_loan_resources.reset_retain_capacity();
    borrow.reborrow_loan_resources.reset_retain_capacity();

    let mut direct_handles = Vec::with_capacity(direct.len());
    for resource in direct {
        let handle = borrow.direct_loan_resources.insert(resource);
        direct_handles.push(handle);
    }

    let mut reborrow_handles: Vec<psi_arena::Handle<CheckedReborrowLoanResource>> =
        Vec::with_capacity(reborrows.len());
    for (draft, parent) in reborrows.iter().zip(installation) {
        let parent_resource = match *parent {
            ParentResourceIndex::Direct(index) => CheckedParentBorrowResource::DirectRoot {
                resource: direct_handles[index],
            },
            ParentResourceIndex::Reborrow(index) => CheckedParentBorrowResource::Reborrow {
                resource: reborrow_handles[index],
            },
        };
        let handle = borrow
            .reborrow_loan_resources
            .insert(draft.close(parent_resource));
        reborrow_handles.push(handle);
    }
}

fn reborrow_resource_drift() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "checked direct-reborrow resource closure drifted from independent topological replay",
    )]
}

fn reconstruct_direct_borrow_resources(
    borrow: &BorrowFacts,
    flow: &FlowFacts,
) -> Result<Vec<CheckedDirectBorrowLoanResource>, Vec<Diagnostic>> {
    let mut resources = Vec::new();
    let mut diagnostics = Vec::new();

    for (_, state) in borrow.states.iter() {
        let Some(flow_state) = flow.control.states.iter().find_map(|(_, candidate)| {
            (candidate.machine_symbol == state.machine_symbol
                && candidate.state_symbol == state.state_symbol)
                .then_some(candidate)
        }) else {
            diagnostics.push(Diagnostic::error(
                "checked direct-root borrow resource has no exact flow-state owner",
            ));
            continue;
        };

        for (loan_handle, loan) in borrow
            .loans
            .iter()
            .filter(|(handle, _)| borrow.state_owns_loan(state, *handle))
        {
            // Direct reborrows close in their own typed parent-resource arena;
            // every derived occurrence remains outside this root-only arena.
            if loan.lineage != BorrowLoanLineage::DirectRoot {
                continue;
            }

            let activations = flow
                .borrow_lifetimes
                .activations
                .span_or_empty(flow_state.borrow_activations)
                .iter()
                .filter(|activation| activation.loan == loan_handle)
                .collect::<Vec<_>>();
            let weakenings = flow
                .borrow_lifetimes
                .weakenings
                .span_or_empty(flow_state.borrow_weakenings)
                .iter()
                .filter(|weakening| weakening.loan == loan_handle)
                .collect::<Vec<_>>();
            let ([activation], [weakening]) = (activations.as_slice(), weakenings.as_slice())
            else {
                diagnostics.push(Diagnostic::error(
                    "checked direct-root borrow resource requires exactly one activation and one weakening",
                ));
                continue;
            };
            if activation.source
                != (FlowInvalidationSource::Statement {
                    statement_index: loan.statement_index,
                })
            {
                diagnostics.push(Diagnostic::error(
                    "checked direct-root borrow activation drifted from loan formation",
                ));
                continue;
            }

            let parent_lifetime = CheckedDirectBorrowParentLifetime {
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                root_symbol: loan.root_symbol,
            };
            let restoration = CheckedDirectBorrowRestorationObligation {
                parent: parent_lifetime.clone(),
                weakening_source: weakening.source,
                weakening_reason: weakening.reason,
            };
            resources.push(CheckedDirectBorrowLoanResource {
                loan: loan_handle,
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                owner_symbol: loan.owner_symbol,
                owner_path: borrow.loan_owner_path(loan).to_vec(),
                captured_place: psi_checked_trees::CapturedPlace {
                    root_symbol: loan.root_symbol,
                    segments: borrow.loan_segments(loan).to_vec(),
                },
                access: loan.kind.clone(),
                activation_source: activation.source,
                weakening_source: weakening.source,
                weakening_reason: weakening.reason,
                parent_lifetime,
                restoration,
            });
        }
    }

    if diagnostics.is_empty() {
        Ok(resources)
    } else {
        Err(diagnostics)
    }
}

fn reconstruct_reborrow_resource_drafts(
    borrow: &BorrowFacts,
    flow: &FlowFacts,
) -> Result<Vec<CheckedReborrowLoanResourceDraft>, Vec<Diagnostic>> {
    let mut resources = Vec::new();
    let mut diagnostics = Vec::new();

    for (_, state) in borrow.states.iter() {
        let Some(flow_state) = flow.control.states.iter().find_map(|(_, candidate)| {
            (candidate.machine_symbol == state.machine_symbol
                && candidate.state_symbol == state.state_symbol)
                .then_some(candidate)
        }) else {
            diagnostics.push(Diagnostic::error(
                "checked direct-reborrow resource has no exact flow-state owner",
            ));
            continue;
        };

        for (loan_handle, loan) in borrow
            .loans
            .iter()
            .filter(|(handle, _)| borrow.state_owns_loan(state, *handle))
        {
            let BorrowLoanLineage::Reborrow { parent_loan } = &loan.lineage else {
                continue;
            };
            let activations = flow
                .borrow_lifetimes
                .activations
                .span_or_empty(flow_state.borrow_activations)
                .iter()
                .enumerate()
                .filter(|(_, activation)| activation.loan == loan_handle)
                .filter_map(|(offset, activation)| {
                    span_handle(flow_state.borrow_activations, offset)
                        .map(|handle| (handle, activation))
                })
                .collect::<Vec<_>>();
            let weakenings = flow
                .borrow_lifetimes
                .weakenings
                .span_or_empty(flow_state.borrow_weakenings)
                .iter()
                .enumerate()
                .filter(|(_, weakening)| weakening.loan == loan_handle)
                .filter_map(|(offset, weakening)| {
                    span_handle(flow_state.borrow_weakenings, offset)
                        .map(|handle| (handle, weakening))
                })
                .collect::<Vec<_>>();
            if activations.len() != 1 || weakenings.len() != 1 {
                diagnostics.push(Diagnostic::error(
                    "checked direct-reborrow resource requires exactly one activation and one weakening",
                ));
                continue;
            }
            let (child_activation, activation) = activations[0];
            let (child_weakening, weakening) = weakenings[0];
            if activation.source
                != (FlowInvalidationSource::Statement {
                    statement_index: loan.statement_index,
                })
            {
                diagnostics.push(Diagnostic::error(
                    "checked direct-reborrow activation drifted from loan formation",
                ));
                continue;
            }
            let parent_weakenings = flow
                .borrow_lifetimes
                .weakenings
                .span_or_empty(flow_state.borrow_weakenings)
                .iter()
                .enumerate()
                .filter(|(_, weakening)| weakening.loan == *parent_loan)
                .filter_map(|(offset, weakening)| {
                    span_handle(flow_state.borrow_weakenings, offset)
                        .map(|handle| (handle, weakening))
                })
                .collect::<Vec<_>>();
            let [(parent_weakening, parent_weakening_fact)] = parent_weakenings.as_slice() else {
                diagnostics.push(Diagnostic::error(
                    "checked direct-reborrow parent status requires exactly one parent weakening",
                ));
                continue;
            };
            let Some(parent_lexical_status) = parent_lexical_status_at_child_end(
                parent_weakening_fact.source,
                parent_weakening_fact.reason,
                weakening.source,
                weakening.reason,
            ) else {
                diagnostics.push(Diagnostic::error(
                    "checked direct-reborrow parent status has an unsupported weakening boundary",
                ));
                continue;
            };

            let Some(statement) = flow
                .control
                .statements
                .span_or_empty(flow_state.statements)
                .iter()
                .find(|statement| statement.statement_index == loan.statement_index)
            else {
                diagnostics.push(Diagnostic::error(
                    "checked direct-reborrow suspension has no exact formation statement",
                ));
                continue;
            };
            let parent_constraints = flow
                .contexts
                .constraint_refs
                .span_or_empty(statement.entry_constraints)
                .iter()
                .enumerate()
                .filter(|(_, constraint)| {
                    constraint.kind
                        == psi_checked_trees::FlowConstraintKind::BorrowLoan { loan: *parent_loan }
                })
                .filter_map(|(offset, _)| span_handle(statement.entry_constraints, offset))
                .collect::<Vec<_>>();
            let [parent_entry_constraint] = parent_constraints.as_slice() else {
                diagnostics.push(Diagnostic::error(
                    "checked direct-reborrow suspension requires exactly one parent entry constraint",
                ));
                continue;
            };

            resources.push(CheckedReborrowLoanResourceDraft {
                loan: loan_handle,
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                owner_symbol: loan.owner_symbol,
                owner_path: borrow.loan_owner_path(loan).to_vec(),
                captured_place: psi_checked_trees::CapturedPlace {
                    root_symbol: loan.root_symbol,
                    segments: borrow.loan_segments(loan).to_vec(),
                },
                access: loan.kind.clone(),
                activation_source: activation.source,
                weakening_source: weakening.source,
                weakening_reason: weakening.reason,
                parent_loan: *parent_loan,
                child_activation,
                parent_entry_constraint: *parent_entry_constraint,
                child_weakening,
                parent_weakening: *parent_weakening,
                parent_lexical_status,
            });
        }
    }

    if diagnostics.is_empty() {
        Ok(resources)
    } else {
        Err(diagnostics)
    }
}

fn parent_lexical_status_at_child_end(
    parent_source: FlowInvalidationSource,
    parent_reason: psi_checked_trees::FlowBorrowWeakeningReason,
    child_source: FlowInvalidationSource,
    child_reason: psi_checked_trees::FlowBorrowWeakeningReason,
) -> Option<ParentLexicalStatusAtChildEnd> {
    let parent = weakening_boundary_key(parent_source, parent_reason)?;
    let child = weakening_boundary_key(child_source, child_reason)?;
    Some(match parent.cmp(&child) {
        std::cmp::Ordering::Less => ParentLexicalStatusAtChildEnd::RetiredBeforeChild,
        std::cmp::Ordering::Equal => ParentLexicalStatusAtChildEnd::RetiredWithChild,
        std::cmp::Ordering::Greater => ParentLexicalStatusAtChildEnd::LivePastChild,
    })
}

fn weakening_boundary_key(
    source: FlowInvalidationSource,
    reason: psi_checked_trees::FlowBorrowWeakeningReason,
) -> Option<(usize, u8)> {
    let FlowInvalidationSource::Statement { statement_index } = source else {
        return None;
    };
    let phase = match reason {
        psi_checked_trees::FlowBorrowWeakeningReason::LastUseExpired => 0,
        psi_checked_trees::FlowBorrowWeakeningReason::LocalReassigned => 1,
        psi_checked_trees::FlowBorrowWeakeningReason::StateExit => 2,
    };
    Some((statement_index, phase))
}

fn span_handle<T>(span: psi_arena::HandleSpan<T>, offset: usize) -> Option<psi_arena::Handle<T>> {
    let offset = u32::try_from(offset).ok()?;
    let arena_index = span.start().arena_index().checked_add(offset)?;
    Some(psi_arena::Handle::from_parts(
        arena_index,
        span.start().generation(),
    ))
}

fn replay_checked_direct_reborrow_lineage(
    program: &psi_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
) -> Result<(), Vec<Diagnostic>> {
    for (_, state) in borrow.states.iter() {
        let Some(typed_state) = crate::semantic_calls::find_state_in_machine(
            program,
            state.machine_symbol,
            state.state_symbol,
        ) else {
            return Err(vec![Diagnostic::error(
                "checked borrow loan lineage has no exact typed state owner",
            )]);
        };
        for (loan_handle, loan) in borrow
            .loans
            .iter()
            .filter(|(handle, _)| borrow.state_owns_loan(state, *handle))
        {
            let expected =
                expected_loan_lineage(program, typed_state, borrow, state, loan_handle, loan);
            if loan.lineage != expected {
                return Err(vec![Diagnostic::error(
                    "checked borrow loan lineage drifted from independent direct-reborrow replay",
                )]);
            }
        }
    }
    Ok(())
}

fn expected_loan_lineage(
    program: &psi_typed_trees::TypedTrees,
    typed_state: &psi_typed_trees::state::State,
    borrow: &BorrowFacts,
    state: &psi_checked_trees::StateBorrowFact,
    loan_handle: psi_arena::Handle<BorrowLoanFact>,
    loan: &BorrowLoanFact,
) -> BorrowLoanLineage {
    let Some(statement) = program
        .statement_table
        .statements(typed_state.statement_nodes)
        .get(loan.statement_index)
    else {
        return if loan.source_owner_symbol.is_valid() {
            BorrowLoanLineage::UnretainedDerived
        } else {
            BorrowLoanLineage::DirectRoot
        };
    };
    let psi_checked_trees::statement::StatementNode::LocalData(local) = statement else {
        if let psi_checked_trees::statement::StatementNode::Assignment(assignment) = statement
            && matches!(
                program.expression_table.expression(assignment.value),
                psi_checked_trees::expression::ExpressionNode::Call(_)
                    | psi_checked_trees::expression::ExpressionNode::Cast(_)
                    | psi_checked_trees::expression::ExpressionNode::ArrayLiteral(_)
                    | psi_checked_trees::expression::ExpressionNode::StructLiteral(_)
            )
        {
            return BorrowLoanLineage::UnretainedDerived;
        }
        return if loan.source_owner_symbol.is_valid() {
            BorrowLoanLineage::UnretainedDerived
        } else {
            BorrowLoanLineage::DirectRoot
        };
    };
    if local.symbol != loan.owner_symbol {
        return BorrowLoanLineage::UnretainedDerived;
    }

    match program.expression_table.expression(local.initial_value) {
        psi_checked_trees::expression::ExpressionNode::Borrow(reborrow) => {
            expected_explicit_reborrow_parent(
                program,
                typed_state,
                borrow,
                state,
                loan_handle,
                loan,
                reborrow.target,
            )
            .map(|parent_loan| BorrowLoanLineage::Reborrow { parent_loan })
            .unwrap_or_else(|| {
                if loan.source_owner_symbol.is_valid() {
                    BorrowLoanLineage::UnretainedDerived
                } else {
                    BorrowLoanLineage::DirectRoot
                }
            })
        }
        psi_checked_trees::expression::ExpressionNode::Call(_)
        | psi_checked_trees::expression::ExpressionNode::Cast(_)
        | psi_checked_trees::expression::ExpressionNode::ArrayLiteral(_)
        | psi_checked_trees::expression::ExpressionNode::StructLiteral(_) => {
            BorrowLoanLineage::UnretainedDerived
        }
        _ if loan.source_owner_symbol.is_valid() => BorrowLoanLineage::UnretainedDerived,
        _ => BorrowLoanLineage::DirectRoot,
    }
}

#[allow(clippy::too_many_arguments)]
fn expected_explicit_reborrow_parent(
    program: &psi_typed_trees::TypedTrees,
    typed_state: &psi_typed_trees::state::State,
    borrow: &BorrowFacts,
    state: &psi_checked_trees::StateBorrowFact,
    child_handle: psi_arena::Handle<BorrowLoanFact>,
    child: &BorrowLoanFact,
    source_expression: psi_checked_trees::expression::ExpressionHandle,
) -> Option<psi_arena::Handle<BorrowLoanFact>> {
    let source = crate::flow::canonical_place_from_expression_in_state(
        program,
        typed_state.symbol,
        child.statement_index,
        source_expression,
    )?;
    let psi_facts::PlaceRoot::Symbol(source_root) = source.root else {
        return None;
    };
    let mut candidates = borrow
        .loans
        .iter()
        .filter(|(parent_handle, parent)| {
            *parent_handle != child_handle
                && borrow.state_owns_loan(state, *parent_handle)
                && parent.statement_index < child.statement_index
                && parent.lineage != BorrowLoanLineage::UnretainedDerived
                && parent.owner_symbol == source_root
                && owner_path_matches_source(
                    program,
                    borrow.loan_owner_path(parent),
                    &source.segments,
                )
                && child.source_owner_symbol == parent.owner_symbol
        })
        .map(|(handle, parent)| (handle, parent));
    let (parent_handle, parent) = candidates.next()?;
    if candidates.next().is_some()
        || !child_place_replays_from_parent(borrow, parent, &source.segments, child)
    {
        return None;
    }
    Some(parent_handle)
}

fn child_place_replays_from_parent(
    borrow: &BorrowFacts,
    parent: &BorrowLoanFact,
    source_segments: &[psi_facts::PlaceSegment],
    child: &BorrowLoanFact,
) -> bool {
    let parent_owner_path = borrow.loan_owner_path(parent);
    let Some(remainder) = source_segments.get(parent_owner_path.len()..) else {
        return false;
    };
    child.root_symbol == parent.root_symbol
        && borrow.loan_segments(child).len() == borrow.loan_segments(parent).len() + remainder.len()
        && borrow
            .loan_segments(child)
            .iter()
            .eq(borrow.loan_segments(parent).iter().chain(remainder))
}

fn owner_path_matches_source(
    program: &psi_typed_trees::TypedTrees,
    owner_path: &[psi_checked_trees::BorrowLoanOwnerSegment],
    source_segments: &[psi_facts::PlaceSegment],
) -> bool {
    owner_path.len() <= source_segments.len()
        && owner_path
            .iter()
            .zip(source_segments)
            .all(|(owner, source)| match (owner, source) {
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::Field(owner_symbol),
                    psi_facts::PlaceSegment::Field {
                        symbol: source_symbol,
                    },
                ) => !source_symbol.is_valid() || owner_symbol == source_symbol,
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::Case(owner_variant),
                    psi_facts::PlaceSegment::Case {
                        variant: source_variant,
                    },
                ) => owner_variant == source_variant,
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::FixedIndex(owner_index),
                    psi_facts::PlaceSegment::FixedIndex {
                        index: source_index,
                    },
                ) => owner_index == source_index,
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::FixedIndex(owner_index),
                    psi_facts::PlaceSegment::Index { expression },
                ) => program
                    .expression_table
                    .constant_integer_value(*expression)
                    .and_then(|value| usize::try_from(value).ok())
                    .is_none_or(|source_index| *owner_index == source_index),
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::DynamicIndex,
                    psi_facts::PlaceSegment::FixedIndex { .. }
                    | psi_facts::PlaceSegment::FixedRange { .. }
                    | psi_facts::PlaceSegment::Index { .. },
                ) => true,
                _ => false,
            })
}
