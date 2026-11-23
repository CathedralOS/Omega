//! Run an admitted entry on the sponsor's actual arguments.
//!
//! Every reached operation and edge is charged before it runs, and every cell
//! is reserved before the value that needs it exists. The executor never
//! stores into a completed value, so a shared reference always reads the value
//! it was formed over. A value that contradicts what admission accepted stops
//! the invocation; it is never repaired or published.
use crate::admission::{AdmittedEntry, literal_value};
use crate::meter::{CELL_SCHEDULE, Ceilings, ChargeSite, Meter, MeterStop, SiteUsage, Usage, WORK_SCHEDULE};
use crate::places::projected_value;
use crate::predicates::holds;
use crate::values::{Argument, Value, conforms, schema, zero_scalar};
use omega_psi::{
    BlockTerminator, CallArgument, CallTarget, CleanupAction, DeclarationId, FieldInitializer, LoweredMachine, LoweredPackage, OperationKind, OperationResult,
    Parameter, PlaceId, ReturnEdge, Site, TypeId, TypeShape, ValueOperand,
};
use std::sync::Arc;

/// Where an invocation stopped: the active machines from the entry inward,
/// the charge site, and that site's source occurrence when the body kept one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Location {
    pub chain: Vec<DeclarationId>,
    pub site: Option<ChargeSite>,
    pub source: Option<Site>,
}

/// An invocation that produced no result. None of these is a machine result,
/// and none says anything about the program's termination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Stop {
    /// The sponsor supplied a different number of arguments than the entry has formals.
    Arity { expected: usize, supplied: usize },
    /// The argument at this position is not a completed value of its formal's type.
    Argument { parameter: usize },
    /// The work budget or the live-cell ceiling refused the next charge.
    Resource { stop: MeterStop, at: Location },
    /// The host could not allocate storage the ceilings allowed.
    Host { at: Location },
    /// A value at this site contradicts the admitted body or its checked
    /// evidence, such as a false data predicate or a place read after its move.
    Inconsistent { what: &'static str, at: Location },
}

impl std::fmt::Display for Stop {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        let place = |f: &mut std::fmt::Formatter<'_>, at: &Location| {
            if let Some(machine) = at.chain.last() {
                write!(f, " in `{}`", machine.path)?;
            }
            match at.site {
                Some(site) => write!(f, " at {site}"),
                None => Ok(()),
            }
        };
        match self {
            Self::Arity { expected, supplied } => write!(f, "the entry takes {expected} arguments and received {supplied}"),
            Self::Argument { parameter } => write!(f, "argument {parameter} is not a completed value of its formal's type"),
            Self::Resource {
                stop: MeterStop::WorkExhausted { used, remaining, unpaid },
                at,
            } => {
                write!(
                    f,
                    "work budget exhausted: {used} used, {remaining} remaining; the next charge requires {unpaid} (schedule {WORK_SCHEDULE})"
                )?;
                place(f, at)
            }
            Self::Resource {
                stop: MeterStop::LiveCells { live, requested, ceiling },
                at,
            } => {
                write!(
                    f,
                    "live-cell ceiling reached: {live} live, {requested} more requested, ceiling {ceiling} (cell schedule {CELL_SCHEDULE})"
                )?;
                place(f, at)
            }
            Self::Resource { stop: MeterStop::Overflow, at } => {
                write!(f, "a resource counter left the range of its account")?;
                place(f, at)
            }
            Self::Host { at } => {
                write!(f, "the host could not allocate the admitted storage")?;
                place(f, at)
            }
            Self::Inconsistent { what, at } => {
                write!(f, "execution contradicts its checked evidence: {what}")?;
                place(f, at)
            }
        }
    }
}

impl std::error::Error for Stop {}

/// A completed invocation: the entry's result and what producing it used.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Completed {
    /// `None` for an entry that returns no value.
    pub result: Option<Value>,
    pub usage: Usage,
}

struct Frame<'unit> {
    slot: usize,
    machine: &'unit LoweredMachine,
    values: Vec<Option<Value>>,
    places: Vec<Option<Arc<Value>>>,
    /// Cells this frame holds: itself, its scalar values and its bound places.
    held: u64,
    /// The operation to run next, or the call whose result this frame awaits.
    next: usize,
}

/// A failure before its location is known.
enum Fault {
    Meter(MeterStop),
    Host,
    Inconsistent(&'static str),
}

impl From<MeterStop> for Fault {
    fn from(stop: MeterStop) -> Self {
        Self::Meter(stop)
    }
}

type Step<T> = Result<T, Fault>;

fn missing() -> Fault {
    Fault::Inconsistent("a value or place is read after its move or before its definition")
}

impl<'unit> Frame<'unit> {
    fn place(
        &self,
        place: PlaceId,
    ) -> Step<&Arc<Value>> {
        self.places
            .get(place.index())
            .and_then(Option::as_ref)
            .ok_or_else(missing)
    }

    fn place_type(
        &self,
        place: PlaceId,
    ) -> Step<TypeId> {
        self.machine
            .body
            .places
            .get(place.index())
            .map(|row| row.ty)
            .ok_or_else(missing)
    }

    fn scalar(
        &self,
        value: omega_psi::ValueId,
    ) -> Step<&Value> {
        self.values
            .get(value.index())
            .and_then(Option::as_ref)
            .ok_or_else(missing)
    }

    /// The cells a bound place holds.
    fn place_cells(
        &self,
        unit: &LoweredPackage,
        place: PlaceId,
    ) -> Step<u64> {
        self.place(place)?
            .cells(unit, self.place_type(place)?)
            .ok_or(Fault::Meter(MeterStop::Overflow))
    }

    /// New cells an operand's delivered value needs. A moved place needs none:
    /// its cells travel with it.
    fn operand_cells(
        &self,
        unit: &LoweredPackage,
        operand: &ValueOperand,
    ) -> Step<u64> {
        match operand {
            ValueOperand::Scalar(value) => self.scalar(*value).map(|_| 1),
            ValueOperand::CopyWhole(place) => self.place_cells(unit, *place),
            ValueOperand::MoveWhole(place) => self.place(*place).map(|_| 0),
            ValueOperand::Borrow { place, .. } => self.place(place.root).map(|_| 1),
        }
    }

    /// Deliver an operand. A moved place leaves this frame with its cells.
    fn operand(
        &mut self,
        unit: &LoweredPackage,
        operand: &ValueOperand,
    ) -> Step<Value> {
        match operand {
            ValueOperand::Scalar(value) => self.scalar(*value).cloned(),
            ValueOperand::CopyWhole(place) => self.place(*place).map(|value| Value::clone(value)),
            ValueOperand::MoveWhole(place) => {
                let cells = self.place_cells(unit, *place)?;
                let value = self.places[place.index()].take().ok_or_else(missing)?;
                self.held = self
                    .held
                    .checked_sub(cells)
                    .ok_or(Fault::Meter(MeterStop::Overflow))?;
                // A shared reference formed over this value keeps reading it.
                Ok(Arc::try_unwrap(value).unwrap_or_else(|shared| Value::clone(&shared)))
            }
            ValueOperand::Borrow { place, .. } => {
                let root = self.place(place.root)?;
                if place.path.is_empty() {
                    return Ok(Value::Shared(Arc::clone(root)));
                }
                let selected = projected_value(unit, root, &place.path).ok_or(Fault::Inconsistent("a borrowed place has another shape than its path"))?;
                Ok(Value::Shared(Arc::new(selected.clone())))
            }
        }
    }
}

impl<'unit> AdmittedEntry<'unit> {
    /// Run the entry on `arguments` under the root's `ceilings`. Work is
    /// metered under schedule 1 and cells under cell schedule 1 whether or
    /// not a ceiling is set.
    pub fn execute(
        &self,
        arguments: Vec<Argument>,
        ceilings: Ceilings,
    ) -> Result<Completed, Stop> {
        let unit = self.unit;
        let entry = self.entry();
        if arguments.len() != entry.parameter_types.len() {
            return Err(Stop::Arity {
                expected: entry.parameter_types.len(),
                supplied: arguments.len(),
            });
        }
        for (parameter, (argument, &ty)) in arguments.iter().zip(&entry.parameter_types).enumerate() {
            let completed = match argument {
                Argument::Owned(value) => conforms(unit, value, ty),
                Argument::Shared(referent) => match unit.types.get(ty) {
                    Some(TypeShape::NominalReference { referent: id, .. }) => schema(unit, *id).is_some_and(|data| conforms(unit, referent, data.carrier)),
                    _ => false,
                },
            };
            if !completed {
                return Err(Stop::Argument { parameter });
            }
        }
        let mut execution = Execution {
            entry: self,
            meter: Meter::new(
                ceilings,
                self.roster
                    .iter()
                    .map(|body| body.machine.body.operations.len() + 1),
            ),
            stack: Vec::new(),
        };
        let result = execution.run(arguments);
        let result = result.map_err(|(fault, site)| execution.stop(fault, site))?;
        let (work_used, peak_live_cells, executions) = execution.meter.finish();
        let mut sites = Vec::new();
        for (body, counts) in self.roster.iter().zip(executions) {
            let body = body.machine;
            let edge = match &body.body.blocks[0].terminator {
                BlockTerminator::Return { id, .. } | BlockTerminator::TerminalCall { id, .. } => Some(*id),
                _ => None,
            };
            for (index, executions) in counts
                .into_iter()
                .enumerate()
                .filter(|(_, executions)| *executions != 0)
            {
                let site = match body.body.operations.get(index) {
                    Some(operation) => ChargeSite::Operation(operation.id),
                    None => ChargeSite::Edge(edge.expect("an admitted body ends in a return or a terminal call")),
                };
                sites.push(SiteUsage {
                    machine: body.id,
                    site,
                    executions,
                });
            }
        }
        Ok(Completed {
            result,
            usage: Usage {
                work_schedule: WORK_SCHEDULE,
                cell_schedule: CELL_SCHEDULE,
                work_used,
                peak_live_cells,
                sites,
            },
        })
    }
}

struct Execution<'entry, 'unit> {
    entry: &'entry AdmittedEntry<'unit>,
    meter: Meter,
    stack: Vec<Frame<'unit>>,
}

/// The operation or edge slot of the top frame that a fault belongs to.
type Faulted = (Fault, Option<usize>);

impl<'unit> Execution<'_, 'unit> {
    fn stop(
        &self,
        fault: Fault,
        site: Option<usize>,
    ) -> Stop {
        let top = self.stack.last();
        let located = top.zip(site).map(|(frame, index)| {
            let body = &frame.machine.body;
            match body.operations.get(index) {
                Some(operation) => (ChargeSite::Operation(operation.id), body.diagnostics.operations.get(index)),
                None => {
                    let (BlockTerminator::Return { id, .. } | BlockTerminator::TerminalCall { id, .. }) = &body.blocks[0].terminator else {
                        unreachable!("an admitted body ends in a return or a terminal call")
                    };
                    (ChargeSite::Edge(*id), body.diagnostics.edges.get(id.get() - 1))
                }
            }
        });
        let at = Location {
            chain: self
                .stack
                .iter()
                .map(|frame| frame.machine.declaration.clone())
                .collect(),
            site: located.map(|(site, _)| site),
            source: located.and_then(|(_, source)| source.cloned()),
        };
        match fault {
            Fault::Meter(stop) => Stop::Resource { stop, at },
            Fault::Host => Stop::Host { at },
            Fault::Inconsistent(what) => Stop::Inconsistent { what, at },
        }
    }

    fn run(
        &mut self,
        arguments: Vec<Argument>,
    ) -> Result<Option<Value>, Faulted> {
        let unit = self.entry.unit;
        // The sponsor holds every shared argument for the whole invocation
        // and gives each owned argument to the entry frame.
        let mut delivered = Vec::new();
        delivered
            .try_reserve_exact(arguments.len())
            .map_err(|_| (Fault::Host, None))?;
        let entry = self.entry.entry();
        for (argument, &ty) in arguments.into_iter().zip(&entry.parameter_types) {
            let (value, cells) = match argument {
                Argument::Owned(value) => {
                    let cells = value
                        .cells(unit, ty)
                        .ok_or((Fault::Meter(MeterStop::Overflow), None))?;
                    (value, cells)
                }
                Argument::Shared(referent) => {
                    let Some(TypeShape::NominalReference { referent: id, .. }) = unit.types.get(ty) else {
                        return Err((Fault::Inconsistent("a shared argument has no reference formal"), None));
                    };
                    let carrier = schema(unit, *id)
                        .ok_or((Fault::Inconsistent("a reference formal has no schema"), None))?
                        .carrier;
                    let held = referent
                        .cells(unit, carrier)
                        .ok_or((Fault::Meter(MeterStop::Overflow), None))?;
                    self.meter.reserve(held).map_err(|stop| (stop.into(), None))?;
                    (Value::Shared(referent), 1)
                }
            };
            self.meter.reserve(cells).map_err(|stop| (stop.into(), None))?;
            delivered.push((value, cells));
        }
        let frame = self.frame(0, delivered).map_err(|fault| (fault, None))?;
        self.stack.push(frame);
        loop {
            let frame = self
                .stack
                .last()
                .expect("the entry frame is active until it returns");
            let (slot, index, machine) = (frame.slot, frame.next, frame.machine);
            let body = &machine.body;
            let located = move |fault: Fault| (fault, Some(index));
            self.meter
                .charge(slot, index)
                .map_err(|stop| located(stop.into()))?;
            if let Some(operation) = body.operations.get(index) {
                if let OperationKind::Call { callee, arguments, .. } = &operation.kind {
                    let callee = self.enter(callee, arguments).map_err(located)?;
                    self.stack.push(callee);
                    continue;
                }
                let produced = self
                    .operation(&operation.kind, operation.result)
                    .map_err(located)?;
                self.install(operation.result, produced).map_err(located)?;
                continue;
            }
            match &body.blocks[0].terminator {
                BlockTerminator::Return { result, cleanup, .. } => {
                    let frame = self.stack.last_mut().expect("the returning frame is active");
                    // The result leaves this frame with its cells: the one
                    // cell of a scalar, or every cell of the returned place.
                    let returned = match *result {
                        ReturnEdge::Unit => None,
                        ReturnEdge::Scalar(value) => Some((frame.scalar(value).map_err(located)?.clone(), 1)),
                        ReturnEdge::Structural { source, .. } => {
                            let cells = frame.place_cells(unit, source).map_err(located)?;
                            let value = frame.places[source.index()]
                                .take()
                                .ok_or_else(|| located(missing()))?;
                            Some((Arc::try_unwrap(value).unwrap_or_else(|shared| Value::clone(&shared)), cells))
                        }
                    };
                    let transferred = returned.as_ref().map_or(0, |(_, cells)| *cells);
                    frame.held = frame
                        .held
                        .checked_sub(transferred)
                        .ok_or_else(|| located(Fault::Meter(MeterStop::Overflow)))?;
                    self.cleanup(cleanup).map_err(located)?;
                    let frame = self.stack.pop().expect("the returning frame is active");
                    self.meter
                        .release(frame.held)
                        .map_err(|stop| (stop.into(), None))?;
                    let Some(caller) = self.stack.last_mut() else {
                        return Ok(returned.map(|(value, _)| value));
                    };
                    let awaiting = caller.next;
                    let expected = caller.machine.body.operations[awaiting].result;
                    caller.held = caller
                        .held
                        .checked_add(transferred)
                        .ok_or((Fault::Meter(MeterStop::Overflow), Some(awaiting)))?;
                    self.install(expected, returned.map(|(value, _)| value))
                        .map_err(|fault| (fault, Some(awaiting)))?;
                }
                BlockTerminator::TerminalCall { completion: Some(_), .. } => {
                    return Err(located(Fault::Inconsistent("an admitted terminal call branches on completion")));
                }
                BlockTerminator::TerminalCall {
                    completion: None, callee, arguments, cleanup, ..
                } => {
                    let callee = self.enter(callee, arguments).map_err(located)?;
                    self.cleanup(cleanup).map_err(located)?;
                    // The callee's formals are bound, so the caller's frame can go.
                    let frame = self.stack.pop().expect("the calling frame is active");
                    self.meter
                        .release(frame.held)
                        .map_err(|stop| located(stop.into()))?;
                    self.stack.push(callee);
                }
                BlockTerminator::Conditional { .. } | BlockTerminator::Jump(_) | BlockTerminator::ExternalExit(_) => {
                    return Err(located(Fault::Inconsistent("an admitted body branches")));
                }
            }
        }
    }

    /// A new frame for the roster body at `slot` with its formals bound. Each
    /// delivered value comes with the cells already reserved or moved for it.
    fn frame(
        &mut self,
        slot: usize,
        delivered: Vec<(Value, u64)>,
    ) -> Step<Frame<'unit>> {
        let machine = self.entry.roster[slot].machine;
        let body = &machine.body;
        self.meter.reserve(1)?;
        let mut values = Vec::new();
        let mut places = Vec::new();
        values
            .try_reserve_exact(body.values.len())
            .map_err(|_| Fault::Host)?;
        places
            .try_reserve_exact(body.places.len())
            .map_err(|_| Fault::Host)?;
        values.resize(body.values.len(), None);
        places.resize(body.places.len(), None);
        let mut held = 1u64;
        for (parameter, (value, cells)) in body.parameters.iter().zip(delivered) {
            held = held
                .checked_add(cells)
                .ok_or(Fault::Meter(MeterStop::Overflow))?;
            match *parameter {
                Parameter::Scalar(id) => values[id.index()] = Some(value),
                Parameter::Structural(id) => places[id.index()] = Some(Arc::new(value)),
            }
        }
        Ok(Frame {
            slot,
            machine,
            values,
            places,
            held,
            next: 0,
        })
    }

    /// Bind a callee's formals from the top frame's operands and return its frame.
    fn enter(
        &mut self,
        callee: &CallTarget,
        arguments: &[CallArgument],
    ) -> Step<Frame<'unit>> {
        let unit = self.entry.unit;
        let CallTarget::Machine(id) = callee else {
            return Err(Fault::Inconsistent("an admitted call names no packaged machine"));
        };
        let slot = self
            .entry
            .slot(*id)
            .ok_or(Fault::Inconsistent("a call reaches a body admission did not"))?;
        let mut delivered = Vec::new();
        delivered
            .try_reserve_exact(arguments.len())
            .map_err(|_| Fault::Host)?;
        for argument in arguments {
            let caller = self.stack.last_mut().expect("a call has a calling frame");
            let fresh = caller.operand_cells(unit, argument)?;
            let moved = match argument {
                ValueOperand::MoveWhole(place) => caller.place_cells(unit, *place)?,
                _ => 0,
            };
            self.meter.reserve(fresh)?;
            let caller = self.stack.last_mut().expect("a call has a calling frame");
            delivered.push((
                caller.operand(unit, argument)?,
                fresh
                    .checked_add(moved)
                    .ok_or(Fault::Meter(MeterStop::Overflow))?,
            ));
        }
        self.frame(slot, delivered)
    }

    /// Run one operation other than a call and return its result.
    fn operation(
        &mut self,
        kind: &OperationKind,
        result: OperationResult,
    ) -> Step<Option<Value>> {
        let unit = self.entry.unit;
        let frame = self.stack.last().expect("an operation has a frame");
        let result_type = match result {
            OperationResult::Structural(place) => Some(frame.place_type(place)?),
            _ => None,
        };
        let structural = result_type.ok_or(Fault::Inconsistent("a structural operation has no result place"));
        Ok(Some(match kind {
            OperationKind::Constant(literal) => {
                self.take_cells(1)?;
                literal_value(literal)
            }
            OperationKind::Integer { operation, operands } => {
                let [left, right] = operands.as_slice() else {
                    return Err(missing());
                };
                let (Value::Integer(left), Value::Integer(right)) = (frame.scalar(*left)?, frame.scalar(*right)?) else {
                    return Err(missing());
                };
                let result = operation
                    .primitive
                    .compare(left.cmp(right))
                    .ok_or_else(missing)?;
                self.take_cells(1)?;
                Value::Boolean(result)
            }
            OperationKind::CopyStructural { source } => {
                // Count the selected value where it stands, pay, then copy it.
                let root = Arc::clone(frame.place(source.root)?);
                let selected = projected_value(unit, &root, &source.path).ok_or(Fault::Inconsistent("a copied place has another shape than its path"))?;
                let cells = selected
                    .cells(unit, structural?)
                    .ok_or(Fault::Meter(MeterStop::Overflow))?;
                self.take_cells(cells)?;
                selected.clone()
            }
            OperationKind::MoveStructural { source } => {
                let frame = self.stack.last_mut().expect("an operation has a frame");
                let cells = frame.place_cells(unit, source.root)?;
                let value = frame.operand(unit, &ValueOperand::MoveWhole(source.root))?;
                // The cells stay in this frame, now under the result place.
                frame.held = frame
                    .held
                    .checked_add(cells)
                    .ok_or(Fault::Meter(MeterStop::Overflow))?;
                value
            }
            OperationKind::EstablishRecord { fields } => {
                let Some(TypeShape::Nominal(id)) = unit.types.get(structural?) else {
                    return Err(Fault::Inconsistent("a record establishment has no nominal result"));
                };
                let value = Value::Record {
                    data: *id,
                    fields: self.initializers(fields, 0)?,
                };
                self.established(*id, &value)?;
                value
            }
            OperationKind::EstablishCase { case, fields } => {
                let Some(TypeShape::Nominal(id)) = unit.types.get(structural?) else {
                    return Err(Fault::Inconsistent("a case establishment has no nominal result"));
                };
                let position = schema(unit, *id)
                    .and_then(|data| {
                        data.cases
                            .iter()
                            .position(|candidate| &candidate.declaration == case)
                    })
                    .ok_or(Fault::Inconsistent("an established case is absent from its sum"))?;
                let value = Value::Case {
                    data: *id,
                    case: position,
                    // One cell for the case tag.
                    fields: self.initializers(fields, 1)?,
                };
                self.established(*id, &value)?;
                value
            }
            OperationKind::EstablishZeroedStructural => {
                let ty = structural?;
                let cells = zero_cells(unit, ty)?;
                self.take_cells(cells)?;
                let value = zero(unit, ty)?;
                check_zero(unit, &value, ty)?;
                value
            }
            OperationKind::EstablishStructuralArray { elements } => {
                let tag = u64::from(matches!(unit.types.get(structural?), Some(TypeShape::ElementSequence { .. })));
                self.take_cells(tag)?;
                let frame = self.stack.last_mut().expect("an operation has a frame");
                let mut roster = Vec::new();
                roster
                    .try_reserve_exact(elements.len())
                    .map_err(|_| Fault::Host)?;
                for &element in elements {
                    let cells = frame.place_cells(unit, element)?;
                    roster.push(frame.operand(unit, &ValueOperand::MoveWhole(element))?);
                    frame.held = frame
                        .held
                        .checked_add(cells)
                        .ok_or(Fault::Meter(MeterStop::Overflow))?;
                }
                Value::Array(roster)
            }
            OperationKind::EstablishScalarArray { elements } => {
                let tag = u64::from(matches!(unit.types.get(structural?), Some(TypeShape::ElementSequence { .. })));
                let cells = u64::try_from(elements.len())
                    .ok()
                    .and_then(|count| count.checked_add(tag))
                    .ok_or(Fault::Meter(MeterStop::Overflow))?;
                self.take_cells(cells)?;
                let frame = self.stack.last().expect("an operation has a frame");
                let mut roster = Vec::new();
                roster
                    .try_reserve_exact(elements.len())
                    .map_err(|_| Fault::Host)?;
                for &element in elements {
                    roster.push(frame.scalar(element)?.clone());
                }
                Value::Array(roster)
            }
            _ => return Err(Fault::Inconsistent("an operation admission did not accept")),
        }))
    }

    /// Reserve `cells` for the top frame.
    fn take_cells(
        &mut self,
        cells: u64,
    ) -> Step<()> {
        self.meter.reserve(cells)?;
        let frame = self.stack.last_mut().expect("an operation has a frame");
        frame.held = frame
            .held
            .checked_add(cells)
            .ok_or(Fault::Meter(MeterStop::Overflow))?;
        Ok(())
    }

    /// Deliver the field operands of a record or case, reserving the cells
    /// the new value needs beyond those its moved operands bring: `tag` plus
    /// each scalar and each copy.
    fn initializers(
        &mut self,
        fields: &[FieldInitializer],
        tag: u64,
    ) -> Step<Vec<Value>> {
        let unit = self.entry.unit;
        let frame = self.stack.last().expect("an operation has a frame");
        let mut fresh = tag;
        let mut moved = 0u64;
        for field in fields {
            fresh = fresh
                .checked_add(frame.operand_cells(unit, &field.operand)?)
                .ok_or(Fault::Meter(MeterStop::Overflow))?;
            if let ValueOperand::MoveWhole(place) = field.operand {
                moved = moved
                    .checked_add(frame.place_cells(unit, place)?)
                    .ok_or(Fault::Meter(MeterStop::Overflow))?;
            }
        }
        self.take_cells(fresh)?;
        let frame = self.stack.last_mut().expect("an operation has a frame");
        let mut delivered = Vec::new();
        delivered
            .try_reserve_exact(fields.len())
            .map_err(|_| Fault::Host)?;
        for field in fields {
            delivered.push(frame.operand(unit, &field.operand)?);
        }
        // The moved cells left their places and now belong to the new value.
        frame.held = frame
            .held
            .checked_add(moved)
            .ok_or(Fault::Meter(MeterStop::Overflow))?;
        Ok(delivered)
    }

    /// Check the schema's own predicates for the value an operation just formed.
    fn established(
        &self,
        data: omega_psi::DataId,
        value: &Value,
    ) -> Step<()> {
        let unit = self.entry.unit;
        let data = schema(unit, data).ok_or(Fault::Inconsistent("an established value has no schema"))?;
        for clause in &data.requires {
            match holds(unit, clause, value) {
                Some(true) => {}
                Some(false) => return Err(Fault::Inconsistent("a data predicate is false for the established value")),
                None => return Err(Fault::Inconsistent("a data predicate does not denote a Boolean for the established value")),
            }
        }
        Ok(())
    }

    /// Install a result for the operation the top frame is at, then advance.
    fn install(
        &mut self,
        result: OperationResult,
        produced: Option<Value>,
    ) -> Step<()> {
        let frame = self.stack.last_mut().expect("a result has a frame");
        match (result, produced) {
            (OperationResult::Unit, None) => {}
            (OperationResult::Scalar(value), Some(produced)) => {
                let slot = frame.values.get_mut(value.index()).ok_or_else(missing)?;
                if slot.replace(produced).is_some() {
                    return Err(Fault::Inconsistent("a value is defined twice"));
                }
            }
            (OperationResult::Structural(place), Some(produced)) => {
                let slot = frame.places.get_mut(place.index()).ok_or_else(missing)?;
                if slot.replace(Arc::new(produced)).is_some() {
                    return Err(Fault::Inconsistent("a place is established twice"));
                }
            }
            _ => return Err(Fault::Inconsistent("a result disagrees with its operation")),
        }
        frame.next += 1;
        Ok(())
    }

    /// Discard the storage an edge's cleanup names, when the frame still holds it.
    fn cleanup(
        &mut self,
        cleanup: &[CleanupAction],
    ) -> Step<()> {
        let unit = self.entry.unit;
        for action in cleanup {
            let (CleanupAction::DiscardRoot(place) | CleanupAction::RetireHome(place)) = action else {
                return Err(Fault::Inconsistent("an admitted edge has cleanup admission did not accept"));
            };
            let frame = self.stack.last_mut().expect("an edge has a frame");
            if frame.places.get(place.index()).is_some_and(Option::is_some) {
                let cells = frame.place_cells(unit, *place)?;
                frame.places[place.index()] = None;
                frame.held = frame
                    .held
                    .checked_sub(cells)
                    .ok_or(Fault::Meter(MeterStop::Overflow))?;
                self.meter.release(cells)?;
            }
        }
        Ok(())
    }
}

/// Cells of the all-zero value of `ty`, counted before any of it exists.
fn zero_cells(
    unit: &LoweredPackage,
    ty: TypeId,
) -> Step<u64> {
    let overflow = Fault::Meter(MeterStop::Overflow);
    match unit.types.get(ty) {
        Some(TypeShape::PrimitiveScalar(_)) => Ok(1),
        Some(TypeShape::FixedArray { element, length }) => zero_cells(unit, *element)?.checked_mul(*length).ok_or(overflow),
        Some(TypeShape::Nominal(id)) => {
            let data = schema(unit, *id).ok_or(Fault::Inconsistent("a zeroed record has no schema"))?;
            data.fields
                .iter()
                .try_fold(0u64, |cells, field| cells.checked_add(zero_cells(unit, field.carrier).ok()?))
                .ok_or(overflow)
        }
        _ => Err(Fault::Inconsistent("a whole-type zero of a type admission did not accept")),
    }
}

fn zero(
    unit: &LoweredPackage,
    ty: TypeId,
) -> Step<Value> {
    match unit.types.get(ty) {
        Some(TypeShape::PrimitiveScalar(scalar)) => Ok(zero_scalar(*scalar)),
        Some(TypeShape::FixedArray { element, length }) => {
            let length = usize::try_from(*length).map_err(|_| Fault::Host)?;
            let mut elements = Vec::new();
            elements.try_reserve_exact(length).map_err(|_| Fault::Host)?;
            if length != 0 {
                elements.resize(length, zero(unit, *element)?);
            }
            Ok(Value::Array(elements))
        }
        Some(TypeShape::Nominal(id)) => {
            let data = schema(unit, *id).ok_or(Fault::Inconsistent("a zeroed record has no schema"))?;
            let fields = data
                .fields
                .iter()
                .map(|field| zero(unit, field.carrier))
                .collect::<Step<_>>()?;
            Ok(Value::Record { data: *id, fields })
        }
        _ => Err(Fault::Inconsistent("a whole-type zero of a type admission did not accept")),
    }
}

/// Each schema inside a zeroed value owes its predicates for that zero. Every
/// element of a zeroed array is the same value, so one stands for all.
fn check_zero(
    unit: &LoweredPackage,
    value: &Value,
    ty: TypeId,
) -> Step<()> {
    match (value, unit.types.get(ty)) {
        (Value::Array(elements), Some(TypeShape::FixedArray { element, .. })) => elements
            .first()
            .map_or(Ok(()), |first| check_zero(unit, first, *element)),
        (Value::Record { fields, .. }, Some(TypeShape::Nominal(id))) => {
            let data = schema(unit, *id).ok_or(Fault::Inconsistent("a zeroed record has no schema"))?;
            for clause in &data.requires {
                if holds(unit, clause, value) != Some(true) {
                    return Err(Fault::Inconsistent("a data predicate is false for the zero value"));
                }
            }
            fields
                .iter()
                .zip(&data.fields)
                .try_for_each(|(field, declared)| check_zero(unit, field, declared.carrier))
        }
        _ => Ok(()),
    }
}
