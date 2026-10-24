//! Declared measure bodies whose projection is justified by resolved
//! declarations. A declared identity view shares natural-number proofs only
//! after its exact input, result, and subject carriers agree. Classification
//! supplies neither range membership nor descent, and never rewrites custom
//! witness identity to a builtin view. The checked termination stage and the
//! runtime call-component judgment read one classification here so a member
//! admitted by either reader carries the same produced rank.

use super::super::{
    Engine, ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticValue,
};
use super::fields::declared_field;
use symbols::{BuiltinTypeAtom, SymbolHandle, SymbolKind};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
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
    /// `{ parameter.a.b.field }`: `path` holds the record-typed steps from
    /// the parameter's own record down to `owner`, the record declaring the
    /// final `u64` `field` (empty for a direct projection, where `owner` is
    /// the parameter's record). Every step is an exact resolved field of the
    /// exact nominal record before it.
    FieldProjection {
        path: Vec<ProjectionStep>,
        field: Identifier,
        owner: SymbolHandle,
        field_type: TypeReferenceHandle,
        field_symbol: SymbolHandle,
    },
}

/// One record-typed step of a nested measure projection: `field` of `owner`,
/// whose declared type is the next step's (or the final field's) record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectionStep {
    pub field: Identifier,
    pub field_symbol: SymbolHandle,
    pub owner: SymbolHandle,
}

/// A declared scalar view applied to one exact subject. The produced rank is
/// the subject itself (`computation` is `None`) or the measure body with its
/// parameter bound to the subject; either way the subject and the measure
/// share `carrier`, and the rank is a natural of that carrier once the range
/// judgment proves its formation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeclaredScalarView {
    pub measure: SymbolHandle,
    pub carrier: BuiltinTypeAtom,
    pub computation: Option<ScalarViewComputation>,
}

/// The measure parameter and body producing a computed scalar rank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScalarViewComputation {
    pub parameter: SymbolHandle,
    pub body: ExpressionHandle,
}

/// A body computing over its single parameter with `+` and `*` on integer
/// literals (`{ value + 1 }`, `{ value * 2 }`, `{ value * value + 3 }`), on
/// one unsigned carrier shared by the parameter and the result. The shape is
/// syntactic: operator meaning, strict monotonicity, and carrier formation
/// are judged where the view is applied to a subject (`declared_scalar_view`).
/// It is deliberately not a `MeasureBodyShape`: `measure_body_shape` keeps
/// classifying only forwards and field projections, so a consumer that admits
/// those two shapes does not admit a computation by omission.
pub struct ComputationBodyShape {
    pub carrier: BuiltinTypeAtom,
    pub constraints: Vec<(ExpressionHandle, ExpressionHandle, bool)>,
    pub parameter: SymbolHandle,
    pub body: ExpressionHandle,
}

/// Classify a computation body; `None` for a forward, a projection, or any
/// other body (`measure_body_shape` owns the first two).
pub fn computation_body_shape(
    program: &dyn crate::CheckingInput,
    measure: &MeasureDefinition,
) -> Option<ComputationBodyShape> {
    let [body] = program.expression_table.expression_handles(measure.body) else {
        return None;
    };
    let parameter = measure.parameter.as_ref()?;
    let binder = program.symbols.get(parameter.symbol);
    if !parameter.symbol.is_valid()
        || program.symbols.get(measure.symbol).kind != SymbolKind::Measure
        || binder.kind != SymbolKind::Parameter
        || binder.parent != measure.symbol
        || !computation_shape(program, *body, parameter.symbol, 0)
    {
        return None;
    }
    let (carrier, mut constraints) = unsigned_carrier(program, parameter.type_reference)?;
    let (result, result_constraints) = unsigned_carrier(program, measure.return_type)?;
    if result != carrier {
        return None;
    }
    constraints.extend(result_constraints);
    Some(ComputationBodyShape {
        carrier,
        constraints,
        parameter: parameter.symbol,
        body: *body,
    })
}

/// Resolve `view_path` to a unique declared scalar measure and admit it for
/// `subject` in `state`: an identity forward exactly as `declared_identity_view`,
/// or a computation whose every operator has builtin meaning in the measure's
/// declaration and whose normalized polynomial is strictly increasing on
/// the naturals (every coefficient positive, some term of degree at least
/// one). Strict monotonicity is what lets a subject's descent stand for the
/// produced rank's descent; formation inside the carrier is a separate
/// obligation the range judgment proves from the entry hypotheses, so an
/// unbounded subject with `{ value * 2 }` still rejects there. Anything else
/// -- a subtraction, a constant body, a non-builtin operator, an uncovered
/// domain -- is `None`.
pub fn declared_scalar_view(
    program: &dyn crate::CheckingInput,
    state: &State,
    subject: ExpressionHandle,
    view_path: &str,
) -> Option<DeclaredScalarView> {
    let path = view_path
        .split("::")
        .filter(|member| !member.is_empty())
        .collect::<Vec<_>>();
    let measure = find_declared_measure(program, &path)?;
    if measure.lexicographic {
        return None;
    }
    match measure_body_shape(program, measure) {
        Some(MeasureBodyShape::ParameterForward {
            carrier,
            constraints,
        }) => (identity_subject_matches(program, state, subject, carrier)
            && measure_constraints_cover_subject(program, state, subject, &constraints))
        .then_some(DeclaredScalarView {
            measure: measure.symbol,
            carrier,
            computation: None,
        }),
        Some(MeasureBodyShape::FieldProjection { .. }) => None,
        None => {
            let ComputationBodyShape {
                carrier,
                constraints,
                parameter,
                body,
            } = computation_body_shape(program, measure)?;
            if !identity_subject_matches(program, state, subject, carrier)
                || !measure_constraints_cover_subject(program, state, subject, &constraints)
            {
                return None;
            }
            let machine = owning_machine(program, state)?;
            // The syntactic shape admits only `+`, `*`, literals and the
            // parameter, so the polynomial has no negative coefficient unless a
            // literal is negative; monotonicity still needs a term the
            // parameter actually reaches.
            let polynomial = computed_measure_polynomial(program, machine, parameter, body)?;
            let strictly_increasing = polynomial
                .terms
                .values()
                .all(|coefficient| !coefficient.is_negative() && !coefficient.is_zero())
                && polynomial.terms.keys().any(|monomial| !monomial.is_empty());
            strictly_increasing.then_some(DeclaredScalarView {
                measure: measure.symbol,
                carrier,
                computation: Some(ScalarViewComputation { parameter, body }),
            })
        }
    }
}

/// A private template coordinate shared by classification and rank application.
pub(super) const COMPUTATION_PARAMETER_ATOM: &str = "\0ranking:view:parameter";

/// Normalize a measure body under its own declared formal and selected meaning.
/// The selecting machine supplies proof context, not the measure's operand
/// types or operator namespace. Rejoin the exact declaration before publishing
/// the scoped roster; rank membership and carrier formation remain separate.
pub(super) fn computed_measure_polynomial(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    parameter: SymbolHandle,
    body: ExpressionHandle,
) -> Option<super::Polynomial> {
    if !parameter.is_valid() {
        return None;
    }
    let formal = program.symbols.get(parameter);
    let owner = formal.parent;
    if formal.kind != SymbolKind::Parameter
        || !owner.is_valid()
        || program.symbols.get(owner).kind != SymbolKind::Measure
    {
        return None;
    }
    let measure = program
        .measures()
        .iter()
        .find(|measure| measure.symbol == owner)?;
    if measure.parameter.as_ref()?.symbol != parameter
        || program.expression_table.expression_handles(measure.body) != [body]
    {
        return None;
    }
    let mut engine = Engine::strict(program, machine, &[]);
    engine.normalize_scoped_value(&ScopedArithmeticValue::Term(ScopedArithmeticExpression {
        owner,
        expression: body,
        bindings: vec![ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Symbol(parameter),
            value: ScopedArithmeticValue::Atom {
                identity: COMPUTATION_PARAMETER_ATOM.to_owned(),
                unsigned: true,
            },
        }],
    }))
}

/// Classify only bodies whose projection is justified by resolved declarations.
pub fn measure_body_shape(
    program: &dyn crate::CheckingInput,
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
    // The body is a member chain rooted at the parameter: collect it from the
    // parameter outward, then resolve each step against the exact record the
    // previous step declared.
    let mut chain = Vec::new();
    let mut cursor = *body;
    loop {
        match program.expression_table.expression(cursor) {
            ExpressionNode::Member(member) => {
                if !member.member_symbol.is_valid() || member.case_variant.is_some() {
                    return None;
                }
                chain.push(member);
                cursor = member.receiver;
            }
            _ if is_parameter(program, cursor, parameter.symbol) => break,
            _ => return None,
        }
        if chain.len() > 128 {
            return None;
        }
    }
    chain.reverse();
    let (last, steps) = chain.split_last()?;
    let TypeReferenceNode::Named { symbol: root, .. } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    let mut owner = *root;
    let mut path = Vec::with_capacity(steps.len());
    for step in steps {
        let field = declared_field(program, owner, step.member_symbol)?;
        // The authored step must spell the resolved field's own name: a
        // same-named field of another record is not a projection of this one.
        if field.name != step.member {
            return None;
        }
        // An intermediate step must itself be an exact declared record so the
        // next member resolves against one nominal owner.
        let Some(next) = program
            .type_reference_table
            .unconstrained_named_symbol(field.type_reference)
        else {
            return None;
        };
        if !crate::declarations::data_queries::data_definition_by_symbol(program, next).is_some() {
            return None;
        }
        path.push(ProjectionStep {
            field: field.name.clone(),
            field_symbol: field.symbol,
            owner,
        });
        owner = next;
    }
    let field = declared_field(program, owner, last.member_symbol)?;
    // The authored step must spell the resolved field's own name.
    if field.name != last.member {
        return None;
    }
    // Returning a field with another carrier does not establish a u64 measure.
    // Field constraints narrow its values without changing that carrier.
    let field_type = program
        .type_reference_table
        .unconstrained_type_reference(field.type_reference);
    if ![field_type, measure.return_type]
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
        path,
        field: field.name.clone(),
        owner,
        field_type: field.type_reference,
        field_symbol: field.symbol,
    })
}

/// The unique declared measure spelled by `order` (`["Owner", "Name"]`).
pub fn find_declared_measure<'program>(
    program: &'program dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
                program.expression_table.name_path_is_single_member(path)
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
    matches!(program.type_reference_table.type_reference(program.type_reference_table.unconstrained_type_reference(reference)),
        TypeReferenceNode::Named { symbol, .. }
            if program.symbols.builtin_type_atom(*symbol) == Some(carrier))
}

fn owning_machine<'program>(
    program: &'program dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
    let symbol = &program
        .type_reference_table
        .type_reference(reference)
        .as_named_symbol()?;
    let carrier = program.symbols.builtin_type_atom(*symbol)?;
    matches!(
        carrier,
        BuiltinTypeAtom::U8 | BuiltinTypeAtom::U16 | BuiltinTypeAtom::U32 | BuiltinTypeAtom::U64
    )
    .then_some((carrier, constraints))
}

/// `+`/`*` trees over the parameter and integer literals, mentioning the
/// parameter at least once. Meaning and monotonicity are judged separately.
fn computation_shape(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    binder: SymbolHandle,
    depth: usize,
) -> bool {
    fn shape(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        binder: SymbolHandle,
        depth: usize,
        mentions: &mut bool,
    ) -> bool {
        let Some(node) = program
            .expression_table
            .expression_at_depth(expression, depth)
        else {
            return false;
        };
        match node {
            ExpressionNode::Integer(_) => true,
            ExpressionNode::Name(_) => {
                let parameter = is_parameter(program, expression, binder);
                *mentions |= parameter;
                parameter
            }
            ExpressionNode::Atomic(atomic) => {
                shape(program, atomic.value, binder, depth + 1, mentions)
            }
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::Add | BinaryOperator::Multiply
                ) =>
            {
                shape(program, binary.left, binder, depth + 1, mentions)
                    && shape(program, binary.right, binder, depth + 1, mentions)
            }
            _ => false,
        }
    }
    let mut mentions = false;
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Binary(_)
    ) && shape(program, expression, binder, depth, &mut mentions)
        && mentions
}

fn is_parameter(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    binder: SymbolHandle,
) -> bool {
    let Some(path) = program.expression_table.name_path_node(expression) else {
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
