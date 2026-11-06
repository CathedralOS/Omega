//! Calls that may pause must preserve data the program will use afterward.
//! Connect each emitted call to the values and loans checked for that purpose.
//!
//! Source bindings, scalar and structural arguments, and machine-local claims
//! use different coordinates. The call companion names their current homes,
//! access and established identities; the callee's checked parameter sequence
//! maps the actual argument lanes. These joins preserve existing storage,
//! checked claims and carry obligations.
//! Runtime backing and parking remain selected-provider responsibilities.

use super::{
    CheckedTrees, LoweredSourceCallOccurrence, LoweringError, OperationKind, TerminalModule,
    ValueDeclaration, ValueId, terminal_scalar_type, unsupported,
};
use crate::expression_preparation::bindings::structural_paths::lower_static_argument_path;
use crate::shared::direct_calls::direct_call_parts;
use crate::shared::terminal_identities::lookup_claim_id;
use arena::{Handle, HandleSpan};
use terminal_psi::{TerminalSuspensionPlace, TerminalSuspensionValueType};
pub(crate) fn retain_suspension_call_plans(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    source_machines: &[symbols::SymbolHandle],
    source_calls: &[LoweredSourceCallOccurrence],
    state_blocks: &[lowered_psi::LoweredStateBlock],
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    if !module.suspension_call_plans.is_empty() {
        return unsupported("suspension call plans must be produced exactly once");
    }

    let mut plans = Vec::new();
    let mut source_call_bindings = SourceCallBindings::default();
    // A fused recursive component lowers its member machines' states into the
    // composed Terminal machine of the component's entry, so those members are
    // not source machines of their own. Every emitted state still owes its
    // crossings: a parking call inside a member keeps its plan, and a transfer
    // between members is recognised below as a fused edge.
    for crossing in checked
        .facts
        .carry
        .suspension_crossings
        .iter()
        .filter(|crossing| {
            source_machines.contains(&crossing.machine)
                || state_blocks
                    .iter()
                    .any(|block| block.state == crossing.state)
        })
    {
        // A fused transfer has no standalone call to bind. Other missing sites
        // remain errors at the shared required-call join.
        if !source_calls.iter().any(|occurrence| {
            occurrence.matches_statement(
                crossing.state,
                crossing.statement_index,
                crossing.call_ordinal,
            )
        }) && fused_component_transfer(checked, state_blocks, crossing)
        {
            continue;
        }
        let call_binding = source_call_bindings.bind(
            checked,
            source_calls,
            module,
            crossing.state,
            crossing.statement_index,
            crossing.call_ordinal,
            crossing.target,
        )?;
        let call_binding = &mut source_call_bindings.calls[call_binding];
        let occurrence = &source_calls[call_binding.occurrence_index];
        let (machine_index, block_index, operation_index) = call_binding.location;
        let terminal_operation = call_binding.operation;
        let target = call_binding.target;

        let mut live_values = Vec::with_capacity(crossing.live_values.len());
        for live in &crossing.live_values {
            let (place, value_type) = match live.origin {
                checked_trees::SuspensionCrossingValueOrigin::Parameter { position, .. }
                | checked_trees::SuspensionCrossingValueOrigin::Local {
                    environment_position: position,
                    ..
                } => source_binding(&occurrence.frontier_before_call, position)?,
                checked_trees::SuspensionCrossingValueOrigin::CallArgument { position } => {
                    call_binding.argument(checked, module, position)?
                }
            };
            if let TerminalSuspensionPlace::Scalar(value) = place {
                if !scalar_value_available_before(
                    &module.machines[machine_index],
                    block_index,
                    operation_index,
                    value,
                ) {
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
            source_calls,
            &mut source_call_bindings,
            module,
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

/// Translate checked loan relationships without reconstructing their meaning.
/// Emitted captures still belong to the current call companion and its exact
/// older call sites; source handles never enter the canonical artifact.
fn retain_loan_graph(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    crossing: &checked_trees::SuspensionCrossingCarryFact,
    occurrence: &LoweredSourceCallOccurrence,
    live_values: &mut [(
        &checked_trees::SuspensionCrossingLiveValueFact,
        terminal_psi::TerminalSuspensionLiveValue,
    )],
    source_calls: &[LoweredSourceCallOccurrence],
    source_call_bindings: &mut SourceCallBindings,
    module: &TerminalModule,
) -> Result<terminal_psi::TerminalSuspensionLoans, LoweringError> {
    use checked_trees::{
        CheckedParentBorrowResource, SuspensionCrossingLoanBackingPlan as BackingPlan,
        SuspensionCrossingLoanResource as Resource, SuspensionCrossingLoanSource,
        SuspensionValueBorrowFrontier,
    };
    use terminal_psi::{
        StructuralAccess, TerminalSuspensionLoan, TerminalSuspensionLoanBacking,
        TerminalSuspensionLoanCapture, TerminalSuspensionLoanResource,
        TerminalSuspensionValueLoans,
    };
    let carry = &checked.facts.carry;
    let borrow = &checked.facts.borrow;
    let types = &module.structural_types;
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
    for (source_row, source_loan, terminal) in &loan_identities {
        let row = carry.suspension_loans.get(*source_row);
        let relations = row.relations.as_ref().ok_or(LoweringError::Unsupported(
            "suspension loan has no completed checked relation plan",
        ))?;
        let capture = match &row.source {
            SuspensionCrossingLoanSource::Incoming { identity, .. } => {
                let (position, path, _) = incoming_reference_binding(
                    identities,
                    checked,
                    *identity,
                    rows,
                    live_values,
                    types,
                )?;
                TerminalSuspensionLoanCapture::RetainedValue {
                    live_value: u32::try_from(position).map_err(|_| {
                        LoweringError::Unsupported(
                            "incoming loan index exceeds Terminal vocabulary",
                        )
                    })?,
                    path: graph.paths.insert_many(path),
                }
            }
            SuspensionCrossingLoanSource::LifetimeDependency { .. } => {
                TerminalSuspensionLoanCapture::InputDependencies
            }
            SuspensionCrossingLoanSource::RetainedInput { input, .. } => {
                let input = borrow.retained_borrow_inputs.get(*input);
                let path = borrow.call_loan_paths.span(input.capture_path).ok_or(
                    LoweringError::Unsupported(
                        "retained input capture path is unavailable at emission",
                    ),
                )?;
                let (operation, argument, path) = captured_call_leaf(
                    identities,
                    checked,
                    source_calls,
                    source_call_bindings,
                    module,
                    input.state_symbol,
                    input.call,
                    input.parameter_index,
                    path,
                )?;
                TerminalSuspensionLoanCapture::CallInput {
                    call: operation,
                    argument,
                    path: graph.paths.insert_many(path),
                    stored: input.source_kind == checked_trees::ViewReturnInputSource::Stored,
                }
            }
            SuspensionCrossingLoanSource::Stored { resource, .. } => {
                if relations.descriptor_transfer {
                    TerminalSuspensionLoanCapture::DescriptorTransfer
                } else if let CheckedParentBorrowResource::SuppliedOutput { resource } = resource {
                    let output = borrow.supplied_output_resources.get(*resource);
                    let path = output
                        .capture_path
                        .and_then(|path| borrow.call_loan_paths.span(path))
                        .ok_or(LoweringError::Unsupported(
                            "completed output capture path is unavailable at emission",
                        ))?;
                    let (operation, argument, path) = captured_call_leaf(
                        identities,
                        checked,
                        source_calls,
                        source_call_bindings,
                        module,
                        output.state_symbol,
                        output.call,
                        output.parameter_index,
                        path,
                    )?;
                    TerminalSuspensionLoanCapture::CompletedOutput {
                        call: operation,
                        argument,
                        path: graph.paths.insert_many(path),
                    }
                } else {
                    let mut captures = occurrence
                        .frontier_before_call
                        .loan_captures
                        .iter()
                        .filter(|capture| capture.loan == *source_loan);
                    if let Some(capture) = captures.next() {
                        if captures.next().is_some() {
                            return unsupported("suspension loan has multiple formation captures");
                        }
                        TerminalSuspensionLoanCapture::Reference {
                            formation: capture.formation,
                            carrier: capture.carrier,
                            source: capture.source.place,
                            source_access: capture.source.access,
                            source_path: graph
                                .paths
                                .insert_many(capture.source.path.iter().cloned()),
                        }
                    } else {
                        TerminalSuspensionLoanCapture::Unresolved
                    }
                }
            }
            SuspensionCrossingLoanSource::CapturedActual { resource, .. } => {
                let actual = borrow.call_loan_resources.get(*resource);
                let position = actual.parameter_index;
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
                let path = borrow.call_loan_paths.span(actual.capture_path).ok_or(
                    LoweringError::Unsupported(
                        "suspension actual capture path is unavailable at emission",
                    ),
                )?;
                let TerminalSuspensionValueType::Structural(root) =
                    live_values[live_value].1.value_type
                else {
                    return unsupported("suspension captured loan has no structural operand");
                };
                let path = lower_static_argument_path(identities, path, root, types)?;
                TerminalSuspensionLoanCapture::RetainedValue {
                    live_value: u32::try_from(live_value).map_err(|_| {
                        LoweringError::Unsupported(
                            "suspension actual index exceeds Terminal vocabulary",
                        )
                    })?,
                    path: graph.paths.insert_many(path),
                }
            }
        };
        // These are checked-row to emitted-row joins, not source-resource
        // judgments. A missing namespace edge refuses the complete lowering.
        let emitted_row = |source| {
            loan_identities
                .iter()
                .find(|(candidate, _, _)| *candidate == source)
                .map(|(_, _, target)| *target)
                .ok_or(LoweringError::Unsupported(
                    "checked suspension relation has no emitted occurrence",
                ))
        };
        let parent = if relations.parent.is_valid() {
            emitted_row(relations.parent)?
        } else {
            Handle::invalid()
        };
        let relation_links = |span| {
            carry
                .suspension_loan_relations
                .span(span)
                .ok_or(LoweringError::Unsupported(
                    "checked suspension relation span is unavailable",
                ))?
                .iter()
                .map(|source| emitted_row(*source))
                .collect::<Result<Vec<_>, _>>()
        };
        let suppliers = graph
            .supplier_links
            .insert_many(relation_links(relations.suppliers)?);
        let inputs = graph
            .input_links
            .insert_many(relation_links(relations.inputs)?);
        let resource = match relations.resource {
            Resource::IncomingReference => TerminalSuspensionLoanResource::IncomingReference,
            Resource::DirectRoot => TerminalSuspensionLoanResource::DirectRoot,
            Resource::Reborrow => TerminalSuspensionLoanResource::Reborrow,
            Resource::CompletedOutput => TerminalSuspensionLoanResource::CompletedOutput,
            Resource::DescriptorTransfer => TerminalSuspensionLoanResource::DescriptorTransfer,
            Resource::OutputInput => TerminalSuspensionLoanResource::OutputInput,
            Resource::LifetimeDependency => TerminalSuspensionLoanResource::LifetimeDependency,
        };
        let backing = match relations.backing {
            BackingPlan::StateValue { origin } => {
                let position = live_values
                    .iter()
                    .position(|(live, _)| live.origin == origin)
                    .ok_or(LoweringError::Unsupported(
                        "suspension checked backing has no emitted retained value",
                    ))?;
                TerminalSuspensionLoanBacking::RetainedOwner {
                    live_value: u32::try_from(position).map_err(|_| {
                        LoweringError::Unsupported(
                            "suspension backing index exceeds Terminal vocabulary",
                        )
                    })?,
                }
            }
            BackingPlan::IncomingReferent { identity } => {
                let (_, _, argument) = incoming_reference_binding(
                    identities,
                    checked,
                    identity,
                    rows,
                    live_values,
                    types,
                )?;
                TerminalSuspensionLoanBacking::IncomingReferent { argument }
            }
            BackingPlan::CapturedInput => TerminalSuspensionLoanBacking::CapturedInput,
            BackingPlan::CompletedOutput { loan } => {
                TerminalSuspensionLoanBacking::CompletedOutput {
                    loan: emitted_row(loan)?,
                }
            }
            BackingPlan::InputDependencies => TerminalSuspensionLoanBacking::InputDependencies,
        };
        *graph.occurrences.get_mut(*terminal) = TerminalSuspensionLoan {
            capture,
            backing,
            resource,
            parent,
            suppliers,
            inputs,
            access: match relations.access {
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

/// Translate the checked ingress permission into the existing parameter
/// presentation. Root borrowed parameters already present their referents;
/// contained descriptors require their own projection and dereference.
fn incoming_reference_binding(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    identity: Handle<checked_trees::IncomingReferenceIdentity>,
    loans: &[checked_trees::SuspensionCrossingLoanFact],
    live_values: &[(
        &checked_trees::SuspensionCrossingLiveValueFact,
        terminal_psi::TerminalSuspensionLiveValue,
    )],
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<
    (
        usize,
        Vec<terminal_psi::StructuralPathSegment>,
        terminal_psi::StructuralArgument,
    ),
    LoweringError,
> {
    let borrow = &checked.facts.borrow;
    if !borrow.incoming_references.is_valid(identity) {
        return unsupported("suspension incoming referent is unavailable");
    }
    let incoming = borrow.incoming_references.get(identity);
    let (position, (_, live)) = live_values.iter().enumerate().find(|(_, (source, _))| {
        matches!(source.origin, checked_trees::SuspensionCrossingValueOrigin::Parameter { symbol, .. }
            if symbol == incoming.parameter_symbol())
    }).ok_or(LoweringError::Unsupported("suspension ingress parameter is not retained"))?;
    let (
        TerminalSuspensionPlace::Structural(argument),
        TerminalSuspensionValueType::Structural(root),
    ) = (&live.place, live.value_type)
    else {
        return unsupported("suspension ingress permission has no structural argument");
    };
    let owner_path =
        borrow
            .owner_segments
            .span(incoming.owner_path)
            .ok_or(LoweringError::Unsupported(
                "suspension ingress descriptor path is unavailable",
            ))?;
    let path = validation::borrow_contracts::structural_owner_path(checked, owner_path).ok_or(
        LoweringError::Unsupported("suspension ingress path has no exact declaration selectors"),
    )?;
    let path = lower_static_argument_path(identities, &path, root, types)?;
    let mut referent = argument.clone();
    let access = loans
        .iter()
        .find_map(|loan| match &loan.source {
            checked_trees::SuspensionCrossingLoanSource::Incoming {
                identity: permission,
                access,
            } if *permission == identity && loan.active_at_call && loan.retained_for_crossing => {
                Some(access)
            }
            _ => None,
        })
        .ok_or(LoweringError::Unsupported(
            "suspension ingress access has no retained permission",
        ))?;
    referent.access = match access {
        checked_trees::BorrowAccessKind::Read => terminal_psi::StructuralAccess::SharedBorrow,
        checked_trees::BorrowAccessKind::Mutable => terminal_psi::StructuralAccess::MutableBorrow,
        checked_trees::BorrowAccessKind::WriteOnly => {
            terminal_psi::StructuralAccess::WriteOnlyBorrow
        }
    };
    referent.path.extend(path.iter().cloned());
    let root_borrow = matches!(
        incoming.origin,
        checked_trees::IncomingReferenceOrigin::ParameterReferent { .. }
    );
    if !root_borrow {
        referent
            .path
            .push(terminal_psi::StructuralPathSegment::Referent);
    }
    Ok((position, path, referent))
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
    let mut lane_position = parameters[..position]
        .iter()
        .filter(|parameter| {
            !parameter.relevance.is_erased()
                && checked
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
                    == scalar
        })
        .count();
    let (scalars, structural, structural_type) = if let Some(call) = direct_call_parts(operation) {
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
            callee
                .structural_parameters
                .get(lane_position)
                .map(|parameter| parameter.structural_type),
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
            callee
                .structural_parameters
                .get(lane_position)
                .map(|parameter| parameter.structural_type),
        )
    } else if let OperationKind::CallDynamic {
        selector,
        interface,
        slot,
        arguments,
        structural_arguments,
        ..
    } = operation
    {
        // The checked telescope includes the selector; the emitted data lane does not.
        if scalar && lane_position == 0 {
            return scalar_argument_binding(module, *selector);
        }
        if scalar {
            lane_position = lane_position
                .checked_sub(1)
                .ok_or(LoweringError::Unsupported(
                    "suspension dynamic scalar argument has no data lane",
                ))?;
        }
        let signature = module
            .dynamic_dispatch
            .interface(*interface)
            .and_then(|interface| interface.slots.get(usize::try_from(*slot).ok()?))
            .ok_or(LoweringError::Unsupported(
                "suspension dynamic call has no public slot",
            ))?;
        (
            arguments.as_slice(),
            structural_arguments.as_slice(),
            signature
                .structural_parameters
                .get(lane_position)
                .map(|parameter| parameter.structural_type),
        )
    } else {
        return unsupported("suspension call argument is not a call");
    };
    if scalar {
        let value = scalars
            .get(lane_position)
            .copied()
            .ok_or(LoweringError::Unsupported(
                "suspension call scalar argument has no emitted value",
            ))?;
        return scalar_argument_binding(module, value);
    }
    let argument = structural
        .get(lane_position)
        .ok_or(LoweringError::Unsupported(
            "suspension call structural argument has no emitted storage",
        ))?;
    let structural_type = structural_type.ok_or(LoweringError::Unsupported(
        "suspension call structural argument has no emitted formal",
    ))?;
    Ok((
        TerminalSuspensionPlace::Structural(argument.clone()),
        TerminalSuspensionValueType::Structural(structural_type),
    ))
}

fn scalar_argument_binding(
    module: &TerminalModule,
    value: ValueId,
) -> Result<(TerminalSuspensionPlace, TerminalSuspensionValueType), LoweringError> {
    let declaration = scalar_declaration(module, value).ok_or(LoweringError::Unsupported(
        "suspension call scalar argument has no emitted value",
    ))?;
    Ok((
        TerminalSuspensionPlace::Scalar(value),
        TerminalSuspensionValueType::Scalar(declaration.scalar_type),
    ))
}

/// A terminal transition into another state of the same composed recursive
/// component lowers to a successor edge, not a call: it adds no activation and
/// has no continuation in the transferring invocation. Its checked crossing
/// (and the carry checks it recorded) stays authoritative, but there is no
/// Terminal call operation to bind it to. The component's actual parking
/// points are the target states' own possibly-suspending calls, which keep
/// their crossings and plans. Stage 05's composed plan must name the edge: a
/// successor of the source state at the crossing's statement coordinate enters
/// the target. Every Terminal machine holding the source state must also hold
/// that target; any other missing occurrence refuses.
fn fused_component_transfer(
    checked: &CheckedTrees,
    state_blocks: &[lowered_psi::LoweredStateBlock],
    crossing: &checked_trees::SuspensionCrossingCarryFact,
) -> bool {
    let target = validation::call_target_entry_symbol(checked, crossing.target);
    let planned = crate::shared::checked_facts::flow_facts(checked)
        .terminal_unit_effects
        .composed_machines
        .iter()
        .flat_map(|plan| &plan.states)
        .filter(|state| state.state == crossing.state)
        .flat_map(|state| state.successors())
        .any(|successor| {
            successor.statement_ordinal as usize == crossing.statement_index
                && successor.target_state == target
        });
    let mut holders = state_blocks
        .iter()
        .filter(|block| block.state == crossing.state)
        .peekable();
    planned
        && holders.peek().is_some()
        && holders.all(|holder| {
            state_blocks
                .iter()
                .any(|block| block.machine == holder.machine && block.state == target)
        })
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
        OperationKind::CallDynamic {
            interface, slot, ..
        } => Some(terminal_psi::TerminalSuspensionCallTarget::Dynamic {
            interface: *interface,
            slot: *slot,
        }),
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
/// The crossing's own located call supplies the scope: the scan that found the
/// operation already names the block whose earlier declarations may hold it, so
/// the check must not search the module for the same operation again. A value
/// declared in another block is visible only when that block dominates the
/// call's block: a composed state's parameters live on its header block while
/// a guarded arm's call runs in the arm's own block.
fn scalar_value_available_before(
    machine: &terminal_psi::TerminalMachine,
    block_index: usize,
    operation_index: usize,
    value: ValueId,
) -> bool {
    let block = &machine.blocks[block_index];
    if machine
        .parameters
        .iter()
        .chain(block.parameters.iter())
        .chain(
            block.operations[..operation_index]
                .iter()
                .filter_map(|candidate| candidate.result.scalar_ref()),
        )
        .any(|declaration| declaration.id == value)
    {
        return true;
    }
    let Some(definition) = machine.blocks.iter().find(|candidate| {
        candidate
            .parameters
            .iter()
            .chain(
                candidate
                    .operations
                    .iter()
                    .filter_map(|operation| operation.result.scalar_ref()),
            )
            .any(|declaration| declaration.id == value)
    }) else {
        return false;
    };
    definition.id != block.id
        && terminal_semantics::control_graph::dominators(machine).dominates(definition.id, block.id)
}

/// Each required source call keeps one connection to its emitted arguments.
/// Current crossings and older saved inputs share that connection, so neither
/// can accidentally read the arguments from a different invocation.
#[derive(Default)]
struct SourceCallBindings {
    calls: Vec<SourceCallBinding>,
}

struct SourceCallBinding {
    coordinate: lowered_psi::LoweredStatementCallCoordinate,
    requested_target: symbols::SymbolHandle,
    occurrence_index: usize,
    location: (usize, usize, usize),
    operation: semantic_vocabulary::OperationId,
    target: terminal_psi::TerminalSuspensionCallTarget,
    arguments: Vec<(
        usize,
        (TerminalSuspensionPlace, TerminalSuspensionValueType),
    )>,
}

impl SourceCallBindings {
    /// Bind only calls demanded by this lowering. Unrelated declaration and
    /// external sites need not have an ordinary continuing call operation.
    fn bind(
        &mut self,
        checked: &CheckedTrees,
        source_calls: &[LoweredSourceCallOccurrence],
        module: &TerminalModule,
        state: symbols::SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
        target: symbols::SymbolHandle,
    ) -> Result<usize, LoweringError> {
        let coordinate = lowered_psi::LoweredStatementCallCoordinate {
            state,
            statement_index,
            call_ordinal,
        };
        if let Some(position) = self.calls.iter().position(|binding| {
            binding.coordinate == coordinate && binding.requested_target == target
        }) {
            return Ok(position);
        }
        let mut occurrences = source_calls.iter().enumerate().filter(|(_, occurrence)| {
            occurrence.matches_statement(state, statement_index, call_ordinal)
        });
        let (occurrence_index, occurrence) = occurrences.next().ok_or(
            LoweringError::Unsupported("required source call has no emitted occurrence"),
        )?;
        if occurrences.next().is_some()
            || owning_callable_symbol(checked, occurrence.source_target)
                != owning_callable_symbol(checked, target)
        {
            return unsupported("required source call is duplicate or redirected");
        }
        let operation = occurrence
            .terminal_site
            .operation()
            .ok_or(LoweringError::Unsupported(
                "required source call has no normal call operation",
            ))?;
        let (machine_index, block_index, operation_index, emitted) = module
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
            .find(|(_, _, _, candidate)| candidate.id == operation)
            .ok_or(LoweringError::Unsupported(
                "required source call operation is absent",
            ))?;
        let terminal_target = terminal_call_target(&emitted.kind).ok_or(
            LoweringError::Unsupported("required source call operation is not a call"),
        )?;
        let position = self.calls.len();
        self.calls.push(SourceCallBinding {
            coordinate,
            requested_target: target,
            occurrence_index,
            location: (machine_index, block_index, operation_index),
            operation,
            target: terminal_target,
            arguments: Vec::new(),
        });
        Ok(position)
    }
}

impl SourceCallBinding {
    /// Prepare the requested formal lane once from this call's actual operand.
    /// Erased or otherwise unused formals do not acquire runtime obligations.
    fn argument(
        &mut self,
        checked: &CheckedTrees,
        module: &TerminalModule,
        position: usize,
    ) -> Result<(TerminalSuspensionPlace, TerminalSuspensionValueType), LoweringError> {
        if let Some((_, binding)) = self
            .arguments
            .iter()
            .find(|(formal, _)| *formal == position)
        {
            return Ok(binding.clone());
        }
        let (machine_index, block_index, operation_index) = self.location;
        let operation =
            &module.machines[machine_index].blocks[block_index].operations[operation_index];
        let binding = call_argument_binding(
            checked,
            self.requested_target,
            &operation.kind,
            module,
            position,
        )?;
        self.arguments.push((position, binding.clone()));
        Ok(binding)
    }
}

/// Use the arguments recorded for the original call, even when a later
/// argument or statement replaced the variable that once supplied them.
fn captured_call_leaf(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    source_calls: &[LoweredSourceCallOccurrence],
    source_call_bindings: &mut SourceCallBindings,
    module: &TerminalModule,
    state: symbols::SymbolHandle,
    call: Handle<checked_trees::FlowCallFact>,
    position: usize,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Result<
    (
        semantic_vocabulary::OperationId,
        terminal_psi::StructuralArgument,
        Vec<terminal_psi::StructuralPathSegment>,
    ),
    LoweringError,
> {
    let call = checked.facts.flow.control.calls.get(call);
    let binding = source_call_bindings.bind(
        checked,
        source_calls,
        module,
        state,
        call.statement_index,
        call.call_ordinal,
        call.target_symbol,
    )?;
    let binding = &mut source_call_bindings.calls[binding];
    let operation = binding.operation;
    let (
        TerminalSuspensionPlace::Structural(argument),
        TerminalSuspensionValueType::Structural(root),
    ) = binding.argument(checked, module, position)?
    else {
        return unsupported("captured output/input has no structural argument");
    };
    let path = lower_static_argument_path(identities, path, root, &module.structural_types)?;
    Ok((operation, argument, path))
}
