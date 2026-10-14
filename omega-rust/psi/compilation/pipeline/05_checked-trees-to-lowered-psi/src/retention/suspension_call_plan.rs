//! Join checked suspension demands to the ordinary call emitter's namespaces.
//!
//! Source environments, scalar/structural operand lanes and machine-local
//! claims have different coordinates. The ephemeral call companion names
//! current homes, access and established identities; the callee's checked
//! telescope reindexes actual argument lanes. Neither join recreates storage,
//! infers a claim from a type, nor admits a loan without its checked carry obligations.
//! Runtime backing and parking remain selected-provider responsibilities.

use super::{
    CheckedTrees, LoweredSourceCallOccurrence, LoweringError, OperationId, OperationKind,
    TerminalModule, ValueDeclaration, ValueId, terminal_scalar_type, unsupported,
};
use crate::direct_calls::direct_call_parts;
use crate::terminal_identities::lookup_claim_id;
use arena::{Handle, HandleSpan};
use terminal_psi::{TerminalSuspensionPlace, TerminalSuspensionValueType};
pub(crate) fn retain_suspension_call_plans(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    source_machines: &[symbols::SymbolHandle],
    source_calls: &[LoweredSourceCallOccurrence],
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    if !module.suspension_call_plans.is_empty() {
        return unsupported("suspension call plans must be produced exactly once");
    }

    let mut plans = Vec::new();
    for crossing in checked
        .facts
        .carry
        .suspension_crossings
        .iter()
        .filter(|crossing| source_machines.contains(&crossing.machine))
    {
        let mut occurrences = source_calls.iter().filter(|occurrence| {
            occurrence.source_state == crossing.state
                && occurrence.statement_index == crossing.statement_index
                && occurrence.call_ordinal == crossing.call_ordinal
        });
        let occurrence = occurrences.next().ok_or(LoweringError::Unsupported(
            "possibly-suspending checked call has no exact Terminal operation join",
        ))?;
        let terminal_operation =
            occurrence
                .terminal_site
                .operation()
                .ok_or(LoweringError::Unsupported(
                    "external completion cannot park a continuing activation",
                ))?;
        // Emission and checking name the same callee differently: scalar and
        // structural-result calls record the callee machine symbol while Unit
        // calls and `call.target_symbol` carry the resolved entry state.
        // Comparing owning-machine identities keeps the redirect check sound —
        // one coordinate maps to one authored callee — across both spellings.
        if occurrences.next().is_some()
            || owning_callable_symbol(checked, occurrence.source_target)
                != owning_callable_symbol(checked, crossing.target)
        {
            return unsupported("suspension crossing call join is duplicate or redirected");
        }
        let (machine_index, block_index, operation_index) = module
            .machines
            .iter()
            .enumerate()
            .flat_map(|(machine_index, machine)| {
                machine
                    .blocks
                    .iter()
                    .enumerate()
                    .flat_map(move |(block_index, block)| {
                        block.operations.iter().enumerate().map(
                            move |(operation_index, operation)| {
                                (machine_index, block_index, operation_index, operation)
                            },
                        )
                    })
            })
            .find(|(.., operation)| operation.id == terminal_operation)
            .map(|(machine_index, block_index, operation_index, _)| {
                (machine_index, block_index, operation_index)
            })
            .ok_or(LoweringError::Unsupported(
                "suspension crossing Terminal operation is absent",
            ))?;
        let operation =
            &module.machines[machine_index].blocks[block_index].operations[operation_index];
        let target = terminal_call_target(&operation.kind).ok_or(LoweringError::Unsupported(
            "suspension crossing Terminal operation is not an ordinary call",
        ))?;

        let mut live_values = Vec::with_capacity(crossing.live_values.len());
        for live in &crossing.live_values {
            let (place, value_type) = match live.origin {
                checked_trees::SuspensionCrossingValueOrigin::Parameter { position, .. }
                | checked_trees::SuspensionCrossingValueOrigin::Local {
                    environment_position: position,
                    ..
                } => source_binding(&occurrence.frontier_before_call, position)?,
                checked_trees::SuspensionCrossingValueOrigin::CallArgument { position } => {
                    call_argument_binding(
                        checked,
                        crossing.target,
                        &operation.kind,
                        module,
                        position,
                    )?
                }
            };
            if let TerminalSuspensionPlace::Scalar(value) = place {
                if !scalar_value_available_before(module, terminal_operation, value) {
                    return unsupported("suspension frontier value is unavailable at its call");
                }
                let source_type = checked
                    .primitive_type_reference(live.type_reference)
                    .ok_or(LoweringError::Unsupported(
                        "suspension scalar frontier has no checked scalar type",
                    ))?;
                if TerminalSuspensionValueType::Scalar(terminal_scalar_type(source_type)?)
                    != value_type
                {
                    return unsupported("suspension frontier scalar type join is inconsistent");
                }
            } else if let TerminalSuspensionPlace::Structural(argument) = &place
                && !module.machines[machine_index]
                    .structural_places
                    .iter()
                    .any(|declaration| declaration.id == argument.place)
            {
                return unsupported(
                    "suspension structural frontier has no caller storage declaration",
                );
            }
            let mut claims = live
                .claims
                .iter()
                .map(|identity| lookup_claim_id(&occurrence.frontier_before_call.claims, *identity))
                .collect::<Result<Vec<_>, _>>()?;
            claims.sort();
            live_values.push((
                live,
                terminal_psi::TerminalSuspensionLiveValue {
                    place,
                    value_type,
                    storage: match live.storage {
                        checked_trees::SuspensionCrossingStorage::Parameter => {
                            terminal_psi::TerminalSuspensionStorage::Parameter
                        }
                        checked_trees::SuspensionCrossingStorage::Local => {
                            terminal_psi::TerminalSuspensionStorage::Local
                        }
                        checked_trees::SuspensionCrossingStorage::CallArgument => {
                            terminal_psi::TerminalSuspensionStorage::CallArgument
                        }
                    },
                    claim_count: u32::try_from(claims.len()).map_err(|_| {
                        LoweringError::Unsupported(
                            "suspension frontier claim count exceeds the Terminal vocabulary",
                        )
                    })?,
                    claims,
                    effective: live.effective,
                    loans: terminal_psi::TerminalSuspensionValueLoans::Unresolved,
                },
            ));
        }
        live_values.sort_by(|left, right| {
            (
                &left.1.place,
                left.1.storage,
                left.1.value_type,
                left.1.effective,
            )
                .cmp(&(
                    &right.1.place,
                    right.1.storage,
                    right.1.value_type,
                    right.1.effective,
                ))
        });
        let loans = retain_loan_graph(
            identities,
            checked,
            crossing,
            occurrence,
            &mut live_values,
            &module.structural_types,
        )?;
        let live_values = live_values
            .into_iter()
            .map(|(_, live)| live)
            .collect::<Vec<_>>();
        let crossing_id = checked_trees::canonical_suspension_crossing_id(&checked.typed, crossing)
            .ok_or(LoweringError::Unsupported(
                "suspension crossing identity cannot resolve its source symbols",
            ))?;
        // Bind the authoritative possibly-suspending demand onto the call
        // itself: the site and plan rows below are evidence for this marker,
        // not its source, so deleting the paired rows cannot erase the
        // required crossing.
        module.machines[machine_index].blocks[block_index].operations[operation_index]
            .suspension_crossing = Some(crossing_id);
        plans.push(terminal_psi::TerminalSuspensionCallPlan {
            operation: terminal_operation,
            crossing: crossing_id,
            target,
            effective: crossing.effective,
            live_value_count: u32::try_from(live_values.len()).map_err(|_| {
                LoweringError::Unsupported(
                    "suspension live-value count exceeds the Terminal vocabulary",
                )
            })?,
            live_values,
            loans,
        });
    }
    plans.sort_by_key(|plan| (plan.operation, plan.crossing));
    module.suspension_call_plan_count = u32::try_from(plans.len()).map_err(|_| {
        LoweringError::Unsupported("suspension call plan count exceeds the Terminal vocabulary")
    })?;
    module.suspension_call_sites = plans
        .iter()
        .map(|plan| terminal_psi::TerminalSuspensionCallSite {
            operation: plan.operation,
            crossing: plan.crossing,
            target: plan.target,
            frontier_commitment: terminal_psi::suspension_frontier_commitment(
                &identities.snapshot(),
                plan,
            ),
        })
        .collect();
    module.suspension_call_plans = plans;
    Ok(())
}

/// Transfer the already checked occurrence relationships into this plan's
/// namespace. Source handles are construction scratch only. Capture coordinates
/// come from the emitting operation, never from the descriptor's current home.
fn retain_loan_graph(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    crossing: &checked_trees::SuspensionCrossingCarryFact,
    occurrence: &LoweredSourceCallOccurrence,
    live_values: &mut [(
        &checked_trees::SuspensionCrossingLiveValueFact,
        terminal_psi::TerminalSuspensionLiveValue,
    )],
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<terminal_psi::TerminalSuspensionLoans, LoweringError> {
    use checked_trees::{
        BorrowLoanLineage, CheckedCallBorrowLoanSource, CheckedParentBorrowResource,
        SuspensionCrossingLoanBacking, SuspensionCrossingLoanSource, SuspensionValueBorrowFrontier,
    };
    use terminal_psi::{
        StructuralAccess, StructuralPathSegment, TerminalSuspensionLoan,
        TerminalSuspensionLoanBacking, TerminalSuspensionLoanCapture,
        TerminalSuspensionLoanResource, TerminalSuspensionValueLoans,
    };
    let carry = &checked.facts.carry;
    let borrow = &checked.facts.borrow;
    let rows =
        carry
            .suspension_loans
            .span(crossing.borrows.loans)
            .ok_or(LoweringError::Unsupported(
                "suspension loan roster is unavailable",
            ))?;
    let mut graph = terminal_psi::TerminalSuspensionLoans::default();
    let mut loan_identities = Vec::new();
    for (position, row) in rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.retained_for_crossing)
    {
        let terminal = graph.occurrences.insert(TerminalSuspensionLoan::default());
        loan_identities.push((
            crossing.borrows.loans.handle_at(position as u32),
            row.stored_loan(),
            terminal,
        ));
    }
    let dependency = |loan: Handle<checked_trees::BorrowLoanFact>| {
        if !loan.is_valid() {
            return Handle::invalid();
        }
        loan_identities
            .iter()
            .find(|(_, source, _)| *source == loan)
            .map_or(Handle::invalid(), |(_, _, terminal)| *terminal)
    };
    for (source_row, source_loan, terminal) in &loan_identities {
        let row = carry.suspension_loans.get(*source_row);
        let (capture, resource, parent, suppliers, access) = match row.source {
            SuspensionCrossingLoanSource::Stored { ref resource, .. } => {
                if !borrow.loans.is_valid(*source_loan) {
                    return unsupported("suspension loan occurrence is unavailable");
                }
                let loan = borrow.loans.get(*source_loan);
                let mut captures = occurrence
                    .frontier_before_call
                    .loan_captures
                    .iter()
                    .filter(|capture| capture.loan == *source_loan);
                let capture = if let Some(capture) = captures.next() {
                    if captures.next().is_some() {
                        return unsupported("suspension loan has multiple formation captures");
                    }
                    let mut path = HandleSpan::empty();
                    for segment in &capture.source.path {
                        path.push_contiguous(graph.paths.insert(segment.clone()));
                    }
                    TerminalSuspensionLoanCapture::Reference {
                        formation: capture.formation,
                        carrier: capture.carrier,
                        source: capture.source.place,
                        source_access: capture.source.access,
                        source_path: path,
                    }
                } else {
                    TerminalSuspensionLoanCapture::Unresolved
                };
                let resource = match resource {
                    CheckedParentBorrowResource::DirectRoot { resource }
                        if borrow.direct_loan_resources.is_valid(*resource) =>
                    {
                        TerminalSuspensionLoanResource::DirectRoot
                    }
                    CheckedParentBorrowResource::Reborrow { resource }
                        if borrow.reborrow_loan_resources.is_valid(*resource) =>
                    {
                        TerminalSuspensionLoanResource::Reborrow
                    }
                    _ => TerminalSuspensionLoanResource::Unresolved,
                };
                let parent = match loan.lineage {
                    BorrowLoanLineage::Reborrow { parent_loan } => dependency(parent_loan),
                    _ => Handle::invalid(),
                };
                let mut suppliers = HandleSpan::empty();
                for source in borrow.loan_suppliers.span_or_empty(loan.supplying_loans) {
                    let supplier = dependency(*source);
                    if !supplier.is_valid() {
                        return unsupported("suspension loan supplier is not retained");
                    }
                    suppliers.push_contiguous(graph.supplier_links.insert(supplier));
                }
                (capture, resource, parent, suppliers, &loan.kind)
            }
            SuspensionCrossingLoanSource::CapturedActual { resource } => {
                if !borrow.call_loan_resources.is_valid(resource) {
                    return unsupported("suspension captured actual resource is unavailable");
                }
                let resource = borrow.call_loan_resources.get(resource);
                let position = crossing
                    .borrows
                    .operands
                    .handles()
                    .position(|operand| operand == resource.owner_operand)
                    .ok_or(LoweringError::Unsupported(
                        "suspension loan has no captured actual",
                    ))?;
                let live_value = live_values
                    .iter()
                    .position(|(live, _)| {
                        live.origin
                            == checked_trees::SuspensionCrossingValueOrigin::CallArgument {
                                position,
                            }
                    })
                    .ok_or(LoweringError::Unsupported(
                        "suspension captured actual is not retained",
                    ))?;
                let path = borrow.call_loan_paths.span(resource.capture_path).ok_or(
                    LoweringError::Unsupported("suspension actual loan path is unavailable"),
                )?;
                let TerminalSuspensionValueType::Structural(root) =
                    live_values[live_value].1.value_type
                else {
                    return unsupported("suspension captured loan has no structural operand");
                };
                let path = crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
            identities, path, root, types)?;
                let path = graph.paths.insert_many(path);
                let capture = TerminalSuspensionLoanCapture::CallArgument {
                    live_value: u32::try_from(live_value).map_err(|_| {
                        LoweringError::Unsupported(
                            "suspension actual index exceeds Terminal vocabulary",
                        )
                    })?,
                    path,
                };
                let (kind, parent, suppliers) = match resource.source {
                    CheckedCallBorrowLoanSource::StoredLoan { loan } => {
                        let supplier = dependency(loan);
                        if !supplier.is_valid() {
                            return unsupported("suspension actual loan supplier is not retained");
                        }
                        let kind = match borrow.loans.get(loan).lineage {
                            BorrowLoanLineage::Reborrow { .. } => {
                                TerminalSuspensionLoanResource::Reborrow
                            }
                            BorrowLoanLineage::DirectRoot => {
                                TerminalSuspensionLoanResource::DirectRoot
                            }
                            _ => TerminalSuspensionLoanResource::Unresolved,
                        };
                        (
                            kind,
                            Handle::invalid(),
                            graph.supplier_links.insert_many([supplier]),
                        )
                    }
                    CheckedCallBorrowLoanSource::CallAccess { parents, .. } => {
                        let parents = borrow.call_loan_parents.span(parents).ok_or(
                            LoweringError::Unsupported(
                                "suspension actual parent roster is unavailable",
                            ),
                        )?;
                        if parents.iter().any(|parent| !dependency(*parent).is_valid()) {
                            return unsupported("suspension actual parent is not retained");
                        }
                        let parent = parents
                            .first()
                            .map_or(Handle::invalid(), |parent| dependency(*parent));
                        (
                            if parent.is_valid() {
                                TerminalSuspensionLoanResource::Reborrow
                            } else {
                                TerminalSuspensionLoanResource::DirectRoot
                            },
                            parent,
                            HandleSpan::empty(),
                        )
                    }
                };
                (capture, kind, parent, suppliers, &resource.access)
            }
        };
        let backing = match row.backing {
            SuspensionCrossingLoanBacking::Unresolved => TerminalSuspensionLoanBacking::Unresolved,
            SuspensionCrossingLoanBacking::StateValue { origin } => {
                let position = live_values
                    .iter()
                    .position(|(live, _)| live.origin == origin)
                    .ok_or(LoweringError::Unsupported(
                        "suspension loan backing is not retained",
                    ))?;
                TerminalSuspensionLoanBacking::RetainedOwner {
                    live_value: u32::try_from(position).map_err(|_| {
                        LoweringError::Unsupported(
                            "suspension backing index exceeds Terminal vocabulary",
                        )
                    })?,
                }
            }
            SuspensionCrossingLoanBacking::IncomingReferent { identity } => {
                if !borrow.incoming_references.is_valid(identity) {
                    return unsupported("suspension incoming referent is unavailable");
                }
                let incoming = borrow.incoming_references.get(identity);
                // The checked opaque identity is rooted in the parameter's
                // referent, not in owned storage holding its descriptor.
                let binding = live_values.iter().find(|(live, _)| matches!(live.origin, checked_trees::SuspensionCrossingValueOrigin::Parameter { symbol, .. } if symbol == incoming.parameter_symbol));
                if let Some((_, live)) = binding
                    && let TerminalSuspensionPlace::Structural(argument) = &live.place
                {
                    let mut argument = argument.clone();
                    argument.path.push(StructuralPathSegment::Referent);
                    TerminalSuspensionLoanBacking::IncomingReferent { argument }
                } else {
                    TerminalSuspensionLoanBacking::Unresolved
                }
            }
        };
        *graph.occurrences.get_mut(*terminal) = TerminalSuspensionLoan {
            capture,
            backing,
            resource,
            parent,
            suppliers,
            access: match access {
                checked_trees::BorrowAccessKind::Read => StructuralAccess::SharedBorrow,
                checked_trees::BorrowAccessKind::Mutable => StructuralAccess::MutableBorrow,
                checked_trees::BorrowAccessKind::WriteOnly => StructuralAccess::WriteOnlyBorrow,
            },
            active_at_call: row.active_at_call,
        };
    }
    for (source, live) in live_values {
        live.loans =
            match source.borrow_frontier {
                SuspensionValueBorrowFrontier::ViewFree => TerminalSuspensionValueLoans::ViewFree,
                SuspensionValueBorrowFrontier::RetainedLoans { loans } => {
                    let links = carry.suspension_value_loans.span(loans).ok_or(
                        LoweringError::Unsupported("suspension value loan edges are unavailable"),
                    )?;
                    let mut retained = HandleSpan::empty();
                    for link in links {
                        let terminal = loan_identities
                            .iter()
                            .find(|(source, _, _)| source == link)
                            .map(|(_, _, terminal)| *terminal)
                            .ok_or(LoweringError::Unsupported(
                                "suspension value names an unretained loan",
                            ))?;
                        retained.push_contiguous(graph.value_links.insert(terminal));
                    }
                    TerminalSuspensionValueLoans::Retained { links: retained }
                }
                SuspensionValueBorrowFrontier::Unresolved => {
                    TerminalSuspensionValueLoans::Unresolved
                }
            };
    }
    Ok(graph)
}

/// Source positions and Terminal lanes are different namespaces. A structural
/// binding keeps its existing storage operand, including access; an SSA
/// binding keeps its value. Neither a caller binding nor a callee formal may
/// replace the actual operand's access with ownership just because their
/// physical representations agree.
fn source_binding(
    bindings: &lowered_psi::LoweredSourceFrontierBindings,
    position: usize,
) -> Result<(TerminalSuspensionPlace, TerminalSuspensionValueType), LoweringError> {
    if let Some(binding) = bindings
        .scalars
        .iter()
        .find(|binding| binding.source_position == position)
    {
        return Ok((
            TerminalSuspensionPlace::Scalar(binding.value.id),
            TerminalSuspensionValueType::Scalar(binding.value.scalar_type),
        ));
    }
    let binding = bindings
        .structural
        .iter()
        .find(|binding| binding.source_position == position)
        .ok_or(LoweringError::Unsupported(
            "suspension frontier source position has no storage binding",
        ))?;
    Ok((
        TerminalSuspensionPlace::Structural(binding.argument.clone()),
        TerminalSuspensionValueType::Structural(binding.structural_type),
    ))
}

/// Reindex one checked ordinary argument through the callee's explicit formal
/// telescope, excluding erased formals from both runtime lanes. Unit, scalar
/// and structural results do not change the argument namespace. The existing
/// emitted operand and callee formal supply the storage and type; this join
/// does not materialize another argument or infer linear claims from its type.
fn call_argument_binding(
    checked: &CheckedTrees,
    target: symbols::SymbolHandle,
    operation: &OperationKind,
    module: &TerminalModule,
    position: usize,
) -> Result<(TerminalSuspensionPlace, TerminalSuspensionValueType), LoweringError> {
    let parameters = validation::call_target_parameters(&checked.typed, target).ok_or(
        LoweringError::Unsupported("suspension call argument has no checked formal telescope"),
    )?;
    let parameter = parameters
        .get(position)
        .filter(|parameter| !parameter.relevance.is_erased())
        .ok_or(LoweringError::Unsupported(
            "suspension call argument has no runtime formal",
        ))?;
    let scalar = checked
        .primitive_type_reference(parameter.type_reference)
        .is_some();
    let lane_position = parameters[..position]
        .iter()
        .filter(|parameter| {
            !parameter.relevance.is_erased()
                && checked
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
                    == scalar
        })
        .count();
    let (scalars, structural, structural_parameters) =
        if let Some(call) = direct_call_parts(operation) {
            let callee = module
                .machines
                .iter()
                .find(|machine| machine.id == call.callee)
                .ok_or(LoweringError::Unsupported(
                    "suspension call argument has no emitted callee",
                ))?;
            (
                call.arguments,
                call.structural_arguments,
                callee.structural_parameters.as_slice(),
            )
        } else if let OperationKind::BoundaryCall {
            boundary,
            arguments,
            structural_arguments,
            ..
        } = operation
        {
            let callee = module
                .boundary_machines
                .iter()
                .find(|callee| callee.id == *boundary)
                .ok_or(LoweringError::Unsupported(
                    "suspension call argument has no emitted requirement",
                ))?;
            (
                arguments.as_slice(),
                structural_arguments.as_slice(),
                callee.structural_parameters.as_slice(),
            )
        } else {
            return unsupported("suspension call argument is not an ordinary call");
        };
    if scalar {
        let value = scalars
            .get(lane_position)
            .copied()
            .and_then(|value| scalar_declaration(module, value))
            .ok_or(LoweringError::Unsupported(
                "suspension call scalar argument has no emitted value",
            ))?;
        return Ok((
            TerminalSuspensionPlace::Scalar(value.id),
            TerminalSuspensionValueType::Scalar(value.scalar_type),
        ));
    }
    let argument = structural
        .get(lane_position)
        .ok_or(LoweringError::Unsupported(
            "suspension call structural argument has no emitted storage",
        ))?;
    let parameter = structural_parameters
        .get(lane_position)
        .ok_or(LoweringError::Unsupported(
            "suspension call structural argument has no emitted formal",
        ))?;
    Ok((
        TerminalSuspensionPlace::Structural(argument.clone()),
        TerminalSuspensionValueType::Structural(parameter.structural_type),
    ))
}

fn owning_callable_symbol(
    checked: &CheckedTrees,
    symbol: symbols::SymbolHandle,
) -> symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| {
            typed_trees::state::state_by_symbol(checked.machine_states(machine), symbol).is_some()
        })
        .map_or(symbol, |machine| machine.symbol)
}

fn terminal_call_target(
    kind: &OperationKind,
) -> Option<terminal_psi::TerminalSuspensionCallTarget> {
    if let Some(call) = direct_call_parts(kind) {
        return Some(terminal_psi::TerminalSuspensionCallTarget::Machine(
            call.callee,
        ));
    }
    match kind {
        OperationKind::BoundaryCall { boundary, .. } => Some(
            terminal_psi::TerminalSuspensionCallTarget::Boundary(*boundary),
        ),
        _ => None,
    }
}

fn scalar_declaration(module: &TerminalModule, value: ValueId) -> Option<ValueDeclaration> {
    module.machines.iter().find_map(|machine| {
        machine
            .parameters
            .iter()
            .chain(machine.block_parameters())
            .chain(
                machine
                    .operations()
                    .filter_map(|operation| operation.result.scalar_ref()),
            )
            .find(|declaration| declaration.id == value)
            .copied()
    })
}

/// The checked binding supplies source provenance; SSA visibility supplies the
/// emitted scope. A local can alias an ingress value or travel through a private
/// join parameter without manufacturing a new operation result at every call.
fn scalar_value_available_before(
    module: &TerminalModule,
    operation: OperationId,
    value: ValueId,
) -> bool {
    module.machines.iter().any(|machine| {
        machine.blocks.iter().any(|block| {
            block
                .operations
                .iter()
                .position(|candidate| candidate.id == operation)
                .is_some_and(|operation_index| {
                    machine
                        .parameters
                        .iter()
                        .chain(block.parameters.iter())
                        .chain(
                            block.operations[..operation_index]
                                .iter()
                                .filter_map(|candidate| candidate.result.scalar_ref()),
                        )
                        .any(|declaration| declaration.id == value)
                })
        })
    })
}
