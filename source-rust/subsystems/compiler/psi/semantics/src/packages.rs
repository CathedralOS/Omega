//! Replay the per-construction type substitution and the scope of each
//! subsequent opening. The operational record and the descriptor in it keep
//! the same representation throughout.
use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{CallableContractId, MachineId, OperationId, ScalarType, StructuralFieldId, StructuralTypeId, ValueId};
use terminal_psi::{Operation, OperationKind, RecordFieldValue, StructuralAccess, StructuralArgument,
    StructuralFieldDeclaration, StructuralFieldType, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeShape, TerminalMachine, TerminalModule, TerminalPackageEvent,
    TerminalPackageOpening, TerminalPackageSchema};

type Site = (MachineId, OperationId);

#[derive(Default, Debug)]
pub struct CheckedPackageSites {
    fields: BTreeMap<(MachineId, OperationId, StructuralFieldId), StructuralFieldType>,
    calls: BTreeMap<Site, CallableContractId>,
}

impl CheckedPackageSites {
    pub fn field_type(&self, machine: MachineId, operation: OperationId, field: StructuralFieldId) -> Option<&StructuralFieldType> {
        self.fields.get(&(machine, operation, field))
    }

    pub fn invocation_contract(&self, machine: MachineId, operation: OperationId) -> Option<CallableContractId> {
        self.calls.get(&(machine, operation)).copied()
    }
}

fn scalar_type(machine: &TerminalMachine, id: ValueId) -> Option<ScalarType> {
    let mut declarations = machine.parameters.iter()
        .chain(machine.block_parameters())
        .chain(machine.operations().filter_map(|operation| operation.result.scalar_ref()))
        .filter(|value| value.id == id);
    let value = declarations.next()?;
    declarations.next().is_none().then_some(value.scalar_type)
}

fn argument_type(module: &TerminalModule, machine: &TerminalMachine, argument: &StructuralArgument) -> Option<StructuralTypeId> {
    let root = machine.structural_place_type(argument.place)?;
    crate::runtime_structural_path_tip(module.types.structural_types.iter(), root, &argument.path).map(|ty| ty.id)
}

fn fields(module: &TerminalModule, record: StructuralTypeId) -> Result<&[StructuralFieldDeclaration], &'static str> {
    match &module.structural_type(record).ok_or("package has an unknown record")?.shape {
        StructuralTypeShape::Record { fields, .. } => Ok(fields),
        _ => Err("package must have an ordinary record shape"),
    }
}

struct Substitution<'a> {
    public: &'a [StructuralTypeId],
    actual: &'a [StructuralTypeId],
}

impl Substitution<'_> {
    fn witness(&self, id: StructuralTypeId) -> Option<StructuralTypeId> {
        self.public.iter().position(|formal| *formal == id).map(|index| self.actual[index])
    }

    // A stored reference has fixed representation. Its access and every
    // declaration property other than the referent and nominal identity stay
    // identical. Owning an unknown value inline has no such rule.
    fn reference(&self, module: &TerminalModule, from: StructuralTypeId, to: StructuralTypeId) -> bool {
        let (Some(public), Some(actual)) = (module.structural_type(from), module.structural_type(to)) else { return false };
        let (StructuralTypeShape::Reference {referent, access}, StructuralTypeShape::Reference {referent: target, access: mode}) = (&public.shape, &actual.shape) else { return false };
        if self.witness(*referent) != Some(*target) || access != mode { return false; }
        let mut expected = public.clone();
        expected.id = actual.id;
        expected.identity = actual.identity;
        expected.declaration_identity = actual.declaration_identity;
        expected.shape = actual.shape.clone();
        expected == *actual
    }

    fn parameters(&self, parameters: &mut [StructuralParameterDeclaration]) -> bool {
        for parameter in parameters {
            if let Some(to) = self.witness(parameter.structural_type) {
                if parameter.access == StructuralAccess::Owned { return false; }
                parameter.structural_type = to;
            }
        }
        true
    }

    fn callable(&self, module: &TerminalModule, from: CallableContractId, to: CallableContractId) -> bool {
        let catalog = &module.execution.callables;
        let (Some(public), Some(actual)) = (catalog.contract(from), catalog.contract(to)) else { return false };
        let mut expected = public.clone();
        expected.id = actual.id;
        expected.identity = actual.identity;
        expected.proof.id = actual.proof.id;
        if !self.parameters(&mut expected.structural_parameters)
            || !self.parameters(&mut expected.proof.structural_parameters) { return false; }
        // This equality includes the proof header, checked effects, evidence
        // lanes, reference regions, scalar qualifications and result.
        expected == *actual
    }

    fn admits(&self, module: &TerminalModule, public: &StructuralFieldType, actual: &StructuralFieldType) -> bool {
        match (public, actual) {
            (StructuralFieldType::Structural(from), StructuralFieldType::Structural(to)) => {
                if let Some(StructuralTypeShape::Reference {referent, ..}) = module.structural_type(*from).map(|ty| &ty.shape)
                    && self.witness(*referent).is_some() {
                    self.reference(module, *from, *to)
                } else { from == to && self.witness(*from).is_none() }
            }
            (StructuralFieldType::Scalar(ScalarType::Callable(from)), StructuralFieldType::Scalar(ScalarType::Callable(to))) =>
                self.callable(module, *from, *to),
            _ => public == actual,
        }
    }
}

fn schema(module: &TerminalModule, record: StructuralTypeId) -> Result<&TerminalPackageSchema, &'static str> {
    let schemas = &module.execution.packages.schemas;
    schemas.binary_search_by_key(&record, |row| row.record).ok()
        .map(|i| &schemas[i]).ok_or("operation names an undeclared package")
}

fn opening(module: &TerminalModule, machine: MachineId, index: u32) -> Result<&TerminalPackageOpening, &'static str> {
    module.execution.packages.openings.get(index as usize).filter(|row| row.machine == machine)
        .ok_or("package operation names another machine's opening")
}

fn construction(module: &TerminalModule, machine: &TerminalMachine, operation: &Operation,
    witnesses: &[StructuralTypeId], abstract_types: &BTreeSet<StructuralTypeId>, result: &mut CheckedPackageSites) -> Result<(), &'static str> {
    let OperationKind::EstablishRecord {fields: supplied} = &operation.kind else { return Err("package construction is not record establishment") };
    let record = operation.result.structural().ok_or("package construction lacks a record result")?.structural_type;
    let schema = schema(module, record)?;
    if witnesses.len() != schema.witnesses.len() || witnesses.iter().any(|id| abstract_types.contains(id) || module.structural_type(*id).is_none()) {
        return Err("package construction requires the complete concrete witness tuple");
    }
    let substitutions = Substitution {public: &schema.witnesses, actual: witnesses};
    let mut declarations = fields(module, record)?.iter().filter(|field| !field.relevance.is_erased());
    for field in supplied {
        let declaration = declarations.next().filter(|decl| decl.id == field.field).ok_or("package construction fields differ from its declaration")?;
        let actual = match &field.value {
            RecordFieldValue::Scalar {value, ..} => {
                let ty = scalar_type(machine, *value).ok_or("package field lacks a unique scalar producer")?;
                if declaration.field_type.scalar_type() == Some(ty) { declaration.field_type.clone() }
                else { StructuralFieldType::Scalar(ty) }
            }
            RecordFieldValue::Structural(arg) if arg.access == StructuralAccess::Owned =>
                StructuralFieldType::Structural(argument_type(module, machine, arg).ok_or("package field operand has no structural type")?),
            _ => return Err("package construction does not own its initializer"),
        };
        if !substitutions.admits(module, &declaration.field_type, &actual) { return Err("package field differs from the shared witness substitution"); }
        result.fields.insert((machine.id, operation.id, field.field), actual);
    }
    if declarations.next().is_some() { return Err("package construction omits a field"); }
    Ok(())
}

fn projection(module: &TerminalModule, machine: &TerminalMachine, operation: &Operation, index: u32, field: StructuralFieldId, checked: &mut CheckedPackageSites) -> Result<ValueId, &'static str> {
    let opened = opening(module, machine.id, index)?;
    let schema = schema(module, opened.record)?;
    let declaration = fields(module, opened.record)?.iter().find(|decl| decl.id == field && !decl.relevance.is_erased())
        .ok_or("package projection names an absent field")?;
    let OperationKind::PrimitiveScalarRead {source, path} = &operation.kind else { return Err("package descriptor projection requires a scalar field read") };
    let Some((StructuralPathSegment::Field(identity), parent)) = path.split_last() else { return Err("package projection does not read a field") };
    if *source != opened.subject.place || !crate::structural_paths_select_same_location(parent, &opened.subject.path) || *identity != declaration.identity {
        return Err("package projection reads a field of another subject");
    }
    let value = operation.result.scalar().ok_or("package projection lacks a scalar result")?;
    let actual = StructuralFieldType::Scalar(value.scalar_type);
    let substitution = Substitution { public: &schema.witnesses, actual: &opened.witnesses };
    if !matches!(value.scalar_type, ScalarType::Callable(_)) || !substitution.admits(module, &declaration.field_type, &actual) {
        return Err("package projection publishes the wrong opened callable");
    }
    checked.fields.insert((machine.id, operation.id, field), actual);
    Ok(value.id)
}

fn subject_field<'a>(module: &'a TerminalModule, opening: &TerminalPackageOpening, arg: &StructuralArgument) -> Option<&'a StructuralFieldDeclaration> {
    if arg.place != opening.subject.place || arg.path.len() != opening.subject.path.len() + 2 { return None; }
    let (parent, tail) = arg.path.split_at(opening.subject.path.len());
    let [StructuralPathSegment::Field(field), StructuralPathSegment::Referent] = tail else { return None; };
    if !crate::structural_paths_select_same_location(parent, &opening.subject.path) { return None; }
    fields(module, opening.record).ok()?.iter().find(|decl| decl.identity == *field && !decl.relevance.is_erased())
}

// A descriptor captured from a mutable record cannot outlive a replacement
// of that record. In this fragment, descriptor production and use share a
// block, and every intervening operation must be free of writes and calls.
fn same_storage_version(machine: &TerminalMachine, projection: OperationId, call: OperationId) -> bool {
    for block in &machine.blocks {
        let Some(start) = block.operations.iter().position(|op| op.id == projection) else { continue };
        let Some(end) = block.operations.iter().position(|op| op.id == call) else { return false };
        if start >= end { return false; }
        return block.operations[start + 1..end].iter().all(|op| matches!(
            op.kind,
            OperationKind::PrimitiveScalarRead { .. } | OperationKind::IntegerConstant { .. }
                | OperationKind::BooleanConstant { .. }
        ));
    }
    false
}

fn invocation(module: &TerminalModule, machine: &TerminalMachine, operation: &Operation, index: u32, projected: &BTreeMap<(MachineId, ValueId), (u32, OperationId)>) -> Result<CallableContractId, &'static str> {
    let opened = opening(module, machine.id, index)?;
    let OperationKind::CallMachine {callee, contract, structural_arguments, ..} = &operation.kind else { return Err("package invocation is not an evaluated machine call") };
    let Some(&(origin, producer)) = projected.get(&(machine.id, *callee)) else { return Err("package invocation has no projected descriptor") };
    if origin != index || !same_storage_version(machine, producer, operation.id) { return Err("package invocation has a different opening or stale descriptor"); }
    let interface = module.execution.callables.contract(*contract).ok_or("package invocation lacks a callable contract")?;
    if scalar_type(machine, *callee) != Some(ScalarType::Callable(*contract)) || interface.structural_parameters.len() != structural_arguments.len() {
        return Err("package call differs from its projected callable");
    }
    let schema = schema(module, opened.record)?;
    for (formal, arg) in interface.structural_parameters.iter().zip(structural_arguments) {
        let Some(position) = opened.witnesses.iter().position(|ty| *ty == formal.structural_type) else { continue };
        let field = subject_field(module, opened, arg).ok_or("package argument comes from another opening")?;
        let Some(StructuralTypeShape::Reference { referent, access }) = field.field_type.as_structural().and_then(|ty| module.structural_type(ty)).map(|ty| &ty.shape) else {
            return Err("package argument is not a borrowed field");
        };
        if *referent != schema.witnesses[position] || *access != arg.access || arg.access != formal.access {
            return Err("package argument differs from its opened parameter");
        }
    }
    Ok(*contract)
}

pub fn check_package_sites(module: &TerminalModule) -> Result<CheckedPackageSites, &'static str> {
    let catalog = &module.execution.packages;
    let mut checked = CheckedPackageSites::default();
    if catalog.schemas.is_empty() && catalog.openings.is_empty() && catalog.sites.is_empty() { return Ok(checked); }
    if !catalog.schemas.windows(2).all(|pair| pair[0].record < pair[1].record)
        || !catalog.sites.windows(2).all(|pair| (pair[0].machine, pair[0].operation) < (pair[1].machine, pair[1].operation)) {
        return Err("package tables have duplicated or unordered rows");
    }
    let mut abstract_types = BTreeSet::new();
    let opaque = |id: &StructuralTypeId| module.structural_type(*id).is_some_and(|ty|
        matches!(ty.shape, StructuralTypeShape::Opaque) && ty.nominal_cleanup.is_none() && ty.requires.is_empty());
    for row in &catalog.schemas {
        if row.witnesses.is_empty() || !row.witnesses.iter().all(opaque) ||
            row.witnesses.iter().collect::<BTreeSet<_>>().len() != row.witnesses.len() { return Err("package schema requires distinct abstract referents"); }
        fields(module, row.record)?;
        abstract_types.extend(&row.witnesses);
    }
    for row in &catalog.openings {
        let machine = module.machine(row.machine).ok_or("package opening has an unknown owner")?;
        let declaration = schema(module, row.record)?;
        if argument_type(module, machine, &row.subject) != Some(row.record) ||
            row.witnesses.len() != declaration.witnesses.len() ||
            !row.witnesses.iter().all(|id| opaque(id) && abstract_types.insert(*id)) { return Err("package opening lacks a typed subject and fresh witness identities"); }
    }
    let mut projected = BTreeMap::new();
    let mut events = BTreeMap::new();
    for site in &catalog.sites {
        let machine = module.machine(site.machine).ok_or("package operation has an unknown owner")?;
        let operation = machine.operation(site.operation).ok_or("package site names an absent operation")?;
        events.insert((machine.id, operation.id), &site.event);
        match &site.event {
            TerminalPackageEvent::Construct {witnesses} => construction(module, machine, operation, witnesses, &abstract_types, &mut checked)?,
            TerminalPackageEvent::Project {opening,field} => {
                let value = projection(module, machine, operation, *opening, *field, &mut checked)?;
                if projected.insert((machine.id, value), (*opening, operation.id)).is_some() { return Err("two package fields claim one descriptor result"); }
            }
            TerminalPackageEvent::Invoke {..} => (),
        }
    }
    for machine in &module.machines {
        for operation in machine.operations() {
            let event = events.get(&(machine.id, operation.id)).copied();
            if let Some(TerminalPackageEvent::Invoke {opening}) = event {
                let contract = invocation(module, machine, operation, *opening, &projected)?;
                checked.calls.insert((machine.id, operation.id), contract);
            }
            if matches!(operation.kind, OperationKind::EstablishRecord {..})
                && operation.result.structural().is_some_and(|r| schema(module,r.structural_type).is_ok())
                && !matches!(event, Some(TerminalPackageEvent::Construct {..})) { return Err("package construction lacks its witness substitution"); }
            if let OperationKind::CallMachine {callee, ..} = operation.kind && projected.contains_key(&(machine.id, callee))
                && !matches!(event, Some(TerminalPackageEvent::Invoke {..})) { return Err("projected descriptor invocation lacks its opening"); }
        }
    }
    Ok(checked)
}
