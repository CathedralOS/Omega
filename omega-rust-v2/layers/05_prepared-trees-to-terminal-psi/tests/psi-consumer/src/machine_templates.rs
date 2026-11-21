//! Read an open machine template the way a specializer or a foreign caller
//! would: from the published Psi package alone, with the caller's selections.
use omega_psi::{
    ArithmeticPolicy, DeclarationId, IntegerObligation, IntegerPrimitive, LoweredPackage, LoweredRequirement, MachineTemplate, MachineTemplateBinder,
    MemberRestriction, ScalarType, TemplateCallTarget, TemplateOperand, TemplateResult, TemplateStep, TypeExpression, TypeExpressionStep,
};

/// What the caller supplies for one application of an open template.
pub struct Application<'a> {
    /// One unsigned value per runtime formal.
    pub formals: &'a [u64],
    /// The value of each `Const` binder, by binder index; unused for other binders.
    pub statics: &'a [u64],
    /// The machine selected for a callable binder: binder index and arguments to result.
    pub select: &'a dyn Fn(usize, &[u64]) -> u64,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Evaluation {
    Returned(u64),
    /// The step's published question is false for these inputs, so the
    /// checker must reject this application.
    QuestionFails {
        step: usize,
        question: IntegerObligation,
    },
}

fn unsigned_width(ty: &TypeExpression) -> Option<u64> {
    match ty.0.as_slice() {
        [TypeExpressionStep::Scalar(ScalarType::Integer { signed: false, width })] if *width <= 64 => Some(*width),
        _ => None,
    }
}

/// Evaluate an unsigned open body under one application. `None` means the
/// template uses a form outside this reader: a signed or wide carrier, a
/// member or declaration call, a value step such as a construction, or an
/// operand that names a later step.
pub fn evaluate(
    template: &MachineTemplate,
    application: &Application<'_>,
) -> Option<Evaluation> {
    let mut results: Vec<u64> = Vec::with_capacity(template.body.len());
    let read = |operand: &TemplateOperand, results: &[u64]| match operand {
        TemplateOperand::Formal(i) => application.formals.get(*i).copied(),
        TemplateOperand::Step(i) => results.get(*i).copied(),
        // Field reads, borrows, zeros and constants are outside this reader.
        _ => None,
    };
    for (index, step) in template.body.iter().enumerate() {
        let value = match step {
            TemplateStep::Call(call) => {
                let TemplateCallTarget::StaticBinder(binder) = call.target else { return None };
                if !matches!(template.parameters.get(binder), Some(MachineTemplateBinder::Callable { .. })) {
                    return None;
                }
                let arguments = call
                    .operands
                    .iter()
                    .map(|o| read(o, &results))
                    .collect::<Option<Vec<_>>>()?;
                (application.select)(binder, &arguments)
            }
            TemplateStep::Integer(integer) => {
                let width = unsigned_width(&integer.result_type)?;
                let limit = if width == 64 { u64::MAX } else { (1u64 << width) - 1 };
                let [left, right] = integer.operands.as_slice() else { return None };
                let (left, right) = (read(left, &results)?, read(right, &results)?);
                let exact = match integer.operation.primitive {
                    IntegerPrimitive::Add => u128::from(left) + u128::from(right),
                    IntegerPrimitive::Multiply => u128::from(left) * u128::from(right),
                    _ => return None,
                };
                match integer.operation.policy {
                    ArithmeticPolicy::Exact if exact > u128::from(limit) => {
                        let question = IntegerObligation::ResultFitsCarrier;
                        integer
                            .operation
                            .obligations
                            .contains(&question)
                            .then_some(())?;
                        return Some(Evaluation::QuestionFails { step: index, question });
                    }
                    ArithmeticPolicy::Exact => exact as u64,
                    ArithmeticPolicy::Wrapping => (exact & u128::from(limit)) as u64,
                    ArithmeticPolicy::Saturating => exact.min(u128::from(limit)) as u64,
                }
            }
            _ => return None,
        };
        results.push(value);
    }
    Some(Evaluation::Returned(match template.result.as_ref()? {
        TemplateResult::Operand(operand) => read(operand, &results)?,
        TemplateResult::StaticParameter { owner, index, .. } if owner == &template.declaration => {
            matches!(template.parameters.get(*index), Some(MachineTemplateBinder::Const(_))).then_some(())?;
            *application.statics.get(*index)?
        }
        TemplateResult::StaticParameter { .. } => return None,
    }))
}

/// A member call through an operation-record binder whose bound is a
/// restricted trait, resolved through the package's published traits.
#[derive(Debug)]
pub struct RestrictedMember<'a> {
    pub restriction: &'a DeclarationId,
    pub base: &'a DeclarationId,
    pub requirement: &'a LoweredRequirement,
    /// The limits the checker must hold every caller's selection to.
    pub limits: &'a [MemberRestriction],
}

pub fn restricted_member<'a>(
    package: &'a LoweredPackage,
    template: &MachineTemplate,
    step: usize,
) -> Option<RestrictedMember<'a>> {
    let TemplateCallTarget::Member { binder, requirement } = &template.body.get(step)?.call()?.target else {
        return None;
    };
    let Some(MachineTemplateBinder::Implementation(slot)) = template.parameters.get(*binder) else {
        return None;
    };
    let alias = package
        .traits
        .iter()
        .find(|t| t.declaration == slot.declaration)?;
    let restriction = alias.restriction.as_ref()?;
    let base = package
        .traits
        .iter()
        .find(|t| t.declaration == restriction.base.declaration)?;
    Some(RestrictedMember {
        restriction: &alias.declaration,
        base: &base.declaration,
        requirement: base
            .requirements
            .iter()
            .find(|r| &r.declaration == requirement)?,
        limits: &restriction.all_members,
    })
}
