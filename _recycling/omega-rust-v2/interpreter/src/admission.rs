//! Decide, before anything runs, whether a closed entry can execute here.
//!
//! Admission reads only the Psi unit. It follows the entry to every machine a
//! call can reach and refuses the whole entry unless each of those bodies is
//! one this executor runs: a single block of supported operations that ends
//! in a return or a terminal call, with no service reach, pause, crash route,
//! precondition or stored mutation, and with the checker's evidence for that
//! exact body. Nothing recursive is admitted, so an admitted entry terminates
//! and its schedule-1 work is known before it runs.
use crate::places::{Use, holds_reference, projected_type, same_type, supports};
use crate::predicates::evaluable;
use crate::values::{Value, scalar_conforms, schema};
use omega_psi::{
    BlockTerminator, CallArgument, CallTarget, CleanupAction, ContractClause, DeclarationId, LoweredMachine, LoweredPackage, MachineBody, MachineId,
    OperationKind, OperationResult, Parameter, PauseEffects, PlaceOrigin, ReferenceAccess, ReturnEdge, ScalarLiteral, Site, TypeId, TypeShape, ValueOperand,
    ValueOrigin,
};

/// The checker's judgment of one closed machine body in one immutable unit.
/// The driver constructs it from the checker's proof of that body and for
/// nothing else; admission compares both by address, so evidence for an equal
/// body in another unit does not apply.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CheckedBody<'unit> {
    unit: &'unit LoweredPackage,
    machine: &'unit LoweredMachine,
}

impl<'unit> CheckedBody<'unit> {
    pub(crate) fn new(
        unit: &'unit LoweredPackage,
        machine: &'unit LoweredMachine,
    ) -> Self {
        Self { unit, machine }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reason {
    /// The unit has no machine with this identity.
    UnknownMachine,
    /// No checker evidence covers this body.
    Unchecked,
    /// The body can reach itself through a call.
    Recursive,
    /// The body branches, jumps or exits externally.
    NotStraightLine,
    /// The machine owes a precondition, or the call owes one of its callee's.
    Precondition,
    /// The machine reaches a service.
    Reach,
    /// The machine can suspend or block.
    Pause,
    /// The machine publishes a crash route.
    CrashRoute,
    /// The body stores into a home it can mutate.
    Mutation,
    /// The operation, operand or edge form named here is not executed.
    Unsupported(&'static str),
    /// A type this executor holds no value of, used as named here.
    Type(&'static str),
    /// A data predicate has a step with no closed actual-value check.
    Predicate,
    /// The body disagrees with its own tables, as named here.
    Malformed(&'static str),
    /// The certified work does not fit its account.
    WorkOverflow,
    /// The entry's result can hold a reference, which could name storage of the run itself.
    EscapingReference,
}

/// A refused entry: the body that stopped admission, and the operation or
/// edge site when one did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Refusal {
    pub machine: DeclarationId,
    pub site: Option<Site>,
    pub reason: Reason,
}

impl std::fmt::Display for Refusal {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        let machine = &self.machine.path;
        match self.reason {
            Reason::UnknownMachine => write!(f, "`{machine}` has no closed body in this unit"),
            Reason::Unchecked => write!(f, "`{machine}` has no checked body evidence"),
            Reason::Recursive => write!(f, "`{machine}` can reach itself, and no ranking evidence is accepted here"),
            Reason::NotStraightLine => write!(f, "`{machine}` is not a straight-line body"),
            Reason::Precondition => write!(f, "`{machine}` owes a precondition with no evidence at this invocation"),
            Reason::Reach => write!(f, "`{machine}` reaches a service"),
            Reason::Pause => write!(f, "`{machine}` can suspend or block"),
            Reason::CrashRoute => write!(f, "`{machine}` publishes a crash route"),
            Reason::Mutation => write!(f, "`{machine}` mutates stored state"),
            Reason::Unsupported(form) => write!(f, "`{machine}` uses {form}, which this executor does not run"),
            Reason::Type(usage) => write!(f, "`{machine}` uses a type this executor cannot {usage}"),
            Reason::Predicate => write!(f, "`{machine}` establishes data whose predicate has no closed actual-value check"),
            Reason::Malformed(what) => write!(f, "`{machine}` is malformed: {what}"),
            Reason::WorkOverflow => write!(f, "`{machine}` needs more schedule-1 work than its account counts"),
            Reason::EscapingReference => write!(f, "`{machine}` returns a value that can hold a reference into the invocation"),
        }
    }
}

impl std::error::Error for Refusal {}

/// A call an admitted body makes, resolved to the roster slot of its callee.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AdmittedBody<'unit> {
    pub(crate) machine: &'unit LoweredMachine,
    /// Schedule-1 work of one activation, including everything it calls.
    pub(crate) work: u64,
}

/// A closed entry and every body it can reach, each admitted for execution.
#[derive(Debug)]
pub struct AdmittedEntry<'unit> {
    pub(crate) unit: &'unit LoweredPackage,
    /// The entry, then each callee in the order admission first reached it.
    pub(crate) roster: Vec<AdmittedBody<'unit>>,
    /// Roster slot of each machine of the unit, by its position in `unit.machines`.
    pub(crate) slots: Vec<Option<usize>>,
}

impl<'unit> AdmittedEntry<'unit> {
    pub fn entry(&self) -> &'unit LoweredMachine {
        self.roster[0].machine
    }

    /// The entry, then each callee in the order admission first reached it.
    #[cfg(test)]
    pub fn reached(&self) -> impl ExactSizeIterator<Item = &'unit LoweredMachine> + '_ {
        self.roster.iter().map(|body| body.machine)
    }

    /// The schedule-1 work of this entry. Every admitted body is one path with
    /// no loop, so this is both the maximum and the exact charge of a run that
    /// completes: its operations, its one edge, and the same for each call.
    pub fn certified_work(&self) -> u64 {
        self.roster[0].work
    }

    pub(crate) fn slot(
        &self,
        id: MachineId,
    ) -> Option<usize> {
        self.slots
            .get(machine_position(self.unit, id)?)
            .copied()
            .flatten()
    }
}

pub(crate) fn machine_position(
    unit: &LoweredPackage,
    id: MachineId,
) -> Option<usize> {
    let direct = usize::try_from(id.get() - 1).ok();
    direct
        .filter(|&index| unit.machines.get(index).is_some_and(|machine| machine.id == id))
        .or_else(|| unit.machines.iter().position(|machine| machine.id == id))
}

/// Admit `entry` of `unit` with the checker's evidence for its reachable bodies.
pub(crate) fn admit<'unit>(
    unit: &'unit LoweredPackage,
    entry: &DeclarationId,
    checked: &[CheckedBody<'unit>],
) -> Result<AdmittedEntry<'unit>, Refusal> {
    let mut found = unit
        .machines
        .iter()
        .enumerate()
        .filter(|(_, machine)| &machine.declaration == entry);
    let (Some((position, _)), None) = (found.next(), found.next()) else {
        // Several bodies of one declaration are specializations; the entry must name one.
        return Err(Refusal {
            machine: entry.clone(),
            site: None,
            reason: Reason::UnknownMachine,
        });
    };
    let mut admission = Admission {
        unit,
        checked,
        roster: Vec::new(),
        slots: vec![None; unit.machines.len()],
        active: vec![false; unit.machines.len()],
    };
    admission.body(position)?;
    // Only a snapshot of values crosses back to the caller. A byte view names
    // the caller's own frozen bytes; a reference could name a value this run made.
    let machine = &unit.machines[position];
    if machine.result_type.is_some_and(|ty| holds_reference(unit, ty)) {
        return Err(Refusal {
            machine: machine.declaration.clone(),
            site: Some(machine.body.diagnostics.body.clone()),
            reason: Reason::EscapingReference,
        });
    }
    Ok(AdmittedEntry {
        unit,
        roster: admission.roster,
        slots: admission.slots,
    })
}

struct Admission<'unit, 'evidence> {
    unit: &'unit LoweredPackage,
    checked: &'evidence [CheckedBody<'unit>],
    roster: Vec<AdmittedBody<'unit>>,
    slots: Vec<Option<usize>>,
    active: Vec<bool>,
}

/// What a body has defined so far, by table slot.
struct Defined {
    values: Vec<bool>,
    places: Vec<bool>,
}

impl<'unit> Admission<'unit, '_> {
    /// Admit the machine at `position` in the unit and return its roster slot.
    fn body(
        &mut self,
        position: usize,
    ) -> Result<usize, Refusal> {
        let unit = self.unit;
        let machine = &unit.machines[position];
        let refuse = |site: Option<&Site>, reason| Refusal {
            machine: machine.declaration.clone(),
            site: site.cloned(),
            reason,
        };
        // A body still being admitted has a slot already, so test for the cycle first.
        if self.active[position] {
            return Err(refuse(None, Reason::Recursive));
        }
        if let Some(slot) = self.slots[position] {
            return Ok(slot);
        }
        if !self
            .checked
            .iter()
            .any(|evidence| std::ptr::eq(evidence.unit, unit) && std::ptr::eq(evidence.machine, machine))
        {
            return Err(refuse(None, Reason::Unchecked));
        }
        let (contract, body) = (&machine.contract, &machine.body);
        let whole = Some(&body.diagnostics.body);
        if !contract.requires.is_empty() {
            return Err(refuse(contract.requires.first().map(|clause| &clause.site), Reason::Precondition));
        }
        if !contract.realized_reach.is_empty() || contract.reach.as_ref().is_some_and(|services| !services.is_empty()) {
            return Err(refuse(whole, Reason::Reach));
        }
        if contract.realized_pause != PauseEffects::default() || !body.suspension_sites.is_empty() || !body.call_acknowledgements.is_empty() {
            return Err(refuse(whole, Reason::Pause));
        }
        if !contract.crash_routes.is_empty() || !contract.callback_crash_routes.is_empty() {
            return Err(refuse(whole, Reason::CrashRoute));
        }
        if !body.mutable_homes.is_empty() {
            return Err(refuse(whole, Reason::Mutation));
        }
        if !body.callback_invocations.is_empty() {
            return Err(refuse(whole, Reason::Unsupported("a static callback invocation")));
        }
        let [block] = body.blocks.as_slice() else {
            return Err(refuse(whole, Reason::NotStraightLine));
        };
        if block.id.get() != 1
            || block.operations != (0..body.operations.len())
            || !block.scalar_parameters.is_empty()
            || !block.structural_parameters.is_empty()
            || !block.scalar_domain_invariants.is_empty()
        {
            return Err(refuse(whole, Reason::Malformed("the entry block does not own every operation")));
        }
        let malformed = |what| refuse(whole, Reason::Malformed(what));
        for ty in body
            .values
            .iter()
            .map(|value| value.ty)
            .chain(body.places.iter().map(|place| place.ty))
        {
            if !supports(unit, ty, Use::Hold) {
                return Err(refuse(whole, Reason::Type("hold")));
            }
        }
        let mut defined = Defined {
            values: vec![false; body.values.len()],
            places: vec![false; body.places.len()],
        };
        if machine.parameter_types.len() != body.parameters.len() {
            return Err(malformed("the formals disagree with the signature"));
        }
        for (parameter, &ty) in body.parameters.iter().zip(&machine.parameter_types) {
            let agrees = match *parameter {
                Parameter::Scalar(value) => body.values.get(value.index()).is_some_and(|row| {
                    row.origin == ValueOrigin::Parameter && same_type(unit, row.ty, ty) && !std::mem::replace(&mut defined.values[value.index()], true)
                }),
                Parameter::Structural(place) => body.places.get(place.index()).is_some_and(|row| {
                    row.origin == PlaceOrigin::Parameter && same_type(unit, row.ty, ty) && !std::mem::replace(&mut defined.places[place.index()], true)
                }),
            };
            if !agrees {
                return Err(malformed("a formal disagrees with its signature type"));
            }
        }
        let mut work = u64::try_from(body.operations.len())
            .ok()
            .and_then(|operations| operations.checked_add(1))
            .ok_or_else(|| refuse(whole, Reason::WorkOverflow))?;
        // The entry takes slot zero and each callee the next free slot when
        // admission first reaches it.
        let slot = self.roster.len();
        self.roster.push(AdmittedBody { machine, work: 0 });
        self.slots[position] = Some(slot);
        self.active[position] = true;
        for (index, operation) in body.operations.iter().enumerate() {
            let site = body.diagnostics.operations.get(index);
            if operation.id.index() != index {
                return Err(refuse(site, Reason::Malformed("an operation identity disagrees with its slot")));
            }
            let reason = match self.operation(machine, &defined, &operation.kind, operation.result) {
                Ok(None) => None,
                Ok(Some(callee)) => {
                    let callee = self.body(callee)?;
                    work = work
                        .checked_add(self.roster[callee].work)
                        .ok_or_else(|| refuse(site, Reason::WorkOverflow))?;
                    None
                }
                Err(reason) => Some(reason),
            };
            if let Some(reason) = reason {
                return Err(refuse(site, reason));
            }
            let fresh = match operation.result {
                OperationResult::Unit => true,
                OperationResult::Scalar(value) => {
                    body.values[value.index()].origin == ValueOrigin::Operation(operation.id) && !std::mem::replace(&mut defined.values[value.index()], true)
                }
                OperationResult::Structural(place) => {
                    body.places[place.index()].origin == PlaceOrigin::Operation(operation.id) && !std::mem::replace(&mut defined.places[place.index()], true)
                }
            };
            if !fresh {
                return Err(refuse(site, Reason::Malformed("a result does not name its defining operation")));
            }
        }
        let edge_site = |id: omega_psi::EdgeId| body.diagnostics.edges.get(id.get() - 1);
        match &block.terminator {
            BlockTerminator::Return { id, result, cleanup } => {
                let agrees = match (*result, machine.result_type) {
                    (ReturnEdge::Unit, None) => true,
                    (ReturnEdge::Scalar(value), Some(ty)) => {
                        defined.values.get(value.index()) == Some(&true) && same_type(unit, body.values[value.index()].ty, ty)
                    }
                    (ReturnEdge::Structural { source, result }, Some(ty)) => {
                        defined.places.get(source.index()) == Some(&true)
                            && same_type(unit, body.places[source.index()].ty, ty)
                            && body
                                .places
                                .get(result.index())
                                .is_some_and(|row| row.origin == PlaceOrigin::Result && same_type(unit, row.ty, ty))
                    }
                    _ => false,
                };
                if !agrees {
                    return Err(refuse(edge_site(*id), Reason::Malformed("the return disagrees with the result type")));
                }
                cleanup_supported(&defined, cleanup).map_err(|reason| refuse(edge_site(*id), reason))?;
            }
            BlockTerminator::TerminalCall { completion: Some(_), id, .. } => {
                return Err(refuse(edge_site(*id), Reason::NotStraightLine));
            }
            BlockTerminator::TerminalCall {
                completion: None,
                id,
                callee,
                arguments,
                requirements,
                cleanup,
            } => {
                let site = edge_site(*id);
                if !requirements.is_empty() {
                    return Err(refuse(site, Reason::Precondition));
                }
                let callee = self
                    .call(body, &defined, callee, arguments, machine.result_type)
                    .map_err(|reason| refuse(site, reason))?;
                cleanup_supported(&defined, cleanup).map_err(|reason| refuse(site, reason))?;
                let callee = self.body(callee)?;
                work = work
                    .checked_add(self.roster[callee].work)
                    .ok_or_else(|| refuse(site, Reason::WorkOverflow))?;
            }
            BlockTerminator::Conditional { .. } | BlockTerminator::Jump(_) | BlockTerminator::ExternalExit(_) => {
                return Err(refuse(whole, Reason::NotStraightLine));
            }
        }
        self.active[position] = false;
        self.roster[slot].work = work;
        Ok(slot)
    }

    /// Check one operation. A call returns the unit position of its callee.
    fn operation(
        &self,
        machine: &'unit LoweredMachine,
        defined: &Defined,
        kind: &OperationKind,
        result: OperationResult,
    ) -> Result<Option<usize>, Reason> {
        let unit = self.unit;
        let body = &machine.body;
        let structural = || match result {
            OperationResult::Structural(place) => body
                .places
                .get(place.index())
                .map(|row| row.ty)
                .ok_or(Reason::Malformed("a result place is out of range")),
            _ => Err(Reason::Malformed("a structural operation has no result place")),
        };
        let value_type = |value: omega_psi::ValueId| {
            (defined.values.get(value.index()) == Some(&true))
                .then(|| body.values[value.index()].ty)
                .ok_or(Reason::Malformed("a value is read before its definition"))
        };
        let place_type = |place: omega_psi::PlaceId| {
            (defined.places.get(place.index()) == Some(&true))
                .then(|| body.places[place.index()].ty)
                .ok_or(Reason::Malformed("a place is read before its definition"))
        };
        match kind {
            OperationKind::Constant(literal) => {
                let OperationResult::Scalar(value) = result else {
                    return Err(Reason::Malformed("a constant has no scalar result"));
                };
                let row = body
                    .values
                    .get(value.index())
                    .ok_or(Reason::Malformed("a result value is out of range"))?;
                let Some(TypeShape::PrimitiveScalar(scalar)) = unit.types.get(row.ty) else {
                    return Err(Reason::Type("form a constant of"));
                };
                if !scalar_conforms(&literal_value(literal), *scalar) {
                    return Err(Reason::Malformed("a constant lies outside its carrier"));
                }
            }
            OperationKind::Integer { operation, operands } if operation.is_total_comparison() => {
                let [left, right] = operands.as_slice() else {
                    return Err(Reason::Malformed("integer comparison arity"));
                };
                let left = unit.types.get(value_type(*left)?);
                let OperationResult::Scalar(value) = result else {
                    return Err(Reason::Malformed("expected Boolean comparison result"));
                };
                if !matches!(left, Some(TypeShape::PrimitiveScalar(omega_psi::ScalarType::Integer { .. })))
                    || unit.types.get(value_type(*right)?) != left
                    || body
                        .values
                        .get(value.index())
                        .and_then(|row| unit.types.get(row.ty))
                        != Some(&TypeShape::PrimitiveScalar(omega_psi::ScalarType::Boolean))
                {
                    return Err(Reason::Malformed("integer comparison carrier mismatch"));
                }
            }
            OperationKind::CopyStructural { source } => {
                place_type(source.root)?;
                let selected = projected_type(unit, body, source).ok_or(Reason::Unsupported("a copy through this projection"))?;
                if !same_type(unit, selected, structural()?) {
                    return Err(Reason::Malformed("a copy disagrees with its result type"));
                }
                if !supports(unit, selected, Use::Copy) {
                    return Err(Reason::Type("copy"));
                }
            }
            OperationKind::MoveStructural { source } => {
                if !source.path.is_empty() {
                    return Err(Reason::Unsupported("a partial move"));
                }
                if !same_type(unit, place_type(source.root)?, structural()?) {
                    return Err(Reason::Malformed("a move disagrees with its result type"));
                }
            }
            OperationKind::EstablishRecord { fields } => {
                let Some(TypeShape::Nominal(id)) = unit.types.get(structural()?) else {
                    return Err(Reason::Type("establish a record of"));
                };
                let data = schema(unit, *id).ok_or(Reason::Malformed("a record has no schema"))?;
                if !data.cases.is_empty() {
                    return Err(Reason::Malformed("a record establishment names a sum"));
                }
                self.initializers(body, defined, &data.fields, fields)?;
                clauses_evaluable(&data.requires)?;
            }
            OperationKind::EstablishCase { case, fields } => {
                let Some(TypeShape::Nominal(id)) = unit.types.get(structural()?) else {
                    return Err(Reason::Type("establish a case of"));
                };
                let data = schema(unit, *id).ok_or(Reason::Malformed("a case has no schema"))?;
                let selected = data
                    .cases
                    .iter()
                    .find(|candidate| &candidate.declaration == case)
                    .ok_or(Reason::Malformed("a case is absent from its sum"))?;
                self.initializers(body, defined, &selected.fields, fields)?;
                clauses_evaluable(&data.requires)?;
            }
            OperationKind::EstablishZeroedStructural => zero_supported(unit, structural()?)?,
            OperationKind::EstablishStructuralArray { elements } => {
                let element = match unit.types.get(structural()?) {
                    Some(TypeShape::FixedArray { element, length }) if u64::try_from(elements.len()) == Ok(*length) => *element,
                    Some(TypeShape::ElementSequence { element }) => *element,
                    _ => return Err(Reason::Malformed("an array roster disagrees with its result type")),
                };
                for &place in elements {
                    if !same_type(unit, place_type(place)?, element) {
                        return Err(Reason::Malformed("an array element disagrees with its type"));
                    }
                }
            }
            OperationKind::EstablishScalarArray { elements } => {
                let scalar = match unit.types.get(structural()?) {
                    Some(TypeShape::FixedArray { element, length }) if u64::try_from(elements.len()) == Ok(*length) => match unit.types.get(*element) {
                        Some(TypeShape::PrimitiveScalar(scalar)) => *scalar,
                        _ => return Err(Reason::Malformed("a scalar array has structural elements")),
                    },
                    Some(TypeShape::ElementSequence { element }) => match unit.types.get(*element) {
                        Some(TypeShape::PrimitiveScalar(scalar)) => *scalar,
                        _ => return Err(Reason::Malformed("a scalar sequence has structural elements")),
                    },
                    _ => return Err(Reason::Malformed("a scalar roster disagrees with its result type")),
                };
                for &value in elements {
                    if unit.types.get(value_type(value)?) != Some(&TypeShape::PrimitiveScalar(scalar)) {
                        return Err(Reason::Malformed("a scalar element disagrees with its type"));
                    }
                }
            }
            OperationKind::Call {
                callee,
                arguments,
                requirements,
            } => {
                if !requirements.is_empty() {
                    return Err(Reason::Precondition);
                }
                let returned = match result {
                    OperationResult::Unit => None,
                    OperationResult::Scalar(value) => Some(
                        body.values
                            .get(value.index())
                            .ok_or(Reason::Malformed("a result value is out of range"))?
                            .ty,
                    ),
                    OperationResult::Structural(_) => Some(structural()?),
                };
                return self.call(body, defined, callee, arguments, returned).map(Some);
            }
            other => return Err(Reason::Unsupported(operation_name(other))),
        }
        Ok(None)
    }

    /// Check the operands of a record or case against its declared fields.
    fn initializers(
        &self,
        body: &MachineBody,
        defined: &Defined,
        declared: &[omega_psi::LoweredField],
        fields: &[omega_psi::FieldInitializer],
    ) -> Result<(), Reason> {
        if declared.len() != fields.len() {
            return Err(Reason::Malformed("an establishment omits a declared field"));
        }
        for (declared, field) in declared.iter().zip(fields) {
            if declared.declaration != field.field {
                return Err(Reason::Malformed("an establishment lists its fields out of declaration order"));
            }
            if matches!(field.operand, ValueOperand::Borrow { .. }) {
                return Err(Reason::Unsupported("a stored borrow"));
            }
            if !operand_agrees(self.unit, body, defined, &field.operand, declared.carrier)? {
                return Err(Reason::Malformed("a field operand disagrees with its declared type"));
            }
        }
        Ok(())
    }

    /// Check a call against its callee's signature and return the callee's unit position.
    fn call(
        &self,
        body: &MachineBody,
        defined: &Defined,
        callee: &CallTarget,
        arguments: &[CallArgument],
        returned: Option<TypeId>,
    ) -> Result<usize, Reason> {
        let unit = self.unit;
        let CallTarget::Machine(id) = callee else {
            return Err(Reason::Unsupported("a call that does not name a packaged machine"));
        };
        let position = machine_position(unit, *id).ok_or(Reason::Malformed("a call names no machine of this unit"))?;
        let target = &unit.machines[position];
        if target.parameter_types.len() != arguments.len() {
            return Err(Reason::Malformed("a call disagrees with its callee's formals"));
        }
        for (argument, &formal) in arguments.iter().zip(&target.parameter_types) {
            if !operand_agrees(unit, body, defined, argument, formal)? {
                return Err(Reason::Malformed("a call argument disagrees with its formal"));
            }
        }
        let agrees = match (returned, target.result_type) {
            (None, None) => true,
            (Some(left), Some(right)) => same_type(unit, left, right),
            _ => false,
        };
        if !agrees {
            return Err(Reason::Malformed("a call disagrees with its callee's result"));
        }
        Ok(position)
    }
}

/// Whether an operand delivers `expected`: its value, a copy, a transferred
/// place, or a shared reference to a whole completed record or sum.
fn operand_agrees(
    unit: &LoweredPackage,
    body: &MachineBody,
    defined: &Defined,
    operand: &ValueOperand,
    expected: TypeId,
) -> Result<bool, Reason> {
    let place = |place: omega_psi::PlaceId| {
        (defined.places.get(place.index()) == Some(&true))
            .then(|| body.places[place.index()].ty)
            .ok_or(Reason::Malformed("a place is read before its definition"))
    };
    Ok(match operand {
        ValueOperand::Scalar(value) => {
            let ty = (defined.values.get(value.index()) == Some(&true))
                .then(|| body.values[value.index()].ty)
                .ok_or(Reason::Malformed("a value is read before its definition"))?;
            same_type(unit, ty, expected)
        }
        ValueOperand::CopyWhole(source) => {
            let ty = place(*source)?;
            if !supports(unit, ty, Use::Copy) {
                return Err(Reason::Type("copy"));
            }
            same_type(unit, ty, expected)
        }
        ValueOperand::MoveWhole(source) => same_type(unit, place(*source)?, expected),
        ValueOperand::Borrow {
            place: source,
            access: ReferenceAccess::Shared,
        } => {
            place(source.root)?;
            let referent = projected_type(unit, body, source).ok_or(Reason::Unsupported("a borrow through this projection"))?;
            matches!(
                (unit.types.get(referent), unit.types.get(expected)),
                (Some(TypeShape::Nominal(borrowed)), Some(TypeShape::NominalReference { referent, access: ReferenceAccess::Shared })) if borrowed == referent
            )
        }
        ValueOperand::Borrow { .. } => return Err(Reason::Unsupported("an exclusive or write-only borrow")),
    })
}

fn clauses_evaluable(clauses: &[ContractClause]) -> Result<(), Reason> {
    clauses
        .iter()
        .all(evaluable)
        .then_some(())
        .ok_or(Reason::Predicate)
}

/// A whole-type zero: scalars, records of them and fixed arrays of either.
/// Every schema inside owes its own predicates for the all-zero value.
pub(crate) fn zero_supported(
    unit: &LoweredPackage,
    ty: TypeId,
) -> Result<(), Reason> {
    let mut pending = vec![ty];
    while let Some(ty) = pending.pop() {
        match unit.types.get(ty) {
            Some(TypeShape::PrimitiveScalar(_)) => {}
            Some(TypeShape::FixedArray { element, .. }) => pending.push(*element),
            Some(TypeShape::Nominal(id)) => {
                let data = schema(unit, *id).ok_or(Reason::Malformed("a zeroed record has no schema"))?;
                if !data.cases.is_empty() {
                    return Err(Reason::Unsupported("a whole-type zero of a sum"));
                }
                clauses_evaluable(&data.requires)?;
                pending.extend(data.fields.iter().map(|field| field.carrier));
            }
            _ => return Err(Reason::Unsupported("a whole-type zero of this type")),
        }
    }
    Ok(())
}

/// Admitted cleanup discards storage the frame still holds. It runs no machine.
fn cleanup_supported(
    defined: &Defined,
    cleanup: &[CleanupAction],
) -> Result<(), Reason> {
    for action in cleanup {
        let root = match action {
            CleanupAction::DiscardRoot(place) | CleanupAction::RetireHome(place) => *place,
            CleanupAction::DiscardResidual { .. } => return Err(Reason::Unsupported("cleanup of a partly moved value")),
            CleanupAction::Invoke { .. } => return Err(Reason::Unsupported("cleanup that invokes a machine")),
        };
        if defined.places.get(root.index()) != Some(&true) {
            return Err(Reason::Malformed("cleanup names an undefined place"));
        }
    }
    Ok(())
}

pub(crate) fn literal_value(literal: &ScalarLiteral) -> Value {
    match literal {
        ScalarLiteral::Integer(value) => Value::Integer(value.clone()),
        ScalarLiteral::Boolean(value) => Value::Boolean(*value),
        ScalarLiteral::IeeeFloat(bits) => Value::Float(*bits),
    }
}

fn operation_name(kind: &OperationKind) -> &'static str {
    match kind {
        OperationKind::BeginAssembly(_) | OperationKind::AssemblyInstruction(_) => "assembly",
        OperationKind::Boolean { .. } => "a Boolean operation",
        OperationKind::ExactNumericCast { .. } => "a numeric cast",
        OperationKind::Float { .. } => "a float operation",
        OperationKind::TrappingInteger { .. } => "a trapping integer operation",
        OperationKind::Integer { .. } => "an integer operation",
        OperationKind::ConstructCallableDescriptor { .. } | OperationKind::RefineCallableDescriptor { .. } => "a callable descriptor",
        OperationKind::DescribeMachine(_) => "a declaration description",
        OperationKind::ApplyTheorem(_) => "a theorem application",
        OperationKind::ByteSequenceLength { .. } | OperationKind::ByteSequenceRead { .. } => "a byte-view read",
        OperationKind::BorrowPlace { .. } => "a stored borrow",
        OperationKind::ScalarBitRecast { .. } | OperationKind::MutableScalarBitRecast { .. } => "a recast",
        OperationKind::BorrowedMove { .. } => "a move through a loan",
        OperationKind::StructuralStore { .. } | OperationKind::StructuralScalarFieldStore { .. } | OperationKind::ProjectedScalarStore { .. } => "a store",
        OperationKind::ElementViewLength { .. } | OperationKind::ElementViewRead { .. } | OperationKind::ElementViewWrite { .. } => "an element-view operation",
        OperationKind::DecomposeRecord { .. } => "a record decomposition",
        OperationKind::StructuralCaseMembership { .. } => "a case test",
        OperationKind::ProjectedScalarRead { .. } | OperationKind::PrimitiveScalarRead { .. } | OperationKind::StructuralLeafCopy { .. } => "a scalar read",
        OperationKind::EstablishPrimitiveLocal { .. } => "a primitive local home",
        OperationKind::Constant(_)
        | OperationKind::Call { .. }
        | OperationKind::EstablishZeroedStructural
        | OperationKind::EstablishCase { .. }
        | OperationKind::EstablishRecord { .. }
        | OperationKind::CopyStructural { .. }
        | OperationKind::MoveStructural { .. }
        | OperationKind::EstablishScalarArray { .. }
        | OperationKind::EstablishStructuralArray { .. } => "an admitted operation",
    }
}
