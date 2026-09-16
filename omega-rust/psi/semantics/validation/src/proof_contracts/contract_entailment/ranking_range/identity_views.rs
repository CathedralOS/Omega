//! Declared measure bodies whose projection is justified by resolved
//! declarations. A declared identity view shares natural-number proofs only
//! after its exact input, result, and subject carriers agree. Classification
//! supplies neither range membership nor descent, and never rewrites custom
//! witness identity to a builtin view. The checked termination stage and the
//! runtime call-component judgment read one classification here so a member
//! admitted by either reader carries the same produced rank.

use symbols::{BuiltinTypeAtom, SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::measure::MeasureDefinition;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub enum MeasureBodyShape {
    ParameterForward {
        carrier: BuiltinTypeAtom,
        /// Every `Range` constraint declared on the measure's parameter and
        /// result types. They are the view's domain contract and rank claim:
        /// the caller must prove the subject's enforced bounds fit inside
        /// each one before the forward can be selected.
        constraints: Vec<(ExpressionHandle, ExpressionHandle, bool)>,
    },
    FieldProjection {
        field: Identifier,
        owner: SymbolHandle,
        field_type: TypeReferenceHandle,
        field_symbol: SymbolHandle,
    },
}

/// A declared identity view applied to one exact subject: the measure whose
/// body forwards its parameter, on the unsigned carrier the subject shares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeclaredIdentityView {
    pub measure: SymbolHandle,
    pub carrier: BuiltinTypeAtom,
}

/// Resolve `view_path` (the witness's authored `Owner::Name` spelling) to a
/// unique declared measure and admit it as the identity view of `subject` in
/// `state`: the body must forward the exact parameter, the parameter, result
/// and subject must share one unsigned carrier, and the subject's enforced
/// bounds must fit inside every declared range refinement. A builtin path,
/// a lexicographic measure, a field projection, or an uncovered domain is
/// `None`; nothing here relabels the witness as `Nat::Descending`.
pub fn declared_identity_view(
    program: &TypedTrees,
    state: &State,
    subject: ExpressionHandle,
    view_path: &str,
) -> Option<DeclaredIdentityView> {
    let path = view_path
        .split("::")
        .filter(|member| !member.is_empty())
        .collect::<Vec<_>>();
    let measure = find_declared_measure(program, &path)?;
    if measure.lexicographic {
        return None;
    }
    let MeasureBodyShape::ParameterForward {
        carrier,
        constraints,
    } = measure_body_shape(program, measure)?
    else {
        return None;
    };
    (identity_subject_matches(program, state, subject, carrier)
        && measure_constraints_cover_subject(program, state, subject, &constraints))
    .then_some(DeclaredIdentityView {
        measure: measure.symbol,
        carrier,
    })
}

/// Classify only bodies whose projection is justified by resolved declarations.
pub fn measure_body_shape(
    program: &TypedTrees,
    measure: &MeasureDefinition,
) -> Option<MeasureBodyShape> {
    let [body] = program.expression_table.expression_handles(measure.body) else {
        return None;
    };
    let parameter = measure.parameter.as_ref()?;
    let binder = program.symbols.get(parameter.symbol);
    if !parameter.symbol.is_valid()
        || program.symbols.get(measure.symbol).kind != SymbolKind::Measure
        || binder.kind != SymbolKind::Parameter
        || binder.parent != measure.symbol
    {
        return None;
    }
    if is_parameter(program, *body, parameter.symbol) {
        // Identity does not widen a value or discharge a qualification on the
        // measure's input/result. Both must share one unsigned carrier, and
        // only range refinements can be discharged against the subject's
        // enforced bounds when the view is applied.
        let (carrier, mut constraints) = unsigned_carrier(program, parameter.type_reference)?;
        let (result, result_constraints) = unsigned_carrier(program, measure.return_type)?;
        if result != carrier {
            return None;
        }
        constraints.extend(result_constraints);
        return Some(MeasureBodyShape::ParameterForward {
            carrier,
            constraints,
        });
    }
    let ExpressionNode::Member(member) = program.expression_table.expression(*body) else {
        return None;
    };
    if !is_parameter(program, member.receiver, parameter.symbol)
        || !member.member_symbol.is_valid()
        || member.case_variant.is_some()
    {
        return None;
    }
    let TypeReferenceNode::Named { symbol: owner, .. } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    let declaration = program
        .data_definitions()
        .iter()
        .find(|data| owner.is_valid() && data.symbol == *owner)?;
    let field_symbol = program.symbols.get(member.member_symbol);
    if field_symbol.kind != SymbolKind::Field || field_symbol.parent != *owner {
        return None;
    }
    let mut fields = program
        .data_members(declaration)
        .iter()
        .filter_map(|member_definition| match member_definition {
            DataMember::Field(field) if field.symbol == member.member_symbol => Some(field),
            _ => None,
        });
    let field = fields.next()?;
    // Returning a field with another carrier does not establish a u64 measure.
    // Field constraints narrow its values without changing that carrier.
    let field_type = unwrap_constraint_shells(program, field.type_reference);
    if fields.next().is_some()
        || field.name != member.member
        || ![field_type, measure.return_type]
            .into_iter()
            .all(|reference| {
                matches!(program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Named { symbol, .. }
                    if program.symbols.builtin_type_atom(*symbol) == Some(BuiltinTypeAtom::U64))
            })
    {
        return None;
    }
    Some(MeasureBodyShape::FieldProjection {
        field: field.name.clone(),
        owner: *owner,
        field_type: field.type_reference,
        field_symbol: field.symbol,
    })
}

/// The unique declared measure spelled by `order` (`["Owner", "Name"]`).
pub fn find_declared_measure<'program>(
    program: &'program TypedTrees,
    order: &[&str],
) -> Option<&'program MeasureDefinition> {
    let mut matching = program.measures().iter().filter(|measure| {
        let actual = program.measure_path_members(measure.name);
        actual.len() == order.len()
            && actual
                .iter()
                .zip(order.iter())
                .all(|(actual, expected)| actual.as_str() == *expected)
    });
    let measure = matching.next()?;
    matching.next().is_none().then_some(measure)
}

/// A constrained identity view applies only where its declared refinements
/// hold: the subject's enforced bounds must fit inside every `Range` on the
/// measure's parameter and result. Since the produced rank forwards the
/// subject itself, one containment discharges the domain and the result claim
/// together. Declared endpoints must be exact integers — an approximate bound
/// cannot promise a fixed domain.
pub fn measure_constraints_cover_subject(
    program: &TypedTrees,
    state: &State,
    subject: ExpressionHandle,
    constraints: &[(ExpressionHandle, ExpressionHandle, bool)],
) -> bool {
    if constraints.is_empty() {
        return true;
    }
    let Some(machine) = owning_machine(program, state) else {
        return false;
    };
    let Some((subject_low, subject_high)) =
        crate::immutable_integer_expression_bounds(program, machine, state, subject)
    else {
        return false;
    };
    constraints.iter().all(|(minimum, maximum, end_inclusive)| {
        let Some((minimum, minimum_high)) =
            crate::immutable_integer_expression_bounds(program, machine, state, *minimum)
        else {
            return false;
        };
        let Some((maximum_low, mut maximum)) =
            crate::immutable_integer_expression_bounds(program, machine, state, *maximum)
        else {
            return false;
        };
        // The declared endpoints must be exact integers before the exclusive
        // end is normalized onto its greatest included value.
        if minimum != minimum_high || maximum != maximum_low {
            return false;
        }
        if !end_inclusive {
            maximum -= 1;
        }
        subject_low >= minimum && subject_high <= maximum
    })
}

/// The subject's exact carrier under range shells equals the view's carrier.
pub fn identity_subject_matches(
    program: &TypedTrees,
    state: &State,
    subject: ExpressionHandle,
    carrier: BuiltinTypeAtom,
) -> bool {
    let reference = if let ExpressionNode::Name(path) = program.expression_table.expression(subject)
    {
        program
            .state_parameters(state)
            .iter()
            .find(|parameter| {
                path.symbol.is_valid()
                    && path.head_symbol == path.symbol
                    && program
                        .expression_table
                        .name_path_members(path.members)
                        .len()
                        == 1
                    && !parameter.is_self
                    && parameter.symbol == path.symbol
            })
            .map(|parameter| parameter.type_reference)
    } else {
        let Some(machine) = owning_machine(program, state) else {
            return false;
        };
        // Computed subjects need their selected result type; `.len` spelling
        // or a subtraction node cannot manufacture a u64 result carrier.
        crate::expression_result_type_reference(program, machine, state, subject)
    };
    let Some(reference) = reference else {
        return false;
    };
    matches!(program.type_reference_table.type_reference(unwrap_constraint_shells(program, reference)),
        TypeReferenceNode::Named { symbol, .. }
            if program.symbols.builtin_type_atom(*symbol) == Some(carrier))
}

/// Strip every `Constrained` shell down to the carrier reference.
pub fn unwrap_constraint_shells(
    program: &TypedTrees,
    mut handle: TypeReferenceHandle,
) -> TypeReferenceHandle {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(handle)
    {
        handle = *base_type;
    }
    handle
}

fn owning_machine<'program>(
    program: &'program TypedTrees,
    state: &State,
) -> Option<&'program typed_trees::machine::Machine> {
    program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|candidate| candidate.symbol == state.symbol)
    })
}

/// Unwrap only `Range` refinement shells so the carrier stays exact: other
/// constraint kinds change what the view may assume and cannot be discharged
/// here. Returns the carrier and the collected `(minimum, maximum,
/// end_inclusive)` constraints for the caller to prove against the subject.
fn unsigned_carrier(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<(
    BuiltinTypeAtom,
    Vec<(ExpressionHandle, ExpressionHandle, bool)>,
)> {
    let mut constraints = Vec::new();
    let mut reference = reference;
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints: declared,
    } = program.type_reference_table.type_reference(reference)
    {
        for constraint in program.type_reference_table.constraints(*declared) {
            let TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } = constraint
            else {
                return None;
            };
            constraints.push((*minimum, *maximum, *end_inclusive));
        }
        reference = *base_type;
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    let carrier = program.symbols.builtin_type_atom(*symbol)?;
    matches!(
        carrier,
        BuiltinTypeAtom::U8 | BuiltinTypeAtom::U16 | BuiltinTypeAtom::U32 | BuiltinTypeAtom::U64
    )
    .then_some((carrier, constraints))
}

fn is_parameter(program: &TypedTrees, expression: ExpressionHandle, binder: SymbolHandle) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    path.symbol == binder
        && path.head_symbol == binder
        && program
            .expression_table
            .name_path_members(path.members)
            .len()
            == 1
}
