use omega_psi::{
    ContractClause, DeclarationId, DomainAtom, IeeeComparisonKind, IeeeFloatBits, IeeeFloatFormat, LoweredPackage, Parameter, PredicateOperator, PredicateStep,
    ScalarType, TypeShape, ValueOrigin,
};

/// Read a float formal's format and complete domain applications, checking
/// that its body input has the type declared at that position.
pub fn float_formal(
    package: &LoweredPackage,
    machine_name: &str,
    position: usize,
) -> Option<(IeeeFloatFormat, Vec<DomainAtom>)> {
    let machine = super::machine(package, machine_name)?;
    let declared = *machine.parameter_types.get(position)?;
    let Parameter::Scalar(input) = machine.body.parameters.get(position)? else { return None };
    let value = machine.body.values.get(input.index())?;
    if value.ty != declared || value.origin != ValueOrigin::Parameter {
        return None;
    }
    match package.types.get(declared)? {
        TypeShape::PrimitiveScalar(ScalarType::IeeeFloat(format)) => Some((*format, vec![])),
        TypeShape::QualifiedScalar { carrier: ScalarType::IeeeFloat(format), domains } => {
            let atoms = package.types.domain_atoms(*domains)?
                .map(|(declaration, arguments)| DomainAtom { declaration: declaration.clone(), arguments: arguments.to_vec() })
                .collect();
            Some((*format, atoms))
        }
        _ => None,
    }
}

/// A value this reader can substitute for a clause subject.
#[derive(Clone, Debug, PartialEq)]
pub enum PredicateValue {
    Bits(IeeeFloatBits),
    Bool(bool),
    ReadableReference(Box<PredicateValue>),
    /// A record subject: its declared fields and their values.
    Record(Vec<(DeclarationId, PredicateValue)>),
}

/// Decide one published clause using only Psi, substituting `formals`,
/// `result` and `self` for their terms. IEEE comparisons follow IEEE 754:
/// an ordered kind or `Equal` is false when either operand is NaN, `NotEqual`
/// is true then, and signed zeros compare equal. Each operand must have the
/// step's format. `None` means the clause uses something else or is ill-formed.
pub fn evaluate_ieee_clause(
    clause: &ContractClause,
    formals: &[PredicateValue],
    result: Option<&PredicateValue>,
    self_value: Option<&PredicateValue>,
) -> Option<bool> {
    let mut stack: Vec<PredicateValue> = Vec::new();
    for step in &clause.predicate {
        let value = match step {
            PredicateStep::Formal(index) => formals.get(*index)?.clone(),
            PredicateStep::Result => result?.clone(),
            PredicateStep::SelfValue => self_value?.clone(),
            PredicateStep::Referent => match stack.pop()? {
                PredicateValue::ReadableReference(value) => *value,
                _ => return None,
            },
            PredicateStep::Field(field) => match stack.pop()? {
                PredicateValue::Record(fields) => fields.into_iter().find(|(declared, _)| declared == field)?.1,
                _ => return None,
            },
            PredicateStep::IeeeFloat(bits) => PredicateValue::Bits(*bits),
            PredicateStep::Boolean(value) => PredicateValue::Bool(*value),
            PredicateStep::IeeeArithmetic { operation, format, semantics: omega_psi::FloatSemantics::Ieee754NearestEvenV1 } => {
                use omega_psi::FloatOperation::*;
                let PredicateValue::Bits(last) = stack.pop()? else { return None };
                let result = match operation {
                    Negate => super::floats::apply(*operation, *format, &[last])?,
                    Add | Subtract | Multiply | Divide => {
                        let PredicateValue::Bits(first) = stack.pop()? else { return None };
                        super::floats::apply(*operation, *format, &[first, last])?
                    }
                    _ => return None,
                };
                let super::FloatScalar::Bits(bits) = result else { return None };
                PredicateValue::Bits(bits)
            }
            PredicateStep::IeeeComparison { kind, format } => {
                let right = stack.pop()?;
                let left = stack.pop()?;
                PredicateValue::Bool(compare(*kind, *format, &left, &right)?)
            }
            PredicateStep::Apply(PredicateOperator::Not) => match stack.pop()? {
                PredicateValue::Bool(value) => PredicateValue::Bool(!value),
                _ => return None,
            },
            PredicateStep::Apply(operator @ (PredicateOperator::And | PredicateOperator::Or)) => {
                let (PredicateValue::Bool(right), PredicateValue::Bool(left)) = (stack.pop()?, stack.pop()?) else {
                    return None;
                };
                PredicateValue::Bool(if *operator == PredicateOperator::And { left && right } else { left || right })
            }
            _ => return None,
        };
        stack.push(value);
    }
    match stack.as_slice() {
        [PredicateValue::Bool(value)] => Some(*value),
        _ => None,
    }
}

// Host `f32` and `f64` comparisons are IEEE 754 comparisons.
fn compare(
    kind: IeeeComparisonKind,
    format: IeeeFloatFormat,
    left: &PredicateValue,
    right: &PredicateValue,
) -> Option<bool> {
    use IeeeComparisonKind::*;
    let (left, right) = match (format, left, right) {
        (IeeeFloatFormat::Binary32, PredicateValue::Bits(IeeeFloatBits::Binary32(a)), PredicateValue::Bits(IeeeFloatBits::Binary32(b))) => {
            (f64::from(f32::from_bits(*a)), f64::from(f32::from_bits(*b)))
        }
        (IeeeFloatFormat::Binary64, PredicateValue::Bits(IeeeFloatBits::Binary64(a)), PredicateValue::Bits(IeeeFloatBits::Binary64(b))) => {
            (f64::from_bits(*a), f64::from_bits(*b))
        }
        _ => return None,
    };
    // Widening binary32 to binary64 is exact and keeps NaN and signed zero,
    // so the comparison is unchanged.
    Some(match kind {
        Equal => left == right,
        NotEqual => left != right,
        LessThan => left < right,
        LessOrEqual => left <= right,
    })
}

/// Resolve a binary predicate on record formals to its selected private
/// machine and its full requirement range. This reader inspects the returned
/// Boolean literal of the two test helpers independently of their spelling.
pub fn record_predicate_application<'a>(
    package: &'a LoweredPackage, owner: &str,
) -> Option<(&'a DeclarationId, [usize; 2], &'a [ContractClause], bool)> {
    use omega_psi::{BlockTerminator, OperationKind, OperationResult, PredicateProgress, ReturnEdge, ScalarLiteral, TypeExpressionStep};
    let enclosing = super::machine(package, owner)?;
    let [clause] = &enclosing.contract.requires[..] else { return None };
    let [PredicateStep::Formal(a), PredicateStep::Formal(b), PredicateStep::MachineApplication {
        callee, operands, result: ScalarType::Boolean, requirements, progress: PredicateProgress::CheckPrivateBody, ..
    }] = &clause.predicate[..] else { return None };
    let helper = package.machines.iter().find(|m| &m.declaration == callee)?;
    if operands.len() != 2 || helper.parameter_types.len() != 2 || !helper.contract.has_empty_crash_ceiling
        || helper.contract.public || *requirements != (0..helper.contract.requires.len()) { return None; }
    for (formal, (&index, operand)) in [a, b].into_iter().zip(operands).enumerate() {
        let [TypeExpressionStep::Named { declaration, argument_count: 0 }] = &operand.0[..] else { return None };
        let (source, domains) = super::record_formal(package, owner, index)?;
        if !domains.is_empty() || source.declaration != *declaration || source.carrier != helper.parameter_types[formal] { return None; }
    }
    let [op] = &helper.body.operations[..] else { return None };
    let (OperationResult::Scalar(result), OperationKind::Constant(ScalarLiteral::Boolean(value))) = (op.result, &op.kind) else { return None };
    let [block] = &helper.body.blocks[..] else { return None };
    if !matches!(block.terminator, BlockTerminator::Return { result: ReturnEdge::Scalar(id), .. } if id == result) { return None; }
    Some((callee, [*a, *b], &helper.contract.requires, *value))
}
