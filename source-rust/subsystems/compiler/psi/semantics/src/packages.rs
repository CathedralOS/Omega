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
mod owned;
mod requirements;
mod restoration;
mod returned;
pub(crate) use returned::{ReturnedReferences, returned_referents};
mod store;
mod subjects;
mod substitution;
mod versions;
mod witnesses;

use construction::construction;
use fields::{check_field_accesses, enters_package};
use opening::{invocation, projection};
pub use restoration::local_call_write_origins;
pub(crate) use restoration::local_mutable_field_homes;
use substitution::Substitution;

/// The hidden referent of the inline reference actually stored at this path.
/// Following the reference into its pointee no longer selects the stored loan.
pub fn hidden_reference_referent(
    module: &TerminalModule,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    fields::hidden_reference_field(module, root, path).map(|member| member.referent)
}

type Site = (MachineId, OperationId);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Projected {
    Descriptor(ValueId),
    Reference(semantic_vocabulary::PlaceId),
    Owner(semantic_vocabulary::PlaceId),
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

    /// A pack or a projection can rename the indices on its own field.
    /// Both sides retain the same field path and every domain's exact theory.
    pub(crate) fn field_memberships(
        &self,
        module: &TerminalModule,
        machine: MachineId,
        operation: &Operation,
        field: StructuralFieldId,
        public: &crate::structural_qualifications::Memberships,
        actual: &crate::structural_qualifications::Memberships,
    ) -> bool {
        if self
            .field_type(machine, operation.id, field)
            .and_then(StructuralFieldType::as_structural)
            .is_none()
        {
            return false;
        }
        let sites = &module.execution.packages.sites;
        let Ok(index) = sites.binary_search_by_key(&(machine, operation.id), |site| {
            (site.machine, site.operation)
        }) else {
            return false;
        };
        let (record, witnesses) = match &sites[index].event {
            TerminalPackageEvent::Construct { witnesses } => {
                let Some(result) = operation.result.structural() else {
                    return false;
                };
                (result.structural_type, witnesses.as_slice())
            }
            TerminalPackageEvent::Project {
                opening: index,
                field: selected,
            } if *selected == field => {
                let Ok(row) = opening(module, machine, *index) else {
                    return false;
                };
                (row.record, row.witnesses.as_slice())
            }
            _ => return false,
        };
        let Ok(schema) = schema(module, record) else {
            return false;
        };
        Substitution {
            public: &schema.witnesses,
            actual: witnesses,
        }
        .memberships(module, public, actual)
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

    /// Structural type produced by an independently checked projection of
    /// this operation's exact package field.
    pub fn projected_structural_type(
        &self,
        machine: MachineId,
        operation: OperationId,
    ) -> Option<StructuralTypeId> {
        let field = self.projected_field(machine, operation)?;
        self.field_type(machine, operation, field)?.as_structural()
    }

    /// Check the complete membership set before a declared owner field is
    /// moved into its opening. Only this operation's checked field can rename
    /// a carrier, and every live domain must follow that same substitution.
    pub fn moved_owner_carries(
        &self,
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
        public_type: StructuralTypeId,
        public: &crate::structural_qualifications::Memberships,
    ) -> bool {
        let OperationKind::MoveStructuralValue { source } = &operation.kind else {
            return false;
        };
        let Some(result) = operation.result.structural() else {
            return false;
        };
        let Some(field) = self.projected_field(machine.id, operation.id) else {
            return false;
        };
        let declared = machine
            .structural_place_type(source.place)
            .and_then(|root| {
                crate::canonical_static_projection(
                    module.types.structural_types.iter(),
                    root,
                    &source.path,
                )
                .map(|(_, selected)| selected.id)
            });
        declared == Some(public_type)
            && self.projected_structural_type(machine.id, operation.id)
                == Some(result.structural_type)
            && self.field_memberships(
                module,
                machine.id,
                operation,
                field,
                public,
                &crate::structural_qualifications::Memberships {
                    whole: result.qualifications.clone(),
                    projected: result.projected_qualifications.clone(),
                },
            )
    }

    pub(crate) fn invocation_witnesses<'m>(
        &self,
        module: &'m TerminalModule,
        machine: MachineId,
        operation: OperationId,
    ) -> Option<&'m [StructuralTypeId]> {
        let &(index, _) = self.descriptors.get(&(machine, operation))?;
        Some(&opening(module, machine, index).ok()?.witnesses)
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
        opened_callable_admits(module, &opened.witnesses, abstract_contract, actual)
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

/// Does this scalar read transport the declared descriptor of its exact
/// package field into the opened callable type? It establishes no equation
/// between the descriptor and a proof term.
pub fn projects_stored_callable(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    stored: ScalarType,
) -> bool {
    if !matches!(stored, ScalarType::Callable(_)) {
        return false;
    }
    let sites = &module.execution.packages.sites;
    let Ok(i) = sites.binary_search_by_key(&(machine.id, operation.id), |site| {
        (site.machine, site.operation)
    }) else {
        return false;
    };
    let TerminalPackageEvent::Project {
        opening: index,
        field,
    } = sites[i].event
    else {
        return false;
    };
    let Ok(opened) = opening(module, machine.id, index) else {
        return false;
    };
    fields(module, opened.record).is_ok_and(|fields| {
        fields
            .iter()
            .any(|decl| decl.id == field && decl.field_type.scalar_type() == Some(stored))
    }) && matches!(
        projection(
            module,
            machine,
            operation,
            index,
            field,
            &mut CheckedPackageSites::default()
        ),
        Ok(Projected::Descriptor(_))
    )
}

/// Determine whether the descriptor at an already checked opening preserves
/// its entire callable contract under the inferred witness substitution.
pub fn opened_callable_admits(
    module: &TerminalModule,
    opened_witnesses: &[StructuralTypeId],
    abstract_contract: CallableContractId,
    actual: CallableContractId,
) -> bool {
    let catalog = &module.execution.callables;
    let (Some(required), Some(supplied)) = (
        catalog.contract(abstract_contract),
        catalog.contract(actual),
    ) else {
        return false;
    };
    let mut bindings = BTreeMap::new();
    let arguments = required
        .structural_parameters
        .iter()
        .zip(&supplied.structural_parameters)
        .map(|(from, to)| (from.structural_type, to.structural_type));
    let result = required
        .result
        .structural()
        .zip(supplied.result.structural());
    for (from, to) in arguments.chain(result) {
        let mut bind = |formal, actual| {
            bindings
                .insert(formal, actual)
                .is_none_or(|previous| previous == actual)
        };
        if opened_witnesses.contains(&from) {
            if module.structural_type(to).is_none() || !bind(from, to) {
                return false;
            }
        } else if opened_witnesses
            .iter()
            .any(|&witness| fields::nominal_uses(module, from, witness))
        {
            let name = |id| {
                module
                    .structural_type(id)
                    .and_then(|ty| module.identities.text(ty.identity))
            };
            let Some(formals) = opened_witnesses
                .iter()
                .map(|&id| name(id))
                .collect::<Option<Vec<_>>>()
            else {
                return false;
            };
            let Some(arguments) = name(from).zip(name(to)).and_then(|(public, actual)| {
                application_identity::type_identity::package_applications::supplied(
                    public, actual, &formals,
                )
            }) else {
                return false;
            };
            for (formal, actual) in arguments {
                let Some(index) = formals.iter().position(|&name| name == formal) else {
                    return false;
                };
                let Some(identity) = module.identities.index_of(actual) else {
                    return false;
                };
                let mut candidates = module
                    .types
                    .structural_types
                    .iter()
                    .filter(|ty| ty.identity == identity);
                let Some(actual) = candidates.next() else {
                    return false;
                };
                if candidates.next().is_some() || !bind(opened_witnesses[index], actual.id) {
                    return false;
                }
            }
        }
    }
    let (public, actual_types): (Vec<_>, Vec<_>) = bindings.into_iter().unzip();
    Substitution {
        public: &public,
        actual: &actual_types,
    }
    .callable(module, abstract_contract, actual)
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
    }
    let mut binders = witnesses::WitnessBinders::from_catalog(catalog)?;
    let mut projected = BTreeMap::new();
    let mut events = BTreeMap::new();
    for site in &catalog.sites {
        let machine = module
            .machine(site.machine)
            .ok_or("package operation has an unknown owner")?;
        let operation = machine
            .operation(site.operation)
            .ok_or("package site names an absent operation")?;
        if let TerminalPackageEvent::Project { opening, .. }
        | TerminalPackageEvent::Invoke { opening }
        | TerminalPackageEvent::Store { opening, .. } = site.event
        {
            binders.check_use(module, machine, operation, opening)?;
        }
        events.insert((machine.id, operation.id), &site.event);
        let origin = match &site.event {
            TerminalPackageEvent::Project { opening, field } => Some((
                projection(module, machine, operation, *opening, *field, &mut checked)?,
                *opening,
            )),
            TerminalPackageEvent::Invoke { opening } => {
                opening::returned_owner(module, machine, operation, *opening)
                    .map(|owner| (owner, *opening))
            }
            TerminalPackageEvent::Construct { .. } | TerminalPackageEvent::Store { .. } => None,
        };
        if let Some((value, opening)) = origin
            && projected
                .insert((machine.id, value), (opening, operation.id))
                .is_some()
        {
            return Err("two package operations claim one opened result");
        }
    }
    binders.require_copy_evidence()?;
    let abstract_types = &binders.types;
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
                    abstract_types,
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
                                    || abstract_types.iter().any(|&witness| {
                                        fields::nominal_uses(
                                            module,
                                            formal.structural_type,
                                            witness,
                                        )
                                    })
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
