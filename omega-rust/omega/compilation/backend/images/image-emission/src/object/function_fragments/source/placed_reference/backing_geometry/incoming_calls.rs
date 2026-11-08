//! Keep each selected incoming call, including terminal transfers. The source
//! snapshot supplies actual contents; a no-normal edge supplies no return facts.
mod source_sites;
use abstract_operations::AbstractOperationPlan;
use semantic_vocabulary::{InvocationSite, MachineId};
use source_sites::{SourceCallTarget, SourceSite, SourceSites};
use target_operations::{
    CallSiteOwner, TargetControlTerminator, TargetOperationPlan, TargetUnitOperation,
};
use terminal_semantics::descriptor_lifetimes::{
    DescriptorContentsQuery, DescriptorLocation, DescriptorPosition, descriptor_location,
};
use terminal_semantics::descriptor_results::{
    DescriptorActualInputs, DescriptorAnalysis, DescriptorMachine, DescriptorSourceOrigin,
    descriptor_contents_query, descriptor_machine,
};

pub(super) struct IncomingCalls {
    rows: Vec<IncomingCall>,
}
struct IncomingCall {
    caller: MachineId,
    callee: MachineId,
    site: CallSiteOwner,
    inputs: Result<Inputs, &'static str>,
}
enum Inputs {
    Normal(DescriptorPosition),
    Entry {
        position: DescriptorPosition,
        arguments: Vec<Option<DescriptorLocation>>,
        contents: Vec<Option<DescriptorContentsQuery>>,
    },
}
impl IncomingCalls {
    pub(super) fn new(
        source: &AbstractOperationPlan,
        targeted: &TargetOperationPlan,
        graph: &[DescriptorMachine],
    ) -> Self {
        let mut rows = Vec::new();
        let source_sites = SourceSites::new(source);
        for caller in &targeted.functions {
            for operation in caller.graph.operations() {
                match operation {
                    TargetUnitOperation::Call {
                        target,
                        psi_operation,
                        ..
                    } => {
                        let Some(callee) = target.machine() else {
                            continue;
                        };
                        let inputs = normal(graph, caller.machine, *psi_operation, callee)
                            .map(Inputs::Normal);
                        rows.push(IncomingCall {
                            caller: caller.machine,
                            callee,
                            site: CallSiteOwner::Operation(*psi_operation),
                            inputs,
                        });
                    }
                    TargetUnitOperation::CleanupCall { owner, callee, .. } => {
                        // Keep the caller in the census even though cleanup has
                        // no source descriptor observation we can substitute.
                        rows.push(IncomingCall {
                            caller: caller.machine,
                            callee: *callee,
                            site: *owner,
                            inputs: Err("cleanup arrival has no reconstructed backing observation"),
                        });
                    }
                    TargetUnitOperation::InstallStaticInterrupt {
                        psi_operation,
                        calls,
                        ..
                    } => {
                        for call in calls {
                            rows.push(IncomingCall {
                                caller: caller.machine, callee: call.callee,
                                site: CallSiteOwner::Installation { operation: *psi_operation, role: call.role },
                                inputs: Err("installation helper arrival has no reconstructed backing observation"),
                            });
                        }
                    }
                    _ => {}
                }
            }
            for block in &caller.graph.blocks {
                match &block.terminator {
                    TargetControlTerminator::InvokeNoNormal {
                        psi_edge,
                        callee,
                        arguments,
                        ..
                    } => {
                        let inputs = source_sites.get(caller.machine, InvocationSite::InvokeNoNormal(*psi_edge)).and_then(|site| {
                            if !matches!(site.target, SourceCallTarget::Machine(source_callee) if source_callee == *callee) {
                                return Err("no-normal source and target callee disagree");
                            }
                            // Existing call-ABI replay owns formal types and native
                            // placements. Match its exact logical actuals here before
                            // reading the caller's state at this transfer.
                            if site.arguments.len() != arguments.len() || site.arguments.iter().zip(arguments).any(|(source, target)| source.place != target.place || source.path != target.path || source.access != target.access) {
                                return Err("no-normal source and target argument transport disagree");
                            }
                            entry(source, graph, caller.machine, *callee, site)
                        });
                        rows.push(IncomingCall {
                            caller: caller.machine,
                            callee: *callee,
                            site: CallSiteOwner::InvokeNoNormal(*psi_edge),
                            inputs,
                        });
                    }
                    TargetControlTerminator::ExternalCompletion {
                        psi_edge,
                        boundary,
                        kind,
                        binding:
                            target_operations::ExternalCompletionBinding::Checked {
                                callee, origin, ..
                            },
                        structural_arguments,
                        ..
                    } => {
                        let inputs = source_sites.get(caller.machine, InvocationSite::ExternalCompletion(*psi_edge)).and_then(|site| {
                            if !matches!(site.target, SourceCallTarget::Boundary { boundary: source_boundary, kind: source_kind } if source_boundary == *boundary && source_kind == *kind)
                                || site.arguments != structural_arguments
                                || !selected_origin(origin, *boundary, *callee)
                            {
                                return Err("external completion differs from its selected source binding");
                            }
                            entry(source, graph, caller.machine, *callee, site)
                        });
                        rows.push(IncomingCall {
                            caller: caller.machine,
                            callee: *callee,
                            site: CallSiteOwner::ExternalCompletion(*psi_edge),
                            inputs,
                        });
                    }
                    _ => {}
                }
            }
        }
        rows.sort_by_key(|row| (row.callee, row.caller, row.site));
        Self { rows }
    }
    pub(super) fn index(
        &self,
        caller: MachineId,
        site: CallSiteOwner,
        callee: MachineId,
    ) -> Result<usize, &'static str> {
        let mut matches = self.range(callee).filter(|index| {
            let row = &self.rows[*index];
            row.caller == caller && row.site == site
        });
        let (Some(index), None) = (matches.next(), matches.next()) else {
            return Err("invocation input has no unique selected caller, site and callee");
        };
        Ok(index)
    }
    pub(super) fn range(&self, callee: MachineId) -> std::ops::Range<usize> {
        self.rows.partition_point(|row| row.callee < callee)
            ..self.rows.partition_point(|row| row.callee <= callee)
    }
    pub(super) fn substitute(
        &self,
        index: usize,
        analysis: &mut DescriptorAnalysis<'_>,
        origin: &DescriptorSourceOrigin,
    ) -> Result<(MachineId, Vec<DescriptorSourceOrigin>), &'static str> {
        let row = self
            .rows
            .get(index)
            .ok_or("incoming backing site is absent")?;
        let result = match row.inputs.as_ref().map_err(|reason| *reason)? {
            Inputs::Normal(position) => {
                analysis.substitute_call_source(row.caller, *position, origin)
            }
            Inputs::Entry {
                position,
                arguments,
                contents,
            } => analysis.substitute_input_source(
                row.caller,
                *position,
                row.callee,
                DescriptorActualInputs {
                    arguments,
                    argument_contents: contents,
                },
                origin,
            ),
        }
        .ok_or("call argument has no complete backing substitution")?;
        if result.is_empty() {
            return Err("call argument has no backing alternatives");
        }
        Ok((row.caller, result))
    }
}
fn normal(
    graph: &[DescriptorMachine],
    caller: MachineId,
    operation: semantic_vocabulary::OperationId,
    callee: MachineId,
) -> Result<DescriptorPosition, &'static str> {
    let caller =
        descriptor_machine(graph, caller).ok_or("selected caller has no reference-origin graph")?;
    let mut calls = caller
        .calls
        .iter()
        .filter(|(_, call)| call.operation == operation);
    let (Some((position, call)), None) = (calls.next(), calls.next()) else {
        return Err("selected call has no unique reference-origin observation");
    };
    if call.callee() != Some(callee) {
        return Err("selected target differs from the reference-origin call");
    }
    Ok(*position)
}
fn selected_origin(
    origin: &target_operations::NativeCallOrigin,
    boundary: semantic_vocabulary::BoundaryMachineId,
    callee: MachineId,
) -> bool {
    match origin {
        target_operations::NativeCallOrigin::Authored => false,
        target_operations::NativeCallOrigin::ProgramEntry {
            boundary: actual,
            entry,
            ..
        } => *actual == boundary && *entry == callee,
        target_operations::NativeCallOrigin::TargetFamily {
            boundary: actual,
            member,
            ..
        } => *actual == boundary && *member == callee,
        target_operations::NativeCallOrigin::InstalledProvider {
            boundary: actual,
            provider,
            ..
        } => *actual == boundary && provider.boundary == boundary && provider.candidate == callee,
    }
}
fn entry(
    source: &AbstractOperationPlan,
    graph: &[DescriptorMachine],
    caller: MachineId,
    callee: MachineId,
    site: &SourceSite<'_>,
) -> Result<Inputs, &'static str> {
    let declaration = source
        .functions
        .iter()
        .find(|body| body.machine == callee)
        .ok_or("terminal callee has no abstract declaration")?;
    if site.arguments.len() != declaration.structural_parameters.len() {
        return Err("terminal source and callee argument arity disagree");
    }
    let position = site.position?;
    let caller =
        descriptor_machine(graph, caller).ok_or("terminal caller has no descriptor graph")?;
    if !caller.flow.contains_key(&position) {
        return Err("terminal call lost its current descriptor snapshot");
    }
    let mut locations = Vec::with_capacity(site.arguments.len());
    let mut contents = Vec::with_capacity(site.arguments.len());
    for actual in site.arguments {
        let root = caller
            .place_types
            .get(&actual.place)
            .copied()
            .ok_or("terminal actual has no declared structural type")?;
        locations.push(descriptor_location(
            source.structural_types.as_slice(),
            actual.place,
            root,
            &actual.path,
            None,
        ));
        contents.push(descriptor_contents_query(
            source.structural_types.as_slice(),
            actual.place,
            root,
            &actual.path,
            None,
        ));
    }
    Ok(Inputs::Entry {
        position,
        arguments: locations,
        contents,
    })
}
