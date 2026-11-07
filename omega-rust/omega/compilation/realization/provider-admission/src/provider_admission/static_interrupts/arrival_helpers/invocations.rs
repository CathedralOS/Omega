//! Actual invocation operands and selected executable alternatives.
//! Code-retention roots are not calls. Opaque calls keep an explicit empty
//! target roster; a caller must not treat that absence as token completion.

use std::collections::BTreeSet;

use semantic_vocabulary::{IdTable, InvocationSite, MachineId};
use terminal_installation::SynchronousCallGraph;
use terminal_psi::{
    OperationKind, OperationResult, StructuralArgument, TerminalAffineCleanupAction,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
};

pub(super) struct Call<'a> {
    pub site: Option<InvocationSite>,
    pub callees: Vec<MachineId>,
    pub arguments: &'a [StructuralArgument],
    pub normal_return: bool,
}

pub(super) struct Calls<'a> {
    bodies: IdTable<MachineId, Option<BodyCalls<'a>>>,
}

struct BodyCalls<'a> {
    calls: Vec<Call<'a>>,
    has_helpers: bool,
}

impl<'a> Calls<'a> {
    pub fn new(
        module: &'a TerminalModule,
        bodies: &IdTable<MachineId, Option<&'a TerminalMachine>>,
        graph: &SynchronousCallGraph<'a>,
        roots: impl IntoIterator<Item = MachineId>,
    ) -> Result<Self, String> {
        let mut pending: Vec<_> = roots.into_iter().collect();
        let mut prepared = bodies.map(|_, _| None);
        while let Some(machine) = pending.pop() {
            if prepared.get(machine).is_some_and(Option::is_some) {
                continue;
            }
            let body = bodies
                .get(machine)
                .copied()
                .flatten()
                .ok_or_else(|| format!("arrival invocation body {machine:?} is missing"))?;
            let mut calls = Vec::new();
            for block in &body.blocks {
                for invocation in block.module_invocations() {
                    let callee = bodies
                        .get(invocation.inputs.callee)
                        .copied()
                        .flatten()
                        .ok_or("arrival direct invocation has no exact body")?;
                    calls.push(Call {
                        site: Some(invocation.site),
                        callees: vec![callee.id],
                        arguments: invocation.inputs.structural_arguments,
                        normal_return: matches!(invocation.site, InvocationSite::Operation(_))
                            && callee.result == TerminalMachineResult::Unit
                            && invocation.inputs.crash_continuations.is_empty(),
                    });
                }
                for operation in &block.operations {
                    let (callees, arguments, crash_free) = match &operation.kind {
                        OperationKind::BoundaryCall {
                            boundary,
                            structural_arguments,
                            crash_continuations,
                            ..
                        } => {
                            // Core arrival operations have their own token
                            // checks. They are not opaque provider delegation.
                            if super::operation_kind(module, &operation.kind).is_some() {
                                continue;
                            }
                            (
                                graph.boundary_callee(*boundary)?.into_iter().collect(),
                                structural_arguments.as_slice(),
                                crash_continuations.is_empty(),
                            )
                        }
                        OperationKind::CallDynamic {
                            interface,
                            slot,
                            family_actuals,
                            structural_arguments,
                            crash_continuations,
                            ..
                        } => {
                            let mut callees = graph.dynamic_callees(
                                *interface,
                                *slot,
                                !family_actuals.is_empty(),
                            )?;
                            callees.sort_unstable();
                            callees.dedup();
                            // Preserve an unresolved route in the inventory.
                            // The invocation-specific token check refuses it
                            // when it would otherwise discard an Ack/permit;
                            // unrelated calls cannot establish helper facts.
                            (
                                callees,
                                structural_arguments.as_slice(),
                                crash_continuations.is_empty(),
                            )
                        }
                        _ => continue,
                    };
                    calls.push(Call {
                        site: Some(InvocationSite::Operation(operation.id)),
                        callees,
                        arguments,
                        normal_return: operation.result == OperationResult::Unit && crash_free,
                    });
                }
                if let Terminator::ExternalCompletion {
                    edge,
                    boundary,
                    structural_arguments,
                    ..
                } = &block.terminator
                {
                    calls.push(Call {
                        site: Some(InvocationSite::ExternalCompletion(*edge)),
                        callees: graph.boundary_callee(*boundary)?.into_iter().collect(),
                        arguments: structural_arguments,
                        normal_return: false,
                    });
                }
                for (_, actions) in block.terminator.cleanup_rosters() {
                    for action in actions {
                        if let TerminalAffineCleanupAction::InvokeNominal(cleanup) = action {
                            // A cleanup receiver is a borrowed destruction
                            // subject, not an ordinary Ack/permit actual. No
                            // token identity is invented from its home/path.
                            // Its body is still checked, with no seeded token.
                            calls.push(Call {
                                site: None,
                                callees: vec![cleanup.cleanup_machine],
                                arguments: &[],
                                normal_return: false,
                            });
                        }
                    }
                }
            }
            for call in &calls {
                if call.site.is_some() {
                    for &callee in &call.callees {
                        let callee = bodies
                            .get(callee)
                            .copied()
                            .flatten()
                            .ok_or("arrival invocation lost its exact callee")?;
                        if callee.structural_parameters.len() != call.arguments.len()
                            || callee
                                .structural_parameters
                                .iter()
                                .zip(call.arguments)
                                .any(|(formal, actual)| formal.access != actual.access)
                        {
                            return Err(format!(
                                "arrival invocation {:?} changed its actual/formal roster",
                                call.site,
                            ));
                        }
                    }
                }
            }
            pending.extend(calls.iter().flat_map(|call| call.callees.iter()).copied());
            let has_helpers = body.operations().any(|operation| {
                super::operation_kind(module, &operation.kind).is_some_and(|kind| {
                    kind != language_core::static_interrupts::StaticInterruptOperation::Install
                })
            });
            *prepared
                .get_mut(machine)
                .ok_or("arrival invocation has no body slot")? =
                Some(BodyCalls { calls, has_helpers });
        }
        Ok(Self { bodies: prepared })
    }

    pub fn get(&self, machine: MachineId) -> Result<&[Call<'a>], String> {
        self.bodies
            .get(machine)
            .and_then(Option::as_ref)
            .map(|body| body.calls.as_slice())
            .ok_or_else(|| format!("arrival invocation roster for {machine:?} is missing"))
    }

    /// Descriptor replay belongs only to handler invocation closures. Other
    /// reached functions are indexed for incoming-call coverage, not analyzed
    /// for unrelated reference facts.
    pub fn reachable_from(
        &self,
        roots: impl IntoIterator<Item = MachineId>,
    ) -> Result<Vec<MachineId>, String> {
        let mut pending: Vec<_> = roots.into_iter().collect();
        let mut reached = self.bodies.map(|_, _| false);
        while let Some(machine) = pending.pop() {
            let seen = reached
                .get_mut(machine)
                .ok_or("arrival closure has no body slot")?;
            if !std::mem::replace(seen, true) {
                pending.extend(
                    self.get(machine)?
                        .iter()
                        .flat_map(|call| &call.callees)
                        .copied(),
                );
            }
        }
        Ok(reached
            .iter()
            .filter_map(|(machine, &seen)| seen.then_some(machine))
            .collect())
    }

    /// One good caller cannot cover a different, unvalidated incoming route
    /// merely because both eventually execute the same helper operation.
    pub fn require_associated_calls(
        &self,
        reached: &IdTable<MachineId, Option<()>>,
        associated: &BTreeSet<(MachineId, Option<InvocationSite>, MachineId)>,
    ) -> Result<(), String> {
        let mut reverse = self.bodies.map(|_, _| Vec::new());
        let mut helpers = self.bodies.map(|_, _| false);
        let mut pending = Vec::new();
        for (caller, body) in self.bodies.iter() {
            let Some(body) = body else {
                continue;
            };
            if body.has_helpers {
                pending.push(caller);
            }
            for call in &body.calls {
                for &callee in &call.callees {
                    reverse
                        .get_mut(callee)
                        .ok_or("arrival incoming call has no callee slot")?
                        .push(caller);
                }
            }
        }
        while let Some(callee) = pending.pop() {
            let seen = helpers
                .get_mut(callee)
                .ok_or("arrival helper has no body slot")?;
            if !std::mem::replace(seen, true) {
                let callers = reverse
                    .get(callee)
                    .ok_or("arrival reverse call has no body slot")?;
                pending.extend(callers.iter().copied());
            }
        }
        for (caller, body) in self.bodies.iter() {
            if !reached.get(caller).is_some_and(Option::is_some) {
                continue;
            }
            let Some(body) = body else {
                continue;
            };
            for call in &body.calls {
                for &callee in &call.callees {
                    if helpers.get(callee) == Some(&true)
                        && !associated.contains(&(caller, call.site, callee))
                    {
                        return Err(format!(
                            "arrival invocation {:?} from {caller:?} to {callee:?} has no registered token association",
                            call.site,
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}
