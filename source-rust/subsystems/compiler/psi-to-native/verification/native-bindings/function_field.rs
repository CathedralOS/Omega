//! Locate one described function pointer in the native layout of the borrowed record.

use calling_conventions::FunctionFieldLocation;
use provider_plans::EvaluatedFunctionField;
use semantic_vocabulary::{CanonicalStructuralPathSegment, ScalarType};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryParameterKind, StructuralFieldType, StructuralTypeShape,
    TerminalModule,
};

pub fn resolve_function_field(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
    evaluated: &EvaluatedFunctionField,
) -> Result<FunctionFieldLocation, String> {
    let selector = evaluated.selector();
    let (formal, table) = selected_table(module, boundary, evaluated)?;
    let record = module
        .structural_type(table.structural_type)
        .ok_or("function table formal lost its borrowed type")?;
    if module
        .identities
        .text(record.declaration_identity)
        .map(str::as_bytes)
        != Some(selector.field.owner_identity.as_slice())
    {
        return Err("function field owner differs from the borrowed table".into());
    }
    let StructuralTypeShape::Record { fields, .. } = &record.shape else {
        return Err("function table formal does not borrow a record".into());
    };
    let mut fields = fields.iter().filter(|field| {
        module
            .identities
            .text(field.declaration_identity)
            .is_some_and(|identity| identity.as_bytes() == selector.field.member_identity)
    });
    let field = fields
        .next()
        .ok_or("function field has no declared member in the borrowed table")?;
    if fields.next().is_some()
        || !matches!(field.field_type, StructuralFieldType::Scalar(ScalarType::Integer(integer)) if integer.is_address())
        || field.relevance.is_erased()
    {
        return Err("function field must be one live non-atomic address".into());
    }
    let geometry =
        calling_conventions::terminal_layout::NativeGeometry::new(&module.types.structural_types)
            .map_err(|error| format!("function table has no canonical geometry: {error:?}"))?;
    let layout = geometry
        .borrowed_storage(record.id)
        .ok_or("function table has no closed native layout")?;
    let mut leaves = layout.fields().iter().filter(|leaf| {
        layout
            .field_path(leaf)
            .eq([CanonicalStructuralPathSegment::Field(field.id)])
            && layout.field_repetitions(leaf).is_empty()
    });
    let leaf = leaves
        .next()
        .ok_or("function field has no native pointer-sized storage")?;
    if leaves.next().is_some()
        || leaf.stored_scalar != leaf.logical_scalar
        || !matches!(leaf.logical_scalar, ScalarType::Integer(integer) if integer.is_address())
    {
        return Err("function field has no single native address representation".into());
    }
    Ok(FunctionFieldLocation {
        selector: evaluated.receipt().locator_identity_digest(),
        formal,
        byte_offset: leaf.byte_offset,
    })
}

fn selected_table<'module>(
    module: &TerminalModule,
    boundary: &'module BoundaryMachineDeclaration,
    evaluated: &EvaluatedFunctionField,
) -> Result<(u32, &'module terminal_psi::StructuralParameterDeclaration), String> {
    let selector = evaluated.selector();
    if module.identities.text(boundary.identity).map(str::as_bytes)
        != Some(selector.table.requirement_application.as_slice())
    {
        return Err("function table formal belongs to a different requirement application".into());
    }
    let formal = u32::try_from(selector.table.declaration_position)
        .map_err(|_| "function table formal exceeds its native coordinate")?;
    // A selector names a source formal. Terminal Psi indexes scalar and
    // structural arguments separately and retains their interleaving here.
    let (prior, rest) = boundary
        .parameter_order
        .split_at_checked(formal as usize)
        .ok_or("function table formal is outside the boundary signature")?;
    if rest.first() != Some(&BoundaryParameterKind::Structural) {
        return Err("function table formal must be structural".into());
    }
    let lane = prior
        .iter()
        .filter(|&&kind| kind == BoundaryParameterKind::Structural)
        .count();
    let table = boundary
        .structural_parameters
        .get(lane)
        .filter(|parameter| {
            parameter.position as usize == lane
                && matches!(
                    parameter.access,
                    terminal_psi::reading_borrow_pair_pattern!()
                )
        })
        .ok_or("function table formal must be a readable record borrow")?;
    Ok((formal, table))
}

/// Trace the table's protection to an external supplier in this Product. An
/// arbitrary source record can contain exactly the same `address` bits.
/// A checked helper may preserve an issued domain while forwarding its table.
pub(super) fn check_table_issuers(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
    evaluated: &EvaluatedFunctionField,
    retained: &super::RetainedNativeBindings,
    profile: target::TargetProfile,
) -> Result<(), String> {
    let (_, table) = selected_table(module, boundary, evaluated)?;
    let required = table
        .additional_input_qualifications
        .iter()
        .filter(|q| q.path.is_empty())
        .map(|q| q.domain);
    for id in table.qualifications.iter().copied().chain(required) {
        let Some(domain) = module.structural_domain(id) else {
            continue;
        };
        let Some(routes) = &domain.establishment_routes else {
            continue;
        };
        if domain.carrier != table.structural_type || routes.is_empty() {
            continue;
        }
        let all_external = routes
            .iter()
            .all(|route| admits_table_issuer(module, retained, profile, id, route));
        if all_external {
            return Ok(());
        }
    }
    Err("native function table needs provenance from an admitted foreign issuer; a readable record alone cannot establish its function address".into())
}

fn admits_table_issuer(
    module: &TerminalModule,
    retained: &super::RetainedNativeBindings,
    profile: target::TargetProfile,
    domain: semantic_vocabulary::StructuralDomainId,
    route: &terminal_psi::StructuralEstablishmentRoute,
) -> bool {
    let terminal_psi::StructuralEstablishmentRoute::BoundaryRequirement {
        declaration,
        requirement_identity,
    } = route
    else {
        return false;
    };
    retained.rows.iter().any(|row| {
        if !matches!(row.binding, provider_plans::EvaluatedNativeBinding::Import(_) | provider_plans::EvaluatedNativeBinding::Syscall(_)) {
            return false;
        }
        let Some(issuer) = module.boundary_machine(row.boundary) else { return false };
        if issuer.identity != *requirement_identity
            || issuer.declaration.as_ref().is_none_or(|contract| contract.source.state != *declaration) {
            return false;
        }
        let returns_table = matches!(&issuer.result, terminal_psi::BoundaryMachineResult::Structural(output)
            if output.qualifications.contains(&domain) || output.projected_qualifications.iter().any(|q| q.domain == domain));
        (returns_table || issuer.normal_qualifications.iter().any(|q| q.domain == domain))
            && super::CheckedNativeBinding::admit_requirement(row, &retained.identities, module, profile).is_ok()
    })
}

/// The calling plan owns whether the declared table formal is passed to the native callee.
pub struct FunctionFieldDispatch {
    pub byte_offset: u32,
    pub table_operand: u32,
    pub dispatch_only: bool,
}

pub fn function_field_dispatch(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
    evaluated: &EvaluatedFunctionField,
    application: &calling_conventions::BoundaryCallingApplication,
) -> Result<FunctionFieldDispatch, String> {
    let FunctionFieldLocation {
        formal,
        byte_offset,
        ..
    } = resolve_function_field(module, boundary, evaluated)?;
    if application
        .dispatch_only_formal
        .is_some_and(|selected| selected != formal)
    {
        return Err("calling plan omits a different table formal".into());
    }
    Ok(FunctionFieldDispatch {
        byte_offset,
        table_operand: application.call_projection()?.operand_for_formal(formal)?,
        dispatch_only: application.dispatch_only_formal == Some(formal),
    })
}
