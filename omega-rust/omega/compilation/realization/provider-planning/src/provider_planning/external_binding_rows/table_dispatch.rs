//! Resolve a checked table-field use in this artifact's storage namespace.
//! Provider derivation owns the Source correspondence. This consumer joins it
//! to the original semantic ABI formal, then asks the foreign-layout owner for
//! that exact live field; neither field spelling nor equal shape is authority.

use crate::calling_policy_plans::CallingProgram;
use effects::provider_plan::TableFieldStorage;
use semantic_vocabulary::CallingAbiIdentity;
use terminal_psi::StructuralTypeShape;

pub(super) fn field_offset(
    program: &CallingProgram,
    target: target::TargetProfile,
    applications: &[&CallingAbiIdentity],
    storage: &Result<TableFieldStorage, String>,
) -> Result<u32, String> {
    let storage = storage.as_ref().map_err(Clone::clone)?;
    let module = program.artifact.semantic_module();
    let mut records = module.structural_types.iter().filter(|record| {
        module.identities.text(record.identity) == Some(storage.table_application.as_str())
    });
    let record = records
        .next()
        .ok_or("dispatch table has no canonical storage application")?;
    if records.next().is_some() {
        return Err("dispatch table has competing canonical storage applications".into());
    }
    let StructuralTypeShape::Record { fields, .. } = &record.shape else {
        return Err("dispatch table does not have record storage".into());
    };
    let native = module
        .native_layouts
        .record(record.id)
        .ok_or("dispatch table lost its nominal field correspondence")?;
    let mut candidates = module
        .native_layouts
        .fields(native)?
        .iter()
        .filter(|field| {
            module.identities.text(field.declaration_identity)
                == Some(storage.field_declaration.as_str())
        });
    let field = candidates
        .next()
        .ok_or("dispatch field has no canonical declaration correspondence")?;
    if candidates.next().is_some() {
        return Err("dispatch field has competing canonical declaration correspondences".into());
    }
    let declaration = fields
        .iter()
        .find(|candidate| candidate.id == field.field && !candidate.relevance.is_erased())
        .ok_or("dispatch field is not a live field of the exact table")?;
    if declaration.field_type.atomic_scalar_type().is_some()
        || !matches!(declaration.field_type.scalar_type(),
        Some(semantic_vocabulary::ScalarType::Integer(integer)) if integer.is_address())
    {
        return Err("table dispatch field must have type addr".into());
    }
    let mut selected_offset = None;
    for application in applications {
        let signature = module
            .calling_abi_signature(application.id())
            .ok_or("table dispatch has no original semantic ABI")?;
        if signature.identity(&module.identities)? != **application {
            return Err("table dispatch differs from its full selected ABI key".into());
        }
        let parameter = signature
            .storage
            .as_ref()
            .map_err(Clone::clone)?
            .parameters
            .first()
            .ok_or("table dispatch has no first semantic formal")?;
        if parameter.structural_type != record.id
            || !matches!(
                parameter.access,
                terminal_psi::reading_borrow_pair_pattern!()
            )
        {
            return Err(
                "table dispatch requires the exact readable table borrow as semantic formal zero"
                    .into(),
            );
        }
        let offset = program.describe_borrowed_storage(application, target, 0, |extents| {
            let mut recipes =
                layout::TerminalBoundaryRecipes::new(module, target.target_name(), extents);
            let (recipe, ordinal) = recipes.record_field(record.id, field.field)?;
            let mut layout =
                layout::BoundaryValueLayout::new(layout::LayoutTarget::for_profile(target)?);
            let (_, root) = layout.value_type(recipes.recipes(), recipe)?;
            let (shapes, fields, _) = layout.into_parts();
            let calling_conventions::BoundaryValueClass::Record {
                first_field,
                field_count,
            } = shapes[usize::from(root)].class
            else {
                return Err("dispatch table does not have a foreign record layout".into());
            };
            if ordinal >= usize::from(field_count) {
                return Err("dispatch field is absent from the foreign record layout".into());
            }
            fields
                .get(usize::from(first_field) + ordinal)
                .map(|field| u32::from(field.byte_offset))
                .ok_or_else(|| "dispatch field lost its physical layout coordinate".into())
        })?;
        if selected_offset.is_some_and(|prior| prior != offset) {
            return Err("selected table ABIs disagree on their dispatch field offset".into());
        }
        selected_offset = Some(offset);
    }
    selected_offset.ok_or_else(|| "table dispatch has no selected semantic ABI application".into())
}
