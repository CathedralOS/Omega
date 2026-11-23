//! Follows a conformance member to the template that implements it and reads
//! the value that template returns, from published Psi alone. A call to a
//! template this package declares is followed into that template. A call to
//! another package's declaration stays that provider's call: the package must
//! carry no template or machine for it, and this reader never runs it.
use omega_psi::{
    DeclarationId, LoweredPackage, MachineTemplate, ScalarLiteral, TemplateCall, TemplateCallTarget, TemplateOperand, TemplateResult, TemplateSelection,
    TemplateStep, TypeExpression,
};

/// Where a call's declaration lives.
#[derive(Debug, Eq, PartialEq)]
pub enum TemplateCallee<'a> {
    /// A template published in this package under the exact declaration.
    Packaged(&'a MachineTemplate),
    /// A declaration of another package, named by its canonical package and path.
    Provided { package: &'a str, path: &'a str },
}

#[derive(Debug, Eq, PartialEq)]
pub struct ReturnedTemplateCall<'a> {
    pub implementation: &'a MachineTemplate,
    pub call: &'a TemplateCall,
    pub callee: TemplateCallee<'a>,
}

/// A value an open body forms, with packaged helpers followed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateValue<'a> {
    /// The caller's argument for this formal of the outermost template.
    Argument(usize),
    /// A borrow of that argument.
    Borrowed(Box<TemplateValue<'a>>),
    Constant(ScalarLiteral),
    Zero(&'a TypeExpression),
    /// `establishes`: the construction owes its data's `requires` clauses.
    Construct {
        declaration: &'a str,
        fields: Vec<(&'a str, TemplateValue<'a>)>,
        establishes: bool,
    },
    Array(Vec<TemplateValue<'a>>),
    Descriptor(&'a TemplateSelection),
    /// A call left to the package that declares it, with its evaluated operands.
    Provided {
        package: &'a str,
        path: &'a str,
        operands: Vec<TemplateValue<'a>>,
    },
}

/// The call that the implementation of `requirement` in conformance `conformance` returns.
pub fn member_template_call<'a>(
    package: &'a LoweredPackage,
    conformance: &str,
    requirement: &str,
) -> Result<ReturnedTemplateCall<'a>, String> {
    returned_template_call(package, member_template(package, conformance, requirement)?)
}

/// The template that implements `requirement` in conformance `conformance`.
pub fn member_template<'a>(
    package: &'a LoweredPackage,
    conformance: &str,
    requirement: &str,
) -> Result<&'a MachineTemplate, String> {
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
    template(package, &member.implementation).ok_or_else(|| format!("{conformance}: implementation {} has no template", member.implementation.path))
}

/// The call whose result `template` returns, with its callee resolved.
pub fn returned_template_call<'a>(
    package: &'a LoweredPackage,
    template: &'a MachineTemplate,
) -> Result<ReturnedTemplateCall<'a>, String> {
    let path = &template.declaration.path;
    let Some(TemplateResult::Operand(TemplateOperand::Step(index))) = template.result else {
        return Err(format!("{path} does not return a step"));
    };
    let call = template
        .body
        .get(index)
        .and_then(TemplateStep::call)
        .ok_or_else(|| format!("{path} returns step {index}, which is not a call"))?;
    Ok(ReturnedTemplateCall {
        implementation: template,
        call,
        callee: callee(package, template, call)?,
    })
}

/// The value `template` returns for these arguments.
pub fn template_result<'a>(
    package: &'a LoweredPackage,
    template: &'a MachineTemplate,
    arguments: &[TemplateValue<'a>],
) -> Result<TemplateValue<'a>, String> {
    let Some(TemplateResult::Operand(result)) = &template.result else {
        return Err(format!("{} returns no operand", template.declaration.path));
    };
    Reader {
        package,
        template,
        arguments,
        depth: 0,
    }
    .operand(result, template.body.len())
}

struct Reader<'r, 'a> {
    package: &'a LoweredPackage,
    template: &'a MachineTemplate,
    arguments: &'r [TemplateValue<'a>],
    depth: usize,
}

impl<'a> Reader<'_, 'a> {
    /// `reader` is the index of the step reading `operand`; a step may read
    /// only earlier steps.
    fn operand(
        &self,
        operand: &'a TemplateOperand,
        reader: usize,
    ) -> Result<TemplateValue<'a>, String> {
        let path = &self.template.declaration.path;
        Ok(match operand {
            TemplateOperand::Formal(i) => self
                .arguments
                .get(*i)
                .cloned()
                .ok_or_else(|| format!("{path}: no argument {i}"))?,
            TemplateOperand::Borrow { formal, .. } | TemplateOperand::BorrowBefore { formal, .. } => TemplateValue::Borrowed(Box::new(
                self.arguments
                    .get(*formal)
                    .cloned()
                    .ok_or_else(|| format!("{path}: no argument {formal}"))?,
            )),
            TemplateOperand::Constant(value) => TemplateValue::Constant(value.clone()),
            TemplateOperand::Zero(ty) => TemplateValue::Zero(ty),
            TemplateOperand::Step(i) if *i < reader && !self.encloses(*i, reader) => self.step(*i)?,
            other => return Err(format!("{path}: operand {other:?} is not readable here")),
        })
    }

    /// Whether `step` is a select whose arm owns `reader`; an arm cannot read its own select.
    fn encloses(
        &self,
        step: usize,
        reader: usize,
    ) -> bool {
        matches!(&self.template.body[step], TemplateStep::Select(select) if select.arms.iter().any(|arm| arm.steps.contains(&reader)))
    }

    fn step(
        &self,
        index: usize,
    ) -> Result<TemplateValue<'a>, String> {
        let path = &self.template.declaration.path;
        Ok(match &self.template.body[index] {
            TemplateStep::Call(call) => {
                let operands = call
                    .operands
                    .iter()
                    .map(|o| self.operand(o, index))
                    .collect::<Result<Vec<_>, _>>()?;
                match callee(self.package, self.template, call)? {
                    TemplateCallee::Provided { package, path } => TemplateValue::Provided { package, path, operands },
                    TemplateCallee::Packaged(helper) => {
                        if self.depth > 16 {
                            return Err(format!("{path}: helper chain is too deep"));
                        }
                        let Some(TemplateResult::Operand(result)) = &helper.result else {
                            return Err(format!("{}: helper returns no operand", helper.declaration.path));
                        };
                        Reader {
                            package: self.package,
                            template: helper,
                            arguments: &operands,
                            depth: self.depth + 1,
                        }
                        .operand(result, helper.body.len())?
                    }
                }
            }
            TemplateStep::Construct(construct) => TemplateValue::Construct {
                declaration: &construct.declaration.path,
                fields: construct
                    .fields
                    .iter()
                    .map(|(field, value)| Ok((field.path.as_str(), self.operand(value, index)?)))
                    .collect::<Result<_, String>>()?,
                establishes: construct.establishes,
            },
            TemplateStep::MoveFormal { formal, .. } => self.arguments.get(*formal).cloned().ok_or_else(|| format!("{path}: no moved formal {formal}"))?,
            TemplateStep::DisposeFormal { .. } => return Err(format!("{path}: normal-return disposal produces no value")),
            TemplateStep::CopyReferent { formal, .. } => {
                let Some(TemplateValue::Borrowed(value)) = self.arguments.get(*formal) else {
                    return Err(format!("{path}: copy at step {index} needs a reference at formal {formal}"));
                };
                *value.clone()
            }
            TemplateStep::Qualify { .. } => return Err(format!("{path}: step {index} changes domain membership")),
            TemplateStep::Integer(_) | TemplateStep::TrappingInteger { .. } => {
                return Err(format!(
                    "{path}: step {index} is an integer operation, which this value reader does not evaluate"
                ));
            }
            TemplateStep::Array(array) => TemplateValue::Array(
                array
                    .elements
                    .iter()
                    .map(|e| self.operand(e, index))
                    .collect::<Result<_, _>>()?,
            ),
            TemplateStep::Describe(descriptor) => TemplateValue::Descriptor(&descriptor.selection),
            TemplateStep::Select(select) => {
                let TemplateValue::Constant(ScalarLiteral::Boolean(subject)) = self.operand(&select.subject, index)? else {
                    return Err(format!("{path}: select subject is not a known Boolean"));
                };
                // Arm ranges follow the select in order and do not overlap.
                let mut next = index + 1;
                for arm in &select.arms {
                    if arm.steps.start != next || arm.steps.end < arm.steps.start {
                        return Err(format!("{path}: select arm steps are not contiguous"));
                    }
                    next = arm.steps.end;
                }
                let arm = select
                    .arms
                    .iter()
                    .find(|arm| arm.when.is_none_or(|when| when == subject))
                    .ok_or_else(|| format!("{path}: no arm matches {subject}"))?;
                match &arm.result {
                    TemplateOperand::Step(i) if !(*i < index || arm.steps.contains(i)) => {
                        return Err(format!("{path}: arm result reads step {i} outside its arm"));
                    }
                    result => self.operand(result, arm.steps.end.max(index))?,
                }
            }
        })
    }
}

fn callee<'a>(
    package: &'a LoweredPackage,
    caller: &MachineTemplate,
    call: &'a TemplateCall,
) -> Result<TemplateCallee<'a>, String> {
    let path = &caller.declaration.path;
    let TemplateCallTarget::Declaration(id) = &call.target else {
        return Err(format!("{path} calls {:?}, not a declaration", call.target));
    };
    let carried = package
        .machines
        .iter()
        .any(|machine| &machine.declaration == id);
    match template(package, id) {
        Some(found) if !carried => Ok(TemplateCallee::Packaged(found)),
        None if !carried && id.package != caller.declaration.package => Ok(TemplateCallee::Provided {
            package: &id.package,
            path: &id.path,
        }),
        _ => Err(format!(
            "{path} calls {}:{}, which this package neither packages once nor leaves to another package",
            id.package, id.path
        )),
    }
}

fn template<'a>(
    package: &'a LoweredPackage,
    id: &DeclarationId,
) -> Option<&'a MachineTemplate> {
    let mut matches = package
        .machine_templates
        .iter()
        .filter(|template| &template.declaration == id);
    let found = matches.next()?;
    matches.next().is_none().then_some(found)
}

/// Read the interval during which a call's borrow is active. A plain call
/// operand forms at the call; an ordered borrow starts before an earlier step.
pub fn formal_borrow_interval(template: &MachineTemplate, call: usize, argument: usize) -> Option<(usize, std::ops::RangeInclusive<usize>)> {
    let operand = template.body.get(call)?.call()?.operands.get(argument)?;
    let (formal, start) = match operand {
        TemplateOperand::BorrowBefore { formal, step, .. } if *step < call => (*formal, *step),
        TemplateOperand::Borrow { formal, .. } => (*formal, call),
        _ => return None,
    };
    template.signature.parameters.get(formal)?;
    Some((formal, start..=call))
}
