//! A call whose value lands in a structural result binding. A returned
//! reference or borrowed view carries a loan whose operands must be the
//! caller's structural parameters or earlier results; any other result must
//! be a claim-free owned value. Either way the call becomes one ordinary
//! call with a structural result that carries the binding.

use checked_trees::{
    CheckedUnitCallParts, CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute,
    CheckedUnitCallTarget, CheckedUnitCallTransfers,
};

use crate::execution::terminal_unit::cleanup::service_reach_is_empty;
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::{
    ShapeCollector, base_type_identity, machine_binders, state_flow,
};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, ExpressionNode, Multiplicity, StatementNode,
    SymbolHandle, TransitionTargetNode, TypeReferenceNode, TypedTrees, is_reference,
};

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    caller_structural_results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    trace: &LocalConstructionTrace,
    planned: super::call_operations::PlannedCall<'_>,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Option<CheckedUnitEffectOperationPlan> {
    let super::call_operations::PlannedCall {
        coordinate,
        source_site,
        call,
        target_machine,
        target_state,
        target_contract,
        scalar_parameters,
        scalar_arguments,
        erased_scalar_arguments,
        erased_proof_arguments,
        structural_arguments,
        transfers,
    } = planned;
    let call_target = CheckedUnitCallTarget {
        machine: target_machine.symbol,
        state: target_state.symbol,
        contract_report_fingerprint: target_contract.report_fingerprint,
        service_reach: call.service_reach,
    };
    let phase = |name: &'static str| {
        trace.phase(name);
        trace.statement(Some(coordinate.statement_index));
    };
    phase("call operation: structural result loan");
    let reference_loan = if crate::execution::terminal_unit::reference_results::parts(
        program,
        target_state.return_type,
    )
    .is_some()
    {
        crate::execution::terminal_unit::reference_results::result_loan(
            program,
            facts,
            machine.symbol,
            state,
            call,
            result,
        )?
    } else if crate::execution::terminal_unit::reference_results::mutable_borrowed_parts(
        program,
        target_state.return_type,
    )
    .is_some()
    {
        view_result_loan(
            program,
            facts,
            machine,
            state,
            call,
            target_machine,
            target_state,
            result,
        )?
    } else if crate::execution::terminal_unit::reference_results::shared_borrowed_parts(
        program,
        target_state.return_type,
    )
    .is_some_and(|(referee, _)| program.primitive_type_reference(referee).is_none())
    {
        // A shared `&T` record referent is Affine like the `&mut` family:
        // the result's caller-side loan is read out of the same custody walk,
        // with the denoted source only needing to carry a readable root.
        // Primitive referents stay on the ordinary result path — their loan
        // ledger shape is the `parts` family's exclusive side only.
        crate::execution::terminal_unit::reference_results::result_loan(
            program,
            facts,
            machine.symbol,
            state,
            call,
            result,
        )?
    } else {
        arena::Handle::invalid()
    };
    // A result whose contents hold no reference, slice or cleanup owner
    // cannot retain a loan of its operands: whatever the callee read or wrote
    // through a borrowed argument stays in the lender's storage.
    let represented_copy_result = result.multiplicity == Multiplicity::Unrestricted
        && validation::has_plain_owned_contents_with_represented_constraints(
            program,
            target_state.return_type,
        );
    let result_retains_no_loan = !reference_loan.is_valid()
        && (represented_copy_result
            || (matches!(
                result.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            ) && validation::has_plain_owned_contents_with_numeric_constraints(
                program,
                target_state.return_type,
            ))
            || (result.multiplicity == Multiplicity::Unrestricted
                && validation::is_closed_primitive_array_type(program, target_state.return_type)));
    // A result signature is available before its ordinary or graph body plan.
    // The closure pass below retains this call only when that complete body
    // was produced, avoiding an authored machine-order dependency.
    phase("call operation: structural result operands");
    let args_ok = structural_arguments
        .iter()
        .enumerate()
        .all(|(argument_index, argument)| {
            if argument.access == CheckedStructuralAccess::Owned
                && (argument.source_parameter_index().is_some()
                    || argument
                        .source_structural_result_binding_ordinal()
                        .is_some())
                && program
                    .state_parameters(target_state)
                    .iter()
                    .filter(|parameter| {
                        program
                            .primitive_type_reference(parameter.type_reference)
                            .is_none()
                            && !(parameter.is_self
                                && is_reference(program, parameter.type_reference))
                    })
                    .nth(argument_index)
                    .is_some_and(|parameter| {
                        // Typed Self names the machine, not its data carrier.
                        // Use the same checked signature that declares the
                        // callee's receiver; this also covers non-generic
                        // attachments without a retained owner application.
                        if parameter.is_self {
                            let Some((_, parameters, _)) = super::structural_scalar_signature(
                                program,
                                &mut ShapeCollector::new(program),
                                target_machine,
                                target_state,
                                &[],
                                false,
                            ) else {
                                return false;
                            };
                            return parameters.iter().find(|formal| formal.is_self).is_some_and(
                                |formal| {
                                    formal.access == CheckedStructuralAccess::Owned
                                        && matches!(
                                            formal.multiplicity,
                                            Multiplicity::Affine | Multiplicity::Unrestricted
                                        )
                                        && formal.type_identity == argument.type_identity
                                        && formal.qualifications.is_empty()
                                        && formal.projected_qualifications.is_empty()
                                        && formal.fused_service_erasure.is_none()
                                        && argument.path.is_empty()
                                },
                            );
                        }
                        let carrier = parameter.type_reference;
                        (validation::is_closed_primitive_array_type(
                                program,
                                carrier,
                            ) || validation::has_plain_owned_contents_with_numeric_constraints(
                                program,
                                carrier,
                            ) || validation::reference_result_custody::is_reference_record(
                                program,
                                carrier,
                            )
                            // An owned actual moved into a by-value linear
                            // formal transfers its claims into the callee;
                            // the call's claim transfers carry them, so the
                            // carrier needs no claim-free contents proof.
                            || crate::checks::type_multiplicity(program, carrier)
                                == Multiplicity::Linear)
                            && base_type_identity(program, carrier, &[])
                                .is_some_and(|identity| identity == argument.type_identity)
                            // A projected owned operand names the exact
                            // declared-field subtree whose captured leaf the
                            // bare reference result loan already replayed.
                            // Without that proven leaf custody the whole
                            // carrier spelling stays mandatory.
                            && (argument.path.is_empty()
                                || (reference_loan.is_valid()
                                    && validation::reference_result_custody::is_reference_record(
                                        program,
                                        carrier,
                                    )
                                    && argument.path.iter().all(|segment| {
                                        matches!(
                                            segment,
                                            checked_trees::CheckedUnitStructuralPathSegment::Field(
                                                _
                                            )
                                        )
                                    })))
                    })
            {
                return true;
            }
            (argument.source_parameter_index().is_some()
                // The argument planner already rejoined this loan
                // to its exact earlier result and captured call access.
                // Borrowing that live storage does not transfer its owner.
                || (matches!(
                    argument.access,
                    CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
                )
                    && argument
                        .source_structural_result_binding_ordinal()
                        .is_some())
                // An exclusive loan of an earlier result or local record,
                // whole or by fixed field and index path, was rejoined the
                // same way to its captured `&mut` access. Only a result that
                // retains no loan admits it, so the loan ends with the call
                // and the lender keeps its owner.
                || (argument.access == CheckedStructuralAccess::MutableBorrow
                    && result_retains_no_loan
                    && argument
                        .source_structural_result_binding_ordinal()
                        .is_some()
                    && argument.path.iter().all(|segment| {
                        matches!(
                            segment,
                            checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                                | checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(_)
                        )
                    }))
                || argument.byte_sequence_literal().is_some()
                // A range descriptor over an established caller view was
                // admitted by the same storage judgment every other call
                // form uses; its root names adjudicated caller storage.
                || super::view_subslice::is_view_subslice(argument)
                || matches!(
                    argument.source,
                    CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
                ))
                && matches!(
                    argument.access,
                    CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
                )
        });
    // An ordinary call that consumes linear claim-carrying inputs keeps the
    // same route and result binding: the consumed set is re-derived as the
    // custody's input transfer roster, and a claim-free result carries no
    // returned frontier. The reconstruction fences the result side itself,
    // so only the empty-transfer rosters keep the inline emit below.
    if args_ok
        && !transfers.is_empty()
        && program
            .machine_states(target_machine)
            .first()
            .is_some_and(|entry| entry.symbol == target_state.symbol)
        && machine_binders(program, target_machine).is_empty()
    {
        let mut operation =
            CheckedUnitEffectOperationPlan::Call(CheckedUnitCallPlan::new(CheckedUnitCallParts {
                coordinate,
                source_site,
                target: call_target,
                route: CheckedUnitCallRoute::Module {
                    contract_commitment: Some(target_contract.commitment),
                    erased_scalar_arguments,
                    erased_proof_arguments,
                },
                scalar_arguments,
                structural_arguments,
                result: CheckedUnitCallResult::Structural {
                    binding: result.clone(),
                    discard_result_on_return: result.multiplicity == Multiplicity::Affine
                        && !reference_loan.is_valid(),
                },
                transfers: CheckedUnitCallTransfers::StructuralCustody(Default::default()),
            })?);
        let custody = validation::reconstruct_structural_call_custody(
            program,
            facts,
            machine.symbol,
            state.symbol,
            &operation,
        )
        .ok()?;
        *operation.call_mut()?.structural_custody_mut()? = custody;
        return Some(operation);
    }
    if args_ok
        && transfers.is_empty()
        && (represented_copy_result || reference_loan.is_valid()
            || crate::execution::terminal_unit::reference_results::is_reference_record(
                program,
                target_state.return_type,
            )
            // Closed contents and the retained body determine the carrier,
            // including lifetime-only applications. Named syntax is not an
            // extra ownership proof; the classifier still rejects open types,
            // carried loans, nominal cleanup and unknown qualifications.
            || (matches!(
                result.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            ) && validation::has_plain_owned_contents_with_numeric_constraints(
                program,
                target_state.return_type,
            ))
            || (result.multiplicity == Multiplicity::Unrestricted
                && validation::is_closed_primitive_array_type(
                    program,
                    target_state.return_type,
                ))
            // A sum or record carrying shared-view members — the same
            // descriptor loans the bare-view arm below names, nested inside
            // owned contents. The classifier admits no loan that mutates, no
            // erased or generic member, and no cleanup owner, so the result
            // keeps the same claim-free custody the plain-contents arm gives.
            || (matches!(
                result.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            ) && validation::has_owned_or_shared_view_contents(
                program,
                target_state.return_type,
            ))
            // Shared descriptors and materialized reference carriers borrow
            // storage but retain their distinct result-custody obligations.
            || (result.multiplicity
                == validation::reference_result_custody::result_multiplicity(
                    program,
                    target_state.return_type,
                )
                && (crate::execution::terminal_unit::types::borrowed_slice_view(
                    program,
                    target_state.return_type,
                ) || crate::execution::terminal_unit::types::borrowed_named_view(
                    program,
                    target_state.return_type,
                ))))
        && program
            .machine_states(target_machine)
            .first()
            .is_some_and(|entry| entry.symbol == target_state.symbol)
        && machine_binders(program, target_machine).is_empty()
    {
        return CheckedUnitCallPlan::new(CheckedUnitCallParts {
            coordinate,
            source_site,
            target: call_target,
            route: CheckedUnitCallRoute::Module {
                contract_commitment: Some(target_contract.commitment),
                erased_scalar_arguments,
                erased_proof_arguments,
            },
            scalar_arguments,
            structural_arguments,
            result: CheckedUnitCallResult::Structural {
                binding: result.clone(),
                discard_result_on_return: result.multiplicity == Multiplicity::Affine
                    && !reference_loan.is_valid(),
            },
            transfers: CheckedUnitCallTransfers::StructuralCustody(
                checked_trees::CheckedStructuralCallCustodyPlan {
                    reference_loan,
                    result_qualifications: if validation::shared_slice_view_result(
                        program,
                        target_state.return_type,
                    ) {
                        // A shared view's declared byte-predicate domains
                        // ride the same carrier identity its result spells.
                        validation::view_result_carrier_qualifications(
                            program,
                            target_state.return_type,
                        )
                        .ok()?
                    } else if represented_copy_result {
                        validation::structural_result_qualifications(
                            program,
                            target_state.return_type,
                        )
                        .ok()?
                    } else {
                        Vec::new()
                    },
                    ..Default::default()
                },
            ),
        })
        .map(CheckedUnitEffectOperationPlan::Call);
    }
    phase("call operation: claim-free affine result");
    let target = facts
        .flow
        .terminal_structural_returns
        .claim_free_affine_for_machine(target_machine.symbol)?;
    let [argument] = structural_arguments.as_slice() else {
        return None;
    };
    let source_matches = if let Some(index) = argument.source_parameter_index() {
        let source = caller_parameters.get(usize::try_from(index).ok()?)?;
        source.type_identity == argument.type_identity
            && source.multiplicity == Multiplicity::Affine
            && source.access == CheckedStructuralAccess::Owned
            && source.qualifications.is_empty()
    } else if let Some(ordinal) = argument.source_structural_result_binding_ordinal() {
        caller_structural_results.iter().any(|(source, _)| {
            source.binding_ordinal == ordinal
                && source.statement_index <= coordinate.statement_index
                && source.type_identity == argument.type_identity
                && source.multiplicity == Multiplicity::Affine
        })
    } else {
        false
    };
    if target.state != target_state.symbol
        || target.result.type_identity != result.type_identity
        || result.multiplicity != Multiplicity::Affine
        || target.scalar_parameters != scalar_parameters
        || target.structural_parameter.type_identity != argument.type_identity
        || !source_matches
        || argument.access != CheckedStructuralAccess::Owned
        || !argument.path.is_empty()
        || !transfers.is_empty()
        || !service_reach_is_empty(facts, call.service_reach)
    {
        return None;
    }
    CheckedUnitCallPlan::new(CheckedUnitCallParts {
        coordinate,
        source_site,
        target: call_target,
        route: CheckedUnitCallRoute::Module {
            contract_commitment: Some(target_contract.commitment),
            erased_scalar_arguments,
            erased_proof_arguments,
        },
        scalar_arguments,
        structural_arguments,
        result: CheckedUnitCallResult::Structural {
            binding: result.clone(),
            discard_result_on_return: true,
        },
        transfers: CheckedUnitCallTransfers::StructuralCustody(Default::default()),
    })
    .map(CheckedUnitEffectOperationPlan::Call)
}

/// A call whose returned `&mut` structural view loans caller storage mints the
/// same custody a `parts` result does, at the coarser granularity signatures
/// declare: the binding's recorded loan is the receiver's or one named input's
/// whole place, not the projected leaf the callee actually returned. The
/// callee's returned borrow must still denote caller-frame storage — a `&mut`
/// parameter or a `self` member, reached through the callee's own minted view
/// loans — because the caller-side loan only ever spans caller places.
fn view_result_loan(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    target_machine: &typed_trees::machine::Machine,
    target_state: &typed_trees::state::State,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Option<arena::Handle<checked_trees::BorrowLoanFact>> {
    let Some(StatementNode::LocalData(local)) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return None;
    };
    if crate::execution::terminal_unit::reference_results::mutable_borrowed_parts(
        program,
        local.type_reference,
    )
    .is_none()
        || local.is_mutable
    {
        return None;
    }
    let ExpressionNode::Call(expression) = program.expression_table.expression(local.initial_value)
    else {
        return None;
    };
    if crate::flow::call_target_symbol(program, expression) != call.target_symbol
        || call.authored_expression != local.initial_value
        || call.statement_index != result.statement_index as usize
        || call.call_ordinal != 0
    {
        return None;
    }
    // The callee's own return analysis names the borrow source the elision
    // helper cannot decide: signatures carrying two `&mut` inputs read as
    // ambiguous there while the returned expression's loan walk still
    // resolves one exact caller-side place.
    let source = view_return_loan_source(program, facts, target_machine, target_state)?;
    let expected = match source {
        ViewLoanSource::SelfReceiver => {
            if !expression.receiver.is_valid() {
                return None;
            }
            crate::borrow::accesses::borrow_access_place(
                program,
                state.symbol,
                call.statement_index,
                expression.receiver,
                machine.symbol,
            )?
        }
        ViewLoanSource::Parameter(non_self_index) => {
            let argument = program
                .expression_table
                .expression_handles(expression.arguments)
                .get(non_self_index as usize)
                .copied()?;
            // The minted result loan names the argument's literal place
            // root — a re-borrow through whatever the argument denotes,
            // not its loan-substituted storage root.
            crate::borrow::accesses::borrow_access_place(
                program,
                state.symbol,
                call.statement_index,
                argument,
                machine.symbol,
            )?
        }
    };
    let mut borrow_states = facts
        .borrow
        .states
        .iter()
        .map(|(_, candidate)| candidate)
        .filter(|candidate| {
            candidate.machine_symbol == machine.symbol && candidate.state_symbol == state.symbol
        });
    let Some(borrowing) = borrow_states.next() else {
        return None;
    };
    if borrow_states.next().is_some() {
        return None;
    }
    let mut loans = facts.borrow.loans.iter().filter(|(handle, loan)| {
        facts.borrow.state_owns_loan(borrowing, *handle) && loan.owner_symbol == local.symbol
    });
    let Some((handle, loan)) = loans.next() else {
        return None;
    };
    if loans.next().is_some()
        || loan.statement_index != call.statement_index
        || loan.root_symbol != expected.root_symbol
        || loan.kind != checked_trees::BorrowAccessKind::Mutable
        || facts.borrow.loan_segments(loan) != expected.segments.as_slice()
        || !facts.borrow.loan_owner_path(loan).is_empty()
    {
        return None;
    }
    let Some(flow) = state_flow(facts, machine.symbol, state.symbol) else {
        return None;
    };
    let mut activations = facts
        .flow
        .borrow_lifetimes
        .activations
        .span_or_empty(flow.borrow_activations)
        .iter()
        .filter(|activation| activation.loan == handle);
    let Some(activation) = activations.next() else {
        return None;
    };
    if activations.next().is_some()
        || activation.source
            != (checked_trees::FlowInvalidationSource::Statement {
                statement_index: loan.statement_index,
            })
    {
        return None;
    }
    validation::reference_result_custody::release_statement(
        facts,
        machine.symbol,
        state.symbol,
        handle,
    )?;
    Some(handle)
}

/// The caller-side place a `&mut`-view result loans: the call receiver when
/// the returned storage lives inside `*self`, or the one non-self argument
/// whose place the returned borrow denotes.
enum ViewLoanSource {
    SelfReceiver,
    Parameter(u32),
}

/// Every `&mut` a `mutable_borrowed_parts` state returns must denote the same
/// caller-frame storage: a `&mut` parameter of the callee, the `*self`
/// machine authority, or a `self` member — reached through the callee's
/// minted view loans when the return names a view local. A root in
/// callee-owned storage would mint a caller loan over storage that dies at
/// return; returns resolving to different caller places are equally
/// unmintable, since the result's single loan names one root.
fn view_return_loan_source(
    program: &TypedTrees,
    facts: &CheckFacts,
    target_machine: &typed_trees::machine::Machine,
    target_state: &typed_trees::state::State,
) -> Option<ViewLoanSource> {
    let statements = program
        .statement_table
        .statements(target_state.statement_nodes);
    let parameters = program.state_parameters(target_state);
    let mut source: Option<ViewLoanSource> = None;
    for (ordinal, statement) in statements.iter().enumerate() {
        let returned = match statement {
            StatementNode::Transition(transition) => {
                match program.statement_table.transition_target(transition.target) {
                    TransitionTargetNode::Value(expression) => Some(*expression),
                    _ => None,
                }
            }
            StatementNode::Expression(expression) if ordinal + 1 == statements.len() => {
                Some(*expression)
            }
            _ => None,
        };
        let Some(returned) = returned else {
            continue;
        };
        let ExpressionNode::Borrow(borrow) = program.expression_table.expression(returned) else {
            return None;
        };
        if borrow.access != language_core::ReferenceAccess::Mutable {
            return None;
        }
        let root = view_return_caller_root(
            program,
            facts,
            target_machine.symbol,
            target_state,
            ordinal,
            borrow.target,
        )?;
        let resolved = if let Some(position) = parameters
            .iter()
            .position(|parameter| parameter.symbol == root)
        {
            if !view_source_parameter_is_mutable(program, &parameters[position]) {
                return None;
            }
            let non_self = parameters[..position]
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count();
            ViewLoanSource::Parameter(u32::try_from(non_self).ok()?)
        } else if root == target_machine.symbol
            || view_self_member_symbol(program, target_state, target_machine.symbol, root).is_some()
        {
            ViewLoanSource::SelfReceiver
        } else {
            return None;
        };
        let agrees = match (&source, &resolved) {
            (None, _) => true,
            (Some(ViewLoanSource::SelfReceiver), ViewLoanSource::SelfReceiver) => true,
            (Some(ViewLoanSource::Parameter(prior)), ViewLoanSource::Parameter(next)) => {
                prior == next
            }
            _ => false,
        };
        if !agrees {
            return None;
        }
        source = Some(resolved);
    }
    source
}

/// The returned borrow's place, with view-local roots substituted by their
/// minted loans, ends at caller-frame storage: a `&mut` parameter, the
/// machine's `*self` authority, or a `self` member. Answers the resolved
/// root symbol so the caller can map it to the argument whose loan the
/// result mirrors.
fn view_return_caller_root(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    ordinal: usize,
    target: checked_trees::expression::ExpressionHandle,
) -> Option<SymbolHandle> {
    let Some(mut place) = crate::borrow::accesses::borrow_access_place(
        program,
        state.symbol,
        ordinal,
        target,
        machine,
    ) else {
        return None;
    };
    for _ in 0..8 {
        if view_source_root_is_caller_storage(program, state, machine, place.root_symbol) {
            return Some(place.root_symbol);
        }
        let Some(loan) = view_local_loan(facts, machine, state.symbol, place.root_symbol) else {
            return None;
        };
        if loan.kind != checked_trees::BorrowAccessKind::Mutable
            || !facts.borrow.loan_owner_path(loan).is_empty()
        {
            return None;
        }
        let mut segments = facts.borrow.loan_segments(loan).to_vec();
        segments.extend(place.segments);
        place = checked_trees::CapturedPlace {
            root_symbol: loan.root_symbol,
            segments,
        };
    }
    None
}

/// The one loan a view local's binding carries inside the callee's own borrow
/// state — the recorded place the local actually names.
fn view_local_loan<'f>(
    facts: &'f CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    owner: SymbolHandle,
) -> Option<&'f checked_trees::BorrowLoanFact> {
    let borrow_state = facts
        .borrow
        .states
        .iter()
        .map(|(_, candidate)| candidate)
        .find(|candidate| candidate.machine_symbol == machine && candidate.state_symbol == state)?;
    let mut loans = facts.borrow.loans.iter().filter(|(handle, loan)| {
        facts.borrow.state_owns_loan(borrow_state, *handle) && loan.owner_symbol == owner
    });
    let (_, loan) = loans.next()?;
    loans.next().is_none().then_some(loan)
}

/// A parameter names caller storage only while it is mutably borrowed:
/// `bag: &mut Bag` spells a `Reference` type; `&mut self` instead marks the
/// parameter's `is_mutable` referent access on the bare data type.
pub(crate) fn view_source_parameter_is_mutable(
    program: &TypedTrees,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    parameter.is_mutable
        || crate::execution::terminal_unit::reference_results::mutable_borrowed_parts(
            program,
            parameter.type_reference,
        )
        .is_some()
}

/// A denoted root denotes caller storage when it is a `&mut` parameter of the
/// callee, the machine's `*self` authority, or a `self` member — the last two
/// only while `self` itself is borrowed mutably.
fn view_source_root_is_caller_storage(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    machine: SymbolHandle,
    root: SymbolHandle,
) -> bool {
    let parameters = program.state_parameters(state);
    if parameters.iter().any(|parameter| {
        parameter.symbol == root && view_source_parameter_is_mutable(program, parameter)
    }) {
        return true;
    }
    let Some(self_parameter) = parameters.iter().find(|parameter| parameter.is_self) else {
        return false;
    };
    if !view_source_parameter_is_mutable(program, self_parameter) {
        return false;
    }
    if root == machine {
        return true;
    }
    view_self_member_symbol(program, state, machine, root).is_some()
}

/// The field symbol `root` names inside the `*self` receiver's attached data,
/// when `self` is mutably borrowed — the root a minted view loan publishes
/// for `self.<field>` storage. `None` for roots that are not `self` members
/// and whenever `self` is not mutably borrowed.
pub(crate) fn view_self_member_symbol(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    machine: SymbolHandle,
    root: SymbolHandle,
) -> Option<SymbolHandle> {
    let parameters = program.state_parameters(state);
    let self_parameter = parameters.iter().find(|parameter| parameter.is_self)?;
    if !view_source_parameter_is_mutable(program, self_parameter) {
        return None;
    }
    let mut type_reference = self_parameter.type_reference;
    let mut symbol = loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            TypeReferenceNode::Named { symbol, .. } => break *symbol,
            _ => return None,
        }
    };
    // The `&mut self` referee names the machine's `Self` alias rather than the
    // data declaration; resolve it through the machine's attached data owner.
    if symbol == machine {
        symbol = program.machine_by_symbol(machine)?.attached_data_symbol;
    }
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == symbol)?;
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.symbol == root => {
                Some(field.symbol)
            }
            _ => None,
        })
}
