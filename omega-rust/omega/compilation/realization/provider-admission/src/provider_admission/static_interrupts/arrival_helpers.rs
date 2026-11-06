//! Check that an arrival helper consumes this handler's acknowledgement.
//!
//! Equal token layouts are insufficient. Calls must forward the unchanged
//! arrival value, context views must name the installation's actual context,
//! and completion must return normally before its permit can be finished.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use language_core::static_interrupts::StaticInterruptOperation;
use semantic_vocabulary::{IdTable, MachineId, OperationId, PsiSemanticId, StructuralTypeId};
use terminal_psi::{
    OperationKind, StructuralAccess, StructuralArgument, StructuralTypeShape,
    TerminalInstalledInterruptRegistration, TerminalMachine, TerminalModule,
};
use terminal_semantics::descriptor_results::{
    self, DescriptorMachine, ValueIdentityAnalysis, ValueIdentitySeed,
};

/// A checked association, still awaiting installation backing and publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ArrivalHelperUse {
    pub installation: OperationId,
    pub callback_application: u32,
    pub operation: OperationId,
    pub kind: StaticInterruptOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Token {
    Acknowledgement,
    Permit {
        machine: MachineId,
        begin: OperationId,
        completed: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Invocation {
    machine: MachineId,
    parameters: Vec<(usize, Token)>,
}

struct Root<'a> {
    registration: &'a TerminalInstalledInterruptRegistration,
    context: StructuralTypeId,
    completion: MachineId,
}

/// Validate each registered handler independently. Sharing a helper body does
/// not merge installations: every incoming invocation must satisfy its own
/// context and completion selection.
pub(super) fn validate(
    module: &TerminalModule,
    occurrences: &[backend_plan::TerminalCallbackOccurrenceProposal],
    profile: target::TargetProfile,
    calling: &[calling_conventions::BoundaryCallingApplication],
    targets: Option<&terminal_installation::AdmittedTargetMachineSelections>,
    reached_operations: &[OperationId],
    reached_machines: &[MachineId],
) -> Result<Vec<ArrivalHelperUse>, String> {
    if module.installed_interrupt_registrations.is_empty() {
        return Ok(Vec::new());
    }
    let bodies = flat_rows(module.machines.iter().map(|body| (body.id, body)).collect())?;
    let reached_operations = membership(reached_operations)?;
    let reached_machines = membership(reached_machines)?;
    let occurrences: BTreeMap<_, _> = occurrences
        .iter()
        .map(|occurrence| (occurrence.application_index(), occurrence))
        .collect();
    let registrations: Vec<_> = module
        .installed_interrupt_registrations
        .iter()
        .filter(|registration| {
            reached_operations
                .get(registration.operation)
                .is_some_and(Option::is_some)
                || registration.callbacks.iter().any(|callback| {
                    occurrences
                        .get(&callback.application_index)
                        .is_some_and(|occurrence| {
                            reached_machines
                                .get(occurrence.callback_thunk_artifact().entry())
                                .is_some_and(Option::is_some)
                        })
                })
        })
        .collect();
    let roots = registrations
        .iter()
        .flat_map(|registration| &registration.callbacks)
        .map(|callback| {
            occurrences
                .get(&callback.application_index)
                .map(|occurrence| occurrence.callback_thunk_artifact().entry())
                .ok_or("installed handler has no retained executable occurrence")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let graph = descriptor_results::terminal_reference_graph_from(module, roots)
        .map_err(|error| format!("arrival reference closure is incomplete: {error:?}"))?;
    let mut uses = Vec::new();
    let mut identities: IdTable<MachineId, Option<ValueIdentityAnalysis<'_>>> =
        bodies.map(|_, _| None);
    let mut orders: IdTable<MachineId, Option<ExecutionOrder<'_>>> = bodies.map(|_, _| None);
    for registration in registrations {
        let acknowledgement_entry =
            acknowledgement_entry(module, registration, profile, calling, targets)?;
        let declaration = module
            .boundary_machine(registration.installer)
            .ok_or("installation lost its core boundary")?;
        if declaration
            .declaration
            .as_ref()
            .and_then(|declaration| declaration.execution.static_interrupt_operation())
            != Some(StaticInterruptOperation::Install)
            || declaration.structural_parameters.len() != 6
        {
            return Err("installation does not have the authenticated six-operand contract".into());
        }
        let root = Root {
            registration,
            context: declaration.structural_parameters[5].structural_type,
            completion: selected_body(module, registration.operation, 3)?,
        };
        let completion = bodies
            .get(root.completion)
            .and_then(|body| *body)
            .ok_or("installation completion body is missing")?;
        if completion.result != terminal_psi::TerminalMachineResult::Unit
            || !completion.contract.crash_routes.is_empty()
        {
            return Err(
                "installation completion must return Unit without a recoverable crash route".into(),
            );
        }
        for callback in &registration.callbacks {
            let occurrence = occurrences
                .get(&callback.application_index)
                .ok_or("installed handler has no retained executable occurrence")?;
            if occurrence.terminal_operation() != registration.operation {
                return Err("installed handler occurrence names another installation".into());
            }
            let body = bodies
                .get(occurrence.callback_thunk_artifact().entry())
                .and_then(|body| *body)
                .ok_or("installed handler body is missing")?;
            if !body.declaration.as_ref().is_some_and(|declaration| {
                declaration.same_identity_as(&module.identities, &callback.body, &module.identities)
            }) {
                return Err("installed arrival changed its exact checked handler body".into());
            }
            // The entry admission separately authenticates the exact owned Ack
            // contract. Non-Ack, nonreturning exception entries carry no token.
            let parameters = if callback.application_index == acknowledgement_entry {
                if body.structural_parameters.len() != 1
                    || body.structural_parameters[0].access != StructuralAccess::Owned
                    || !is_acknowledgement(module, body.structural_parameters[0].structural_type)
                {
                    return Err("the selected timer slot does not supply the exact owned acknowledgement contract".into());
                }
                vec![(0, Token::Acknowledgement)]
            } else {
                Vec::new()
            };
            check_closure(
                module,
                &graph,
                &mut identities,
                &mut orders,
                &bodies,
                &root,
                callback.application_index,
                Invocation {
                    machine: body.id,
                    parameters,
                },
                &mut uses,
            )?;
        }
    }
    uses.sort_by_key(|row| (row.installation, row.callback_application, row.operation));
    uses.dedup();
    Ok(uses)
}

fn check_closure<'a>(
    module: &'a TerminalModule,
    graph: &'a [DescriptorMachine],
    identities: &mut IdTable<MachineId, Option<ValueIdentityAnalysis<'a>>>,
    orders: &mut IdTable<MachineId, Option<ExecutionOrder<'a>>>,
    bodies: &IdTable<MachineId, Option<&'a TerminalMachine>>,
    root: &Root<'_>,
    callback_application: u32,
    initial: Invocation,
    uses: &mut Vec<ArrivalHelperUse>,
) -> Result<(), String> {
    let mut pending = VecDeque::from([initial]);
    let mut visited = BTreeSet::new();
    while let Some(invocation) = pending.pop_front() {
        if !visited.insert(invocation.clone()) {
            continue;
        }
        let body = bodies
            .get(invocation.machine)
            .and_then(|body| *body)
            .ok_or("arrival call body is missing")?;
        let identity = identities
            .get_mut(invocation.machine)
            .ok_or("arrival identity has no body slot")?;
        if identity.is_none() {
            *identity =
                Some(ValueIdentityAnalysis::new(module, graph, body.id).map_err(str::to_owned)?);
        }
        let identity = identity.as_mut().expect("prepared arrival identity");
        let order = orders
            .get_mut(invocation.machine)
            .ok_or("arrival execution order has no body slot")?;
        if order.is_none() {
            *order = Some(ExecutionOrder::new(body)?);
        }
        let order = order.as_ref().expect("prepared arrival execution order");
        let mut births = Vec::new();
        // Birth recognition is independent of block storage order. Identity
        // replay still requires the producer to have completed on every path.
        for operation in body.operations() {
            if operation_kind(module, &operation.kind)
                == Some(StaticInterruptOperation::BeginCompletion)
            {
                let actual = only_argument(&operation.kind)?;
                require_ack(identity, &invocation.parameters, operation.id, actual)?;
                if selected_body(module, operation.id, 0)? != root.completion {
                    return Err(format!(
                        "completion at {:?} selects another installation's controller",
                        operation.id
                    ));
                }
                births.push(operation.id);
            }
        }
        for operation in body.operations() {
            if let Some(kind) = operation_kind(module, &operation.kind) {
                let fail = |reason: &str| format!("arrival helper {:?}: {reason}", operation.id);
                if kind == StaticInterruptOperation::Install {
                    return Err(fail(
                        "a live handler cannot replace its installed interrupt table",
                    ));
                }
                let actual = only_argument(&operation.kind)?;
                match kind {
                    StaticInterruptOperation::Context => {
                        require_ack(identity, &invocation.parameters, operation.id, actual)?;
                        let result = operation
                            .result
                            .as_structural()
                            .ok_or_else(|| fail("context view has no reference result"))?;
                        if !matches!(module.structural_type(result.structural_type).map(|row| &row.shape),
                            Some(StructuralTypeShape::Reference { referent, access: StructuralAccess::SharedBorrow })
                            if *referent == root.context)
                        {
                            return Err(fail(
                                "context view does not name this installation's retained context",
                            ));
                        }
                    }
                    StaticInterruptOperation::BeginCompletion => {}
                    StaticInterruptOperation::FinishCompletion => {
                        let begin = births.iter().copied().find(|begin| {
                            identity
                                .require(ValueIdentitySeed::Result(*begin), operation.id, actual)
                                .is_ok()
                        });
                        if let Some(begin) = begin {
                            require_completed_controller(
                                identity,
                                order,
                                &invocation.parameters,
                                root.completion,
                                Some(begin),
                                operation.id,
                            )?;
                        } else {
                            let inherited =
                                invocation.parameters.iter().any(|&(ordinal, token)| {
                                    let Token::Permit { completed, .. } = token else {
                                        return false;
                                    };
                                    identity
                                        .require(
                                            ValueIdentitySeed::Parameter(ordinal),
                                            operation.id,
                                            actual,
                                        )
                                        .is_ok()
                                        && (completed
                                            || require_completed_controller(
                                                identity,
                                                order,
                                                &invocation.parameters,
                                                root.completion,
                                                None,
                                                operation.id,
                                            )
                                            .is_ok())
                                });
                            if !inherited {
                                return Err(fail(
                                    "permit has no unchanged admitted begin and completed controller",
                                ));
                            }
                        }
                    }
                    StaticInterruptOperation::Install => unreachable!(),
                }
                uses.push(ArrivalHelperUse {
                    installation: root.registration.operation,
                    callback_application,
                    operation: operation.id,
                    kind,
                });
            }
            if let Some(call) = terminal_psi::direct_call_parts(&operation.kind) {
                let mut parameters = Vec::new();
                for (ordinal, actual) in call.structural_arguments.iter().enumerate() {
                    for &(source, token) in &invocation.parameters {
                        if identity
                            .require(ValueIdentitySeed::Parameter(source), operation.id, actual)
                            .is_ok()
                        {
                            let token = match token {
                                Token::Permit {
                                    machine,
                                    begin,
                                    completed,
                                } => Token::Permit {
                                    machine,
                                    begin,
                                    completed: completed
                                        || require_completed_controller(
                                            identity,
                                            order,
                                            &invocation.parameters,
                                            root.completion,
                                            None,
                                            operation.id,
                                        )
                                        .is_ok(),
                                },
                                token => token,
                            };
                            parameters.push((ordinal, token));
                        }
                    }
                    for &begin in &births {
                        if identity
                            .require(ValueIdentitySeed::Result(begin), operation.id, actual)
                            .is_ok()
                        {
                            parameters.push((
                                ordinal,
                                Token::Permit {
                                    machine: body.id,
                                    begin,
                                    completed: require_completed_controller(
                                        identity,
                                        order,
                                        &invocation.parameters,
                                        root.completion,
                                        Some(begin),
                                        operation.id,
                                    )
                                    .is_ok(),
                                },
                            ));
                        }
                    }
                }
                parameters.sort();
                parameters.dedup();
                pending.push_back(Invocation {
                    machine: call.callee,
                    parameters,
                });
            }
        }
    }
    Ok(())
}

fn require_ack(
    identity: &mut ValueIdentityAnalysis<'_>,
    parameters: &[(usize, Token)],
    operation: OperationId,
    actual: &StructuralArgument,
) -> Result<(), String> {
    if parameters.iter().any(|&(ordinal, token)| {
        token == Token::Acknowledgement
            && identity
                .require(ValueIdentitySeed::Parameter(ordinal), operation, actual)
                .is_ok()
    }) {
        Ok(())
    } else {
        Err(format!(
            "arrival helper {operation:?} does not receive this arrival's unchanged acknowledgement"
        ))
    }
}

struct ExecutionOrder<'a> {
    positions: IdTable<OperationId, Option<(semantic_vocabulary::BlockId, usize)>>,
    dominance: terminal_semantics::control_graph::DominatorTree,
    calls: IdTable<MachineId, Option<Vec<(OperationId, &'a [StructuralArgument])>>>,
}

impl<'a> ExecutionOrder<'a> {
    fn new(body: &'a TerminalMachine) -> Result<Self, String> {
        let positions = flat_rows(
            body.blocks
                .iter()
                .flat_map(|block| {
                    block
                        .operations
                        .iter()
                        .enumerate()
                        .map(move |(ordinal, operation)| (operation.id, (block.id, ordinal)))
                })
                .collect(),
        )?;
        let mut calls = Vec::new();
        for operation in body.operations() {
            // Dominance describes normal block edges. A handled crash must
            // never count as the controller's successful completion.
            if !matches!(&operation.kind, OperationKind::CallUnit { crash_continuations, .. }
                if crash_continuations.is_empty())
            {
                continue;
            }
            if let Some(call) = terminal_psi::direct_call_parts(&operation.kind) {
                calls.push((call.callee, operation.id, call.structural_arguments));
            }
        }
        calls.sort_by_key(|(callee, operation, _)| (*callee, *operation));
        let mut grouped: Vec<(MachineId, Vec<(OperationId, &'a [StructuralArgument])>)> =
            Vec::new();
        for (callee, operation, actuals) in calls {
            if let Some((_, rows)) = grouped
                .last_mut()
                .filter(|(previous, _)| *previous == callee)
            {
                rows.push((operation, actuals));
            } else {
                grouped.push((callee, vec![(operation, actuals)]));
            }
        }
        Ok(Self {
            positions,
            dominance: terminal_semantics::control_graph::dominators(body),
            calls: flat_rows(grouped)?,
        })
    }

    fn precedes(&self, first: OperationId, second: OperationId) -> bool {
        let lookup = |operation| self.positions.get(operation).copied().flatten();
        let (Some((first_block, first_ordinal)), Some((second_block, second_ordinal))) =
            (lookup(first), lookup(second))
        else {
            return false;
        };
        if first_block == second_block {
            first_ordinal < second_ordinal
        } else {
            self.dominance.dominates(first_block, second_block)
        }
    }
}

fn require_completed_controller(
    identity: &mut ValueIdentityAnalysis<'_>,
    order: &ExecutionOrder<'_>,
    parameters: &[(usize, Token)],
    completion: MachineId,
    begin: Option<OperationId>,
    finish: OperationId,
) -> Result<(), String> {
    if order
        .calls
        .get(completion)
        .and_then(Option::as_ref)
        .is_some_and(|calls| {
            calls.iter().any(|&(operation, actuals)| {
                actuals.len() == 1
                    && begin.is_none_or(|begin| order.precedes(begin, operation))
                    && order.precedes(operation, finish)
                    && require_ack(identity, parameters, operation, &actuals[0]).is_ok()
            })
        })
    {
        Ok(())
    } else {
        Err(format!(
            "completion permit at {finish:?} has no normally completed selected controller call after {begin:?}"
        ))
    }
}

/// Body-local rosters may retain gaps after pruning. Keep those gaps explicit
/// so lookup is an ID offset, without treating a neighboring row as a match.
fn flat_rows<Id: PsiSemanticId, T>(
    mut rows: Vec<(Id, T)>,
) -> Result<IdTable<Id, Option<T>>, String> {
    rows.sort_unstable_by_key(|(identity, _)| *identity);
    if rows.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err("arrival helper roster repeats an identity".into());
    }
    let Some((first, last)) = rows.first().zip(rows.last()) else {
        return Ok(IdTable::default());
    };
    let first = first.0.get();
    let length = last
        .0
        .get()
        .checked_sub(first)
        .and_then(|distance| distance.checked_add(1))
        .and_then(|length| usize::try_from(length).ok())
        .ok_or("arrival helper identity span cannot be indexed")?;
    let mut slots = Vec::new();
    slots
        .try_reserve_exact(length)
        .map_err(|_| "arrival helper identity span cannot be allocated")?;
    slots.resize_with(length, || None);
    for (identity, row) in rows {
        let offset = usize::try_from(identity.get() - first)
            .map_err(|_| "arrival helper identity cannot be indexed")?;
        slots[offset] = Some(row);
    }
    IdTable::from_rows(slots.into_iter().enumerate().map(|(offset, row)| {
        (
            Id::new(first + offset as u64).expect("validated nonzero identity interval"),
            row,
        )
    }))
    .map_err(|_| "arrival helper identity interval is inconsistent".into())
}

fn membership<Id: PsiSemanticId>(identities: &[Id]) -> Result<IdTable<Id, Option<()>>, String> {
    let mut identities = identities.to_vec();
    identities.sort_unstable();
    identities.dedup();
    flat_rows(
        identities
            .into_iter()
            .map(|identity| (identity, ()))
            .collect(),
    )
}

fn operation_kind(
    module: &TerminalModule,
    kind: &OperationKind,
) -> Option<StaticInterruptOperation> {
    let OperationKind::BoundaryCall { boundary, .. } = kind else {
        return None;
    };
    module
        .boundary_machine(*boundary)?
        .declaration
        .as_ref()?
        .execution
        .static_interrupt_operation()
}

fn only_argument(kind: &OperationKind) -> Result<&StructuralArgument, String> {
    let [argument] = terminal_psi::call_structural_arguments(kind) else {
        return Err("arrival helper must retain its one token actual".into());
    };
    Ok(argument)
}

fn is_acknowledgement(module: &TerminalModule, ty: StructuralTypeId) -> bool {
    module.boundary_machines.iter().any(|boundary| {
        boundary.declaration.as_ref().and_then(|declaration| declaration.execution.static_interrupt_operation())
            == Some(StaticInterruptOperation::Context)
            && matches!(boundary.structural_parameters.as_slice(), [parameter] if parameter.structural_type == ty)
    })
}

pub(super) fn selected_body(
    module: &TerminalModule,
    operation: OperationId,
    ordinal: u32,
) -> Result<MachineId, String> {
    let mut selections = module
        .callback_applications
        .iter()
        .filter(|row| row.registrar == operation && row.static_machine_ordinal == ordinal);
    let (Some(selection), None) = (selections.next(), selections.next()) else {
        return Err(format!(
            "operation {operation:?} has no unique selected helper at ordinal {ordinal}"
        ));
    };
    let mut bodies = module.machines.iter().filter(|body| {
        body.declaration.as_ref().is_some_and(|declaration| {
            declaration.same_identity_as(&module.identities, &selection.body, &module.identities)
        })
    });
    let (Some(body), None) = (bodies.next(), bodies.next()) else {
        return Err(format!(
            "operation {operation:?} selected helper has no unique checked body"
        ));
    };
    Ok(body.id)
}

/// Select the sole callback whose actual destination is the request's exact
/// slot application. A sibling with an identical Ack signature is not a birth.
pub(super) fn acknowledgement_entry(
    module: &TerminalModule,
    registration: &TerminalInstalledInterruptRegistration,
    profile: target::TargetProfile,
    calling: &[calling_conventions::BoundaryCallingApplication],
    targets: Option<&terminal_installation::AdmittedTargetMachineSelections>,
) -> Result<u32, String> {
    let boundary =
        crate::selected_calling_boundary(module, targets, registration.registrar_boundary)?;
    let abi = module.calling_abi_identity(boundary)?;
    let mut candidates = calling.iter().filter(|row| {
        row.abi == abi
            && row.calling_owner_application.as_deref()
                == Some(registration.registrar_application.as_str())
    });
    let (Some(registrar), None) = (candidates.next(), candidates.next()) else {
        return Err("installation lost its unique selected registrar Calling application".into());
    };
    if registrar.target != profile.native_target()
        || registrar.recomputed_commitment()? != registrar.commitment
    {
        return Err("installation registrar Calling application changed target or payload".into());
    }
    let telescope = calling_conventions::TerminalCallingTelescope::for_application(
        module, boundary, registrar,
    )?;
    let layouts = registrar
        .signature
        .callback_layouts
        .span(registrar.signature.callback_layout_rows)
        .ok_or("installation registrar has no complete callback layout roster")?;
    let mut selected = None;
    for callback in &registration.callbacks {
        let mut binders = telescope.binders.iter().filter(|binder| {
            binder.static_machine_ordinal == callback.static_machine_ordinal
                && binder.requirement_identity == callback.requirement
        });
        let (Some(binder), None) = (binders.next(), binders.next()) else {
            return Err("installed callback has no unique nominal registrar binder".into());
        };
        let mut materializations = registrar
            .plan
            .plan()
            .call
            .callback_materializations
            .iter()
            .filter(|row| row.binder == binder.binder);
        let (Some(materialization), None) = (materializations.next(), materializations.next())
        else {
            return Err("installed callback has no unique selected materialization".into());
        };
        if !matches!(
            materialization.destination,
            calling_conventions::NativePlace::Field { .. }
        ) {
            continue;
        }
        let mut destinations = layouts
            .iter()
            .filter(|layout| layout.destination == materialization.destination);
        let (Some(layout), None) = (destinations.next(), destinations.next()) else {
            return Err("installed callback has no unique selected destination layout".into());
        };
        let slot =
            calling_conventions::callback_destination_slot(module, profile, registrar, layout)?;
        if slot.application_commitment == registration.slot_commitment {
            if slot.application_encoding.as_ref() != registration.slot_encoding.as_slice() {
                return Err(
                    "installed acknowledgement slot changed its complete application".into(),
                );
            }
            if selected.replace(callback.application_index).is_some() {
                return Err(
                    "installation names more than one acknowledgement-producing destination".into(),
                );
            }
        }
    }
    selected.ok_or_else(|| "installation has no acknowledgement-producing destination".into())
}
