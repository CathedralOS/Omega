//! Match the closed body of a selected policy to its checked open template.
//!
//! [`check_selected_policy_call`](crate::check_selected_policy_call) judges
//! the policy's open template. A driver executes a different product: the
//! closed `plan` body that the selecting package publishes beside the
//! constructor. This judgment reads that body from the constructor's own unit
//! and accepts it only when it performs the checked template's steps and
//! nothing else, so the pair it returns is the one the earlier proofs cover.
//!
//! The accepted body is the one lowering produces for the two issuance forms
//! the template rule admits. It lends its own schema formal at the call, forms
//! the issuance case (after an empty route sequence when the case stores one),
//! calls the checked constructor's body at its own region and returns that
//! call's result. The call is an ordinary operation followed by a return edge.
use crate::CheckedPolicyCall;
use omega_psi::{
    AggregateRegionSlot, BlockTerminator, CallRegions, CallSite, CallTarget, DeclarationId, LoweredData, LoweredMachine, LoweredPackage, MachineRegion,
    Multiplicity, OperationKind, OperationResult, Parameter, Place, PlaceId, PlaceOrigin, ProjectedPlace, PsiOperation, ReferenceAccess, ReturnEdge, Site,
    TemplateStep, TypeExpression, TypeExpressionStep as Step, TypeId, TypeShape, ValueOperand,
};

type Check<T> = Result<T, &'static str>;

const NOT_THE_TEMPLATE: &str = "closed policy body differs from its checked template";

/// The closed `plan` body and the constructor body it calls, both in one unit
/// and both covered by the proofs this value was built from. Only
/// [`check_closed_policy_call`] creates one.
#[derive(Clone, Copy, Debug)]
pub struct CheckedClosedPolicyCall<'a> {
    unit: &'a LoweredPackage,
    plan: &'a LoweredMachine,
    constructor: &'a LoweredMachine,
    case: &'a DeclarationId,
    empty_field: Option<&'a DeclarationId>,
}

impl<'a> CheckedClosedPolicyCall<'a> {
    /// The unit that publishes both bodies.
    pub fn unit(&self) -> &'a LoweredPackage {
        self.unit
    }
    /// The selected policy's closed body: the entry a driver executes.
    pub fn plan(&self) -> &'a LoweredMachine {
        self.plan
    }
    /// The checked constructor body, the only machine `plan` calls.
    pub fn constructor(&self) -> &'a LoweredMachine {
        self.constructor
    }
    /// The issuance case the closed body establishes and transfers.
    pub fn issuance_case(&self) -> &'a DeclarationId {
        self.case
    }
    /// The case's route field when the body fills it with an empty sequence.
    pub fn empty_list_field(&self) -> Option<&'a DeclarationId> {
        self.empty_field
    }
}

fn data(
    unit: &LoweredPackage,
    row: TypeId,
) -> Check<&LoweredData> {
    let Some(TypeShape::Nominal(id)) = unit.types.get(row) else {
        return Err("closed policy body needs a record or sum at this type row");
    };
    unit.data
        .get(id.get() - 1)
        .filter(|data| data.id == *id && data.carrier == row)
        .ok_or("type row names no data declaration of this unit")
}

/// The record a template type names at the template's own region binder.
fn named<'t>(
    ty: &'t TypeExpression,
    owner: &DeclarationId,
) -> Option<&'t DeclarationId> {
    match ty.0.as_slice() {
        [
            Step::GenericParameter { owner: binder, index: 0 },
            Step::Named {
                declaration,
                argument_count: 1,
            },
        ] if binder == owner => Some(declaration),
        _ => None,
    }
}

/// The same admission the template passed, read from the closed body: no
/// static argument, requirement, guarantee, reach, crash route or pause, and a
/// termination promise. The one region fills the schema formal and the result.
fn supports_closed_caller(plan: &LoweredMachine) -> bool {
    let contract = &plan.contract;
    let body = &plan.body;
    let region = MachineRegion {
        aggregate_parameters: vec![AggregateRegionSlot { parameter: 0, binder: 0 }],
        aggregate_result: vec![0],
        ..Default::default()
    };
    plan.regions == [region]
        && plan.arguments.is_empty()
        && plan.positive_widths.is_empty()
        && plan.operator.is_none()
        && plan.scalar_result.is_none()
        && plan.declaration.enclosing_machine.is_none()
        && contract.promises_termination
        && contract.ranking.is_none()
        && contract.requires.is_empty()
        && contract.ensures.is_empty()
        && contract.reach.as_deref() == Some(&[])
        && contract.realized_reach.is_empty()
        && contract.has_empty_crash_ceiling
        && contract.crash_routes.is_empty()
        && contract.callback_crash_routes.is_empty()
        && contract.pause_bounds.declared() == Default::default()
        && contract.realized_pause == Default::default()
        && body.lifetime_bindings.is_empty()
        && body.call_acknowledgements.is_empty()
        && body.callback_invocations.is_empty()
        && body.suspension_sites.is_empty()
        && body.mutable_homes.is_empty()
        && body.values.is_empty()
        && body.diagnostics.bounds.is_empty()
}

/// The structural result of the operation at `index`, which must be a fresh
/// place of row `ty` that the operation itself defines.
fn result_place(
    plan: &LoweredMachine,
    index: usize,
    operation: &PsiOperation,
    ty: TypeId,
) -> Check<PlaceId> {
    let OperationResult::Structural(place) = operation.result else {
        return Err("closed policy operation needs a structural result");
    };
    let expected = Place {
        ty,
        origin: PlaceOrigin::Operation(operation.id),
    };
    if operation.id.index() != index || plan.body.places.get(place.index()) != Some(&expected) {
        return Err("closed policy operation result does not belong to its operation");
    }
    Ok(place)
}

/// Match the closed body of the selected policy to the template `call` checked.
///
/// The subject is the one closed body that the constructor's unit publishes
/// for the template's declaration. A constructor checked in another unit, such
/// as the toolchain's defining package, has no such body beside it and is
/// refused. The body must lend its own schema formal for exactly the call,
/// establish the template's issuance case with the same payload, call the
/// checked constructor's body at its own region, return the call's result, and
/// contain no other operation, place, value, effect or cleanup.
pub fn check_closed_policy_call<'a>(call: &CheckedPolicyCall<'a>) -> Check<CheckedClosedPolicyCall<'a>> {
    let selection = call.selection();
    let template = &selection.plan;
    let checked = call.constructor();
    let unit = checked.unit();
    let constructor = checked.machine();

    // One declaration names one proof subject in the unit that holds the
    // checked constructor body.
    let mut bodies = unit
        .machines
        .iter()
        .filter(|machine| machine.declaration == template.declaration);
    let plan = bodies
        .next()
        .ok_or("selected policy has no closed body in its constructor's unit")?;
    let requirement = unit.traits.iter().any(|owner| {
        owner
            .requirements
            .iter()
            .map(|row| &row.declaration)
            .chain(owner.dependent_requirements.iter().map(|row| &row.declaration))
            .any(|declaration| *declaration == plan.declaration)
    });
    if bodies.next().is_some()
        || plan.id == constructor.id
        || unit.machines.iter().filter(|other| other.id == plan.id).count() != 1
        || unit
            .machines
            .iter()
            .filter(|other| other.id == constructor.id)
            .count()
            != 1
        || unit.catalog_declarations.contains(&plan.declaration)
        || unit
            .machine_templates
            .iter()
            .any(|other| other.declaration == plan.declaration)
        || unit
            .theorems
            .iter()
            .any(|other| other.declaration == plan.declaration)
        || requirement
    {
        return Err("ambiguous closed policy declaration");
    }
    // The unit implements the selected policy with exactly the checked member.
    let mut conformances = unit
        .conformances
        .iter()
        .filter(|row| row.declaration == selection.conformance.declaration);
    if conformances.next() != Some(&selection.conformance) || conformances.next().is_some() {
        return Err("unit publishes a different implementation of the selected policy");
    }
    if !supports_closed_caller(plan) || plan.contract.public != template.public {
        return Err("unsupported closed policy regions, effects or contract");
    }

    // The template steps this body must perform, as the template rule read them.
    let Some((TemplateStep::Call(template_call), template_inputs)) = template.body.split_last() else {
        return Err(NOT_THE_TEMPLATE);
    };
    let Some((TemplateStep::Construct(template_issuance), template_payload)) = template_inputs.split_last() else {
        return Err(NOT_THE_TEMPLATE);
    };
    let loan = call.schema_loan();
    if loan.start() != loan.end() || *loan.end() != template_inputs.len() {
        return Err("closed policy body lends its schema only at the call");
    }

    // Formals and result. The rows erase lifetime arguments; the single region
    // admitted above fills the one binder of the schema and of the result.
    let ([schema_row], [borrowed_row, issuance_row], Some(result_row)) =
        (plan.parameter_types.as_slice(), constructor.parameter_types.as_slice(), plan.result_type)
    else {
        return Err("closed policy body needs one schema formal, a two-formal constructor and a result");
    };
    let (schema, result) = (data(unit, *schema_row)?, data(unit, result_row)?);
    let lent = TypeShape::NominalReference {
        referent: schema.id,
        access: ReferenceAccess::Shared,
    };
    let ([template_schema], Some(template_result)) = (template.signature.parameters.as_slice(), &template.signature.result) else {
        return Err(NOT_THE_TEMPLATE);
    };
    if unit.types.get(*borrowed_row) != Some(&lent)
        || constructor.result_type != Some(result_row)
        || constructor.regions.len() != 1
        || schema.lifetime_count != 1
        || result.lifetime_count != 1
        || named(template_schema, &template.declaration) != Some(&schema.declaration)
        || named(template_result, &template.declaration) != Some(&result.declaration)
    {
        return Err("closed policy formals or result differ from the checked template and constructor");
    }
    // The body leaves its owned schema formal at return without cleanup.
    if schema.multiplicity != Multiplicity::Copy || schema.cleanup_target.is_some() {
        return Err("closed policy body would discard a schema that owes cleanup");
    }

    // Every operation and its site belong to the declaring package.
    let body = &plan.body;
    let sites = &body.diagnostics;
    let package = &plan.declaration.package;
    if &sites.body.package != package
        || sites.operations.len() != body.operations.len()
        || sites
            .operations
            .iter()
            .chain(&sites.edges)
            .chain(&sites.parameters)
            .any(|site| &site.package != package)
    {
        return Err("closed policy body does not belong to its declaration");
    }
    let formal_place = Place {
        ty: *schema_row,
        origin: PlaceOrigin::Parameter,
    };
    let [Parameter::Structural(formal)] = body.parameters.as_slice() else {
        return Err("closed policy formal needs a structural place");
    };
    if body.places.get(formal.index()) != Some(&formal_place) {
        return Err("closed policy formal disagrees with its entry place");
    }
    let [block] = body.blocks.as_slice() else {
        return Err("unsupported control flow in closed policy body");
    };
    if !block.scalar_parameters.is_empty()
        || !block.structural_parameters.is_empty()
        || !block.scalar_domain_invariants.is_empty()
        || block.operations != (0..body.operations.len())
        // The formal, one fresh place per operation and the returned place.
        || body.places.len() != body.operations.len() + 2
    {
        return Err("unsupported control flow in closed policy body");
    }
    let site = |index: usize, expected: &Site| -> Check<()> {
        if &sites.operations[index] == expected {
            Ok(())
        } else {
            Err("closed policy operation is not at its template step's occurrence")
        }
    };

    // The call: the checked constructor's body in this unit, at this body's
    // region, with a loan of the formal and the whole issuance.
    let Some((call_operation, inputs)) = body.operations.split_last() else {
        return Err("closed policy body does not end in its constructor call");
    };
    let OperationKind::Call {
        callee,
        arguments,
        requirements,
    } = &call_operation.kind
    else {
        return Err("closed policy body does not end in its constructor call");
    };
    let returned = result_place(plan, inputs.len(), call_operation, result_row)?;
    site(inputs.len(), &template_call.site)?;
    let regions = CallRegions {
        call: CallSite::Operation(call_operation.id),
        arguments: vec![omega_psi::CallRegion::Binder(0)],
    };
    if *callee != CallTarget::Machine(constructor.id) || !requirements.is_empty() || body.call_regions != [regions] {
        return Err("closed policy call names another callee, region or requirement");
    }
    let schema_loan = ValueOperand::Borrow {
        place: ProjectedPlace {
            root: *formal,
            path: Vec::new(),
        },
        access: ReferenceAccess::Shared,
    };
    let [lent_schema, ValueOperand::MoveWhole(issuance)] = arguments.as_slice() else {
        return Err("closed policy call needs a schema loan and a transferred issuance");
    };
    if lent_schema != &schema_loan {
        return Err("closed policy call does not lend its own schema formal");
    }

    // The return forwards the call's result and leaves no implicit work.
    let BlockTerminator::Return {
        result: ReturnEdge::Structural { source, result: delivered },
        cleanup,
        ..
    } = &block.terminator
    else {
        return Err("closed policy body does not return its constructor's result");
    };
    let result_place_row = Place {
        ty: result_row,
        origin: PlaceOrigin::Result,
    };
    if *source != returned || body.places.get(delivered.index()) != Some(&result_place_row) || !cleanup.is_empty() {
        return Err("closed policy body does not return its constructor's result");
    }

    // The issuance: the template's case of the constructor's moved formal.
    let Some((case_operation, payload)) = inputs.split_last() else {
        return Err("closed policy body constructs no issuance");
    };
    let OperationKind::EstablishCase { case, fields } = &case_operation.kind else {
        return Err("closed policy body constructs no issuance");
    };
    let established = result_place(plan, payload.len(), case_operation, *issuance_row)?;
    site(payload.len(), &template_issuance.site)?;
    let sum = data(unit, *issuance_row)?;
    let mut cases = sum.cases.iter().filter(|row| &row.declaration == case);
    let Some(declared) = cases.next() else {
        return Err("closed policy issuance names no case of the constructor's formal");
    };
    if cases.next().is_some() || established != *issuance || case != call.issuance_case() || !sum.requires.is_empty() {
        return Err("closed policy body transfers a different issuance than its checked template");
    }
    match (call.empty_list_field(), declared.fields.as_slice(), fields.as_slice(), payload) {
        (None, [], [], []) => {}
        (Some(field), [declared_field], [written], [roster]) => {
            // An authored `[]` is its own template step. An omitted field is
            // formed at the construction that omits it.
            let roster_site = match template_payload {
                [] => &template_issuance.site,
                [TemplateStep::Array(array)] => &array.site,
                _ => return Err(NOT_THE_TEMPLATE),
            };
            let empty = result_place(plan, 0, roster, declared_field.carrier)?;
            site(0, roster_site)?;
            if &declared_field.declaration != field
                || &written.field != field
                || written.operand != ValueOperand::CopyWhole(empty)
                || !matches!(unit.types.get(declared_field.carrier), Some(TypeShape::ElementSequence { .. }))
                || !matches!(&roster.kind, OperationKind::EstablishStructuralArray { elements } if elements.is_empty())
            {
                return Err("closed policy issuance does not hold exactly the empty route sequence");
            }
        }
        _ => return Err("closed policy issuance payload differs from its checked template"),
    }
    Ok(CheckedClosedPolicyCall {
        unit,
        plan,
        constructor,
        case: call.issuance_case(),
        empty_field: call.empty_list_field(),
    })
}
