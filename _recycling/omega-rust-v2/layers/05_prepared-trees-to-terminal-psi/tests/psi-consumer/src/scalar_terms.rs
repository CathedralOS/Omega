//! Reads the expression that defines a scalar from published Psi alone, so a check can
//! say which value reaches a call or a comparison. Each operand keeps the carrier its own
//! value type states. A read stops at a constant, a formal or a field read of a formal;
//! it evaluates nothing and stops with `Opaque` at any other origin.
use omega_psi::{
    CallTarget, IntegerPrimitive, LoweredMachine, LoweredPackage, OperationKind, OperationResult, Parameter, PlaceProjection, ScalarLiteral, ScalarType,
    TypeShape, ValueId, ValueOperand, ValueOrigin,
};

/// How one scalar value is defined.
#[derive(Debug, Eq, PartialEq)]
pub enum Term<'a> {
    Constant(&'a ScalarLiteral),
    /// The machine's formal at this position.
    Formal(usize),
    /// A scalar read below a structural formal. `Referent` steps are omitted and each
    /// remaining step names a field declaration path.
    FieldRead { formal: usize, fields: Vec<&'a str> },
    Integer { primitive: IntegerPrimitive, operands: Vec<Operand<'a>> },
    /// Any other origin, with the reason this reader stopped.
    Opaque(&'static str),
}

/// A term with the carrier of the value it defines.
#[derive(Debug, Eq, PartialEq)]
pub struct Operand<'a> {
    pub term: Term<'a>,
    pub carrier: Option<ScalarType>,
}

fn machine<'a>(
    package: &'a LoweredPackage,
    owner: &str,
    path: &str,
) -> Result<&'a LoweredMachine, String> {
    let mut found = package
        .machines
        .iter()
        .filter(|machine| machine.declaration.package == owner && machine.declaration.path == path);
    match (found.next(), found.next()) {
        (Some(machine), None) => Ok(machine),
        (None, _) => Err(format!("no machine {owner}:{path}")),
        (Some(_), Some(_)) => Err(format!("{owner}:{path} names more than one machine")),
    }
}

fn carrier(
    package: &LoweredPackage,
    machine: &LoweredMachine,
    value: ValueId,
) -> Option<ScalarType> {
    let ty = machine.body.values.get(value.index())?.ty;
    match package.types.get(ty)? {
        TypeShape::PrimitiveScalar(scalar) => Some(*scalar),
        TypeShape::QualifiedScalar { carrier, .. } => Some(*carrier),
        _ => None,
    }
}

fn operand<'a>(
    package: &'a LoweredPackage,
    machine: &'a LoweredMachine,
    value: ValueId,
) -> Operand<'a> {
    Operand {
        term: term(package, machine, value),
        carrier: carrier(package, machine, value),
    }
}

fn term<'a>(
    package: &'a LoweredPackage,
    machine: &'a LoweredMachine,
    value: ValueId,
) -> Term<'a> {
    let body = &machine.body;
    let Some(defined) = body.values.get(value.index()) else {
        return Term::Opaque("value outside the body");
    };
    let operation = match defined.origin {
        ValueOrigin::Operation(id) => body.operations.get(id.index()),
        ValueOrigin::Parameter => {
            return body
                .parameters
                .iter()
                .position(|parameter| *parameter == Parameter::Scalar(value))
                .map_or(Term::Opaque("parameter value absent from the formals"), Term::Formal);
        }
        _ => return Term::Opaque("block parameter or component"),
    };
    let Some(operation) = operation.filter(|op| op.result == OperationResult::Scalar(value)) else {
        return Term::Opaque("origin operation does not produce this value");
    };
    match &operation.kind {
        OperationKind::Constant(literal) => Term::Constant(literal),
        OperationKind::Integer { operation, operands } => Term::Integer {
            primitive: operation.primitive,
            operands: operands.iter().map(|value| operand(package, machine, *value)).collect(),
        },
        OperationKind::ProjectedScalarRead { source } => {
            let Some(formal) = body
                .parameters
                .iter()
                .position(|parameter| *parameter == Parameter::Structural(source.root))
            else {
                return Term::Opaque("read below a place that is not a formal");
            };
            let mut fields = Vec::with_capacity(source.path.len());
            for step in &source.path {
                match step {
                    PlaceProjection::Referent => {}
                    PlaceProjection::Field(field) => fields.push(field.path.as_str()),
                    _ => return Term::Opaque("read through a case, index or element step"),
                }
            }
            Term::FieldRead { formal, fields }
        }
        _ => Term::Opaque("operation outside this reader"),
    }
}

/// The scalar arguments of each direct call from `owner:caller` to the same package's
/// `callee`, in operation order. A non-scalar argument stops the read.
pub fn call_arguments<'a>(
    package: &'a LoweredPackage,
    owner: &str,
    caller: &str,
    callee: &str,
) -> Result<Vec<Vec<Operand<'a>>>, String> {
    let caller = machine(package, owner, caller)?;
    let callee = machine(package, owner, callee)?;
    let mut calls = Vec::new();
    for operation in &caller.body.operations {
        let OperationKind::Call {
            callee: CallTarget::Machine(id),
            arguments,
            ..
        } = &operation.kind
        else {
            continue;
        };
        if *id != callee.id {
            continue;
        }
        let mut operands = Vec::with_capacity(arguments.len());
        for argument in arguments {
            let ValueOperand::Scalar(value) = argument else {
                return Err(format!("call to {} passes a structural operand", callee.declaration.path));
            };
            operands.push(operand(package, caller, *value));
        }
        calls.push(operands);
    }
    Ok(calls)
}

/// Each integer comparison in `owner:path`, in operation order, with its ordered operands.
pub fn integer_comparisons<'a>(
    package: &'a LoweredPackage,
    owner: &str,
    path: &str,
) -> Result<Vec<(IntegerPrimitive, Vec<Operand<'a>>)>, String> {
    use IntegerPrimitive::{Equal, GreaterOrEqual, GreaterThan, LessOrEqual, LessThan, NotEqual};
    let machine = machine(package, owner, path)?;
    Ok(machine
        .body
        .operations
        .iter()
        .filter_map(|operation| match &operation.kind {
            OperationKind::Integer { operation, operands }
                if matches!(operation.primitive, Equal | NotEqual | LessThan | LessOrEqual | GreaterThan | GreaterOrEqual) =>
            {
                Some((
                    operation.primitive,
                    operands.iter().map(|value| operand(package, machine, *value)).collect(),
                ))
            }
            _ => None,
        })
        .collect())
}
