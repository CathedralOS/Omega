//! Check a record constructor against its defining Psi unit with
//! [`check_record_constructor`]. The proof subject is the constructor's one
//! closed machine body in that unit. [`check_selected_policy_call`] then checks
//! a selected policy's argument formation, call and return against that proof.
//! Applying the policy still requires evaluation and validation for its schema.
#![forbid(unsafe_code)]

mod computed_ranks;
pub use computed_ranks::{ComputedRankError, check_computed_ranks};

mod closed_policy_call;
pub use closed_policy_call::{CheckedClosedPolicyCall, check_closed_policy_call};
mod constructor_body;
mod policy_call;
mod policy_construction;
pub use policy_construction::{CheckedPolicyConstruction, check_policy_construction};
mod storage;
mod scalar_replacements;
pub use scalar_replacements::{ScalarReplacementError, check_scalar_replacements};

pub use policy_call::{CheckedPolicyCall, check_selected_policy_call};

use omega_psi::{
    DeclarationId, DeclarationKind, LoweredMachine, LoweredPackage, MachineTemplate, MachineTemplateBinder, PredicateOperator, PredicateStep, Site,
    TemplateBinder, TypeExpression,
};

/// A field's value in terms of the entry arguments of the checked machine.
/// Each type is written with the constructor's own region binder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldOrigin {
    /// A copy read through the shared formal; `ty` is the copied referent.
    CopySharedFormal { formal: usize, ty: TypeExpression },
    /// The whole owned formal, transferred once.
    MoveFormal { formal: usize, ty: TypeExpression },
    /// The zero of the field's declared type, formed by the body's own operations.
    Zero(TypeExpression),
}

/// The record a constructor returns, as its closed body establishes it.
#[derive(Debug)]
pub struct ConstructedRecord<'unit> {
    pub declaration: &'unit DeclarationId,
    /// The result type, written with the constructor's own region binder.
    pub ty: TypeExpression,
    /// The establishing operation's occurrence in the defining source.
    pub site: &'unit Site,
    /// Whether the record's declaration states predicates that the
    /// establishing operation owes.
    pub establishes: bool,
}

/// The exact ordered inputs, result and immutable Psi unit whose obligations
/// were checked. This judgment assumes established arguments. A shared
/// argument must designate a stable readable value for the call.
#[derive(Debug)]
pub struct CheckedRecordConstructor<'unit> {
    unit: &'unit LoweredPackage,
    machine: &'unit LoweredMachine,
    parameters: Vec<TypeExpression>,
    result: ConstructedRecord<'unit>,
    fields: Vec<(&'unit DeclarationId, FieldOrigin)>,
}

impl<'unit> CheckedRecordConstructor<'unit> {
    pub fn unit(&self) -> &'unit LoweredPackage {
        self.unit
    }
    /// The closed body that was checked.
    pub fn machine(&self) -> &'unit LoweredMachine {
        self.machine
    }
    /// Each formal's declared type. The body's type rows erase lifetime
    /// arguments, so these are read from the rows together with the machine's
    /// region slots and written with the constructor's region binder.
    pub fn parameters(&self) -> &[TypeExpression] {
        &self.parameters
    }
    pub fn result(&self) -> &ConstructedRecord<'unit> {
        &self.result
    }
    pub fn fields(&self) -> &[(&'unit DeclarationId, FieldOrigin)] {
        &self.fields
    }
}

/// The caller-side judgment admits a single region, pure terminating
/// evaluation, established formals and no implicit work left at return.
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

/// The same admission for a closed body: one region, no static argument, no
/// requirement, reach, crash route or pause, and a termination promise.
fn supports_straight_line_body(machine: &LoweredMachine) -> bool {
    let contract = &machine.contract;
    let body = &machine.body;
    machine.regions.len() == 1
        && machine.arguments.is_empty()
        && machine.positive_widths.is_empty()
        && machine.operator.is_none()
        && machine.scalar_result.is_none()
        && contract.promises_termination
        && contract.ranking.is_none()
        && contract.requires.is_empty()
        && contract.reach.as_deref() == Some(&[])
        && contract.realized_reach.is_empty()
        && contract.has_empty_crash_ceiling
        && contract.crash_routes.is_empty()
        && contract.callback_crash_routes.is_empty()
        && contract.pause_bounds.declared() == Default::default()
        && contract.realized_pause == Default::default()
        && body.lifetime_bindings.is_empty()
        && body.call_regions.is_empty()
        && body.call_acknowledgements.is_empty()
        && body.callback_invocations.is_empty()
        && body.suspension_sites.is_empty()
        && body.mutable_homes.is_empty()
}

/// Check the constructor, its data establishment and every declared guarantee.
/// The subject is the one closed machine body `unit` publishes for
/// `declaration`. The supported body copies or transfers its entry arguments,
/// forms the zero of every other field with its own operations and returns one
/// record. Each input is used once. Other bodies require other rules.
pub fn check_record_constructor<'unit>(
    unit: &'unit LoweredPackage,
    declaration: &DeclarationId,
) -> Result<CheckedRecordConstructor<'unit>, &'static str> {
    let mut bodies = unit.machines.iter().filter(|m| &m.declaration == declaration);
    let machine = bodies.next().ok_or("missing constructor body")?;
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
    // One declaration must name one proof subject. An open template of the
    // same declaration would be a second one, and this judgment reads no
    // template in place of the body.
    if declaration.kind != DeclarationKind::Machine
        || bodies.next().is_some()
        || unit.catalog_declarations.contains(declaration)
        || unit
            .machine_templates
            .iter()
            .any(|m| &m.declaration == declaration)
        || unit.theorems.iter().any(|m| &m.declaration == declaration)
        || unit
            .machines
            .iter()
            .filter(|other| other.id == machine.id)
            .count()
            != 1
        || requirement
    {
        return Err("ambiguous constructor declaration");
    }
    if declaration.enclosing_machine.is_some() || !supports_straight_line_body(machine) {
        return Err("unsupported constructor binders, effects or contract");
    }
    let body = constructor_body::read(unit, machine)?;
    let result = ConstructedRecord {
        declaration: &body.record.declaration,
        ty: body.result,
        site: body.site,
        establishes: !body.record.requires.is_empty(),
    };
    // A zero field has the type its declaration gives it at the result's
    // lifetimes. The body's operations were followed to that zero above.
    let declared = storage::record_fields(unit, &result.ty)?;
    if declared.len() != body.fields.len() {
        return Err("constructor does not establish its complete record declaration");
    }
    let mut fields = Vec::with_capacity(declared.len());
    for ((field, origin), (declared_field, declared_type)) in body.fields.into_iter().zip(declared) {
        if field != declared_field {
            return Err("constructor field order disagrees with its record declaration");
        }
        let origin = match origin {
            constructor_body::Origin::CopySharedFormal(formal) => {
                let mut ty = body.parameters[formal].clone();
                ty.0.pop();
                FieldOrigin::CopySharedFormal { formal, ty }
            }
            constructor_body::Origin::MoveFormal(formal) => FieldOrigin::MoveFormal {
                formal,
                ty: body.parameters[formal].clone(),
            },
            constructor_body::Origin::Zero => FieldOrigin::Zero(declared_type),
        };
        fields.push((field, origin));
    }
    storage::check_fields(unit, &machine.declaration, machine.regions.len(), &result, &fields)?;
    for clause in &machine.contract.ensures {
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
    Ok(CheckedRecordConstructor {
        unit,
        machine,
        parameters: body.parameters,
        result,
        fields,
    })
}
