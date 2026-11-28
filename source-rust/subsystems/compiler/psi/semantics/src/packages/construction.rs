//! Check all fields against a single tuple of witnesses at record establishment.
use super::*;

pub(super) fn construction(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    witnesses: &[StructuralTypeId],
    abstract_types: &BTreeSet<StructuralTypeId>,
    origins: &ProjectedOrigins,
    result: &mut CheckedPackageSites,
) -> Result<(), &'static str> {
    let OperationKind::EstablishRecord { fields: supplied } = &operation.kind else {
        return Err("package construction is not record establishment");
    };
    let record = operation
        .result
        .structural()
        .ok_or("package construction lacks a record result")?
        .structural_type;
    let schema = schema(module, record)?;
    if witnesses.len() != schema.witnesses.len()
        || witnesses
            .iter()
            .any(|id| module.structural_type(*id).is_none())
    {
        return Err("package construction requires the complete concrete witness tuple");
    }
    // A closed construction supplies nominal witness types. Repacking may
    // instead use a live opening's abstract witnesses, whose actual value is
    // known only to the presented package.
    let mut repacked = BTreeMap::new();
    for (&public, &actual) in schema.witnesses.iter().zip(witnesses) {
        if abstract_types.contains(&actual) {
            let (index, row) = module
                .execution
                .packages
                .openings
                .iter()
                .enumerate()
                .find(|(_, row)| row.machine == machine.id && row.witnesses.contains(&actual))
                .ok_or("construction uses an abstract witness outside its opening")?;
            let index = u32::try_from(index).map_err(|_| "package opening index overflow")?;
            repacked.insert(public, (index, row));
        }
    }
    let substitutions = Substitution {
        public: &schema.witnesses,
        actual: witnesses,
    };
    let mut declarations = fields(module, record)?
        .iter()
        .filter(|field| !field.relevance.is_erased());
    for field in supplied {
        let declaration = declarations
            .next()
            .filter(|decl| decl.id == field.field)
            .ok_or("package construction fields differ from its declaration")?;
        let actual = match &field.value {
            RecordFieldValue::Scalar { value, .. } => {
                let ty = scalar_type(machine, *value)
                    .ok_or("package field lacks a unique scalar producer")?;
                if declaration.field_type.scalar_type() == Some(ty) {
                    declaration.field_type.clone()
                } else {
                    StructuralFieldType::Scalar(ty)
                }
            }
            RecordFieldValue::Structural(arg) if arg.access == StructuralAccess::Owned => {
                StructuralFieldType::Structural(
                    argument_type(module, machine, arg)
                        .ok_or("package field operand has no structural type")?,
                )
            }
            _ => return Err("package construction does not own its initializer"),
        };
        if !substitutions.admits(module, &declaration.field_type, &actual) {
            return Err("package field differs from the shared witness substitution");
        }
        for (&public, &(index, row)) in &repacked {
            if !fields::uses_witness(module, &declaration.field_type, public) {
                continue;
            }
            let input = match &field.value {
                RecordFieldValue::Scalar { value, .. } => Projected::Descriptor(*value),
                RecordFieldValue::Structural(arg) if arg.path.is_empty() => {
                    Projected::Reference(arg.place)
                }
                _ => return Err("repacked field has no materialized opened operand"),
            };
            let (origin, producer) = origins
                .get(&(machine.id, input))
                .ok_or("repacked field has no opening origin")?;
            if *origin != index
                || !versions::same_storage_version(module, machine, row, *producer, operation.id)
            {
                return Err("repacked field comes from a different or stale opening");
            }
            result
                .repacked_fields
                .insert((machine.id, operation.id, field.field), *producer);
        }
        result
            .fields
            .insert((machine.id, operation.id, field.field), actual);
    }
    if declarations.next().is_some() {
        return Err("package construction omits a field");
    }
    Ok(())
}
