//! Check the argument formation and return of one selected policy template.
//! The judgment holds for an established schema supplied at entry.
use crate::{CheckedRecordConstructor, FieldOrigin, storage};
use omega_psi::{
    DeclarationId, DeclarationKind, SelectedPolicyDefinition, TemplateAccess, TemplateBinder, TemplateCallTarget, TemplateOperand, TemplateResult,
    TemplateStep, TypeExpressionStep as Step,
};
use std::ops::RangeInclusive;

#[derive(Debug)]
pub struct CheckedPolicyCall<'a> {
    selection: &'a SelectedPolicyDefinition,
    constructor: &'a CheckedRecordConstructor<'a>,
    case: &'a DeclarationId,
    empty_field: Option<&'a DeclarationId>,
    schema_loan: RangeInclusive<usize>,
}

impl<'a> CheckedPolicyCall<'a> {
    pub fn selection(&self) -> &'a SelectedPolicyDefinition {
        self.selection
    }
    pub fn constructor(&self) -> &'a CheckedRecordConstructor<'a> {
        self.constructor
    }
    /// Exact constructed case. An empty case and a case carrying an empty
    /// list retain different identities even though neither names a route.
    pub fn issuance_case(&self) -> &'a DeclarationId {
        self.case
    }
    pub fn empty_list_field(&self) -> Option<&'a DeclarationId> {
        self.empty_field
    }
    pub fn schema_loan(&self) -> RangeInclusive<usize> {
        self.schema_loan.clone()
    }
}

/// Rebind a nominal type's sole lifetime, retaining its full nominal identity.
fn same_application(
    left: &[Step],
    from: &DeclarationId,
    right: &[Step],
    to: &DeclarationId,
) -> bool {
    matches!((left, right), (
        [Step::GenericParameter {owner: a, index: 0}, Step::Named { declaration: x, argument_count: 1 }],
        [Step::GenericParameter {owner: b, index: 0}, Step::Named { declaration: y, argument_count: 1 }]
    ) if a == from && b == to && x == y)
}

fn owns_member(
    owner: &DeclarationId,
    member: &DeclarationId,
) -> bool {
    owner.package == member.package
        && owner.enclosing_machine == member.enclosing_machine
        && member
            .path
            .rsplit_once("::")
            .is_some_and(|(path, name)| path == owner.path && !name.is_empty())
}

fn check_conformance(
    selection: &SelectedPolicyDefinition,
    constructor: &CheckedRecordConstructor<'_>,
) -> Result<(), &'static str> {
    let unit = constructor.unit();
    let conformance = &selection.conformance;
    let plan = &selection.plan;
    let [member] = conformance.members.as_slice() else {
        return Err("expected a single selected policy member");
    };
    if selection.domain.kind != DeclarationKind::Domain
        || conformance.declaration.kind != DeclarationKind::Conformance
        || plan.declaration.kind != DeclarationKind::Machine
        || selection.domain.package != selection.site.package
        || conformance.implemented.declaration.package != constructor.machine().declaration.package
        || !selection.arguments.is_empty()
        || !conformance.implemented.arguments.is_empty()
        || !member.requirement.arguments.is_empty()
        || member.implementation != plan.declaration
        || !owns_member(&conformance.declaration, &member.implementation)
    {
        return Err("unsupported selected policy application or implementation");
    }
    let mut traits = unit
        .traits
        .iter()
        .filter(|t| t.declaration == conformance.implemented.declaration);
    let Some(trait_decl) = traits.next() else {
        return Err("selected policy trait is absent from the constructor unit");
    };
    let [required] = trait_decl.dependent_requirements.as_slice() else {
        return Err("unsupported policy requirement roster");
    };
    if traits.next().is_some()
        || trait_decl.declaration.kind != DeclarationKind::Trait
        || trait_decl.boundary
        || trait_decl.parent.is_some()
        || trait_decl.restriction.is_some()
        || !trait_decl.requirements.is_empty()
        || !trait_decl.parameters.is_empty()
        || member.requirement.declaration != required.declaration
        || required.declaration.kind != DeclarationKind::Machine
        || !owns_member(&trait_decl.declaration, &required.declaration)
        || required.parameters != [TemplateBinder::Lifetime]
        || required.signature.result_lifetime_input.is_some()
        || !required.signature.requires.is_empty()
        || !required.signature.ensures.is_empty()
        || !required.signature.crash_routes.is_empty()
    {
        return Err("unsupported or mismatched policy requirement");
    }
    let ([schema], [required_schema], Some(result), Some(required_result)) = (
        plan.signature.parameters.as_slice(),
        required.signature.parameters.as_slice(),
        &plan.signature.result,
        &required.signature.result,
    ) else {
        return Err("policy requirement needs one schema and one result");
    };
    if !same_application(&schema.0, &plan.declaration, &required_schema.0, &required.declaration)
        || !same_application(&result.0, &plan.declaration, &required_result.0, &required.declaration)
    {
        return Err("policy signature does not implement the selected requirement");
    }
    Ok(())
}

/// Verify the call in an open policy body against its separately checked callee.
/// This rule covers straight-line calls whose issuance has no fields or one
/// empty slice. Evaluating the policy for a schema requires a separate judgment.
pub fn check_selected_policy_call<'a>(
    selection: &'a SelectedPolicyDefinition,
    constructor: &'a CheckedRecordConstructor<'a>,
) -> Result<CheckedPolicyCall<'a>, &'static str> {
    check_conformance(selection, constructor)?;
    let plan = &selection.plan;
    let sig = &plan.signature;
    if plan.declaration.enclosing_machine.is_some() || !crate::supports_straight_line_template(plan) || !sig.ensures.is_empty() {
        return Err("unsupported policy binders, obligations or effects");
    }
    let Some((TemplateStep::Call(call), inputs)) = plan.body.split_last() else {
        return Err("policy does not end in its constructor call");
    };
    let Some((TemplateStep::Construct(issuance), payload)) = inputs.split_last() else {
        return Err("policy needs a constructed issuance argument");
    };
    // The callee is a closed body. Its declared formals and result are the
    // ones its proof read from that body at the constructor's region.
    let callee = constructor.machine();
    let ([schema], [borrowed, moved], Some(result)) = (sig.parameters.as_slice(), constructor.parameters(), sig.result.as_ref()) else {
        return Err("policy constructor requires one borrowed and one moved input");
    };
    let Some((Step::Reference(TemplateAccess::Shared), referent)) = borrowed.0.split_last() else {
        return Err("constructor needs a shared schema");
    };
    let [region] = call.arguments.as_slice() else {
        return Err("policy call needs one lifetime");
    };
    if !matches!(&call.target, TemplateCallTarget::Declaration(id) if *id == callee.declaration)
        || !call.enclosing_arguments.is_empty()
        || call.acknowledgements != Default::default()
        || !matches!(region.0.as_slice(), [Step::GenericParameter { owner, index: 0 }] if owner == &plan.declaration)
        || !same_application(&schema.0, &plan.declaration, referent, &callee.declaration)
        || !same_application(&issuance.ty.0, &plan.declaration, &moved.0, &callee.declaration)
        || !same_application(&result.0, &plan.declaration, &constructor.result().ty.0, &callee.declaration)
        || call.result_type.as_ref() != Some(result)
        || plan.result != Some(TemplateResult::Operand(TemplateOperand::Step(inputs.len())))
        || !constructor
            .fields()
            .iter()
            .any(|(_, origin)| matches!(origin, FieldOrigin::CopySharedFormal { formal: 0, .. }))
        || !constructor
            .fields()
            .iter()
            .any(|(_, origin)| matches!(origin, FieldOrigin::MoveFormal { formal: 1, .. }))
    {
        return Err("policy call disagrees with its checked constructor or return");
    }
    let [borrow, TemplateOperand::Step(argument)] = call.operands.as_slice() else {
        return Err("policy call needs schema loan and issuance value");
    };
    if *argument != payload.len() || issuance.establishes {
        return Err("issuance operand is unproved or belongs to another step");
    }
    let call_index = inputs.len();
    let loan_start = match borrow {
        TemplateOperand::Borrow {
            formal: 0,
            access: TemplateAccess::Shared,
        } => call_index,
        TemplateOperand::BorrowBefore {
            formal: 0,
            access: TemplateAccess::Shared,
            step,
            ..
        } if *step < call_index => *step,
        _ => return Err("policy requires a shared loan of its live schema through the call"),
    };
    // Only these value-forming steps can overlap the loan. Neither reads a
    // formal; every produced value is moved once into its unique consumer.
    let fields = storage::case_fields(constructor.unit(), &issuance.ty, &issuance.declaration)?;
    let empty_field = match (fields.as_slice(), issuance.fields.as_slice(), payload) {
        ([], [], []) => None,
        ([(required, ty)], [(field, operand)], steps)
            if *required == field
                && let Some((Step::Slice, element)) = ty.0.split_last() =>
        {
            match (operand, steps) {
                (TemplateOperand::Zero(zero), []) if zero == ty => {}
                (TemplateOperand::Step(0), [TemplateStep::Array(array)]) if &array.ty == ty && array.elements.is_empty() && array.element.0 == element => {}
                _ => return Err("unsupported issuance payload or incorrect field ownership"),
            }
            storage::check_element_type(constructor.unit(), element)?;
            Some(field)
        }
        _ => return Err("unsupported issuance payload or incorrect field ownership"),
    };
    Ok(CheckedPolicyCall {
        selection,
        constructor,
        case: &issuance.declaration,
        empty_field,
        schema_loan: loan_start..=call_index,
    })
}
