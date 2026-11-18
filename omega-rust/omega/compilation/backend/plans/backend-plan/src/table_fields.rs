//! A table binding uses the original requirement formal and declared field.
//! Resolve their checked storage once, then carry that result to native calls.

use calling_conventions::{
    BoundaryCallingApplication, BoundaryNativeParameterOrigin, TableParameter,
};
use effects::provider_plan::TableFieldStorage;
use semantic_vocabulary::{
    BoundaryMachineId, FieldInfoDescription, ParameterDescription, ScalarType,
};

pub fn table_field_storage(
    closed: &terminal_codec::ClosedTerminalModule,
    boundary: BoundaryMachineId,
    application: &BoundaryCallingApplication,
    target: target::TargetProfile,
    table: &ParameterDescription,
    field: &FieldInfoDescription,
) -> Result<TableFieldStorage, String> {
    let scalars = |scalar| closed.closed_types().scalar(scalar).ok();
    field_storage(
        closed.module(),
        boundary,
        application,
        target,
        table,
        field,
        &scalars,
    )
}

// Foreign replay has raw declaration metadata, not a second compiler product.
// Its existing closed-scalar conversion refuses every unresolved carrier.
pub fn validate_table_field_storage(
    module: &terminal_psi::TerminalModule,
    boundary: BoundaryMachineId,
    application: &BoundaryCallingApplication,
    target: target::TargetProfile,
    table: &ParameterDescription,
    field: &FieldInfoDescription,
    expected: &TableFieldStorage,
) -> Result<(), String> {
    let scalars = |scalar| semantic_vocabulary::ClosedScalarType::try_from(scalar).ok();
    let actual = field_storage(
        module,
        boundary,
        application,
        target,
        table,
        field,
        &scalars,
    )?;
    if &actual != expected {
        return Err(
            "table binding changed its authenticated field layout or argument mapping".into(),
        );
    }
    Ok(())
}

fn field_storage(
    module: &terminal_psi::TerminalModule,
    boundary: BoundaryMachineId,
    application: &BoundaryCallingApplication,
    target: target::TargetProfile,
    table: &ParameterDescription,
    field: &FieldInfoDescription,
    scalars: &dyn Fn(ScalarType) -> Option<semantic_vocabulary::ClosedScalarType>,
) -> Result<TableFieldStorage, String> {
    if application.target != target.native_target()
        || module.calling_abi_identity(boundary)? != application.abi
        || table.requirement_application != application.requirement_identity.as_bytes()
    {
        return Err("table binding changes its exact requirement application or target".into());
    }
    let declaration = module
        .boundary_machine(boundary)
        .and_then(|row| row.declaration.as_ref())
        .ok_or("table binding has no checked requirement declaration")?;
    if module
        .identities
        .text(declaration.source.semantic_declaration)
        .map(str::as_bytes)
        != Some(table.requirement_identity.as_slice())
    {
        return Err("table selector belongs to another source requirement".into());
    }
    let signature = module
        .calling_abi_signature(application.abi.id())
        .ok_or("table binding lost its canonical ABI signature")?;
    let storage = signature.storage.as_ref().map_err(Clone::clone)?;
    let ordinal = u32::try_from(table.declaration_position)
        .map_err(|_| "table formal position exceeds u32")?;
    if declaration
        .source
        .parameter_types
        .get(ordinal as usize)
        .and_then(|identity| module.identities.text(*identity))
        .map(str::as_bytes)
        != Some(table.qualified_type.as_slice())
    {
        return Err(
            "table selector changes its complete declared type or lifetime relationships".into(),
        );
    }
    let formal = storage
        .telescope
        .parameters
        .get(ordinal as usize)
        .ok_or("table selector names no original semantic formal")?;
    let parameter = storage
        .parameters
        .get(ordinal as usize)
        .ok_or("table formal lost its complete storage type")?;
    if !matches!(
        parameter.access,
        terminal_psi::reading_borrow_pair_pattern!()
    ) {
        return Err("table lookup requires an ordinary readable borrow".into());
    }
    let dispatch_only = match application.plan.plan().call.table_parameter {
        TableParameter::Passed {
            formal: selected,
            formal_ordinal,
        } if selected == formal.identity && formal_ordinal == ordinal => false,
        TableParameter::DispatchOnly {
            formal: selected,
            formal_ordinal,
        } if selected == formal.identity && formal_ordinal == ordinal => true,
        _ => return Err("calling policy does not account for this exact table formal".into()),
    };
    if application.dispatch_only_formal != dispatch_only.then_some(ordinal) {
        return Err("table calling application changed its semantic role".into());
    }
    let record = module
        .structural_type(parameter.structural_type)
        .ok_or("table formal has no retained referent declaration")?;
    if module
        .identities
        .text(record.declaration_identity)
        .map(str::as_bytes)
        != Some(field.owner_identity.as_slice())
        || module.identities.text(record.identity).map(str::as_bytes)
            != Some(field.owner_application.as_slice())
    {
        return Err(
            "dispatch field belongs to another exact table declaration or application".into(),
        );
    }
    let terminal_psi::StructuralTypeShape::Record { fields, .. } = &record.shape else {
        return Err("table dispatch requires checked record storage".into());
    };
    let mut matches = fields.iter().filter(|candidate| {
        module
            .identities
            .text(candidate.declaration_identity)
            .map(str::as_bytes)
            == Some(field.member_identity.as_slice())
    });
    let (Some(selected), None) = (matches.next(), matches.next()) else {
        return Err("table binding has no unique exact source field".into());
    };
    if selected.relevance.is_erased()
        || !matches!(selected.field_type, terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(integer)) if integer.is_address())
        || field.is_erased
        || !field.case_identity.is_empty()
    {
        return Err("dispatch requires a live non-atomic addr field".into());
    }
    // Layout owns every offset and width. Unknown extents remain unavailable;
    // a matching description cannot supply missing target geometry.
    let extents = |application| {
        Err(terminal_semantics::StorageGeometryError::UnresolvedExtent(
            application,
        ))
    };
    let mut recipes = layout::TerminalBoundaryRecipes::with_closed_scalars(
        module,
        target.target_name(),
        &extents,
        scalars,
    );
    let (recipe, field_ordinal) = recipes.record_field(record.id, selected.id)?;
    let mut values = layout::BoundaryValueLayout::new(layout::LayoutTarget::for_profile(target)?);
    let (_, root) = values.value_type(recipes.recipes(), recipe)?;
    let (shapes, fields, _) = values.into_parts();
    let Some(calling_conventions::BoundaryValueShape {
        class:
            calling_conventions::BoundaryValueClass::Record {
                first_field,
                field_count,
            },
        ..
    }) = shapes.get(usize::from(root))
    else {
        return Err("table layout lost its exact record field geometry".into());
    };
    if field_ordinal >= usize::from(*field_count) {
        return Err("dispatch field is outside its record layout".into());
    }
    let selected = fields
        .get(usize::from(*first_field) + field_ordinal)
        .ok_or("dispatch field has no actual layout offset")?;
    let shape = shapes
        .get(usize::from(selected.shape))
        .ok_or("dispatch address has no actual layout width")?;
    if shape.class != calling_conventions::BoundaryValueClass::Integer
        || usize::from(shape.byte_size) != target.native_target().pointer_size
        || usize::from(shape.alignment) != target.native_target().pointer_alignment
    {
        return Err("dispatch field is not one target function-address lane".into());
    }
    let table_operand = if dispatch_only {
        0
    } else {
        let rows = application
            .signature
            .native_parameters
            .span(application.signature.native_parameter_rows)
            .ok_or("table calling application lost its native parameter storage")?;
        u32::try_from(
            rows.iter()
                .position(|row| {
                    row.identity == formal.identity
                        && row.origin
                            == BoundaryNativeParameterOrigin::SemanticFormal {
                                formal_ordinal: ordinal,
                            }
                })
                .ok_or("passed table has no exact native argument")?,
        )
        .map_err(|_| "table native operand exceeds u32")?
    };
    Ok(TableFieldStorage {
        table_application: String::from_utf8(field.owner_application.clone())
            .map_err(|_| "table application is not canonical text")?,
        field_declaration: String::from_utf8(field.member_identity.clone())
            .map_err(|_| "table field is not canonical text")?,
        formal: formal.identity,
        formal_ordinal: ordinal,
        table_operand,
        byte_offset: u32::from(selected.byte_offset),
    })
}
