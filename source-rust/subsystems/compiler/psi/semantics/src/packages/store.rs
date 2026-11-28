//! A dependent field can take another projected reference of its opened type.
use super::*;

pub(super) fn checked_store(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    index: u32,
    field: StructuralFieldId,
    projected: &ProjectedOrigins,
) -> Result<StructuralTypeId, &'static str> {
    let opened = opening(module, machine.id, index)?;
    let OperationKind::StoreStructuralField {
        destination,
        path,
        field: selected,
        value,
        ..
    } = &operation.kind
    else {
        return Err("a package field store site needs an ordinary structural store");
    };
    let declaration = fields(module, opened.record)?
        .iter()
        .find(|row| row.id == field && !row.relevance.is_erased())
        .ok_or("a package store names an absent field")?;
    let parent = match selected {
        Some(selected) if *selected == field => path.as_slice(),
        None => {
            let Some((StructuralPathSegment::Field(identity), parent)) = path.split_last() else {
                return Err("a package store must select its declared destination field");
            };
            if *identity != declaration.identity {
                return Err("a package store writes another destination field");
            }
            parent
        }
        _ => return Err("a package store writes another destination field"),
    };
    if *destination != opened.subject.place
        || !crate::structural_paths_select_same_location(parent, &opened.subject.path)
        || enters_package(
            module,
            machine,
            terminal_psi::OperationProjection {
                root: *destination,
                path: parent,
                field: None,
            },
        )
    {
        return Err("a package field store writes outside its checked opening");
    }
    if value.access != StructuralAccess::Owned || !value.path.is_empty() {
        return Err("a package field store needs a whole projected value");
    }
    let &(origin, producer) = projected
        .get(&(machine.id, Projected::Reference(value.place)))
        .ok_or("a package field store has no projected source")?;
    if origin != index
        || !versions::same_storage_version(module, machine, opened, producer, operation.id)
    {
        return Err("a package field store mixes openings or uses a stale projection");
    }
    let schema = schema(module, opened.record)?;
    let actual =
        argument_type(module, machine, value).ok_or("a package field store has no source type")?;
    let substitution = Substitution {
        public: &schema.witnesses,
        actual: &opened.witnesses,
    };
    if !schema
        .witnesses
        .iter()
        .any(|w| fields::uses_witness(module, &declaration.field_type, *w))
        || !substitution.admits(
            module,
            &declaration.field_type,
            &StructuralFieldType::Structural(actual),
        )
    {
        return Err("a package field store changes the destination's witness type");
    }
    Ok(actual)
}
