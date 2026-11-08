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
            if let Some(invocation) = operation.module_invocation()
                && invocation.site == InvocationSite::InvokeNoNormal(*edge)
            {
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

/// Check a local array slice without treating its descriptor as its backing.
pub(super) fn inspect_container(
    report: &compilation_products::CompileReport,
    fixture: &Path,
) -> Result<(), String> {
    let oracle = fixture.join("expected_native_inline_container_controls.txt");
    if !oracle.exists() {
        return Ok(());
    }
    if std::fs::read_to_string(oracle)
        .map_err(|error| error.to_string())?
        .trim()
        != "required"
    {
        return Err("inline-container control oracle must say required".into());
    }
    let object = report
        .retained_native_artifact()
        .ok_or("container control needs a real native object")?
        .object();
    let rows = object
        .native_instruction_source_rows()
        .ok_or("container control lost native source")?;
    let current = rows.frame_application.source().source();
    let source = current.optimized_target().optimized().plan();
    let types = source.structural_types.as_slice();
    let target = current.optimized_target().target_operations();
    let graph =
        optimization_unit_semantics::descriptor_results::abstract_borrowed_storage_catalog_graph(
            &source.functions,
            &source.boundary_machines,
            types,
            &source.dynamic_dispatch,
        )
        .ok_or("container control needs a complete descriptor graph")?;
    let mut calls = Vec::new();
    for caller in &target.functions {
        for row in caller.graph.operations() {
            let target_operations::TargetUnitOperation::Call {
                target,
                psi_operation,
                ..
            } = row
            else {
                continue;
            };
            let Some(callee) = target.machine() else {
                continue;
            };
            let declaration = source
                .functions
                .iter()
                .find(|row| row.machine == callee)
                .ok_or("selected callee is absent")?;
            let [formal] = declaration.structural_parameters.as_slice() else {
                continue;
            };
            if terminal_psi::find_structural_type(types, formal.structural_type).is_some_and(
                |row| {
                    matches!(
                        row.shape,
                        terminal_psi::StructuralTypeShape::ElementView { .. }
                    )
                },
            ) {
                let formal_origin =
                    DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                        source: formal.place,
                        projection: Projection::new(&[]),
                    });
                let origins = object
                    .observe_invocation_input_sources(
                        caller.machine,
                        CallSiteOwner::Operation(*psi_operation),
                        callee,
                        &formal_origin,
                    )
                    .map_err(|error| error.to_string())?;
                let body = descriptor_machine(&graph, caller.machine)
                    .ok_or("actual slice caller is absent")?;
                // Forwarding calls also take slices. This control needs the call
                // that first borrows the actual array field in its caller.
                let field_window = !origins.is_empty()
                    && origins.iter().all(|origin| {
                        let DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                            source: place,
                            projection,
                        }) = origin
                        else {
                            return false;
                        };
                        let Some(root) = body.place_types.get(place) else {
                            return false;
                        };
                        !projection.exact
                            && !projection.indirect
                            && terminal_semantics::inline_backing_container(
                                types, *root, projection,
                            )
                            .is_some_and(|path| {
                                matches!(
                                    path.as_slice(),
                                    [terminal_psi::StructuralPathSegment::Field(_)]
                                )
                            })
                    });
                if field_window {
                    calls.push((caller.machine, *psi_operation, callee, formal, origins));
                }
            }
        }
    }
    let [(caller, _operation, _callee, formal, origins)] = calls.as_slice() else {
        return Err("control needs one actual array-field slice call".into());
    };
    let body = descriptor_machine(&graph, *caller).ok_or("actual slice caller is absent")?;
    for origin in origins.iter().cloned() {
        let DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
            source: place,
            projection,
        }) = origin
        else {
            return Err("slice source is not its caller's backing".into());
        };
        let root = *body
            .place_types
            .get(&place)
            .ok_or("slice backing type is absent")?;
        if projection.exact || projection.indirect {
            return Err("control did not retain its inexact direct field window".into());
        }
        let before = projection.clone();
        let container = terminal_semantics::inline_backing_container(types, root, &projection)
            .ok_or("real local window lost its inline container")?;
        if !matches!(
            container.as_slice(),
            [terminal_psi::StructuralPathSegment::Field(_)]
        ) || !terminal_semantics::runtime_structural_path_tip(types.iter(), root, &container)
            .is_some_and(|row| {
                matches!(
                    row.shape,
                    terminal_psi::StructuralTypeShape::FixedArray { .. }
                )
            })
            || projection != before
        {
            return Err(
                "container changed the captured window or lost its inline array field".into(),
            );
        }
        let allocated = rows
            .selected
            .function(*caller)
            .ok_or("window caller has no selected body")?
            .local_storage_slots
            .iter()
            .filter(|slot| slot.id.structural_place() == Some(place))
            .collect::<Vec<_>>();
        if !matches!(allocated.as_slice(), [slot] if slot.byte_size >= 20) {
            return Err("window source has no real record-and-array allocation".into());
        }
        let mut indirect = projection.clone();
        indirect.indirect = true;
        let mut outside = container.clone();
        outside.push(terminal_psi::StructuralPathSegment::FixedIndex(u64::MAX));
        if terminal_semantics::inline_backing_container(types, root, &indirect).is_some()
            || terminal_semantics::inline_backing_container(
                types,
                semantic_vocabulary::StructuralTypeId::new(u64::MAX).unwrap(),
                &projection,
            )
            .is_some()
            || terminal_semantics::inline_backing_container(types, root, &Projection::new(&outside))
                .is_some()
            || terminal_semantics::inline_backing_container(
                types,
                formal.structural_type,
                &Projection::new(&[]),
            )
            .is_some()
        {
            return Err(
                "container accepted an indirect, unknown, out-of-range or view-metadata source"
                    .into(),
            );
        }
    }
    if let Some(pointer) = types.iter().find(|row| {
        matches!(
            row.shape,
            terminal_psi::StructuralTypeShape::Reference { .. }
        )
    }) {
        if terminal_semantics::inline_backing_container(types, pointer.id, &Projection::new(&[]))
            .is_some()
            || terminal_semantics::inline_backing_container(
                types,
                pointer.id,
                &Projection::new(&[terminal_psi::StructuralPathSegment::Referent]),
            )
            .is_some()
        {
            return Err("container treated reference metadata as its pointee".into());
        }
        eprintln!("inline-container controls: reference metadata checked");
    } else {
        eprintln!(
            "inline-container controls: no reference declaration; required view-metadata checks passed"
        );
    }
    Ok(())
}
