//! Struct-literal field validation: every named field of a brace construction
//! must be a declared member of the constructed shape. Covers current-shape
//! record literals (`Counter { count: 0 }`), historical-shape literals
//! (for example, an ordinary historical shape `CounterV1 { counter: 3 }`),
//! version's root-level shape definition), and case-payload literals
//! (`Command::Say { text: ... }`). Literals whose head type is not a data
//! definition in this program (or is generic, where member types depend on
//! instantiation) are left to later layers.

use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataDefinition, DataMember};
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use psi_typed_trees::types::PrimitiveType;

mod construction_bounds;
mod field_obligations;

use construction_bounds::validate_literal_default_domain;
use field_obligations::enforce_construction_field_obligations;
pub(crate) use field_obligations::{
    construction_field_type, validate_array_literal_elements,
    validate_array_literal_elements_for_shape,
};

pub(crate) fn validate_struct_literal_fields(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::AssemblyFact(fact) => {
                        scan_expression(program, machine, state, fact.expression, diagnostics);
                    }
                    StatementNode::Assignment(assignment) => {
                        scan_expression(program, machine, state, assignment.target, diagnostics);
                        scan_expression(program, machine, state, assignment.value, diagnostics);
                    }
                    StatementNode::Call(call) => {
                        for argument in program.statement_table.expression_handles(call.arguments) {
                            scan_expression(program, machine, state, *argument, diagnostics);
                        }
                    }
                    StatementNode::Expression(expression) => {
                        scan_expression(program, machine, state, *expression, diagnostics);
                    }
                    StatementNode::LocalData(local_data) => {
                        scan_expression(
                            program,
                            machine,
                            state,
                            local_data.initial_value,
                            diagnostics,
                        );
                    }
                    StatementNode::Transition(transition) => {
                        if let TransitionGuardNode::When(guard) = &transition.guard {
                            scan_expression(program, machine, state, *guard, diagnostics);
                        }
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            scan_transition_target(
                                program,
                                machine,
                                state,
                                program.statement_table.transition_target(target),
                                diagnostics,
                            );
                        }
                    }
                }
            }
        }
    }
}

fn scan_transition_target(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    target: &TransitionTargetNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match target {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                scan_expression(program, machine, state, *argument, diagnostics);
            }
        }
        TransitionTargetNode::Value(expression) => {
            scan_expression(program, machine, state, *expression, diagnostics);
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}

fn scan_expression(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            scan_expression(program, machine, state, atomic.value, diagnostics)
        }
        ExpressionNode::StructLiteral(literal) => {
            validate_literal_field_names(program, machine, state, literal, diagnostics);
            enforce_construction_field_obligations(program, machine, state, literal, diagnostics);
            for field in program.expression_table.struct_fields(literal.fields) {
                scan_expression(program, machine, state, field.value, diagnostics);
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                scan_expression(program, machine, state, *element, diagnostics);
            }
        }
        ExpressionNode::Binary(binary) => {
            scan_expression(program, machine, state, binary.left, diagnostics);
            scan_expression(program, machine, state, binary.right, diagnostics);
        }
        ExpressionNode::Cast(cast) => {
            scan_expression(program, machine, state, cast.value, diagnostics)
        }
        ExpressionNode::Call(call) => {
            scan_expression(program, machine, state, call.receiver, diagnostics);
            for argument in program.expression_table.expression_handles(call.arguments) {
                scan_expression(program, machine, state, *argument, diagnostics);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            scan_expression(program, machine, state, indexed.collection, diagnostics);
            scan_expression(program, machine, state, indexed.index, diagnostics);
        }
        ExpressionNode::Member(member) => {
            scan_expression(program, machine, state, member.receiver, diagnostics)
        }
        ExpressionNode::Borrow(inner) => {
            scan_expression(program, machine, state, inner.target, diagnostics)
        }
        ExpressionNode::Range(range) => {
            scan_expression(program, machine, state, range.start, diagnostics);
            scan_expression(program, machine, state, range.end, diagnostics);
        }
        ExpressionNode::Unary(unary) => {
            scan_expression(program, machine, state, unary.operand, diagnostics)
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

/// Check one literal's named fields against the constructed shape's declared
/// members: record literals (current or historical shape) construct FIELD
/// members; case literals construct the named variant's PAYLOAD fields.
fn validate_literal_field_names(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    literal: &TableStructLiteral,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let type_name = literal.type_name.as_str();

    // A field named more than once in one literal is ambiguous: only the FIRST
    // value is stored and the rest are silently dropped (verified: `Point { x: 1,
    // x: 2, .. }` keeps x == 1). Reject it, mirroring the duplicate-member
    // rejection on the data DECLARATION. Independent of type resolution, so it runs
    // before the definition lookup (and thus for generic/unresolved shapes too);
    // field counts are tiny, so a linear scan is fine.
    let mut seen: Vec<&str> = Vec::new();
    for field in program.expression_table.struct_fields(literal.fields) {
        let name = field.name.as_str();
        if seen.contains(&name) {
            diagnostics.push(Diagnostic::error(format!(
                "data `{type_name}` literal has duplicate field `{name}`"
            )));
        } else {
            seen.push(name);
        }
    }

    let Some(data_definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == type_name)
    else {
        // The literal names a type that is not a data definition -- a primitive
        // (`i32 { a: 1 }`) or an undefined name (`Nonexistent { a: 1 }`). Neither is
        // constructible with `{ ... }`; both used to compile silently, binding a ZII
        // value. Generic data definitions ARE found here (handled just below), so
        // this fires only on genuinely non-constructible names.
        if PrimitiveType::from_name(type_name).is_some() {
            diagnostics.push(Diagnostic::error(format!(
                "cannot construct primitive type `{type_name}` with a struct literal"
            )));
        } else {
            diagnostics.push(Diagnostic::error(format!(
                "struct literal names unknown data type `{type_name}`"
            )));
        }
        return;
    };
    if data_definition.quotient.is_some() {
        diagnostics.push(Diagnostic::error(format!(
            "cannot construct quotient `{type_name}` with a struct or case literal: retained representatives are opaque and quotient values may be minted only from the exact carrier with `as {type_name}`"
        )));
        return;
    }
    if data_definition.type_parameters.count() > 0 {
        return;
    }
    if data_definition.supply_mode == psi_language_semantics::DataSupplyMode::BoundaryOpaque {
        diagnostics.push(Diagnostic::error(format!(
            "opaque boundary data `{type_name}` has no public constructor; a boundary provider must establish its values"
        )));
        return;
    }

    // R2 rung 2b + slice 9 (ch12 "Construction is the gate"): a literal of
    // a domain-carrying type must PROVE the default domain -- every `where`
    // fact folds over the field-value INTERVALS (integer literals as
    // points; ranged places by their DECLARED intervals; omitted fields
    // read 0). Definitely-false refuses as a violation; unprovable refuses
    // with direction.
    validate_literal_default_domain(
        program,
        machine,
        state,
        literal,
        data_definition,
        diagnostics,
    );
    validate_omitted_gated_fields(program, literal, data_definition, diagnostics);

    match &literal.case_name {
        None => {
            // A case-bearing type (sum or mixed) has no record-form literal:
            // construction always names the case, which pins the tag. The
            // zero case with named common fields is `Type::ZeroCase { ... }`.
            if program
                .data_members(data_definition)
                .iter()
                .any(|member| matches!(member, DataMember::Variant(_)))
            {
                diagnostics.push(Diagnostic::error(format!(
                    "data `{type_name}` is case-bearing; construct a case (`{type_name}::Case {{ ... }}`) instead of a record literal"
                )));
                return;
            }
            for field in program.expression_table.struct_fields(literal.fields) {
                if !data_declares_field(program, data_definition, field.name.as_str()) {
                    diagnostics.push(Diagnostic::error(format!(
                        "data `{type_name}` has no field `{}`",
                        field.name.as_str()
                    )));
                }
            }
        }
        Some(case_name) => {
            let Some(variant) = program
                .data_members(data_definition)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.name.as_str() == case_name.as_str() => {
                        Some(variant)
                    }
                    _ => None,
                })
            else {
                return;
            };
            // A case literal names the case's PAYLOAD fields, and -- for mixed
            // shapes -- may name COMMON fields alongside them (unnamed common
            // fields zero-initialize; frozen decision 7's construction rule).
            for field in program.expression_table.struct_fields(literal.fields) {
                let declared = program
                    .data_payload_fields(variant)
                    .iter()
                    .any(|payload_field| payload_field.name.as_str() == field.name.as_str())
                    || data_declares_field(program, data_definition, field.name.as_str());
                if !declared {
                    diagnostics.push(Diagnostic::error(format!(
                        "case `{type_name}::{}` has no payload field `{}`",
                        case_name.as_str(),
                        field.name.as_str()
                    )));
                }
            }
        }
    }
}

/// Omitted fields are physically zeroed. That is construction sugar only when
/// zero is already an established value of the field type; a zero-excluding
/// range or nested gated record makes the field mandatory. Sum construction
/// checks common fields plus the selected case payload only, so a payload-free
/// zero case honestly absorbs gates carried by later cases.
fn validate_omitted_gated_fields(
    program: &TypedTrees,
    literal: &TableStructLiteral,
    data_definition: &DataDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let authored = program.expression_table.struct_fields(literal.fields);
    let field_was_authored = |name: &str| authored.iter().any(|field| field.name.as_str() == name);
    let mut candidates: Vec<&psi_typed_trees::data::DataField> = program
        .data_members(data_definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field),
            DataMember::Variant(_) => None,
        })
        .collect();
    if let Some(case_name) = literal.case_name.as_ref()
        && let Some(variant) = program
            .data_members(data_definition)
            .iter()
            .find_map(|member| match member {
                DataMember::Variant(variant) if variant.name.as_str() == case_name.as_str() => {
                    Some(variant)
                }
                _ => None,
            })
    {
        candidates.extend(program.data_payload_fields(variant));
    }

    for field in candidates {
        if !field_was_authored(field.name.as_str())
            && crate::data::type_requires_establishment(program, field.type_reference)
        {
            diagnostics.push(Diagnostic::error(format!(
                "construction of `{}` omits gated field `{}`: its zero-filled representation is not an established value -- initialize it explicitly",
                literal.type_name.as_str(),
                field.name.as_str(),
            )));
        }
    }
}

pub(crate) fn data_declares_field(
    program: &TypedTrees,
    data_definition: &DataDefinition,
    field_name: &str,
) -> bool {
    program.data_members(data_definition).iter().any(
        |member| matches!(member, DataMember::Field(field) if field.name.as_str() == field_name),
    )
}
