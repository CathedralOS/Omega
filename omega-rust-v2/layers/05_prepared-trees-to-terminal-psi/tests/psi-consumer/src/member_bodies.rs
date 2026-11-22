//! Follows a conformance member to the closed machine that implements it and
//! reads the value that machine returns, from published Psi alone. A call to
//! a machine of the caller's own package is followed into that body. A call to
//! a body the package carries under another package's identity stays that
//! provider's call: this reader names it and how each operand reaches it, and
//! never reads or runs its body.
use omega_psi::{
    BlockTerminator, CallTarget, LoweredMachine, LoweredPackage, OperationId, OperationKind, OperationResult, Parameter, PlaceId, PlaceOrigin, PlaceProjection,
    PsiOperation, ReferenceAccess, ReturnEdge, ScalarLiteral, TemplateSelection, TypeShape, ValueId, ValueOperand, ValueOrigin,
};

/// How a call receives one operand.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Supply {
    Scalar,
    /// A borrow of the caller's storage for the duration of the call.
    Lent(ReferenceAccess),
    /// The caller's storage itself. No operation of the body copied it on the way.
    Moved,
    /// A copy: the operand itself, or an operation between the value's origin and the call.
    Copied,
}

/// A value a closed body forms, with the package's own helpers followed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BodyValue<'a> {
    /// The caller's argument for this formal of the outermost machine.
    Argument(usize),
    Constant(ScalarLiteral),
    /// An established record or case and the value of each field slot.
    Construct {
        declaration: &'a str,
        fields: Vec<(&'a str, BodyValue<'a>)>,
    },
    /// An established array or sequence, in element order.
    Array(Vec<BodyValue<'a>>),
    /// A description of the selected machine, such as `reflect\callable(pick)`.
    /// It names the machine. This reader never reads or runs that machine's body.
    Descriptor(&'a TemplateSelection),
    /// A call left to the package that declares it.
    Provided {
        package: &'a str,
        path: &'a str,
        operands: Vec<(Supply, BodyValue<'a>)>,
    },
}

/// Where the machine a call names is declared.
#[derive(Debug, Eq, PartialEq)]
pub enum BodyCallee<'a> {
    /// A body of the caller's own package.
    Packaged(&'a LoweredMachine),
    /// A body carried under another package's identity.
    Provided { package: &'a str, path: &'a str },
}

#[derive(Debug, Eq, PartialEq)]
pub struct ReturnedBodyCall<'a> {
    pub implementation: &'a LoweredMachine,
    pub call: &'a PsiOperation,
    pub callee: BodyCallee<'a>,
}

/// The one closed body that implements `requirement` in conformance `conformance`.
pub fn member_machine<'a>(
    package: &'a LoweredPackage,
    conformance: &str,
    requirement: &str,
) -> Result<&'a LoweredMachine, String> {
    let row = package
        .conformances
        .iter()
        .find(|row| row.declaration.path == conformance)
        .ok_or_else(|| format!("no conformance {conformance}"))?;
    let member = row
        .members
        .iter()
        .find(|member| member.requirement.declaration.path == requirement)
        .ok_or_else(|| format!("{conformance} implements no {requirement}"))?;
    let mut bodies = package
        .machines
        .iter()
        .filter(|machine| machine.declaration == member.implementation);
    match (bodies.next(), bodies.next()) {
        (Some(machine), None) if machine.arguments.is_empty() => Ok(machine),
        _ => Err(format!(
            "{conformance}: implementation {} has no single closed body",
            member.implementation.path
        )),
    }
}

/// The call whose result `machine` returns, with its callee resolved.
pub fn returned_body_call<'a>(
    package: &'a LoweredPackage,
    machine: &'a LoweredMachine,
) -> Result<ReturnedBodyCall<'a>, String> {
    let path = &machine.declaration.path;
    let [block] = machine.body.blocks.as_slice() else {
        return Err(format!("{path} does not return from one block"));
    };
    let BlockTerminator::Return {
        result: ReturnEdge::Structural { source, .. },
        ..
    } = block.terminator
    else {
        return Err(format!("{path} does not return a structural value"));
    };
    let call = defining(machine, source)?;
    let OperationKind::Call { callee, .. } = &call.kind else {
        return Err(format!("{path} returns {:?}, which is not a call", call.kind));
    };
    Ok(ReturnedBodyCall {
        implementation: machine,
        call,
        callee: resolve(package, machine, callee)?,
    })
}

/// The value `machine` returns for these arguments.
pub fn body_result<'a>(
    package: &'a LoweredPackage,
    machine: &'a LoweredMachine,
    arguments: &[BodyValue<'a>],
) -> Result<BodyValue<'a>, String> {
    let arguments: Vec<_> = arguments
        .iter()
        .map(|value| (Supply::Moved, value.clone()))
        .collect();
    Reader {
        package,
        machine,
        arguments: &arguments,
        depth: 0,
    }
    .result()
}

fn defining(
    machine: &LoweredMachine,
    place: PlaceId,
) -> Result<&PsiOperation, String> {
    let body = &machine.body;
    match body.places.get(place.index()).map(|row| row.origin) {
        Some(PlaceOrigin::Operation(id)) => operation(machine, id),
        other => Err(format!("{}: {place:?} is not an operation result: {other:?}", machine.declaration.path)),
    }
}

fn operation(
    machine: &LoweredMachine,
    id: OperationId,
) -> Result<&PsiOperation, String> {
    machine
        .body
        .operations
        .get(id.index())
        .filter(|operation| operation.id == id)
        .ok_or_else(|| format!("{}: no operation {id:?}", machine.declaration.path))
}

fn resolve<'a>(
    package: &'a LoweredPackage,
    caller: &LoweredMachine,
    callee: &CallTarget,
) -> Result<BodyCallee<'a>, String> {
    let CallTarget::Machine(id) = callee else {
        return Err(format!("{} calls {callee:?}, not a machine", caller.declaration.path));
    };
    let mut bodies = package.machines.iter().filter(|machine| machine.id == *id);
    let (Some(found), None) = (bodies.next(), bodies.next()) else {
        return Err(format!("{}: {id:?} names no single packaged body", caller.declaration.path));
    };
    // An open template for the same declaration would be a second form of the callee.
    if package
        .machine_templates
        .iter()
        .any(|template| template.declaration == found.declaration)
    {
        return Err(format!("{} is both a closed body and a template", found.declaration.path));
    }
    Ok(if found.declaration.package == caller.declaration.package {
        BodyCallee::Packaged(found)
    } else {
        BodyCallee::Provided {
            package: &found.declaration.package,
            path: &found.declaration.path,
        }
    })
}

struct Reader<'r, 'a> {
    package: &'a LoweredPackage,
    machine: &'a LoweredMachine,
    /// How this body received each formal, and its value.
    arguments: &'r [(Supply, BodyValue<'a>)],
    depth: usize,
}

impl<'a> Reader<'_, 'a> {
    fn refuse<T>(
        &self,
        reason: impl std::fmt::Display,
    ) -> Result<T, String> {
        Err(format!("{}: {reason}", self.machine.declaration.path))
    }

    /// Follow the blocks from entry to the return this invocation reaches. A
    /// conditional is followed only when its condition is a known Boolean.
    fn result(&self) -> Result<BodyValue<'a>, String> {
        let body = &self.machine.body;
        let mut block = body.blocks.first();
        let mut reached = Vec::new();
        for _ in 0..=body.blocks.len() {
            let Some(current) = block else {
                return self.refuse("an edge names no block");
            };
            reached.push(current.operations.clone());
            let edge = match &current.terminator {
                BlockTerminator::Return {
                    result: ReturnEdge::Structural { source, .. },
                    ..
                } => return Ok(self.place(*source, &reached)?.1),
                BlockTerminator::Return {
                    result: ReturnEdge::Scalar(value),
                    ..
                } => return self.scalar(*value, &reached),
                BlockTerminator::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => match self.scalar(*condition, &reached)? {
                    BodyValue::Constant(ScalarLiteral::Boolean(true)) => when_true,
                    BodyValue::Constant(ScalarLiteral::Boolean(false)) => when_false,
                    other => return self.refuse(format!("condition is not a known Boolean: {other:?}")),
                },
                BlockTerminator::Jump(edge) => edge,
                other => return self.refuse(format!("unread terminator {other:?}")),
            };
            if !edge.arguments.is_empty() || !edge.structural_arguments.is_empty() {
                return self.refuse("an edge passes block arguments, which this reader does not follow");
            }
            block = body.blocks.iter().find(|next| next.id == edge.target);
        }
        self.refuse("the blocks do not reach a return")
    }

    /// An operation this invocation runs: it lies in a block the walk reached.
    fn run(
        &self,
        id: OperationId,
        reached: &[std::ops::Range<usize>],
    ) -> Result<&'a PsiOperation, String> {
        let found = operation(self.machine, id)?;
        if reached.iter().any(|block| block.contains(&id.index())) {
            Ok(found)
        } else {
            self.refuse(format!("{id:?} belongs to a block this invocation does not reach"))
        }
    }

    fn scalar(
        &self,
        value: ValueId,
        reached: &[std::ops::Range<usize>],
    ) -> Result<BodyValue<'a>, String> {
        let body = &self.machine.body;
        match body.values.get(value.index()).map(|row| &row.origin) {
            Some(ValueOrigin::Parameter) => self.formal(Parameter::Scalar(value)).map(|(_, value)| value),
            Some(ValueOrigin::Operation(id)) => match &self.run(*id, reached)?.kind {
                OperationKind::Constant(literal) => Ok(BodyValue::Constant(literal.clone())),
                other => self.refuse(format!("scalar {value:?} comes from {other:?}, which this reader does not evaluate")),
            },
            other => self.refuse(format!("scalar {value:?} has origin {other:?}")),
        }
    }

    fn formal(
        &self,
        parameter: Parameter,
    ) -> Result<(Supply, BodyValue<'a>), String> {
        let position = self
            .machine
            .body
            .parameters
            .iter()
            .position(|formal| *formal == parameter);
        match position.and_then(|position| self.arguments.get(position)) {
            Some(argument) => Ok(argument.clone()),
            None => self.refuse(format!("{parameter:?} is not a supplied formal")),
        }
    }

    /// The value in `place` and how this body holds it: lent to it, or its
    /// own storage, copied or not since the value was formed.
    fn place(
        &self,
        place: PlaceId,
        reached: &[std::ops::Range<usize>],
    ) -> Result<(Supply, BodyValue<'a>), String> {
        let body = &self.machine.body;
        let Some(row) = body.places.get(place.index()) else {
            return self.refuse(format!("no {place:?}"));
        };
        let id = match row.origin {
            PlaceOrigin::Parameter => return self.formal(Parameter::Structural(place)),
            PlaceOrigin::Operation(id) => id,
            other => return self.refuse(format!("{place:?} has origin {other:?}")),
        };
        let operation = self.run(id, reached)?;
        if operation.result != OperationResult::Structural(place) {
            return self.refuse(format!("{id:?} does not define {place:?}"));
        }
        let fields = |fields: &'a [omega_psi::FieldInitializer]| {
            fields
                .iter()
                .map(|field| Ok((field.field.path.as_str(), self.operand(&field.operand, reached)?.1)))
                .collect::<Result<Vec<_>, String>>()
        };
        let value = match &operation.kind {
            OperationKind::EstablishCase { case, fields: payload } => BodyValue::Construct {
                declaration: &case.path,
                fields: fields(payload)?,
            },
            OperationKind::EstablishRecord { fields: slots } => {
                let Some(TypeShape::Nominal(data)) = self.package.types.get(row.ty) else {
                    return self.refuse(format!("{id:?} establishes a record without a nominal row"));
                };
                let Some(schema) = self.package.data.iter().find(|schema| schema.id == *data) else {
                    return self.refuse(format!("{id:?} establishes an unpublished record"));
                };
                BodyValue::Construct {
                    declaration: &schema.declaration.path,
                    fields: fields(slots)?,
                }
            }
            OperationKind::EstablishStructuralArray { elements } => BodyValue::Array(
                elements
                    .iter()
                    .map(|element| Ok(self.place(*element, reached)?.1))
                    .collect::<Result<_, String>>()?,
            ),
            OperationKind::DescribeMachine(selection) => BodyValue::Descriptor(selection),
            OperationKind::MoveStructural { source } if source.path.is_empty() => return self.moved(source.root, reached),
            OperationKind::CopyStructural { source } => {
                let (held, value) = self.place(source.root, reached)?;
                return match (source.path.as_slice(), held) {
                    ([], _) | ([PlaceProjection::Referent], Supply::Lent(_)) => Ok((Supply::Copied, value)),
                    (path, _) => self.refuse(format!("{id:?} copies through {path:?}, which this reader does not follow")),
                };
            }
            OperationKind::Call { callee, arguments, .. } => {
                let operands = arguments
                    .iter()
                    .map(|argument| self.operand(argument, reached))
                    .collect::<Result<Vec<_>, _>>()?;
                match resolve(self.package, self.machine, callee)? {
                    BodyCallee::Provided { package, path } => BodyValue::Provided { package, path, operands },
                    BodyCallee::Packaged(_) if self.depth > 16 => return self.refuse("helper chain is too deep"),
                    BodyCallee::Packaged(helper) => Reader {
                        package: self.package,
                        machine: helper,
                        arguments: &operands,
                        depth: self.depth + 1,
                    }
                    .result()?,
                }
            }
            other => return self.refuse(format!("{place:?} comes from {other:?}, which this reader does not evaluate")),
        };
        Ok((Supply::Moved, value))
    }

    /// How one operand reaches its call or field slot, and its value.
    fn operand(
        &self,
        operand: &ValueOperand,
        reached: &[std::ops::Range<usize>],
    ) -> Result<(Supply, BodyValue<'a>), String> {
        match operand {
            ValueOperand::Scalar(value) => Ok((Supply::Scalar, self.scalar(*value, reached)?)),
            ValueOperand::CopyWhole(place) => Ok((Supply::Copied, self.place(*place, reached)?.1)),
            ValueOperand::MoveWhole(place) => self.moved(*place, reached),
            ValueOperand::Borrow { place, access } => {
                let (held, value) = self.place(place.root, reached)?;
                match (place.path.as_slice(), held) {
                    ([], Supply::Moved | Supply::Copied) | ([PlaceProjection::Referent], Supply::Lent(_)) => Ok((Supply::Lent(*access), value)),
                    (path, held) => self.refuse(format!("borrow through {path:?} of a {held:?} place, which this reader does not follow")),
                }
            }
        }
    }

    /// The value a transfer of the whole `place` delivers. Among the operations
    /// this invocation runs, exactly one may take it: a second transfer would
    /// duplicate the value, and a place lent to this body is not its to move.
    fn moved(
        &self,
        place: PlaceId,
        reached: &[std::ops::Range<usize>],
    ) -> Result<(Supply, BodyValue<'a>), String> {
        let moves_whole = |operand: &ValueOperand| *operand == ValueOperand::MoveWhole(place);
        let transfers: usize = self
            .machine
            .body
            .operations
            .iter()
            .filter(|operation| {
                reached
                    .iter()
                    .any(|block| block.contains(&operation.id.index()))
            })
            .map(|operation| match &operation.kind {
                OperationKind::MoveStructural { source } => usize::from(source.root == place && source.path.is_empty()),
                OperationKind::Call { arguments, .. } => arguments
                    .iter()
                    .filter(|argument| moves_whole(argument))
                    .count(),
                OperationKind::EstablishCase { fields, .. } | OperationKind::EstablishRecord { fields } => fields
                    .iter()
                    .filter(|field| moves_whole(&field.operand))
                    .count(),
                _ => 0,
            })
            .sum();
        if transfers != 1 {
            return self.refuse(format!("{place:?} is transferred {transfers} times"));
        }
        match self.place(place, reached)? {
            (Supply::Lent(_), _) => self.refuse(format!("{place:?} is lent to this body and cannot be moved")),
            held => Ok(held),
        }
    }
}
