//! Check straight-line scalar bodies and their transitive local calls.
//! Execution and resource accounting belong to the interpreter.
use omega_psi::{
    BlockTerminator, DeclarationId, DeclarationKind, LoweredMachine, LoweredPackage, OperationKind, OperationResult, Parameter, ReturnEdge, ScalarLiteral,
    ScalarResult, ScalarType, TypeId, TypeShape, ValueId, ValueOrigin,
};

pub struct CheckedScalarReturn<'unit> {
    unit: &'unit LoweredPackage,
    machine: &'unit LoweredMachine,
    closure: Vec<&'unit LoweredMachine>,
}

impl<'unit> CheckedScalarReturn<'unit> {
    pub fn bodies(&self) -> impl Iterator<Item = &'unit LoweredMachine> + '_ {
        self.closure.iter().copied()
    }
    pub fn unit(&self) -> &'unit LoweredPackage {
        self.unit
    }
    pub fn machine(&self) -> &'unit LoweredMachine {
        self.machine
    }
}

pub fn check_scalar_return<'unit>(
    unit: &'unit LoweredPackage,
    entry: &DeclarationId,
) -> Result<CheckedScalarReturn<'unit>, &'static str> {
    let mut matching = unit.machines.iter().filter(|m| &m.declaration == entry);
    let (Some(machine), None) = (matching.next(), matching.next()) else {
        return Err("expected a unique closed entry");
    };
    use std::collections::{HashMap, HashSet};
    let mut machines = HashMap::new();
    for candidate in &unit.machines {
        if machines.insert(candidate.id, candidate).is_some() {
            return Err("ambiguous machine identity");
        }
    }
    let mut active = HashSet::new();
    let mut complete = HashSet::new();
    let mut closure = Vec::new();
    let mut work = vec![(machine, false)];
    while let Some((body, leaving)) = work.pop() {
        if leaving {
            active.remove(&body.id);
            complete.insert(body.id);
        } else if !complete.contains(&body.id) {
            if !active.insert(body.id) {
                return Err("recursive scalar entry needs separate termination evidence");
            }
            work.push((body, true));
            closure.push(body);
            let targets = check_body(unit, body, &machines)?;
            work.extend(targets.into_iter().map(|target| (target, false)));
        }
    }
    Ok(CheckedScalarReturn { unit, machine, closure })
}

fn check_body<'unit>(
    unit: &'unit LoweredPackage,
    machine: &'unit LoweredMachine,
    machines: &std::collections::HashMap<omega_psi::MachineId, &'unit LoweredMachine>,
) -> Result<Vec<&'unit LoweredMachine>, &'static str> {
    use omega_psi::{CallTarget, ValueOperand};
    if machine.declaration.kind != DeclarationKind::Machine {
        return Err("expected an ordinary scalar machine");
    }
    let mut calls = Vec::new();
    let contract = &machine.contract;
    let body = &machine.body;
    if !machine.arguments.is_empty()
        || !machine.regions.is_empty()
        || !machine.positive_widths.is_empty()
        || machine.operator.is_some()
        || !contract.requires.is_empty()
        || !contract.ensures.is_empty()
        || contract.ranking.is_some()
        || !contract.realized_reach.is_empty()
        || !contract.crash_routes.is_empty()
        || !contract.callback_crash_routes.is_empty()
        || contract.realized_pause != Default::default()
        || contract.pause_bounds.declared() != Default::default()
    {
        return Err("scalar return requires a plain closed machine with no pending contract obligations");
    }
    let scalar = |ty: TypeId| match unit.types.get(ty) {
        Some(TypeShape::PrimitiveScalar(ScalarType::Integer { width: 0, .. })) => None,
        Some(TypeShape::PrimitiveScalar(scalar)) => Some(*scalar),
        _ => None,
    };
    let result_type = machine
        .result_type
        .filter(|ty| scalar(*ty).is_some())
        .ok_or("entry needs a primitive result")?;
    if machine.scalar_result
        != Some(ScalarResult {
            owner: machine.id,
            ty: result_type,
        })
        || !body.places.is_empty()
        || !body.mutable_homes.is_empty()
        || !body.lifetime_bindings.is_empty()
        || !body.call_regions.is_empty()
        || !body.callback_invocations.is_empty()
        || !body.call_acknowledgements.is_empty()
        || !body.suspension_sites.is_empty()
        || !body.diagnostics.bounds.is_empty()
    {
        return Err("scalar entry has incompatible result, storage or call metadata");
    }
    let [block] = body.blocks.as_slice() else {
        return Err("scalar entry requires one block");
    };
    let BlockTerminator::Return {
        id,
        result: ReturnEdge::Scalar(returned),
        cleanup,
    } = &block.terminator
    else {
        return Err("scalar entry must return a scalar");
    };
    if block.id.get() != 1
        || id.get() != 1
        || block.operations != (0..body.operations.len())
        || !block.structural_parameters.is_empty()
        || !block.scalar_parameters.is_empty()
        || !block.scalar_domain_invariants.is_empty()
        || !cleanup.is_empty()
        || body.parameters.len() != machine.parameter_types.len()
    {
        return Err("invalid scalar entry block or signature");
    }
    let mut defined = vec![false; body.values.len()];
    let define = |defined: &mut [bool], value: ValueId, origin: ValueOrigin, ty: TypeId| {
        let row = body
            .values
            .get(value.index())
            .ok_or("missing scalar definition")?;
        if row.origin != origin || row.ty != ty || std::mem::replace(&mut defined[value.index()], true) {
            return Err("scalar definition differs from its producer");
        }
        Ok(())
    };
    for (parameter, ty) in body.parameters.iter().zip(&machine.parameter_types) {
        let Parameter::Scalar(value) = parameter else {
            return Err("expected a scalar formal");
        };
        scalar(*ty).ok_or("expected a primitive formal type")?;
        define(&mut defined, *value, ValueOrigin::Parameter, *ty)?;
    }
    for (index, op) in body.operations.iter().enumerate() {
        let OperationResult::Scalar(value) = op.result else {
            return Err("scalar operation needs a scalar result");
        };
        let ty = body
            .values
            .get(value.index())
            .ok_or("missing scalar result")?
            .ty;
        let carrier = scalar(ty).ok_or("scalar result needs a primitive type")?;
        let input = |value: ValueId| {
            if defined.get(value.index()) != Some(&true) {
                return Err("scalar operand used before definition");
            }
            scalar(body.values[value.index()].ty).ok_or("nonprimitive scalar operand")
        };
        match &op.kind {
            OperationKind::Constant(literal) => {
                let fits = match literal {
                    ScalarLiteral::Integer(value) => carrier.contains_integer(value),
                    ScalarLiteral::Boolean(_) => carrier == ScalarType::Boolean,
                    ScalarLiteral::IeeeFloat(bits) => carrier == ScalarType::IeeeFloat(bits.format()),
                };
                if !fits {
                    return Err("literal disagrees with its carrier");
                }
            }
            OperationKind::Integer { operation, operands } => {
                let [left, right] = operands.as_slice() else {
                    return Err("integer comparison needs two operands");
                };
                let left = input(*left)?;
                if !matches!(left, ScalarType::Integer { .. }) || input(*right)? != left || carrier != ScalarType::Boolean || !operation.is_total_comparison() {
                    return Err("expected a total integer comparison on matching carriers");
                }
            }
            OperationKind::Call {
                callee: CallTarget::Machine(id),
                arguments,
                requirements,
            } => {
                let target = *machines.get(id).ok_or("unknown scalar call target")?;
                if !requirements.is_empty() || arguments.len() != target.parameter_types.len() || target.result_type.and_then(scalar) != Some(carrier) {
                    return Err("scalar call contract mismatch");
                }
                for (actual, &formal) in arguments.iter().zip(&target.parameter_types) {
                    let ValueOperand::Scalar(actual) = actual else {
                        return Err("scalar call needs a scalar operand");
                    };
                    if Some(input(*actual)?) != scalar(formal) {
                        return Err("scalar call argument type mismatch");
                    }
                }
                calls.push(target);
            }
            _ => return Err("scalar entry contains an unsupported instruction"),
        }
        if op.id.index() != index {
            return Err("operation identity differs from its location");
        }
        define(&mut defined, value, ValueOrigin::Operation(op.id), ty)?;
    }
    if defined.iter().any(|defined| !defined)
        || !body
            .values
            .get(returned.index())
            .is_some_and(|row| row.ty == result_type)
    {
        return Err("scalar return needs a defined value of its declared type");
    }
    Ok(calls)
}
