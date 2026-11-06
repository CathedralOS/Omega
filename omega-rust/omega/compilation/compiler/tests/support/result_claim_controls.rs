//! Exercise possible result slots separately from the live claims a return
//! actually transfers. Every malformed record goes through normal ingress.
use std::collections::BTreeSet;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{
    OperationResult, StructuralPathSegment as Path, StructuralResultClaimBinding, TerminalModule,
    Terminator,
};

#[derive(Clone, Copy)]
struct CallSite {
    machine: usize,
    block: usize,
    operation: usize,
}

pub(super) fn inspect(artifact: &CanonicalTerminalArtifact) -> Result<(), String> {
    let module = artifact.semantic_module();
    let (site, active, unused) = find_guarded_slots(module)
        .ok_or("result controls need a call with an unused declared case")?;
    let original = module.machines[site.machine].blocks[site.block].operations[site.operation]
        .result
        .as_structural()
        .ok_or("result control lost its structural result")?;
    let active_path = original.claims[active].path.clone();
    let unused_path = original.claims[unused].path.clone();
    let case_position = unused_path
        .iter()
        .position(|part| matches!(part, Path::Case(_)))
        .ok_or("unused result slot has no case guard")?;
    let (_, parent) = terminal_semantics::canonical_static_projection(
        module.structural_types.iter(),
        original.structural_type,
        &unused_path[..case_position],
    )
    .ok_or("unused result guard has no exact owned parent")?;
    let cases = parent
        .shape
        .sum_cases()
        .ok_or("unused result guard has no sum declaration")?;
    let maximum = module
        .structural_types
        .iter()
        .filter_map(|declaration| declaration.shape.sum_cases())
        .flat_map(|cases| cases.iter())
        .map(|case| case.id.get())
        .max()
        .ok_or("result guard has no cases")?;
    let unknown = maximum
        .checked_add(1)
        .and_then(semantic_vocabulary::StructuralCaseId::new)
        .ok_or("result control cannot allocate an unknown case")?;
    let foreign = module
        .structural_types
        .iter()
        .filter_map(|declaration| declaration.shape.sum_cases())
        .flat_map(|cases| cases.iter())
        .map(|case| case.id)
        .find(|identity| !cases.iter().any(|case| case.id == *identity))
        .ok_or("result control needs another case namespace")?;
    for (name, case) in [
        ("unknown unused case", unknown),
        ("another result type's unused case", foreign),
    ] {
        let mut invalid = module.clone();
        let result = result_mut(&mut invalid, site)?;
        result.claims[unused].path[case_position] = Path::Case(case);
        super::reject_module_at_ingress(&invalid, name, "structural path has an unknown case")?;
    }
    let mut invalid = module.clone();
    result_mut(&mut invalid, site)?.claims[unused]
        .path
        .insert(0, Path::Referent);
    super::reject_module_at_ingress(
        &invalid,
        "non-owned unused result path",
        "structural call result claim is not static owned storage",
    )?;
    let mut invalid = module.clone();
    let field = unused_path
        .iter()
        .rposition(|part| matches!(part, Path::Field(_)))
        .ok_or("unused result slot has no payload field")?;
    let (prefix, selected_case) = match unused_path[..field].split_last() {
        Some((Path::Case(case), prefix)) => (prefix, Some(*case)),
        _ => (&unused_path[..field], None),
    };
    let (_, parent) = terminal_semantics::canonical_static_projection(
        module.structural_types.iter(),
        original.structural_type,
        prefix,
    )
    .ok_or("unused result field has no exact owned parent")?;
    let fields = if let Some(case) = selected_case {
        parent
            .shape
            .sum_cases()
            .and_then(|cases| cases.iter().find(|row| row.id == case))
            .map(|case| case.fields.as_slice())
    } else {
        parent.shape.direct_fields()
    }
    .ok_or("unused result field has no declared namespace")?;
    let foreign_field = module
        .structural_types
        .iter()
        .flat_map(|declaration| {
            declaration
                .shape
                .direct_fields()
                .into_iter()
                .flatten()
                .chain(
                    declaration
                        .shape
                        .sum_cases()
                        .into_iter()
                        .flatten()
                        .flat_map(|case| &case.fields),
                )
        })
        .filter(|field| !field.relevance.is_erased())
        .map(|field| field.identity)
        .find(|identity| {
            !identity.is_absent() && !fields.iter().any(|field| field.identity == *identity)
        })
        .ok_or("result control needs a field from another namespace")?;
    result_mut(&mut invalid, site)?.claims[unused].path[field] = Path::Field(foreign_field);
    super::reject_module_at_ingress(
        &invalid,
        "unknown unused result field",
        "structural path has an unknown structural field",
    )?;
    let mut invalid = module.clone();
    result_mut(&mut invalid, site)?.claims[unused].path = active_path;
    super::reject_module_at_ingress(
        &invalid,
        "duplicate result payload path",
        "structural call result has overlapping claim paths",
    )?;
    let mut invalid = module.clone();
    result_mut(&mut invalid, site)?.claims.remove(active);
    super::reject_module_at_ingress(
        &invalid,
        "missing returned result path",
        "structural callee return disagrees with the caller's result claims",
    )?;
    let mut invalid = module.clone();
    result_mut(&mut invalid, site)?.claims.swap(active, unused);
    super::reject_module_at_ingress(
        &invalid,
        "unsorted result identities",
        "structural operation result claims by ClaimId",
    )?;
    let mut invalid = module.clone();
    let result = result_mut(&mut invalid, site)?;
    result.claims[unused].claim = result.claims[active].claim;
    super::reject_module_at_ingress(
        &invalid,
        "duplicate result identity",
        "structural operation result claims by ClaimId",
    )?;
    // A separate ordinary call has a whole, unguarded result obligation.
    // Removing its callee's returned row must not be excused as an unused case.
    let (machine, block, returned) = find_unguarded_return(module)
        .ok_or("result controls need an ordinary unguarded claim return")?;
    let mut invalid = module.clone();
    let Terminator::ReturnStructural {
        returned_claims, ..
    } = &mut invalid.machines[machine].blocks[block].terminator
    else {
        return Err("result control lost its return".into());
    };
    returned_claims.remove(returned);
    super::reject_module_at_ingress(
        &invalid,
        "missing unguarded return obligation",
        "structural callee return disagrees with the caller's result claims",
    )?;
    inspect_optimization_replay(artifact, site, unused)?;
    Ok(())
}

fn inspect_optimization_replay(
    artifact: &CanonicalTerminalArtifact,
    site: CallSite,
    unused: usize,
) -> Result<(), String> {
    use abstract_operations::{AbstractOperation, AbstractStructuralCallResult};
    use optimization_unit_semantics::{
        OperationSite, OptimizationUnitValidationError,
        validate_psi_optimization_unit_with_admitted_cycle_machines,
    };

    let module = artifact.semantic_module();
    let source = &module.machines[site.machine];
    let operation = &source.blocks[site.block].operations[site.operation];
    let mut unit = super::optimization_catalog_controls::intact_unit(artifact)?;
    let function = unit
        .functions
        .iter_mut()
        .find(|function| function.machine == source.id)
        .ok_or("result control unit lost its caller")?;
    let mut expected = None;
    for block in &mut function.blocks {
        for (index, node) in block.nodes.iter_mut().enumerate() {
            let AbstractOperation::CallStructural {
                psi_operation,
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            } = &mut node.operation
            else {
                continue;
            };
            if *psi_operation != operation.id {
                continue;
            }
            let (_, endpoint) = terminal_semantics::canonical_static_projection(
                unit.structural_types.iter(),
                result.structural_type,
                &result.claims[unused].path,
            )
            .ok_or("unused optimization result has no owned endpoint")?;
            terminal_semantics::representation_view_declaration(
                unit.structural_types.iter(),
                endpoint.id,
                None,
            )
            .ok_or("result control needs a record endpoint for its interpretation")?;
            // This retains a declared endpoint and case guard, but changes
            // owned custody into a storage interpretation. Geometry must not
            // admit it as an unused possible claim.
            result.claims[unused].path.push(Path::RepresentationView {
                structural_type: endpoint.id,
                stored_scalar: None,
            });
            expected = Some(OperationSite::new(
                source.id,
                block.id,
                u32::try_from(index).map_err(|_| "result control node index overflow")?,
            ));
        }
    }
    let expected = expected.ok_or("result control unit lost its call operation")?;
    unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
    match validate_psi_optimization_unit_with_admitted_cycle_machines(&unit, &[]) {
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch(actual))
            if actual == expected =>
        {
            Ok(())
        }
        result => Err(format!(
            "optimization result interpretation expected a call-contract refusal at {expected:?}, got {result:?}"
        )),
    }
}

fn result_mut(
    module: &mut TerminalModule,
    site: CallSite,
) -> Result<&mut terminal_psi::StructuralOperationResult, String> {
    let OperationResult::Structural(result) =
        &mut module.machines[site.machine].blocks[site.block].operations[site.operation].result
    else {
        return Err("result control lost its structural call".into());
    };
    Ok(result)
}
fn returned_paths(machine: &terminal_psi::TerminalMachine) -> BTreeSet<&[Path]> {
    machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::ReturnStructural {
                returned_claims, ..
            } => Some(returned_claims),
            _ => None,
        })
        .flatten()
        .map(|binding| binding.path.as_slice())
        .collect()
}
fn find_guarded_slots(module: &TerminalModule) -> Option<(CallSite, usize, usize)> {
    for (machine, body) in module.machines.iter().enumerate() {
        for (block, body) in body.blocks.iter().enumerate() {
            for (operation, row) in body.operations.iter().enumerate() {
                let Some(callee) = row.kind.direct_callee().and_then(|id| module.machine(id))
                else {
                    continue;
                };
                let Some(result) = row.result.as_structural() else {
                    continue;
                };
                let returned = returned_paths(callee);
                let active = result
                    .claims
                    .iter()
                    .position(|binding| returned.contains(binding.path.as_slice()));
                let unused = result.claims.iter().position(|binding| {
                    binding
                        .path
                        .iter()
                        .any(|part| matches!(part, Path::Case(_)))
                        && !returned.contains(binding.path.as_slice())
                });
                if let (Some(active), Some(unused)) = (active, unused) {
                    return Some((
                        CallSite {
                            machine,
                            block,
                            operation,
                        },
                        active,
                        unused,
                    ));
                }
            }
        }
    }
    None
}
fn find_unguarded_return(module: &TerminalModule) -> Option<(usize, usize, usize)> {
    for caller in &module.machines {
        for operation in caller.operations() {
            let Some(callee) = operation.kind.direct_callee() else {
                continue;
            };
            let Some(result) = operation.result.as_structural() else {
                continue;
            };
            for StructuralResultClaimBinding { path, .. } in &result.claims {
                if path.iter().any(|part| matches!(part, Path::Case(_))) {
                    continue;
                }
                let machine = module
                    .machines
                    .iter()
                    .position(|machine| machine.id == callee)?;
                for (block, row) in module.machines[machine].blocks.iter().enumerate() {
                    if let Terminator::ReturnStructural {
                        returned_claims, ..
                    } = &row.terminator
                        && let Some(returned) = returned_claims
                            .iter()
                            .position(|binding| &binding.path == path)
                    {
                        return Some((machine, block, returned));
                    }
                }
            }
        }
    }
    None
}
