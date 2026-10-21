//! Applicability of the native whole-value codec, not a new language rule.
//! Runtime schema bytes can move as one owned value. Non-runtime Type fields
//! need the authored custody agreement, which this primitive does not carry.
//! Likewise a narrowing IntegerAt needs its checked fitting evidence; emitting
//! a masked store or inventing a runtime rejection would not discharge it.

use layout_plans::LayoutPlacementReport;
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    StructuralFieldDeclaration, StructuralTypeShape, TerminalModule, TerminalPlacementLifecyclePlan,
};

pub(super) fn require(
    module: &TerminalModule,
    plan: &TerminalPlacementLifecyclePlan,
) -> Result<(), &'static str> {
    let declaration = module
        .structural_type(plan.schema)
        .ok_or("native lifecycle schema is absent")?;
    let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        return Err("native lifecycle has no complete record image");
    };
    if terminal_psi::structural_type_has_borrowed_storage_by(
        |identity| module.structural_type(identity),
        plan.schema,
    ) {
        return Err(
            "native resident content requires retained loan forwarding for reference fields",
        );
    }
    require_runtime_value(module, plan.schema, &mut Vec::new())?;
    // Extraction reads the represented width and extends it; only an encoding
    // can lose high bits and therefore needs a concrete fitting witness.
    if plan.operation != terminal_psi::TerminalPlacementLifecycleOperation::Initialize {
        return Ok(());
    }
    for (field, access) in plan
        .image_fields
        .iter()
        .zip(plan.placement.access().plan().entries())
    {
        let field = fields
            .iter()
            .find(|candidate| candidate.id == *field)
            .ok_or("native lifecycle image field is absent")?;
        for entry in plan
            .placement
            .layout()
            .entries
            .iter()
            .filter(|entry| entry.field == access.field())
        {
            if let LayoutPlacementReport::IntegerAt { stored_width, .. } = entry.placement {
                let scalar = field
                    .field_type
                    .scalar_type()
                    .ok_or("stored integer image field has no scalar carrier")?;
                let bytes = terminal_semantics::reference_storage_byte_width(scalar)
                    .ok_or("stored integer image field has no fixed width")?;
                if stored_width != u64::from(bytes) * 8 {
                    return Err(
                        "native lifecycle narrowing requires checked concrete-value fitting evidence",
                    );
                }
            }
        }
    }
    Ok(())
}

fn require_runtime_value(
    module: &TerminalModule,
    root: StructuralTypeId,
    active: &mut Vec<StructuralTypeId>,
) -> Result<(), &'static str> {
    if active.contains(&root) {
        return Err("native lifecycle value has cyclic inline storage");
    }
    active.push(root);
    let declaration = module
        .structural_type(root)
        .ok_or("native lifecycle value declaration is absent")?;
    match &declaration.shape {
        StructuralTypeShape::Record { fields, .. } => require_fields(module, fields, active)?,
        StructuralTypeShape::Sum { cases } => {
            for case in cases {
                require_fields(module, &case.fields, active)?;
            }
        }
        StructuralTypeShape::Mixed { fields, cases } => {
            require_fields(module, fields, active)?;
            for case in cases {
                require_fields(module, &case.fields, active)?;
            }
        }
        StructuralTypeShape::FixedArray { element, length } if *length != 0 => {
            require_runtime_value(module, *element, active)?
        }
        _ => {}
    }
    active.pop();
    Ok(())
}

fn require_fields(
    module: &TerminalModule,
    fields: &[StructuralFieldDeclaration],
    active: &mut Vec<StructuralTypeId>,
) -> Result<(), &'static str> {
    for field in fields {
        if field.relevance.is_erased() {
            return Err(
                "native lifecycle non-runtime fields require an authored custody agreement",
            );
        }
        if let Some(child) = field.field_type.as_structural() {
            if terminal_semantics::reference_storage_size(module.structural_types.iter(), child)
                == Some(0)
            {
                return Err(
                    "native lifecycle non-runtime fields require an authored custody agreement",
                );
            }
            require_runtime_value(module, child, active)?;
        }
    }
    Ok(())
}
