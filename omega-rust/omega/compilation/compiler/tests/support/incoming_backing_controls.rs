//! Compare real terminal input sources with the callee-formal substitution.
//! These are symbolic rows, not allocation or execution authority.
use semantic_vocabulary::{InvocationSite, MachineId, OperationId};
use std::path::Path;
use target_operations::{CallSiteOwner, TargetControlTerminator};
use terminal_semantics::borrowed_storage::Projection;
use terminal_semantics::descriptor_lifetimes::descriptor_location;
use terminal_semantics::descriptor_results::{
    DescriptorActualInputs, DescriptorAnalysis, DescriptorCallOrigin, DescriptorSourceOrigin,
    descriptor_contents_query, descriptor_machine,
};

pub(super) fn inspect(
    report: &compilation_products::CompileReport,
    fixture: &Path,
) -> Result<(), String> {
    let oracle = fixture.join("expected_native_input_backing_controls.txt");
    if !oracle.exists() {
        return Ok(());
    }
    if std::fs::read_to_string(oracle)
        .map_err(|error| error.to_string())?
        .trim()
        != "required"
    {
        return Err("native input-backing control oracle must say required".into());
    }
    let object = report
        .retained_native_artifact()
        .ok_or("input-backing control needs an actual native product")?
        .object();
    let rows = object
        .native_instruction_source_rows()
        .ok_or("input-backing control lost native source")?;
    let current = rows.frame_application.source().source();
    let source = current.optimized_target().optimized().plan();
    let targeted = current.optimized_target().target_operations();
    let mut candidates = Vec::new();
    for caller in &targeted.functions {
        for block in &caller.graph.blocks {
            if let TargetControlTerminator::InvokeNoNormal {
                psi_edge,
                callee,
                arguments,
                ..
            } = &block.terminator
            {
                let declaration = source
                    .functions
                    .iter()
                    .find(|body| body.machine == *callee)
                    .ok_or("actual no-normal callee is missing")?;
                if arguments.len() == 1
                    && declaration.structural_parameters.len() == 1
                    && declaration.result.structural().is_some()
                    && arguments[0].access != terminal_psi::StructuralAccess::Owned
                {
                    candidates.push((
                        caller.machine,
                        *psi_edge,
                        *callee,
                        declaration.structural_parameters[0].place,
                    ));
                }
            }
        }
    }
    let [(caller, edge, callee, formal)] = candidates.as_slice() else {
        return Err("control needs one actual borrowed no-normal aggregate-result call".into());
    };
    let caller_body = source
        .functions
        .iter()
        .find(|body| body.machine == *caller)
        .ok_or("control lost its source caller")?;
    let mut source_sites = Vec::new();
    for (index, block) in caller_body.block_entries.iter().enumerate() {
        let end = caller_body
            .block_entries
            .get(index + 1)
            .map_or(caller_body.operations.len(), |next| next.operation_offset);
        for (offset, operation) in caller_body.operations[block.operation_offset..end]
            .iter()
            .enumerate()
        {
            if let Some(invocation) = operation.module_invocation() {
                if invocation.site == InvocationSite::InvokeNoNormal(*edge) {
                    source_sites.push((
                        (
                            block.block,
                            u32::try_from(offset).map_err(|_| "control position overflow")?,
                        ),
                        invocation,
                    ));
                }
            }
        }
    }
    let [(position, invocation)] = source_sites.as_slice() else {
        return Err("control lost its exact source terminal cut".into());
    };
    if invocation.callee != *callee || invocation.structural_arguments.len() != 1 {
        return Err("control target differs from its real source invocation".into());
    }
    let actual = &invocation.structural_arguments[0];
    let origin = DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
        source: *formal,
        projection: Projection::new(&[]),
    });
    let observed = object
        .observe_invocation_input_sources(
            *caller,
            CallSiteOwner::InvokeNoNormal(*edge),
            *callee,
            &origin,
        )
        .map_err(|error| format!("actual no-normal input: {error}"))?;
    let graph =
        optimization_unit_semantics::descriptor_results::abstract_borrowed_storage_catalog_graph(
            &source.functions,
            &source.boundary_machines,
            source.structural_types.as_slice(),
            &source.dynamic_dispatch,
        )
        .ok_or("control needs complete source descriptor graph")?;
    let caller_graph =
        descriptor_machine(&graph, *caller).ok_or("control source caller is absent")?;
    let root = *caller_graph
        .place_types
        .get(&actual.place)
        .ok_or("control actual type is absent")?;
    let location = descriptor_location(
        source.structural_types.as_slice(),
        actual.place,
        root,
        &actual.path,
        None,
    )
    .ok_or("control actual location is unresolved")?;
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(source.structural_types.as_slice(), &graph);
    let expected = analysis
        .symbolic_source_origins(*caller, *position, &location)
        .ok_or("control actual has no current source observation")?;
    if observed != expected || observed.is_empty() {
        return Err("no-normal input lost its exact pre-transfer source/version".into());
    }
    let owners = caller_body
        .operations
        .iter()
        .filter_map(|operation| match operation {
            abstract_operations::AbstractOperation::EstablishReference { source, .. } => {
                Some(source)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [owner] = owners.as_slice() else {
        return Err("control needs its original single Word borrow".into());
    };
    if observed.iter().any(|origin| !matches!(origin, DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument { source, projection }) if *source == owner.place && projection.path == owner.path && projection.exact && !projection.indirect)) {
        return Err("no-normal input names a different caller backing".into());
    }
    for (owner, site, target) in [
        (
            MachineId::new(u64::MAX).unwrap(),
            CallSiteOwner::InvokeNoNormal(*edge),
            *callee,
        ),
        (
            *caller,
            CallSiteOwner::Operation(OperationId::new(edge.get()).unwrap()),
            *callee,
        ),
        (
            *caller,
            CallSiteOwner::InvokeNoNormal(*edge),
            MachineId::new(u64::MAX).unwrap(),
        ),
    ] {
        if object
            .observe_invocation_input_sources(owner, site, target, &origin)
            .is_ok()
        {
            return Err("input observation accepted a wrong caller/site/callee".into());
        }
    }
    if analysis
        .substitute_input_source(
            *caller,
            *position,
            *callee,
            DescriptorActualInputs {
                arguments: &[],
                argument_contents: &[],
            },
            &origin,
        )
        .is_some()
    {
        return Err("input substitution accepted missing formal actuals".into());
    }
    let locations = [Some(location)];
    let contents = [descriptor_contents_query(
        source.structural_types.as_slice(),
        actual.place,
        root,
        &actual.path,
        None,
    )];
    let missing = (
        semantic_vocabulary::BlockId::new(u64::MAX).unwrap(),
        u32::MAX,
    );
    if analysis
        .substitute_input_source(
            *caller,
            missing,
            *callee,
            DescriptorActualInputs {
                arguments: &locations,
                argument_contents: &contents,
            },
            &origin,
        )
        .is_some()
    {
        return Err("input substitution accepted an absent snapshot".into());
    }
    Ok(())
}
