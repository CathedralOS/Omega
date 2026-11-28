//! Preparing a child retains its selected body and the values that can
//! survive an actual park. These are portable storage shapes; only the
//! target can determine the byte size of an emitted stack frame.
use language_core::sealed_intrinsics::ActivationOperation;
use semantic_vocabulary::MachineId;
use terminal_psi::*;
use wire::fnv1a::Fnv1a;

type Report<T> = Result<T, &'static str>;

fn text(module: &TerminalModule, id: TerminalIdentityIndex) -> Report<&str> {
    module
        .identities
        .text(id)
        .filter(|s| !s.is_empty())
        .ok_or("activation has an absent type or declaration identity")
}

fn fingerprint(tag: u8, bytes: &[u8]) -> u64 {
    let mut hash = Fnv1a::new();
    hash.byte(tag);
    hash.bytes_framed(bytes);
    hash.finish_or(1)
}

fn frame_id(module: &TerminalModule, machine: &TerminalMachine) -> Report<u64> {
    let declaration = machine
        .declaration
        .as_ref()
        .ok_or("a prepared worker calls a machine with no checked body")?;
    let mut hash = Fnv1a::new();
    hash.byte(0x57);
    hash.string_terminated(text(module, declaration.callable)?);
    hash.string_terminated(text(module, declaration.state.identity())?);
    Ok(hash.finish_or(1))
}

fn signature(
    machine: &TerminalMachine,
) -> Report<(Vec<TerminalTaskSignatureValue>, TerminalTaskSignatureValue)> {
    let declaration = machine
        .declaration
        .as_ref()
        .ok_or("prepared worker has no checked declaration")?;
    machine.validate_parameter_order()?;
    if machine.parameter_order.len() != declaration.parameter_types.len() {
        return Err("prepared worker needs exact source types for its runtime arguments");
    }
    let mut scalars = machine.parameters.iter();
    let mut structural = machine.structural_parameters.iter();
    let mut inputs = Vec::with_capacity(machine.parameter_order.len());
    for (kind, identity) in machine
        .parameter_order
        .iter()
        .zip(&declaration.parameter_types)
    {
        let (value_type, is_mutable) = match kind {
            BoundaryParameterKind::Scalar => (
                TerminalTaskValueType::Scalar(
                    scalars
                        .next()
                        .ok_or("worker lost a scalar input")?
                        .scalar_type,
                ),
                false,
            ),
            BoundaryParameterKind::Structural => {
                let p = structural.next().ok_or("worker lost a structural input")?;
                (
                    TerminalTaskValueType::Structural(p.structural_type),
                    matches!(
                        p.access,
                        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                    ),
                )
            }
        };
        inputs.push(TerminalTaskSignatureValue {
            value_type,
            normalized_identity: *identity,
            is_mutable,
        });
    }
    let value_type = match &machine.result {
        TerminalMachineResult::Unit => TerminalTaskValueType::Unit,
        TerminalMachineResult::Scalar(result) => TerminalTaskValueType::Scalar(result.scalar_type),
        TerminalMachineResult::Structural(result) => {
            TerminalTaskValueType::Structural(result.structural_type)
        }
    };
    Ok((
        inputs,
        TerminalTaskSignatureValue {
            value_type,
            normalized_identity: declaration.result_type,
            is_mutable: false,
        },
    ))
}

fn resident(
    module: &TerminalModule,
    ty: TerminalSuspensionValueType,
) -> Report<TerminalTaskResidentValue> {
    let (value_type, normalized_identity) = match ty {
        TerminalSuspensionValueType::Scalar(scalar) => {
            let key = super::scalar_identity(scalar)
                .ok_or("prepared worker parks with a scalar lacking a closed storage type")?;
            let id = module
                .identities
                .index_of(&key)
                .ok_or("the closed scalar type of a parked value is not in the module")?;
            (TerminalTaskValueType::Scalar(scalar), id)
        }
        TerminalSuspensionValueType::Structural(id) => {
            let ty = module
                .structural_type(id)
                .ok_or("parked value has no structural type")?;
            (TerminalTaskValueType::Structural(id), ty.identity)
        }
    };
    Ok(TerminalTaskResidentValue {
        value_type,
        normalized_identity,
    })
}

fn child(
    module: &TerminalModule,
    callee: TargetMachineCallable,
    children: &mut Vec<MachineId>,
) -> Report<()> {
    match callee {
        TargetMachineCallable::Machine(id) => children.push(id),
        TargetMachineCallable::Boundary(id) => {
            let call = module
                .boundary_machine(id)
                .and_then(|b| b.declaration.as_ref())
                .ok_or("prepared frame has an undeclared boundary call")?;
            if call.execution
                != CheckedBoundaryExecution::OwnedActivation(ActivationOperation::Park)
            {
                return Err("prepared frame calls a boundary with no retained execution plan");
            }
        }
    }
    Ok(())
}

/// Project one selected worker's reachable frames. The source compiler calls
/// this after publishing suspension plans; receivers can recompute the same
/// report without source or a scheduler provider. An unresolved call is never
/// silently counted as an empty frame.
pub fn caller_demand(
    module: &TerminalModule,
    entry: TerminalPreparedActivationEntry,
) -> Report<TerminalTaskActivationDemand> {
    let root = entry
        .selected_body(module)
        .ok_or("prepare selected a different worker or storage plan")?;
    let declaration = root
        .declaration
        .as_ref()
        .ok_or("prepared worker has no checked body header")?;
    let prepare = module
        .boundary_machine(entry.preparation)
        .and_then(|b| b.declaration.as_ref())
        .and_then(|d| d.source.specialization.as_ref())
        .ok_or("preparation has no closed application")?;
    let (signature_arguments, signature_outcome) = signature(root)?;
    let root_frame = frame_id(module, root)?;
    let mut report = Fnv1a::new();
    report.byte(0x59);
    report.bytes(&prepare.commitment);
    report.bytes(&declaration.contract);
    let mut queue = vec![root.id];
    let mut seen = Vec::<MachineId>::new();
    let mut frames = Vec::new();
    let mut crossings = Vec::new();
    while let Some(id) = queue.pop() {
        let Err(at) = seen.binary_search(&id) else {
            continue;
        };
        seen.insert(at, id);
        let body = module
            .machine(id)
            .ok_or("prepared frame names an absent body")?;
        let header = body
            .declaration
            .as_ref()
            .ok_or("prepared frame has no checked declaration")?;
        let identity = frame_id(module, body)?;
        report.u64(identity);
        report.bytes(&header.contract);
        let mut children = Vec::new();
        let mut frontiers = Vec::new();
        for block in &body.blocks {
            for op in &block.operations {
                if let Some(call) = op.declared_invocation() {
                    child(module, call.callee, &mut children)?;
                } else if let OperationKind::CallMachine { contract, .. } = op.kind {
                    // Every actual machine value must resolve to a descriptor in
                    // this closed catalog under this exact contract. Size for
                    // all constructible alternatives, even when the argument
                    // is supplied later and the unique value is unknown.
                    let mut choices = module
                        .execution
                        .callables
                        .descriptors
                        .iter()
                        .filter(|descriptor| descriptor.contract == contract)
                        .peekable();
                    if choices.peek().is_none() {
                        return Err(
                            "prepared frame has no checked descriptor for its callable argument",
                        );
                    }
                    children.extend(choices.map(|descriptor| descriptor.machine));
                } else if matches!(op.kind, OperationKind::CallDynamic { .. }) {
                    return Err("prepared frame has a dynamic call with no selected body bound");
                }
                let Some(crossing) = op.suspension_crossing else {
                    continue;
                };
                let key = (op.id, crossing);
                let plans = &module.execution.suspension_call_plans;
                let index = plans
                    .binary_search_by_key(&key, |p| (p.operation, p.crossing))
                    .map_err(|_| "prepared frame lost its suspension plan")?;
                let plan = &plans[index];
                report.bytes(&suspension_frontier_commitment(&module.identities, plan));
                let mut values = Vec::with_capacity(plan.live_values.len());
                let mut carry = Vec::with_capacity(plan.live_values.len());
                for live in &plan.live_values {
                    let value = resident(module, live.value_type)?;
                    carry.push(TerminalTaskLiveCarry {
                        place: live.place.report_identity(&module.identities, identity),
                        value_type: fingerprint(
                            0x54,
                            text(module, value.normalized_identity)?.as_bytes(),
                        ),
                        storage: match live.storage {
                            TerminalSuspensionStorage::Parameter => {
                                TerminalTaskCarryStorage::Parameter
                            }
                            TerminalSuspensionStorage::CallArgument => {
                                TerminalTaskCarryStorage::CallArgument
                            }
                            TerminalSuspensionStorage::Local => TerminalTaskCarryStorage::Local,
                        },
                        claims: live.claims.iter().map(|claim| claim.get()).collect(),
                    });
                    values.push(value);
                }
                crossings.push(TerminalTaskSuspensionCrossing {
                    identity: crossing.get(),
                    live_carry: carry,
                });
                frontiers.push(values);
            }
            if let Some((_, call)) = block.terminator.declared_invocation() {
                child(module, call.callee, &mut children)?;
            }
            for (_, actions) in block.terminator.cleanup_rosters() {
                children.extend(
                    actions
                        .iter()
                        .filter_map(TerminalAffineCleanupAction::invoked_machine),
                );
            }
        }
        children.sort_unstable();
        children.dedup();
        let calls = children
            .iter()
            .map(|id| {
                Ok(TerminalTaskFrameCall {
                    callee: frame_id(module, module.machine(*id).ok_or("callee frame is absent")?)?,
                })
            })
            .collect::<Report<Vec<_>>>()?;
        queue.extend(children.into_iter().rev());
        // Before the first step, arguments occupy the frame. At a park, the
        // exact published frontier replaces them; the alternatives do not add.
        let (inputs, _) = signature(body)?;
        frontiers.insert(
            0,
            inputs
                .into_iter()
                .map(|v| TerminalTaskResidentValue {
                    value_type: v.value_type,
                    normalized_identity: v.normalized_identity,
                })
                .collect(),
        );
        let state = header.state.identity();
        frames.push(TerminalTaskFrame {
            identity,
            machine_callable_identity: header.callable,
            entry: state,
            states: vec![TerminalTaskFrameState {
                state,
                resident: Vec::new(),
                crossing_arguments: frontiers,
            }],
            calls,
            unresolved_calls: Vec::new(),
        });
    }
    crossings.sort_unstable_by_key(|c| c.identity);
    if crossings.windows(2).any(|c| c[0].identity == c[1].identity) {
        return Err("prepared frames repeat a suspension occurrence");
    }
    Ok(TerminalTaskActivationDemand {
        origin: TerminalActivationOrigin::Caller(entry),
        report_fingerprint: report.finish_or(1),
        machine_callable_identity: declaration.callable,
        entry: declaration.state.identity(),
        entry_report_fingerprint: root_frame,
        contract_report_fingerprint: fingerprint(0x43, &declaration.contract),
        specialization_commitment: prepare.commitment,
        specialization_report_fingerprint: fingerprint(0x53, &prepare.commitment),
        may_suspend: declaration.published_may_suspend,
        may_block: declaration.published_may_block,
        cancellation_required: false,
        signature_arguments,
        signature_outcome,
        root_frame,
        frames,
        crossings,
    })
}
