use target_vocabulary::NearestIeeeFloatArithmeticRealization;
pub fn boundary_contract_matches_selected_schema(
    module: &terminal_psi::TerminalModule,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
    plan: &build_providers::plans::ProviderPlan,
    requirement: &str,
) -> bool {
    // A closed call keeps its own selected row while sharing the declared
    // method's completion and returned facts with the other applications.
    let mut rows = plan
        .rows
        .iter()
        .filter(|row| plan.identities.text(row.requirement_identity) == requirement);
    let Some(row) = rows.next().filter(|_| rows.next().is_none()) else {
        return false;
    };
    let declared = plan.identities.text(row.declared_requirement_identity);
    let Some(requirement_id) = plan.schema.declaration.identities.id_at(declared) else {
        return false;
    };
    let mut matching = plan
        .schema
        .declaration
        .methods
        .iter()
        .filter(|method| method.requirement_identity == requirement_id);
    matching.next().is_some_and(|method| {
        method.completion == boundary.completion
            && boundary.unconditional_termination
                == (method.terminates_guarantee && method.termination_premises.is_empty())
            && terminal_semantics::validate_boundary_memberships(module, boundary, method).is_ok()
    }) && matching.next().is_none()
}

pub(crate) fn numeric_boundary_realization(
    module: &terminal_psi::TerminalModule,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
) -> Result<NearestIeeeFloatArithmeticRealization, &'static str> {
    terminal_semantics::numeric_primitives::validate_boundary(module, boundary)?;
    let meaning = boundary
        .numeric_intrinsic
        .ok_or("numeric boundary has no numeric meaning")?;
    crate::catalog_realization(meaning.into(), meaning.builtin_format().map(Into::into))
        .ok_or("numeric boundary has no closed execution catalog entry")
}
pub fn numeric_boundary_execution(
    module: &terminal_psi::TerminalModule,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
) -> Result<target_vocabulary::CompilerBuiltinExecution, &'static str> {
    numeric_boundary_realization(module, boundary).map(|value| {
        target_vocabulary::CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
            operation: value.operation,
            format: value.format,
        }
    })
}
pub fn float_arithmetic_signature(
    realization: NearestIeeeFloatArithmeticRealization,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
    module: &terminal_psi::TerminalModule,
) -> Result<NearestIeeeFloatArithmeticRealization, String> {
    if realization.operation == target_vocabulary::IeeeFloatArithmeticOperation::Classify {
        let terminal_psi::BoundaryMachineResult::Structural(result) = &boundary.result else {
            return Err("does not match its structural classification result".to_owned());
        };
        let mut declarations = module
            .types.structural_types
            .iter()
            .filter(|declaration| declaration.id == result.structural_type);
        if declarations
            .next()
            .and_then(target_vocabulary::conventional_float_class_layout)
            .is_none()
            || declarations.next().is_some()
            || boundary.scalar_parameters
                != [semantic_vocabulary::ScalarType::IeeeFloat(
                    realization.format,
                )]
            || !boundary.structural_parameters.is_empty()
            || result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
        {
            return Err("does not match its structural classification result".to_owned());
        }
        return Ok(realization);
    }
    let terminal_psi::BoundaryMachineResult::Scalar(result) = boundary.result else {
        return Err("does not match its Terminal boundary signature".to_owned());
    };
    let result_matches = if result.qualifications.is_empty() {
        true
    } else {
        terminal_psi::find_boundary_machine(&module.interfaces.boundary_machines, boundary.id)
            == Some(boundary)
            && numeric_boundary_realization(module, boundary).is_ok_and(|required| {
                required.operation == realization.operation && required.format == realization.format
            })
    };
    if !result_matches
        || !realization.signature_matches(&boundary.scalar_parameters, result.scalar_type)
        || !boundary.structural_parameters.is_empty()
    {
        return Err("does not match its Terminal boundary signature".to_owned());
    }
    Ok(realization)
}
