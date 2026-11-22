//! Check a record constructor against its defining Psi unit with
//! [`check_record_constructor`]. [`check_selected_policy_call`] then checks
//! a selected policy's argument formation, call and return against that proof.
//! Applying the policy still requires evaluation and validation for its schema.
#![forbid(unsafe_code)]

mod policy_call;
mod policy_construction;
pub use policy_construction::{CheckedPolicyConstruction, check_policy_construction};
mod storage;

pub use policy_call::{CheckedPolicyCall, check_selected_policy_call};

use omega_psi::{
    DeclarationId, DeclarationKind, LoweredPackage, MachineTemplate, MachineTemplateBinder, PredicateOperator, PredicateStep, TemplateAccess, TemplateBinder,
    TemplateConstruct, TemplateOperand, TemplateResult, TemplateStep, TypeExpression, TypeExpressionStep,
};

/// A field's value in terms of the entry arguments of the checked machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldOrigin<'unit> {
    CopySharedFormal { formal: usize, ty: &'unit TypeExpression },
    MoveFormal { formal: usize, ty: &'unit TypeExpression },
    Zero(&'unit TypeExpression),
}

/// The exact ordered inputs, result and immutable Psi unit whose obligations
/// were checked. This judgment assumes established arguments. A shared
/// argument must designate a stable readable value for the call.
#[derive(Debug)]
pub struct CheckedRecordConstructor<'unit> {
    unit: &'unit LoweredPackage,
    machine: &'unit MachineTemplate,
    result: &'unit TemplateConstruct,
    fields: Vec<(&'unit DeclarationId, FieldOrigin<'unit>)>,
}

impl<'unit> CheckedRecordConstructor<'unit> {
    pub fn unit(&self) -> &'unit LoweredPackage {
        self.unit
    }
    pub fn machine(&self) -> &'unit MachineTemplate {
        self.machine
    }
    pub fn result(&self) -> &'unit TemplateConstruct {
        self.result
    }
    pub fn fields(&self) -> &[(&'unit DeclarationId, FieldOrigin<'unit>)] {
        &self.fields
    }
}

/// Both judgments below admit a single region, pure terminating evaluation,
/// established formals and no implicit work left at return.
fn supports_straight_line_template(machine: &MachineTemplate) -> bool {
    let sig = &machine.signature;
    matches!(machine.parameters.as_slice(), [MachineTemplateBinder::Type(TemplateBinder::Lifetime)])
        && sig.requires.is_empty()
        && sig.reach.as_deref() == Some(&[])
        && sig.crash_ceiling.as_deref() == Some(&[])
        && sig.crash_routes.is_empty()
        && sig.pause == Default::default()
        && machine.declared_pause.declared() == Default::default()
        && machine.fixed_pause == Default::default()
        && sig.terminates
        && sig.result_lifetime_input.is_none()
        && machine.callback_pause.is_empty()
        && machine.invokes.is_empty()
        && machine.invokes_machines.is_empty()
        && machine.discarded_fields.is_empty()
}

/// Check the constructor, its data establishment and every declared guarantee.
/// The supported body copies or transfers its entry arguments and returns one
/// record. Each input step is used once. Other bodies require other rules.
pub fn check_record_constructor<'unit>(
    unit: &'unit LoweredPackage,
    declaration: &DeclarationId,
) -> Result<CheckedRecordConstructor<'unit>, &'static str> {
    let mut templates = unit
        .machine_templates
        .iter()
        .filter(|m| &m.declaration == declaration);
    let machine = templates.next().ok_or("missing constructor template")?;
    let requirement = unit.traits.iter().any(|owner| {
        owner
            .requirements
            .iter()
            .any(|row| &row.declaration == declaration)
            || owner
                .dependent_requirements
                .iter()
                .any(|row| &row.declaration == declaration)
    });
    if declaration.kind != DeclarationKind::Machine
        || templates.next().is_some()
        || unit.catalog_declarations.contains(declaration)
        || unit.machines.iter().any(|m| &m.declaration == declaration)
        || unit.theorems.iter().any(|m| &m.declaration == declaration)
        || requirement
    {
        return Err("ambiguous constructor declaration");
    }
    let sig = &machine.signature;
    if declaration.enclosing_machine.is_some() || !supports_straight_line_template(machine) {
        return Err("unsupported constructor binders, effects or contract");
    }
    let Some((TemplateStep::Construct(result), inputs)) = machine.body.split_last() else {
        return Err("missing returned record construction");
    };
    if machine.result != Some(TemplateResult::Operand(TemplateOperand::Step(inputs.len())))
        || sig.result.as_ref() != Some(&result.ty)
        || result.declaration.kind != DeclarationKind::Data
    {
        return Err("constructor does not return the declared record");
    }
    let mut used_formals = vec![false; sig.parameters.len()];
    let mut produced = Vec::with_capacity(inputs.len());
    for step in inputs {
        let (formal, value) = match step {
            TemplateStep::CopyReferent { formal, ty, .. } => {
                let parameter = sig.parameters.get(*formal).ok_or("unknown borrowed formal")?;
                if !matches!(parameter.0.split_last(), Some((TypeExpressionStep::Reference(TemplateAccess::Shared), inner)) if inner == ty.0) {
                    return Err("copy requires a shared reference to the indicated carrier");
                }
                (*formal, FieldOrigin::CopySharedFormal { formal: *formal, ty })
            }
            TemplateStep::MoveFormal { formal, ty, .. } => {
                if sig.parameters.get(*formal) != Some(ty) || !matches!(ty.0.last(), Some(TypeExpressionStep::Named { .. })) {
                    return Err("whole move requires the indicated owned formal");
                }
                (*formal, FieldOrigin::MoveFormal { formal: *formal, ty })
            }
            _ => return Err("unsupported operation in record constructor"),
        };
        if std::mem::replace(&mut used_formals[formal], true) {
            return Err("constructor formal used more than once");
        }
        produced.push(Some(value));
    }
    if used_formals.contains(&false) {
        return Err("constructor leaves a formal unaccounted for");
    }
    let mut fields = Vec::with_capacity(result.fields.len());
    for (field, operand) in &result.fields {
        let origin = match operand {
            TemplateOperand::Zero(ty) => FieldOrigin::Zero(ty),
            TemplateOperand::Step(index) => produced
                .get_mut(*index)
                .and_then(Option::take)
                .ok_or("field reads a missing or spent input")?,
            _ => return Err("unsupported record field operand"),
        };
        fields.push((field, origin));
    }
    if produced.iter().any(Option::is_some) {
        return Err("constructor discards a field input");
    }
    storage::check_fields(unit, machine, result, &fields)?;
    for clause in &sig.ensures {
        let [
            PredicateStep::Result,
            PredicateStep::Field(first),
            rest @ ..,
            PredicateStep::Integer(n),
            PredicateStep::Apply(PredicateOperator::Equal),
        ] = &clause.predicate[..]
        else {
            return Err("unsupported constructor guarantee");
        };
        if n != &0.into() {
            return Err("result guarantee asks for an unproved integer");
        }
        let Some((_, FieldOrigin::Zero(ty))) = fields.iter().find(|(field, _)| *field == first) else {
            return Err("result guarantee has no established zero field");
        };
        let path = rest
            .iter()
            .map(|step| match step {
                PredicateStep::Field(field) => Ok(field),
                _ => Err("result guarantee must project a record field"),
            })
            .collect::<Result<Vec<_>, _>>()?;
        storage::zero_field_integer(unit, ty, &path)?;
    }
    Ok(CheckedRecordConstructor { unit, machine, result, fields })
}
