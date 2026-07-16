//! R2 rung 3 slice 1 (ch12 "Dependent Data"): the default-domain WRITE
//! obligation -- every store to a `where`-mentioned field of a
//! domain-carrying place must leave the facts TRUE at the post-write
//! valuation. This is the strict pre-window semantics (ch11's
//! consumption-point windows are the sanctioned ADDITIVE relaxation);
//! obligations land BEFORE hypotheses on purpose -- over-refusal is safe,
//! over-assumption is not, so readers may not assume the facts until the
//! obligation net is total.
//!
//! V1 tracking model: per-state linear walk over `self`-rooted places
//! (machine-owned data is BORN ZEROED -- ch12's machine-owned rule -- so
//! untracked fields read 0). An integer-literal store tracks its value; a
//! runtime-valued store to a where-mentioned field refuses (the entailment
//! integration relaxes this later); a whole-place struct-literal store
//! reseeds the valuation from the literal (already proven at construction
//! by rung 2b); any CALL statement poisons every tracked valuation
//! (conservative aliasing fence).

use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::DataDefinition;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::StatementNode;

pub(crate) fn validate_default_domain_writes(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            validate_state(program, machine, state, diagnostics);
        }
    }
}

/// One tracked place: its rendered spelling, its data definition, and the
/// per-field valuation (`None` value = written with a non-literal).
struct TrackedPlace<'program> {
    spelling: String,
    definition: &'program DataDefinition,
    fields: Vec<(String, Option<i128>)>,
}

fn validate_state(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut tracked: Vec<TrackedPlace> = Vec::new();

    for statement in program.statement_table.statements(state.statement_nodes) {
        match statement {
            StatementNode::Assignment(assignment) => {
                handle_assignment(
                    program,
                    machine,
                    state,
                    assignment.target,
                    assignment.value,
                    &mut tracked,
                    diagnostics,
                );
            }
            // Conservative aliasing fence: a call may write any place.
            StatementNode::Call(_) => tracked.clear(),
            StatementNode::Expression(expression) => {
                if expression_contains_call(program, *expression) {
                    tracked.clear();
                }
            }
            StatementNode::LocalData(local) => {
                if local.initial_value.is_valid()
                    && expression_contains_call(program, local.initial_value)
                {
                    tracked.clear();
                }
            }
            _ => {}
        }
    }
}

fn handle_assignment<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    state: &State,
    target: ExpressionHandle,
    value: ExpressionHandle,
    tracked: &mut Vec<TrackedPlace<'program>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // A whole-place store of a struct literal reseeds the valuation (the
    // literal itself was proven at construction, rung 2b).
    if let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(value)
        && let Some(spelling) = self_place_spelling(program, target)
        && let Some(definition) = domain_definition_by_name(program, literal.type_name.as_str())
    {
        let fields = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .map(|field| {
                (
                    field.name.as_str().to_string(),
                    integer_literal_value(program, field.value),
                )
            })
            .collect();
        tracked.retain(|place| place.spelling != spelling);
        tracked.push(TrackedPlace {
            spelling,
            definition,
            fields,
        });
        return;
    }

    // A FIELD store: `<self-place>.field = value` where the receiver's type
    // carries where facts.
    let ExpressionNode::Member(member) = program.expression_table.expression(target) else {
        return;
    };
    let Some(receiver_spelling) = self_place_spelling(program, member.receiver) else {
        return;
    };
    let Some(receiver_type) = crate::places::declared_place_type(program, machine, Some(state), member.receiver)
    else {
        return;
    };
    let Some(definition) = data_definition_for_type(program, receiver_type) else {
        return;
    };
    if definition.where_facts.is_empty() {
        return;
    }
    let field_name = member.member.as_str().to_string();
    let written = integer_literal_value(program, value);

    let place = if let Some(position) = tracked
        .iter()
        .position(|place| place.spelling == receiver_spelling)
    {
        &mut tracked[position]
    } else {
        tracked.push(TrackedPlace {
            spelling: receiver_spelling,
            definition,
            fields: Vec::new(),
        });
        let last = tracked.len() - 1;
        &mut tracked[last]
    };
    place.fields.retain(|(name, _)| *name != field_name);
    place.fields.push((field_name.clone(), written));

    // Obligation: the facts mentioning this field must hold at the
    // post-write valuation.
    if !field_is_where_mentioned(program, place.definition, &field_name) {
        return;
    }
    let valuation: Vec<(&str, Option<i128>)> = place
        .fields
        .iter()
        .map(|(name, value)| (name.as_str(), *value))
        .collect();
    for fact in program
        .proof_facts
        .span_or_empty(place.definition.where_facts)
    {
        let omega_typed_trees::domain::ProofFact::Expression(expression) = fact else {
            continue;
        };
        match fold_with_valuation(program, &valuation, *expression) {
            Some(value) if value != 0 => {}
            Some(_) => diagnostics.push(Diagnostic::error(format!(
                "write to `{}.{field_name}` violates data `{}`'s default domain: a \
                 `where` fact evaluates FALSE at the post-write valuation (strict \
                 store-time semantics; ch11 windows are the future relaxation)",
                place.spelling,
                place.definition.name.as_str()
            ))),
            None => diagnostics.push(Diagnostic::error(format!(
                "write to `{}.{field_name}` cannot PROVE data `{}`'s default domain: \
                 a `where`-mentioned field's value is not an integer literal here \
                 (the entailment integration relaxes this) -- restructure with \
                 literal stores for now",
                place.spelling,
                place.definition.name.as_str()
            ))),
        }
    }
}

/// Render a `self`-rooted place (`self.map`, `self.a.b`); `None` for
/// anything else (parameters arrive with unknown-but-valid valuations, so
/// v1 does not track them).
fn self_place_spelling(program: &TypedTrees, expression: ExpressionHandle) -> Option<String> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let first = members.first()?;
            if first.as_str() != "self" {
                return None;
            }
            Some(
                members
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("."),
            )
        }
        ExpressionNode::Member(member) => {
            let receiver = self_place_spelling(program, member.receiver)?;
            Some(format!("{receiver}.{}", member.member.as_str()))
        }
        ExpressionNode::Mutable(inner) => self_place_spelling(program, *inner),
        _ => None,
    }
}

fn domain_definition_by_name<'program>(
    program: &'program TypedTrees,
    name: &str,
) -> Option<&'program DataDefinition> {
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == name)
        .filter(|definition| !definition.where_facts.is_empty())
}

fn data_definition_for_type<'program>(
    program: &'program TypedTrees,
    handle: omega_typed_trees::types::TypeReferenceHandle,
) -> Option<&'program DataDefinition> {
    use omega_typed_trees::types::TypeReferenceNode;
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Named { name, .. } => program
            .data_definitions()
            .iter()
            .find(|definition| definition.name == *name),
        TypeReferenceNode::Reference { referee, .. } => data_definition_for_type(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            data_definition_for_type(program, *base_type)
        }
        _ => None,
    }
}

fn field_is_where_mentioned(
    program: &TypedTrees,
    definition: &DataDefinition,
    field: &str,
) -> bool {
    program
        .proof_facts
        .span_or_empty(definition.where_facts)
        .iter()
        .any(|fact| match fact {
            omega_typed_trees::domain::ProofFact::Expression(expression) => {
                expression_mentions_name(program, *expression, field)
            }
            _ => false,
        })
}

fn expression_mentions_name(program: &TypedTrees, expression: ExpressionHandle, name: &str) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .last()
            .is_some_and(|member| member.as_str() == name),
        ExpressionNode::Binary(binary) => {
            expression_mentions_name(program, binary.left, name)
                || expression_mentions_name(program, binary.right, name)
        }
        _ => false,
    }
}

fn integer_literal_value(program: &TypedTrees, expression: ExpressionHandle) -> Option<i128> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.text().parse::<i128>().ok(),
        ExpressionNode::Mutable(inner) => integer_literal_value(program, *inner),
        _ => None,
    }
}

/// Fold a where fact over the tracked valuation: tracked fields read their
/// value (a non-literal write poisons), untracked fields read the ZII zero
/// (machine-owned data is born zeroed).
fn fold_with_valuation(
    program: &TypedTrees,
    valuation: &[(&str, Option<i128>)],
    expression: ExpressionHandle,
) -> Option<i128> {
    use omega_typed_trees::expression::BinaryOperator;
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let last = program
                .expression_table
                .name_path_members(path.members)
                .last()?
                .as_str();
            match valuation.iter().find(|(name, _)| *name == last) {
                Some((_, value)) => *value,
                None => Some(0),
            }
        }
        ExpressionNode::Integer(value) => value.text().parse::<i128>().ok(),
        ExpressionNode::Binary(binary) => {
            let left = fold_with_valuation(program, valuation, binary.left)?;
            let right = fold_with_valuation(program, valuation, binary.right)?;
            match binary.operator {
                BinaryOperator::Add => left.checked_add(right),
                BinaryOperator::Subtract => left.checked_sub(right),
                BinaryOperator::Multiply => left.checked_mul(right),
                BinaryOperator::LessOrEqual => Some(i128::from(left <= right)),
                BinaryOperator::Less => Some(i128::from(left < right)),
                BinaryOperator::GreaterOrEqual => Some(i128::from(left >= right)),
                BinaryOperator::Greater => Some(i128::from(left > right)),
                BinaryOperator::Equal => Some(i128::from(left == right)),
                BinaryOperator::NotEqual => Some(i128::from(left != right)),
                BinaryOperator::And => Some(i128::from(left != 0 && right != 0)),
                BinaryOperator::Or => Some(i128::from(left != 0 || right != 0)),
                _ => None,
            }
        }
        _ => None,
    }
}

fn expression_contains_call(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(_) => true,
        ExpressionNode::Binary(binary) => {
            expression_contains_call(program, binary.left)
                || expression_contains_call(program, binary.right)
        }
        ExpressionNode::Member(member) => expression_contains_call(program, member.receiver),
        ExpressionNode::Mutable(inner) => expression_contains_call(program, *inner),
        _ => false,
    }
}
