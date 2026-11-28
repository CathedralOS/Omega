//! Replay the per-construction type substitution and the scope of each
//! subsequent opening. The operational record and the descriptor in it keep
//! the same representation throughout.
use std::collections::BTreeMap;

use semantic_vocabulary::{
    CallableContractId, MachineId, OperationId, ScalarType, StructuralFieldId, StructuralTypeId,
    ValueId,
};
use terminal_psi::{
    Operation, OperationKind, RecordFieldValue, StructuralAccess, StructuralArgument,
    StructuralFieldDeclaration, StructuralFieldType, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeShape, TerminalMachine, TerminalModule,
    TerminalPackageEvent, TerminalPackageOpening, TerminalPackageSchema,
};

mod construction;
mod fields;
mod opening;
mod store;
mod substitution;
mod versions;

use construction::construction;
use fields::{check_field_accesses, enters_package};
use opening::{invocation, projection};
use substitution::Substitution;

type Site = (MachineId, OperationId);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Projected {
    Descriptor(ValueId),
    Reference(semantic_vocabulary::PlaceId),
}
type ProjectedOrigins = BTreeMap<(MachineId, Projected), (u32, OperationId)>;

#[derive(Default, Debug)]
pub struct CheckedPackageSites {
    fields: BTreeMap<(MachineId, OperationId, StructuralFieldId), StructuralFieldType>,
    calls: BTreeMap<Site, CallableContractId>,
    stored: BTreeMap<Site, StructuralFieldType>,
    repacked_fields: BTreeMap<(MachineId, OperationId, StructuralFieldId), OperationId>,
    projected_fields: BTreeMap<Site, StructuralFieldId>,
    descriptors: BTreeMap<Site, (u32, CallableContractId)>,
    borrowed_arguments: BTreeMap<(MachineId, OperationId, usize), StructuralTypeId>,
}

impl CheckedPackageSites {
    pub fn field_type(
        &self,
        machine: MachineId,
        operation: OperationId,
        field: StructuralFieldId,
    ) -> Option<&StructuralFieldType> {
        self.fields.get(&(machine, operation, field))
    }

    /// The destination field after substituting this package's live witness.
    pub fn stored_field_type(
        &self,
        machine: MachineId,
        operation: OperationId,
    ) -> Option<&StructuralFieldType> {
        self.stored.get(&(machine, operation))
    }

    pub fn stored_type(
        &self,
        machine: MachineId,
        operation: OperationId,
    ) -> Option<StructuralTypeId> {
        self.stored_field_type(machine, operation)?.as_structural()
    }

    /// The projection that supplied a dependent operand for this repacking.
    /// Both operations belong to the same machine and checked storage version.
    pub fn repacked_field(
        &self,
        machine: MachineId,
        operation: OperationId,
        field: StructuralFieldId,
    ) -> Option<OperationId> {
        self.repacked_fields
            .get(&(machine, operation, field))
            .copied()
    }

    pub fn projected_field(
        &self,
        machine: MachineId,
        operation: OperationId,
    ) -> Option<StructuralFieldId> {
        self.projected_fields.get(&(machine, operation)).copied()
    }

    /// Check the concrete contract on the descriptor actually read. Repeated
    /// occurrences of one witness must denote one type. The call also checks
    /// the separately stored argument against the concrete machine parameter.
    pub fn admits_descriptor(
        &self,
        module: &TerminalModule,
        machine: MachineId,
        operation: OperationId,
        actual: CallableContractId,
    ) -> bool {
        let Some(&(index, abstract_contract)) = self.descriptors.get(&(machine, operation)) else {
            return false;
        };
        let Ok(opened) = opening(module, machine, index) else {
            return false;
        };
        opened_callable_admits(
            &module.execution.callables,
            &module.types.structural_types,
            &opened.witnesses,
            abstract_contract,
            actual,
        )
    }

    /// At a checked call, the declared type of the field borrowed from the
    /// opened package. Ordinary custody checking still validates the path.
    pub fn borrowed_argument(
        &self,
        machine: MachineId,
        operation: OperationId,
        index: usize,
    ) -> Option<StructuralTypeId> {
        self.borrowed_arguments
            .get(&(machine, operation, index))
            .copied()
    }

    pub fn invocation_contract(
        &self,
        machine: MachineId,
        operation: OperationId,
    ) -> Option<CallableContractId> {
        self.calls.get(&(machine, operation)).copied()
    }
}

/// Determine whether the descriptor at an already checked opening preserves
/// its entire callable contract under the inferred witness substitution.
pub fn opened_callable_admits(
    catalog: &terminal_psi::TerminalCallableCatalog,
    types: &[terminal_psi::StructuralTypeDeclaration],
    opened_witnesses: &[StructuralTypeId],
    abstract_contract: CallableContractId,
    actual: CallableContractId,
) -> bool {
    let (Some(required), Some(supplied)) = (
        catalog.contract(abstract_contract),
        catalog.contract(actual),
    ) else {
        return false;
    };
    let mut bindings = BTreeMap::new();
    for (from, to) in required
        .structural_parameters
        .iter()
        .zip(&supplied.structural_parameters)
    {
        if opened_witnesses.contains(&from.structural_type)
            && (terminal_psi::find_structural_type(types, to.structural_type).is_none()
                || bindings
                    .insert(from.structural_type, to.structural_type)
                    .is_some_and(|previous| previous != to.structural_type))
        {
            return false;
        }
    }
    let (public, actual_types): (Vec<_>, Vec<_>) = bindings.into_iter().unzip();
    Substitution {
        public: &public,
        actual: &actual_types,
    }
    .callable(catalog, abstract_contract, actual)
}

fn scalar_type(machine: &TerminalMachine, id: ValueId) -> Option<ScalarType> {
    let mut declarations = machine
        .parameters
        .iter()
        .chain(machine.block_parameters())
        .chain(
            machine
                .operations()
                .filter_map(|operation| operation.result.scalar_ref()),
        )
        .filter(|value| value.id == id);
    let value = declarations.next()?;
    declarations.next().is_none().then_some(value.scalar_type)
}

fn argument_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    argument: &StructuralArgument,
) -> Option<StructuralTypeId> {
    let root = machine.structural_place_type(argument.place)?;
    crate::runtime_structural_path_tip(module.types.structural_types.iter(), root, &argument.path)
        .map(|ty| ty.id)
}

fn fields(
    module: &TerminalModule,
    record: StructuralTypeId,
) -> Result<&[StructuralFieldDeclaration], &'static str> {
    match &module
        .structural_type(record)
        .ok_or("package has an unknown record")?
        .shape
    {
        StructuralTypeShape::Record { fields, .. } => Ok(fields),
        _ => Err("package must have an ordinary record shape"),
    }
}

fn schema(
    module: &TerminalModule,
    record: StructuralTypeId,
) -> Result<&TerminalPackageSchema, &'static str> {
    let schemas = &module.execution.packages.schemas;
    schemas
        .binary_search_by_key(&record, |row| row.record)
        .ok()
        .map(|i| &schemas[i])
        .ok_or("operation names an undeclared package")
}

fn opening(
    module: &TerminalModule,
    machine: MachineId,
    index: u32,
) -> Result<&TerminalPackageOpening, &'static str> {
    module
        .execution
        .packages
        .openings
        .get(index as usize)
        .filter(|row| row.machine == machine)
        .ok_or("package operation names another machine's opening")
}

pub fn check_package_sites(module: &TerminalModule) -> Result<CheckedPackageSites, &'static str> {
    let catalog = &module.execution.packages;
    let mut checked = CheckedPackageSites::default();
    if catalog.schemas.is_empty() && catalog.openings.is_empty() && catalog.sites.is_empty() {
        return Ok(checked);
    }
    if !catalog
        .schemas
        .windows(2)
        .all(|pair| pair[0].record < pair[1].record)
        || !catalog
            .sites
            .windows(2)
            .all(|pair| (pair[0].machine, pair[0].operation) < (pair[1].machine, pair[1].operation))
    {
        return Err("package tables have duplicated or unordered rows");
    }
    let mut abstract_types = Vec::new();
    let opaque = |id: &StructuralTypeId| {
        module.structural_type(*id).is_some_and(|ty| {
            matches!(ty.shape, StructuralTypeShape::Opaque)
                && ty.nominal_cleanup.is_none()
                && ty.requires.is_empty()
        })
    };
    for row in &catalog.schemas {
        if row.witnesses.is_empty() || !row.witnesses.iter().all(opaque) {
            return Err("package schema requires distinct abstract referents");
        }
        fields(module, row.record)?;
        abstract_types.extend_from_slice(&row.witnesses);
    }
    for row in &catalog.openings {
        let machine = module
            .machine(row.machine)
            .ok_or("package opening has an unknown owner")?;
        let declaration = schema(module, row.record)?;
        if argument_type(module, machine, &row.subject) != Some(row.record)
            || row.witnesses.len() != declaration.witnesses.len()
            || !row.witnesses.iter().all(opaque)
        {
            return Err("package opening lacks a typed subject and fresh witness identities");
        }
        abstract_types.extend_from_slice(&row.witnesses);
    }
    // Wire identifiers need not be dense at this point in admission.
    // Size this inventory by the number of actual binders.
    abstract_types.sort_unstable();
    if abstract_types.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("package declarations and openings must have distinct witness identities");
    }
    let mut projected = BTreeMap::new();
    let mut events = BTreeMap::new();
    for site in &catalog.sites {
        let machine = module
            .machine(site.machine)
            .ok_or("package operation has an unknown owner")?;
        let operation = machine
            .operation(site.operation)
            .ok_or("package site names an absent operation")?;
        events.insert((machine.id, operation.id), &site.event);
        match &site.event {
            TerminalPackageEvent::Construct { .. } => (),
            TerminalPackageEvent::Project { opening, field } => {
                let value = projection(module, machine, operation, *opening, *field, &mut checked)?;
                if projected
                    .insert((machine.id, value), (*opening, operation.id))
                    .is_some()
                {
                    return Err("two package fields claim one descriptor result");
                }
            }
            TerminalPackageEvent::Invoke { .. } | TerminalPackageEvent::Store { .. } => (),
        }
    }
    for machine in &module.machines {
        for block in &machine.blocks {
            if block
                .terminator
                .storage_uses()
                .into_iter()
                .any(|operand| enters_package(module, machine, operand))
            {
                return Err("terminator transports a package field outside its opening");
            }
        }
        for operation in machine.operations() {
            let event = events.get(&(machine.id, operation.id)).copied();
            if let Some(TerminalPackageEvent::Construct { witnesses }) = event {
                construction(
                    module,
                    machine,
                    operation,
                    witnesses,
                    &abstract_types,
                    &projected,
                    &mut checked,
                )?;
            }
            if let Some(TerminalPackageEvent::Invoke { opening }) = event {
                let contract = invocation(
                    module,
                    machine,
                    operation,
                    *opening,
                    &projected,
                    &mut checked,
                )?;
                checked.calls.insert((machine.id, operation.id), contract);
                checked
                    .descriptors
                    .insert((machine.id, operation.id), (*opening, contract));
            }
            if let Some(TerminalPackageEvent::Store { opening, field }) = event {
                let ty =
                    store::checked_store(module, machine, operation, *opening, *field, &projected)?;
                let site = (machine.id, operation.id);
                if let StructuralFieldType::Scalar(ScalarType::Callable(contract)) = &ty {
                    checked.descriptors.insert(site, (*opening, *contract));
                }
                checked.stored.insert(site, ty);
            }
            check_field_accesses(module, machine, operation, event, &checked)?;
            if matches!(operation.kind, OperationKind::EstablishRecord { .. })
                && operation
                    .result
                    .structural()
                    .is_some_and(|r| schema(module, r.structural_type).is_ok())
                && !matches!(event, Some(TerminalPackageEvent::Construct { .. }))
            {
                return Err("package construction lacks its witness substitution");
            }
            if let OperationKind::CallMachine { callee, .. } = operation.kind
                && (projected.contains_key(&(machine.id, Projected::Descriptor(callee)))
                    || scalar_type(machine, callee)
                        .and_then(|ty| match ty {
                            ScalarType::Callable(contract) => {
                                module.execution.callables.contract(contract)
                            }
                            _ => None,
                        })
                        .is_some_and(|contract| {
                            contract.structural_parameters.iter().any(|formal| {
                                abstract_types
                                    .binary_search(&formal.structural_type)
                                    .is_ok()
                            })
                        }))
                && !matches!(event, Some(TerminalPackageEvent::Invoke { .. }))
            {
                return Err("projected descriptor invocation lacks its opening");
            }
        }
    }
    Ok(checked)
}
