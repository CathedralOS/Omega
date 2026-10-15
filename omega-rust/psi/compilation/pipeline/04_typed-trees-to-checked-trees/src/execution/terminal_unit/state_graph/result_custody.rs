//! Result custody accounting for one composed Unit state.
//!
//! Every structural result a state's operations produce has exactly one
//! owner on each path out of the state: a later operation that consumes it,
//! a store that writes it, a successor edge that transfers it, or a cleanup
//! row that discards it. An all-crash exit abandons still-live custody without
//! manufacturing a disposition. Normal paths with no owner and every path
//! with duplicate dispositions refuse; closed sums account per case.
//! Named homes are not permanent aliases of their first result. Cleanup and
//! construction resolve the value at their operation prefix, sharing operand
//! storage selection so replacement neither loses nor revives an occurrence.
//! Anonymous input views and returned children can share a statement without
//! sharing custody. Loan dispositions join the result's captured holder, never
//! every claim established by that statement.
//! For a holder with captured loans, whole physical cleanup realizes its
//! checked release. Both inventories must close; they are one result death.
//! Projected transfers and residual descriptor releases can jointly exhaust an
//! array. Their exact typed frontier replaces store-only partition recognition;
//! covering a nominal owner's fields still cannot discharge its own cleanup.
use super::{
    CheckFacts, CheckedComposedUnitControlTerminatorPlan, CheckedStructuralAccess,
    CheckedStructuralControlSuccessorPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan, Multiplicity, PermissionEventKind,
    PermissionEventSource, StatementNode, control, local_results,
};
use checked_trees::{
    CheckedConditionalReturnArm, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralParameterPlan,
};

pub(super) fn account(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &super::ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    structural_results: &[(
        checked_trees::CheckedUnitStructuralResultBindingPlan,
        facts::PlaceRoot,
    )],
    operations: &mut [CheckedUnitEffectOperationPlan],
    terminator: &CheckedComposedUnitControlTerminatorPlan,
    trace: &control::LocalConstructionTrace,
) -> Option<()> {
    let statements = program.statement_table.statements(state.statement_nodes);
    trace.phase("state graph: result custody accounting");
    let abandons_custody = terminator.abandons_custody(&facts.flow.guarded_exits)?;
    // The first guard executes before selecting any exit. Later ordered
    // guards execute only on a remaining path; their transfers cannot stand
    // in for unconditional ownership. The retained computation itself owns
    // short-circuit alternatives inside this guard.
    let entry_guard = match terminator {
        CheckedComposedUnitControlTerminatorPlan::Conditional { guard, .. }
        | CheckedComposedUnitControlTerminatorPlan::ConditionalReturn { guard, .. }
        | CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion { guard, .. } => {
            Some(guard.clone())
        }
        CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, .. } => {
            arms.first().map(|arm| arm.guard().clone())
        }
        CheckedComposedUnitControlTerminatorPlan::Guarded { arms, .. } => {
            match facts.flow.guarded_exits.exits.span(*arms)?.first() {
                Some(arm) => Some(super::guards::retained_guard(
                    facts,
                    machine.symbol,
                    state.symbol,
                    arm.guard_statement_ordinal,
                )?),
                None => None,
            }
        }
        _ => None,
    };
    let construction_uses = construction_local_uses(facts, state.symbol, operations.iter())?;
    // Return producers have the same value-place operands as ordinary
    // constructors, but each roster belongs to one mutually exclusive arm.
    // Keeping those inventories separate avoids counting sibling transfers
    // as sequential consumption or overlooking a direct returned local.
    let return_arms = terminator.return_arms();
    let return_construction_uses = return_arms
        .iter()
        .map(|arm| construction_local_uses(facts, state.symbol, arm.operation_dependencies()))
        .collect::<Option<Vec<_>>>()?;
    let mut guarded_discards = Vec::new();
    for (producer_index, operation) in operations.iter().enumerate() {
        let Some((result, _)) = operation.discardable_structural_result() else {
            continue;
        };
        let result_local = operation
            .statement_structural_result_binding()
            .and_then(|binding| statements.get(binding.statement_index as usize))
            .and_then(|statement| match statement {
                StatementNode::LocalData(local) => Some(local.symbol),
                _ => None,
            })
            .unwrap_or_default();
        let consumers = &operations[producer_index + 1..];
        let occurrence_end = consumers
            .iter()
            .position(|consumer| consumer.ends_local_occurrence(result_local));
        let occupies_result = |source: &CheckedUnitStructuralArgumentSourcePlan, before: usize| {
            let source = match source {
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                    super::super::local_storage::source(
                        program,
                        state,
                        &operations[..before],
                        *symbol,
                    )
                }
                source => Some(source.clone()),
            };
            matches!(source,
                Some(CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal })
                    if binding_ordinal == result.binding_ordinal)
        };
        let construction_owns_result =
            |argument: &checked_trees::CheckedUnitStructuralArgumentPlan, before: usize| {
                occupies_result(&argument.source, before)
                    && argument.type_identity == result.type_identity
                    && (result.multiplicity == Multiplicity::Unrestricted
                        || argument.access == CheckedStructuralAccess::Owned)
                    && argument.path.is_empty()
            };
        // Cleanup owns the value occupying its source at that operation, not
        // every value ever produced for the same local. Resolve named storage
        // through the same prefix query as operands: replacement can retire an
        // initial result through its home, while a later cleanup cannot revive
        // that result after a rebind. A whole retirement also excludes lexical
        // exit disposal; moves and duplicate cleanups still count independently.
        let (cleanup_discards, retired_whole) = operations
            .iter()
            .enumerate()
            .skip(producer_index + 1)
            .filter_map(|(operation_index, operation)| match operation {
                CheckedUnitEffectOperationPlan::ContinuationCleanup {
                    boundary,
                    cleanup_actions,
                    ..
                } => Some((operation_index, boundary, cleanup_actions)),
                _ => None,
            })
            .fold(
                (0, false),
                |(count, retired_whole), (operation_index, boundary, discards)| {
                    // Replacing a child retires that old subtree, not the
                    // aggregate that still owns its siblings and new child.
                    // A detached extraction instead has its own whole result.
                    // Other continuations close the owner's residual partition.
                    if let checked_trees::CheckedUnitCleanupBoundary::Replacement {
                        old_value, ..
                    } = boundary
                        && (!old_value.path.is_empty()
                            || old_value.type_identity != result.type_identity
                            || !occupies_result(&old_value.source, operation_index))
                    {
                        return (count, retired_whole);
                    }
                    let mut matched = false;
                    let mut whole = false;
                    for discard in discards {
                        if occupies_result(discard.source(), operation_index) {
                            matched = true;
                            whole |= discard.path().is_empty()
                                && discard.type_identity() == result.type_identity;
                        }
                    }
                    // One continuation closes one owner's exact partition. Its
                    // maximal complement may contain several sibling subtrees;
                    // those are not competing dispositions of the whole root.
                    // Separate cleanup continuations still count independently.
                    (count + usize::from(matched), retired_whole || whole)
                },
            );
        // A partially transferring successor partition must not hide a move
        // after unconditional retirement merely because it has another exit.
        if retired_whole
            && (terminator
                .successors()
                .iter()
                .any(|edge| edge.result_moves(result) != 0)
                || terminator.return_arms().iter().any(|arm| {
                    arm.operation_dependencies().any(|operation| {
                        operation.with_value_calls().any(|operation| {
                            terminator_call_consumes_result(operation, result.binding_ordinal)
                        })
                    })
                }))
        {
            return None;
        }
        // Scheduling and accounting join the same captured holder. A whole
        // reference retirement realizes that holder's selected exit disposal;
        // it cannot also receive a second lexical disposal below. Transfers
        // and distinct physical retirement rows remain independent owners.
        let reference_releases = if result.multiplicity != Multiplicity::Unrestricted {
            control::statement_sequence::captured_reference_result(
                program,
                facts,
                machine.symbol,
                state.symbol,
                structural_results,
                operation,
                operations,
            )
            .map_or(0, |captured| {
                operations[producer_index + 1..]
                    .iter()
                    .filter(|operation| {
                        control::statement_sequence::release_matches(
                            operation,
                            captured.result,
                            captured.loans,
                        )
                    })
                    .count()
            })
        } else {
            0
        };
        let permits_disposal = |statement: Option<u32>, continuation: bool| {
            !retired_whole
                && reference_releases == 0
                && local_results::permits_disposal(
                    program,
                    facts,
                    state,
                    operation,
                    operations,
                    statement,
                    continuation,
                )
        };
        // An owned selection moved each candidate source's custody into
        // the join's residual parameters, which die at the same authored
        // exit. The receipt, not a transfer or disposal row on the dead
        // source place, accounts for that consumption independently of the
        // final control shape. Exact exit cleanup
        // must still account for the selection's surviving result and residuals.
        let selection_residual_source = result.multiplicity == Multiplicity::Affine
            && facts
                .flow
                .ownership
                .owned_selections
                .iter()
                .any(|(_, receipt)| {
                    receipt.machine == machine.symbol
                        && receipt.state == state.symbol
                        && receipt.death == PermissionEventSource::StateExit
                        && facts
                            .flow
                            .ownership
                            .selection_sources
                            .span_or_empty(receipt.sources)
                            .iter()
                            .any(|source| {
                                let boundary = operations
                                    .iter()
                                    .position(|operation| {
                                        operation.source_statement_index().is_some_and(
                                            |statement| statement >= receipt.statement_ordinal,
                                        )
                                    })
                                    .unwrap_or(operations.len());
                                super::super::local_storage::source(
                                    program,
                                    state,
                                    &operations[..boundary],
                                    source.symbol,
                                ) == Some(
                                    CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                        binding_ordinal: result.binding_ordinal,
                                    },
                                )
                            })
                });
        // A mutable view's result owner is its one checked loan release. A
        // selected edge may additionally transfer that view or return a
        // reference reborrowed through it, but those mutually exclusive uses
        // validate the edge partition rather than adding another owner beside
        // the release. Resolve the source once so every control family applies
        // the same exact-root and descendant-path test.
        let view_source = match operation {
            CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
            ) if facts.values.structural_values.nodes.is_valid(*value) => {
                let node = facts.values.structural_values.nodes.get(*value);
                match &node.kind {
                    checked_trees::CheckedStructuralValueKind::BorrowedSliceView {
                        source, ..
                    }
                    | checked_trees::CheckedStructuralValueKind::MutableSliceView {
                        source, ..
                    }
                    | checked_trees::CheckedStructuralValueKind::Reference { source, .. }
                        if super::super::types::borrowed_slice_result(
                            program,
                            node.type_reference,
                        ) =>
                    {
                        // Formation and transport share the captured view's
                        // lifetime owner, not a producer-specific disposition.
                        Some(source)
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        let parameter_symbol = |parameter_index: u32| {
            let parameter = structural_parameters.get(parameter_index as usize)?;
            let authored = program
                .state_parameters(state)
                .get(parameter.position as usize)?;
            false.then_some(machine.symbol).or(Some(authored.symbol))
        };
        let same_view_root = |argument: &checked_trees::CheckedUnitStructuralArgumentPlan| {
            let Some(view_source) = view_source else {
                return false;
            };
            match (&argument.source, &view_source.source) {
                (
                    CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index },
                    CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
                ) => parameter_symbol(*parameter_index) == Some(*symbol),
                (
                    CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
                    CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index },
                ) => Some(*symbol) == parameter_symbol(*parameter_index),
                _ => argument.source == view_source.source,
            }
        };
        let exact_view_reborrow = |source: &checked_trees::CheckedUnitStructuralArgumentPlan,
                                   before: usize| {
            let Some(view_source) = view_source else {
                return false;
            };
            let observed_source = match source.source {
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow {
                    symbol,
                    root_type_identity,
                } if root_type_identity == result.type_identity => {
                    CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
                }
                _ => source.source.clone(),
            };
            // A captured child names the view's current carrier, not the
            // declaration's backing expression. Resolve that home at the
            // observation so a later rebind cannot redirect an earlier child.
            let through_carrier =
                occupies_result(&observed_source, before) && !source.path.is_empty();
            let through_backing = same_view_root(source)
                && source.path.starts_with(&view_source.path)
                && (source.path.len() > view_source.path.len()
                    || source.type_identity == view_source.type_identity);
            (through_carrier || through_backing)
                && matches!(
                    (view_source.access, source.access),
                    (
                        CheckedStructuralAccess::MutableBorrow,
                        checked_trees::checked_borrowed_access_pair_pattern!(),
                    ) | (
                        CheckedStructuralAccess::SharedBorrow,
                        CheckedStructuralAccess::SharedBorrow,
                    )
                )
        };
        // Forming a child does not transfer or retire its parent carrier.
        // The release scheduler retains complete parent/child lineage and
        // waits for actual holder retirement. Reference-return arms below
        // still validate their exact returned source independently; counting
        // all child establishments here would charge observations (and
        // mutually exclusive returns) as competing parent dispositions.
        // The release (`view_releases` below) owns an affine view. An edge
        // carrying the view whole continues that same loan in its target:
        // the carry is checked as at most one move per edge, and the
        // ordinary transfer count below must not book it as a second owner.
        let release_owned_view =
            view_source.is_some() && result.multiplicity == Multiplicity::Affine;
        if release_owned_view
            && terminator
                .successors()
                .iter()
                .any(|edge| edge.result_moves(result) > 1)
        {
            return None;
        }
        // A successor moves the result either as a direct transfer or, for
        // a result with ownership debt, into one of its own operand calls.
        let transferred = |edge: &CheckedStructuralControlSuccessorPlan| {
            let transfers = edge.result_moves(result);
            // Unrestricted successor arguments are copies. Several target
            // parameters can observe the same completed local, while its
            // source home still has one disposition on this selected exit.
            // Affine and linear carriers must retain exactly one transfer.
            !release_owned_view
                && if result.multiplicity == Multiplicity::Unrestricted {
                    transfers != 0
                } else {
                    transfers == 1
                }
        };
        // A hole-restore stores a whole owned result into borrowed
        // storage; custody moved into the field, which owns it after
        // this state. Count it with the terminator exits: a disposable
        // local stored into a hole is not a second consumption. A copy
        // of an `Unrestricted` value has no transfer debt, including a
        // selected child: copying does not require consuming its siblings.
        // Match the producer occurrence, not a named home's historical origin:
        // rebinding already transferred that result, and later stores from the
        // receiving home must not count as a second result disposition.
        let stored_parts = operations
            .iter()
            .skip(producer_index + 1)
            .filter_map(|operation| {
                operation.installed_value().filter(|value| {
                    value.access == CheckedStructuralAccess::Owned
                        && value.source_structural_result_binding_ordinal()
                            == Some(result.binding_ordinal)
                })
            })
            .collect::<Vec<_>>();
        let stored = if result.multiplicity == Multiplicity::Unrestricted {
            !stored_parts.is_empty()
        } else {
            matches!(stored_parts.as_slice(), [whole]
                if whole.path.is_empty() && whole.type_identity == result.type_identity)
        };
        // A parent's custody can end through several projected receivers, not
        // just through stores. The retained typed transfer graph owns operand
        // identity and access; accounting asks whether its guaranteed frontier
        // exhausts this parent. Each receiver still owes its own disposition.
        // Whole moves retain their separate operation accounting below, and an
        // incomplete frontier still requires the ordinary residual cleanup.
        let projected_disposition = if result.multiplicity == Multiplicity::Unrestricted {
            false
        } else {
            let mut paths = super::super::storage_transfers::transfers(
                program,
                facts,
                structural_results,
                result.binding_ordinal,
                operations,
            )?
            .definite;
            // Definite paths are coverage, not transfer occurrences: mutually
            // exclusive alternatives may establish the same guarantee. Keep
            // the actual occurrences in the transfer graph for move checking.
            paths.sort();
            paths.dedup();
            // Transferred children and independently released residual loans
            // form one parent disposition. A release must name this exact
            // result and a projected descriptor; a child's whole release is
            // that child's obligation, not another receipt for this parent.
            paths.extend(consumers.iter().filter_map(|operation| {
                let CheckedUnitEffectOperationPlan::ReleaseReference {
                    source,
                    path,
                    defer_to_exit: false,
                    ..
                } = operation
                else {
                    return None;
                };
                (source
                    == &(CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: result.binding_ordinal,
                    })
                    && !path.is_empty())
                .then(|| path.clone())
            }));
            // Physical releases are occurrences, not alternative guarantees.
            // Preserve duplicates and overlaps with a moved path so the exact
            // partition rejects a second disposition of the same child.
            paths.sort();
            let mut remaining = paths.as_slice();
            !paths.is_empty()
                && paths.iter().all(|path| !path.is_empty())
                && producer_claim_paths(program, facts, operation).is_some_and(|claims| {
                    paths.iter().all(|path| {
                        claims
                            .iter()
                            .all(|claim| path.len() <= claim.len() || !path.starts_with(claim))
                    })
                })
                && disposed_partition(
                    shapes,
                    result.type_identity,
                    &mut Vec::new(),
                    &mut remaining,
                )
                && remaining.is_empty()
        };
        // A returning arm owns the result when its call or construction
        // consumes it, or when the return retires it. Abnormal completion
        // abandons custody; it must not invent a normal disposal obligation
        // or run cleanup.
        let return_arm_owns =
            |arm: &CheckedConditionalReturnArm,
             uses: &[(usize, &CheckedUnitStructuralArgumentPlan)]| {
                matches!(arm, CheckedConditionalReturnArm::Crash { .. })
                    || arm.definite_call_transfers(
                        &facts.values,
                        state.symbol,
                        operation,
                        result_local,
                    ) == 1
                    || uses
                        .iter()
                        .any(|(_, argument)| construction_owns_result(argument, operations.len()))
                    || permits_disposal(arm.statement_ordinal(), false)
            };
        // With no normal exit there is no transfer/disposal owner here to
        // count beside an earlier in-sequence consumption.
        let has_normal_exit = !terminator.successors().is_empty()
            || return_arms
                .iter()
                .any(|arm| !matches!(arm, CheckedConditionalReturnArm::Crash { .. }));
        let mut closed_sum_all_reference_returns = false;
        let consumed = match terminator {
            CheckedComposedUnitControlTerminatorPlan::Guarded { return_values, .. } => {
                result.multiplicity == Multiplicity::Unrestricted
                    || return_values.iter().any(|value| {
                        value.with_value_calls().skip(1).any(|operation| {
                            terminator_call_consumes_result(operation, result.binding_ordinal)
                        })
                    })
            }
            // A normal return retires every local it does not return, with
            // the same lexical state-exit disposal whatever the result kind:
            // an affine owner through the exit permit, an unrestricted value
            // through dominance. The returned binding itself is the return's.
            CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
                completion: completion @ checked_trees::CheckedScalarReturnPlan::Exits(_),
            } => {
                // Scalar completion keeps its selected computations outside
                // the flat operation roster. Each normal path owns either
                // its actual whole transfer or its exact cleanup receipt;
                // the minimum transfer count cannot describe mixed arms.
                let exits = completion.selected_exit_call_transfers(
                    &facts.values,
                    &facts.flow.guarded_exits,
                    state.symbol,
                    operation,
                    result_local,
                )?;
                !exits.is_empty()
                    && exits.iter().all(|(destination, transfers)| {
                        let checked_trees::CheckedScalarBranchDestination::Return {
                            statement_ordinal,
                            is_continuation,
                        } = destination
                        else {
                            return false;
                        };
                        // Repeated unrestricted observations create no
                        // additional custody owner. Affine and linear
                        // transfers remain exact-one; zero still requires
                        // the selected exit's real cleanup receipt.
                        (result.multiplicity == Multiplicity::Unrestricted && *transfers > 0)
                            || *transfers == 1
                            || (*transfers == 0
                                && permits_disposal(Some(*statement_ordinal), *is_continuation))
                    })
            }
            CheckedComposedUnitControlTerminatorPlan::ReturnUnit
            | CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. } => {
                permits_disposal(None, false)
            }
            CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result: returned } => {
                matches!(returned.source,
                    CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal }
                        if binding_ordinal == result.binding_ordinal)
                    || permits_disposal(None, false)
            }
            CheckedComposedUnitControlTerminatorPlan::Jump { .. }
            | CheckedComposedUnitControlTerminatorPlan::Conditional { .. }
            | CheckedComposedUnitControlTerminatorPlan::ConditionalReturn { .. }
            | CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion { .. }
            | CheckedComposedUnitControlTerminatorPlan::GuardedJumps { .. } => {
                // Each selected exit owns its own disposition. A returning
                // call consumes operands just as a successor's operand call
                // does, regardless of its result type. Mutually exclusive
                // moves are one partition, not multiple sequential uses;
                // conversely, a moving arm cannot cover an uncovered sibling.
                // The source checker owns move validity; normal-edge cleanup
                // retains the exact disposal schedule for each coordinate.
                has_normal_exit
                    && terminator.successors().iter().all(|edge| {
                        transferred(edge)
                            || permits_disposal(Some(edge.statement_ordinal), edge.is_continuation)
                    })
                    && return_arms
                        .iter()
                        .zip(&return_construction_uses)
                        .all(|(arm, uses)| return_arm_owns(arm, uses))
            }
            CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } => {
                // Each selected case owns a disposition, just as an
                // ordinary conditional does. A shared view can end on a
                // non-transferring edge through its local dominance
                // permit; it need not survive an unrelated sum dispatch.
                // Reference returns additionally admit an exact reborrow,
                // whose custody is counted below rather than booked twice
                // as a synthetic unrestricted disposition.
                let mut return_uses = return_construction_uses.iter();
                let complete_partition = !cases.is_empty()
                        && cases.iter().all(|case| match &case.outcome {
                            checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(
                                successor,
                            ) => transferred(successor) || permits_disposal(Some(successor.statement_ordinal), successor.is_continuation),
                            checked_trees::CheckedClosedSumCaseOutcomePlan::Return(arm) => {
                                let exact_reborrow = matches!(arm,
                                    CheckedConditionalReturnArm::Structural(returned)
                                        if returned
                                            .established_reference_source(&facts.values.structural_values)
                                            .is_some_and(|source| exact_view_reborrow(source, operations.len())));
                                return_uses.next().is_some_and(|uses| {
                                    exact_reborrow || return_arm_owns(arm, uses)
                                })
                            }
                        });
                if view_source.is_some() {
                    // Rebinding ends this named occurrence before the
                    // selected exit. Its checked loan release still owes
                    // exactly one disposition below; returning the newer
                    // occurrence cannot also be charged to this producer.
                    // A stored RHS likewise transferred its custody into
                    // the receiving home before the selected exit.
                    if !complete_partition && occurrence_end.is_none() && !stored {
                        // An expression-rooted temporary has no named
                        // local occurrence to rebind and no local
                        // disposal permit a selected exit could honor:
                        // the call it was formed for already consumed
                        // its custody in-sequence. The nested schedule
                        // may emit its establishment after that call,
                        // so scan every operation for the consuming
                        // argument rather than only consumers.
                        let consumed_by_forming_call = !result_local.is_valid()
                            && operations.iter().any(|consumer| {
                                consumer.call_structural_arguments().iter().any(|argument| {
                                    argument.source_structural_result_binding_ordinal()
                                        == Some(result.binding_ordinal)
                                })
                            });
                        if !consumed_by_forming_call {
                            return None;
                        }
                    }
                    closed_sum_all_reference_returns = cases.iter().all(|case| {
                        let checked_trees::CheckedClosedSumCaseOutcomePlan::Return(
                            checked_trees::CheckedConditionalReturnArm::Structural(returned),
                        ) = &case.outcome
                        else {
                            return false;
                        };
                        returned
                            .established_reference_source(&facts.values.structural_values)
                            .is_some_and(|source| exact_view_reborrow(source, operations.len()))
                    });
                    has_normal_exit
                        && occurrence_end.is_none()
                        && !stored
                        && result.multiplicity == Multiplicity::Unrestricted
                } else {
                    has_normal_exit && complete_partition
                }
            }
            // A crash abandons custody; it owns no normal disposal.
            CheckedComposedUnitControlTerminatorPlan::Crash { .. } => false,
        } || selection_residual_source;
        // A whole owned argument ends an affine local's custody at the call,
        // just as it ends a linear result or an inline operand's custody.
        // Sequencing retires the producer's discard flag; count the exact
        // later use here instead of still demanding state-exit disposal.
        // Source permission replay rejects repeated moves inside selected
        // computations; this minimum-transfer query does not replace it.
        // The final sum rejects competing operation/cleanup/return owners.
        // Unrestricted locals remain copies, not one-use ownership debt.
        let call_transfers = if result.multiplicity == Multiplicity::Unrestricted {
            0
        } else {
            consumers[..occurrence_end.map_or(consumers.len(), |position| position + 1)]
                .iter()
                .map(|consumer| {
                    consumer.definite_call_transfers(&facts.values, operation, result_local)
                })
                .sum::<usize>()
                + if occurrence_end.is_none() {
                    let guard_transfers = entry_guard.as_ref().map_or(0, |guard| {
                        guard.definite_call_transfers(&facts.values, operation, result_local)
                    });
                    // Scalar exit computations are accounted per selected
                    // path above, together with that path's cleanup receipt.
                    // Counting their minimum here would book a second owner
                    // when every arm transfers the same result.
                    guard_transfers
                } else {
                    0
                }
        };
        // A constructor takes its whole owned local operands just as a call
        // does. Statement sequencing already retires their disposal flags;
        // rejoin the exact value children here instead of requiring that the
        // operand itself also be the returned outer binding. Selected values
        // retain their separate per-arm ownership receipts.
        let construction_count = construction_uses
            .iter()
            .filter(|(consumer_index, argument)| {
                *consumer_index > producer_index
                    && construction_owns_result(argument, *consumer_index)
            })
            .count();
        // Copyable local leaves may be retained as shared case/record reads,
        // or as owned operands. Neither form moves the original owner; repeated
        // observations do not create or discharge an affine obligation.
        let copied_into_construction =
            result.multiplicity == Multiplicity::Unrestricted && construction_count != 0;
        let construction_transfers = if result.multiplicity == Multiplicity::Unrestricted {
            0
        } else {
            construction_count
        };
        // A view result's custody is also its loan's release: the loan
        // minted at the establishment ends in exactly one Consume row —
        // on the selected edge that carries it when control moves, or at
        // the state exit when the scope simply closes. Count that release
        // as the owner; an owned-storage disposal never applies to a
        // reference shell. The Consume's root names the lent place, not
        // the view local, so the establishment's own claim identifies the
        // release. Only a loan-bound result needs the owner: an
        // Unrestricted view is admitted by its dominance permit, and
        // counting its release beside that permit would double-book.
        // An exact rebind ends the anonymous producer's residence. The
        // receiving home keeps the same loan until its later weakening;
        // that weakening cannot also dispose the transferred producer.
        let installed_view_claims = release_owned_view
            .then(|| {
                installed_view_claims(
                    facts,
                    machine.symbol,
                    state.symbol,
                    operation,
                    result,
                    &operations[producer_index + 1..],
                )
            })
            .flatten()
            .unwrap_or_default();
        let (has_captured_view_loans, view_releases) = if release_owned_view {
            view_loan_releases(
                facts,
                machine.symbol,
                state.symbol,
                result,
                operation,
                &installed_view_claims,
            )
            .map_err(|reason| {
                trace.phase(reason);
                trace.statement(Some(result.statement_index));
            })
            .ok()?
        } else {
            (false, 0)
        };
        // Whole cleanup and a scheduled reference release are physical
        // realizations of the captured holder's semantic loan disposition.
        // Both joins above resolve this exact storage occurrence and its loan
        // roster; another operand or returned child's Consume cannot discharge
        // it. Keep physical receipts separately counted, so two retirements
        // still fail even when they name the same semantic release.
        let retirement_realizes_view_release =
            (retired_whole || reference_releases != 0) && has_captured_view_loans;
        if retirement_realizes_view_release && view_releases != 1 {
            trace.phase("state graph: view cleanup has no unique captured loan disposition");
            trace.statement(Some(result.statement_index));
            return None;
        }
        let independent_view_releases = if retirement_realizes_view_release {
            0
        } else {
            view_releases
        };
        // The checked loan release is the semantic owner of an affine view,
        // while Terminal's structural frontier owns the materialized carrier.
        // On an all-return guarded state the same one release therefore
        // restores the carrier's physical discard row; it is not a second
        // source-custody disposition in `total` below.
        let returning_view_disposal = (matches!(
            terminator,
            CheckedComposedUnitControlTerminatorPlan::Guarded { .. }
        ) || closed_sum_all_reference_returns)
            && view_releases == 1;
        // A byte replacement copies the completed live prefix and length;
        // it neither moves an affine owner nor invents a disposal receipt.
        // The store's source replay separately rejoins the exact call.
        let copied_into_byte_store = result.multiplicity == Multiplicity::Unrestricted
            && operations[producer_index + 1..].iter().any(|operation| {
                operation.stored_bytes().is_some_and(|value| {
                    value.completed_call_binding_ordinal() == Some(result.binding_ordinal)
                })
            });
        // A whole unrestricted result can be copied or lent by a later call,
        // independently of whether a call or construction produced it. Every
        // admitted access leaves the unrestricted carrier without affine
        // transfer debt; exclusive access still owes the consumer's exact
        // loan checks. Access is not a second multiplicity classification.
        // The admitted operand names the occurrence it observes. Its formal
        // presentation can differ from the stored descriptor's identity, so
        // do not repeat representation correspondence as a custody equality.
        let observed_by_call = result.multiplicity == Multiplicity::Unrestricted
            && operations[producer_index + 1..]
                .iter()
                .flat_map(CheckedUnitEffectOperationPlan::call_structural_arguments)
                .any(|argument| {
                    argument.source_structural_result_binding_ordinal()
                        == Some(result.binding_ordinal)
                        && argument.path.is_empty()
                });
        // A whole local rebind retires the previous unrestricted occurrence
        // at the store, before any exit observes the replacement.
        let replaced_unrestricted =
            result.multiplicity == Multiplicity::Unrestricted && occurrence_end.is_some();
        let implicit_disposition = consumed
            || stored
            || projected_disposition
            || copied_into_construction
            || copied_into_byte_store
            || observed_by_call
            || replaced_unrestricted;
        // Unrestricted copies and observations leave the supplying occurrence
        // in place. Its explicit whole cleanup, when present, realizes the same
        // disposal otherwise accounted by dominance or rebinding. This does
        // not merge affine transfers or separate physical cleanup receipts.
        let implicit_disposition = implicit_disposition
            && !(result.multiplicity == Multiplicity::Unrestricted && retired_whole);
        let total = usize::from(implicit_disposition)
            + call_transfers
            + cleanup_discards
            + construction_transfers
            + reference_releases
            + independent_view_releases;
        if total != 1 && !(total == 0 && abandons_custody) {
            trace.phase(if total == 0 {
                "state graph: structural result has no selected disposition"
            } else {
                "state graph: structural result has duplicate dispositions"
            });
            trace.statement(Some(result.statement_index));
            return None;
        }
        if returning_view_disposal {
            guarded_discards.push(producer_index);
        }
    }
    for producer_index in guarded_discards {
        let (_, discard_result_on_return) = operations
            .get_mut(producer_index)?
            .discardable_structural_result_mut()?;
        *discard_result_on_return = true;
    }
    Some(())
}

/// Use the actual producer's claim frontier; storage shape alone cannot tell
/// whether a containing claim forbids moving an otherwise ordinary child.
fn producer_claim_paths(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    producer: &CheckedUnitEffectOperationPlan,
) -> Option<Vec<Vec<checked_trees::CheckedUnitStructuralPathSegment>>> {
    if let Some(call) = producer.call() {
        return Some(
            call.structural_custody()?
                .result_claims
                .iter()
                .map(|claim| claim.path.clone())
                .collect(),
        );
    }
    let CheckedUnitEffectOperationPlan::EstablishLocal(
        checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
    ) = producer
    else {
        return None;
    };
    if !facts.values.structural_values.nodes.is_valid(*value) {
        return None;
    }
    let actual_type = facts
        .values
        .structural_values
        .nodes
        .get(*value)
        .type_reference;
    validation::linear_claim_frontier(program, actual_type)
        .into_iter()
        .map(|claim| validation::structural_claim_path(program, actual_type, &claim.path).ok())
        .collect()
}

/// A completed disposition partition accounts for exactly one whole source value.
/// Arrays own their elements, so all of an array's children can transfer its
/// custody. Other values remain indivisible here: covering a record's fields
/// does not discharge its own nominal cleanup. The caller separately preserves
/// containing claims. These paths come from typed owned operands, not from
/// receiver shape or source syntax. Consume their sorted guaranteed coverage,
/// rejecting missing siblings and overlapping ancestor/descendant coverage.
fn disposed_partition(
    shapes: &super::ShapeCollector<'_>,
    identity: typed_trees::type_identity::TypeIdentityId,
    prefix: &mut Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    remaining: &mut &[Vec<checked_trees::CheckedUnitStructuralPathSegment>],
) -> bool {
    let Some(first) = remaining.first() else {
        return false;
    };
    if first == prefix {
        *remaining = &remaining[1..];
        return true;
    }
    let Some(checked_trees::CheckedUnitStructuralTypeShape::FixedArray {
        element_type_identity,
        length,
    }) = shapes.types.get(&identity).map(|plan| &plan.shape)
    else {
        return false;
    };
    // A declared extent never authorizes work proportional to an unchecked
    // integer. Each nonempty child needs at least one actual transferred path.
    let Some(length) = length.literal() else {
        return false;
    };
    if length == 0 || u64::try_from(remaining.len()).map_or(true, |count| count < length) {
        return false;
    }
    for index in 0..length {
        prefix.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
            index,
        ));
        let complete = disposed_partition(shapes, *element_type_identity, prefix, remaining);
        prefix.pop();
        if !complete {
            return false;
        }
    }
    true
}

/// Statement coordinates can contain several independent formations: an
/// anonymous input descriptor, a returned child, or another operand's view.
/// Only the loans captured by this exact holder can dispose its result.
/// A temporary descriptor with no source loan still needs its own physical
/// cleanup; another holder's Consume cannot stand in for that disposition.
/// Captured loan handles rejoin their original claims: equal backing paths can
/// belong to different holders even within one statement.
fn view_loan_releases(
    facts: &CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
    producer: &CheckedUnitEffectOperationPlan,
    installed_claims: &[language_semantics::PermissionClaimIdentity],
) -> Result<(bool, usize), &'static str> {
    // The producer names its holder even for an anonymous return operand,
    // which need not appear in the state's named-result inventory.
    let owner = if result.local.symbol.is_valid() {
        facts::PlaceRoot::Symbol(result.local.symbol)
    } else {
        let CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
        ) = producer
        else {
            return Err("state graph: view result has no captured formation");
        };
        let formation = facts.values.structural_values.nodes.get(*value).expression;
        if !formation.is_valid() {
            return Err("state graph: view result has an invalid captured formation");
        }
        facts::PlaceRoot::Expression(formation)
    };
    let mut states = facts
        .borrow
        .states
        .iter()
        .map(|(_, borrowing)| borrowing)
        .filter(|borrowing| borrowing.machine_symbol == machine && borrowing.state_symbol == state);
    let Some(borrowing) = states.next() else {
        return Ok((false, 0));
    };
    if states.next().is_some() {
        return Err("state graph: view result has ambiguous borrow states");
    }
    let loans = facts
        .borrow
        .storage_loans(borrowing, owner, result.statement_index as usize)
        .ok_or("state graph: view holder has no complete captured loan group")?;
    let captured_loans = facts
        .borrow
        .loans
        .span(loans)
        .ok_or("state graph: view holder has an invalid captured loan group")?;
    let releases = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.kind == PermissionEventKind::Consume
                && !installed_claims.contains(&event.claim_identity)
                && event.machine_symbol == machine
                && event.state_symbol == state
                && matches!(event.claim_identity,
                        language_semantics::PermissionClaimIdentity::Established {
                            machine_symbol, state_symbol,
                            source: PermissionEventSource::Statement { statement_index }, ..
                        } if machine_symbol == machine && state_symbol == state
                            && statement_index == result.statement_index as usize)
                && facts.flow.ownership.borrow_claims.iter().any(|(_, claim)| {
                    claim.claim_identity == event.claim_identity
                        && facts.borrow.loans_contain(loans, claim.loan)
                        && facts
                            .borrow
                            .loan_owner_path(facts.borrow.loans.get(claim.loan))
                            .is_empty()
                })
        })
        .count();
    Ok((!captured_loans.is_empty(), releases))
}

/// A statement can form several unrelated descriptors. A permission release
/// belongs to this result only when its exact formation expression rejoins the
/// checked loan and that loan's establishment claim. Statement coincidence
/// alone used to charge an anonymous argument for the returned child's release.
fn view_formation_claims(
    facts: &CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    producer: &CheckedUnitEffectOperationPlan,
    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
) -> Option<Vec<language_semantics::PermissionClaimIdentity>> {
    let CheckedUnitEffectOperationPlan::EstablishLocal(
        checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
    ) = producer
    else {
        return None;
    };
    let formation = facts.values.structural_values.nodes.get(*value).expression;
    if !formation.is_valid() {
        return None;
    }
    let mut states = facts
        .borrow
        .states
        .iter()
        .map(|(_, row)| row)
        .filter(|row| row.machine_symbol == machine && row.state_symbol == state);
    let borrowing = states.next()?;
    if states.next().is_some() {
        return None;
    }
    let loans = facts
        .borrow
        .loan_group(borrowing.loans)?
        .filter(|(_, loan)| {
            loan.statement_index == result.statement_index as usize
                && loan.formation_expression == formation
                && loan.kind == checked_trees::BorrowAccessKind::Mutable
        })
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    Some(
        facts
            .flow
            .ownership
            .borrow_claims
            .iter()
            .filter(|(_, claim)| loans.contains(&claim.loan))
            .map(|(_, claim)| claim.claim_identity)
            .collect(),
    )
}

/// Installation transfers the completed descriptor, regardless of whether its
/// receiving place is a binding, field or indexed slot. The source checker has
/// already joined formation to exact loan claims; rejoining by backing or by
/// an empty owner path confuses the producer's death with the resident loan's
/// later release. Require one whole-value installation and one later release
/// per exact claim. Destination access and replacement cleanup remain the
/// store's obligations, not permission granted by this accounting join.
fn installed_view_claims(
    facts: &CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    producer: &CheckedUnitEffectOperationPlan,
    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
    following: &[CheckedUnitEffectOperationPlan],
) -> Option<Vec<language_semantics::PermissionClaimIdentity>> {
    let mut installations = following.iter().filter_map(|operation| {
        let CheckedUnitEffectOperationPlan::Store(store) = operation else {
            return None;
        };
        let (checked_trees::CheckedStoreLeaf::OwnedRebind { type_identity }
        | checked_trees::CheckedStoreLeaf::StructuralStore { type_identity, .. }) =
            &store.destination.leaf
        else {
            return None;
        };
        let supplied = store.installed_value()?;
        (*type_identity == result.type_identity
            && supplied.type_identity == result.type_identity
            && supplied.source_structural_result_binding_ordinal() == Some(result.binding_ordinal)
            && supplied.access == CheckedStructuralAccess::Owned
            && supplied.path.is_empty())
        .then_some(store.statement_index)
    });
    if installations.next()? != result.statement_index || installations.next().is_some() {
        return None;
    }
    let claims = view_formation_claims(facts, machine, state, producer, result)?;
    if claims.is_empty() {
        return None;
    }
    let source = PermissionEventSource::Statement {
        statement_index: result.statement_index as usize,
    };
    for claim in &claims {
        let matching = |event: &checked_trees::FlowPermissionEventFact| {
            event.claim_identity == *claim
                && event.machine_symbol == machine
                && event.state_symbol == state
                && event.access == language_semantics::PermissionAccess::Exclusive
                && event.multiplicity == Multiplicity::Affine
        };
        let mut establishments =
            facts
                .flow
                .ownership
                .permissions
                .iter()
                .filter_map(|(_, event)| {
                    (matching(event) && event.kind == PermissionEventKind::Establish)
                        .then_some(event)
                });
        let establishment = establishments.next()?;
        if establishments.next().is_some()
            || establishment.source != source
            || establishment.evaluation != checked_trees::FlowPermissionEvaluation::Statement
        {
            return None;
        }
        let mut releases = facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter_map(|(_, event)| {
                (matching(event) && event.kind == PermissionEventKind::Consume).then_some(event)
            });
        let release = releases.next()?;
        let after_installation = match release.source {
            PermissionEventSource::StateExit => true,
            PermissionEventSource::Statement { statement_index }
            | PermissionEventSource::Call {
                statement_index, ..
            } => statement_index >= result.statement_index as usize,
            PermissionEventSource::StateEntry => false,
        };
        if releases.next().is_some()
            || !after_installation
            || release.evaluation != checked_trees::FlowPermissionEvaluation::Statement
        {
            return None;
        }
    }
    Some(claims)
}

/// Borrow whole local operands once per supplied construction sequence. The
/// sequence mutation walker also enters dispatch/projection nodes to update
/// disposal flags; accounting cannot flatten those per-arm/residual receipts.
/// Keep this inventory restricted to ordinary constructor composition.
fn construction_local_uses<'a, 'operation>(
    facts: &'a CheckFacts,
    state: symbols::SymbolHandle,
    operations: impl IntoIterator<Item = &'operation CheckedUnitEffectOperationPlan>,
) -> Option<Vec<(usize, &'a checked_trees::CheckedUnitStructuralArgumentPlan)>> {
    use checked_trees::{CheckedStructuralRecordFieldValue, CheckedStructuralValueKind};

    let values = &facts.values.structural_values;
    let mut uses = Vec::new();
    let mut pending = Vec::new();
    let mut visited = Vec::new();
    for (index, operation) in operations.into_iter().enumerate() {
        let CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { result, value, .. },
        ) = operation
        else {
            continue;
        };
        if facts
            .flow
            .ownership
            .owned_selection_at(state, result.statement_index)
            .is_some()
        {
            continue;
        }
        pending.push(*value);
        visited.clear();
        while let Some(value) = pending.pop() {
            if !values.nodes.is_valid(value) || visited.contains(&value) {
                return None;
            }
            visited.push(value);
            match &values.nodes.get(value).kind {
                CheckedStructuralValueKind::Place(argument)
                | CheckedStructuralValueKind::CopiedStructuralPlace {
                    source: argument, ..
                } => {
                    uses.push((index, argument));
                }
                checked_trees::record_or_structural_case_pattern!(fields, ..) => {
                    for field in values.record_fields.span(*fields)? {
                        if let CheckedStructuralRecordFieldValue::Structural(value) = field.value {
                            pending.push(value);
                        }
                    }
                }
                CheckedStructuralValueKind::FixedArray { elements } => {
                    pending.extend(elements.iter().copied());
                }
                _ => {}
            }
        }
    }
    Some(uses)
}

/// A call retained inside a value-returning terminator (`Guarded` arm
/// payloads, `ConditionalReturn`'s value arm) consumes its producers through
/// owned whole arguments the same way an in-sequence call does: the argument
/// names the exact result binding it moves into the call's custody.
pub(in crate::execution::terminal_unit) fn terminator_call_consumes_result(
    operation: &CheckedUnitEffectOperationPlan,
    binding_ordinal: u32,
) -> bool {
    operation
        .call_structural_arguments()
        .iter()
        .any(|argument| {
            argument.source_structural_result_binding_ordinal() == Some(binding_ordinal)
                && argument.access == CheckedStructuralAccess::Owned
                && argument.path.is_empty()
        })
}

/// Whole owned call arguments consume affine and linear results. Accounting
/// requires one disposition, and lowering rejoins a moved local's exact
/// establishment and call-transfer receipts instead of state-exit cleanup.
/// An inline argument construction also belongs to its consuming call when
/// copyable; other unrestricted locals retain their reusable statement owner.
pub(in crate::execution::terminal_unit) fn call_owns_result(
    producer: &CheckedUnitEffectOperationPlan,
) -> bool {
    // Custody belongs to the produced value, not its source idiom. An
    // extracted old referent owns the same affine obligation as a call or
    // constructor result. Unrestricted inline operands alone retain their
    // consuming-call establishment rather than a reusable local binding.
    producer
        .structural_result_binding()
        .is_some_and(|result| result.multiplicity != Multiplicity::Unrestricted)
        || matches!(
            producer,
            CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                    operand_source: Some(
                        checked_trees::CheckedArrayConstructionSource::CallArgument { .. }
                    ),
                    ..
                }
            )
        )
}

/// An available structural binding, independently of whether its producer
/// carries disposal debt. An actual automatic return discard retires it.
pub(in crate::execution::terminal_unit) fn retained_structural_result(
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<&checked_trees::CheckedUnitStructuralResultBindingPlan> {
    if operation
        .discardable_structural_result()
        .is_some_and(|(_, discard)| discard)
    {
        return None;
    }
    operation.structural_result_binding()
}

/// An operand-owned construction retained beside the call that consumes it:
/// `operand_source` names the consuming call's coordinate and formal
/// position, so its statement is the call's — any local declared on that
/// statement is a sibling, never this construction's binding. Such rows must
/// not stand in for a local's unique producer.
pub(in crate::execution::terminal_unit) fn operand_owned_construction(
    operation: &CheckedUnitEffectOperationPlan,
) -> bool {
    matches!(
        operation,
        CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                operand_source: Some(_),
                ..
            }
        )
    )
}
