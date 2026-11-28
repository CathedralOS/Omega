//! A dependent field can take a value projected under the same live witness.
use super::*;

pub(super) fn checked_store(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    index: u32,
    field: StructuralFieldId,
    projected: &ProjectedOrigins,
) -> Result<StructuralFieldType, &'static str> {
    let opened = opening(module, machine.id, index)?;
    let (destination, path, selected, source, actual) = match &operation.kind {
        OperationKind::StoreStructuralField {
            destination,
            path,
            field,
            value,
            ..
        } => {
            if value.access != StructuralAccess::Owned || !value.path.is_empty() {
                return Err("a package field store needs a whole projected reference");
            }
            let ty = argument_type(module, machine, value)
                .ok_or("a package field store has no source type")?;
            (
                *destination,
                path,
                *field,
                Projected::Reference(value.place),
                StructuralFieldType::Structural(ty),
            )
        }
        OperationKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            value,
            range_obligation: None,
        } => {
            let Some(ty @ ScalarType::Callable(_)) = scalar_type(machine, *value) else {
                return Err("a package scalar field store needs a projected callable");
            };
            (
                *destination,
                path,
                Some(*field),
                Projected::Descriptor(*value),
                StructuralFieldType::Scalar(ty),
            )
        }
        _ => return Err("a package field store site needs an ordinary field store"),
    };
    let declaration = fields(module, opened.record)?
        .iter()
        .find(|row| row.id == field && !row.relevance.is_erased())
        .ok_or("a package store names an absent field")?;
    let parent = match selected {
        Some(selected) if selected == field => path.as_slice(),
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
    if destination != opened.subject.place
        || !crate::structural_paths_select_same_location(parent, &opened.subject.path)
        || enters_package(
            module,
            machine,
            terminal_psi::OperationProjection {
                root: destination,
                path: parent,
                field: None,
            },
        )
    {
        return Err("a package field store writes outside its checked opening");
    }
    let &(origin, producer) = projected
        .get(&(machine.id, source))
        .ok_or("a package field store has no projected source")?;
    let materialized = match source {
        Projected::Reference(place) => Some(place),
        Projected::Descriptor(_) => None,
    };
    if origin != index
        || !versions::same_storage_version(
            module,
            machine,
            opened,
            producer,
            operation.id,
            materialized,
        )
    {
        return Err("a package field store mixes openings or uses a stale projection");
    }
    let schema = schema(module, opened.record)?;
    let substitution = Substitution {
        public: &schema.witnesses,
        actual: &opened.witnesses,
    };
    if !schema
        .witnesses
        .iter()
        .any(|w| fields::uses_witness(module, &declaration.field_type, *w))
        || !substitution.admits(module, &declaration.field_type, &actual)
    {
        return Err("a package field store changes the destination's witness type");
    }
    Ok(actual)
}
