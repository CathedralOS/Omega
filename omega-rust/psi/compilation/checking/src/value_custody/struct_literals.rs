//! Struct-literal field validation: every named field of a brace construction
//! must be a declared member of the constructed shape. Covers current-shape
//! record literals (`Counter { count: 0 }`), historical-shape literals
//! (for example, an ordinary historical shape `CounterV1 { counter: 3 }`),
//! version's root-level shape definition), and case-payload literals
//! (`Command::Say { text: ... }`). Literals whose head type is not a data
//! definition in this program reject. Generic member checks may wait for
//! concrete instantiation; declaration-level constructor authority never does.
//!
//! Normalized layout-bearing constructors also owe representable physical
//! stores. The existing scan retains their exact field/value decisions and
//! statement coordinates for CheckFacts; occurrence-bound analysis supplies
//! prefix and operand effects without replaying local initializers here.
//! Default-domain and case predicates consume the normal operation's captured
//! operands before constructor admission, including completed child guarantees.
//! An unrelated mutable operand does not erase an earlier guard; an overlapping
//! write does. The syntax scan locates obligations, not a second fact-flow model.
//!
//! Bare case values obey construction obligations too. A case on the right of
//! membership instead denotes a tag predicate: only exact subject/owner/case
//! evidence permits skipping its construction checks. Invalid membership must
//! reject, not be reinterpreted as equality between independently valid values.

use diagnostics::Diagnostic;
use typed_trees::data::{DataDefinition, DataMember};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use typed_trees::types::PrimitiveType;

mod construction_bounds;
mod field_obligations;
mod observations;
mod window_elements;

use observations::ConstructionObservations;

pub(crate) use window_elements::validate_array_window_elements;
pub use window_elements::{ArrayWindowShape, array_window_shape};

pub use construction_bounds::case_predicates_admit_zero;
use construction_bounds::{validate_literal_case_constraints, validate_literal_default_domain};
use field_obligations::enforce_construction_field_obligations;
pub use field_obligations::selected_construction_field_type;
pub(crate) use field_obligations::{construction_application, selected_construction_field};
pub(crate) use field_obligations::{
    construction_field_type, each_selected_literal_field, validate_array_literal_elements,
    validate_array_literal_elements_for_shape,
};

/// Establish a compiler-supplied literal with the ordinary construction rules.
/// The caller supplies an exact-carrier expression in its own statement
/// context. This judges that construction only; it does not admit a machine
/// body, executable closure, or returned layout plan.
pub fn validate_value_construction(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let observations = &ConstructionObservations::new(program, machine, state, expression);
    scan_expression(
        program,
        machine,
        state,
        observations,
        expression,
        &mut diagnostics,
        &mut Vec::new(),
        0,
        formation_obligations,
    );
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub(crate) fn validate_struct_literal_fields(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) -> Vec<checked_trees::CheckedStoredIntegerWrite> {
    let mut stored_integer_writes = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let observations = &ConstructionObservations::new(
                program,
                machine,
                state,
                ExpressionHandle::invalid(),
            );
            for (statement_index, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                // Arm markers duplicate the dispatch subject for independent
                // destructure/exhaustiveness validation. They do not evaluate
                // a value; construction is checked at its real guard or root.
                if crate::proof_contracts::contract_entailment::is_arm_pattern_marker(statement) {
                    continue;
                }
                match statement {
                    StatementNode::Assembly(_) => {}
                    StatementNode::RootBinding(binding) => {
                        for expression in [binding.receiver, binding.implementation_operand] {
                            if expression.is_valid() {
                                scan_expression(
                                    program,
                                    machine,
                                    state,
                                    observations,
                                    expression,
                                    diagnostics,
                                    &mut stored_integer_writes,
                                    statement_index,
                                    formation_obligations,
                                );
                            }
                        }
                    }
                    StatementNode::AssemblyFact(fact) => {
                        for expression in program.tables.proof_fact_expression_roots(fact.fact) {
                            scan_expression(
                                program,
                                machine,
                                state,
                                observations,
                                expression,
                                diagnostics,
                                &mut stored_integer_writes,
                                statement_index,
                                formation_obligations,
                            );
                        }
                    }
                    StatementNode::Assignment(assignment) => {
                        scan_expression(
                            program,
                            machine,
                            state,
                            observations,
                            assignment.target,
                            diagnostics,
                            &mut stored_integer_writes,
                            statement_index,
                            formation_obligations,
                        );
                        scan_expression(
                            program,
                            machine,
                            state,
                            observations,
                            assignment.value,
                            diagnostics,
                            &mut stored_integer_writes,
                            statement_index,
                            formation_obligations,
                        );
                    }
                    StatementNode::Call(call) => {
                        for argument in program.statement_table.expression_handles(call.arguments) {
                            scan_expression(
                                program,
                                machine,
                                state,
                                observations,
                                *argument,
                                diagnostics,
                                &mut stored_integer_writes,
                                statement_index,
                                formation_obligations,
                            );
                        }
                    }
                    StatementNode::Expression(expression) => {
                        scan_expression(
                            program,
                            machine,
                            state,
                            observations,
                            *expression,
                            diagnostics,
                            &mut stored_integer_writes,
                            statement_index,
                            formation_obligations,
                        );
                    }
                    StatementNode::LocalData(local_data) => {
                        scan_expression(
                            program,
                            machine,
                            state,
                            observations,
                            local_data.initial_value,
                            diagnostics,
                            &mut stored_integer_writes,
                            statement_index,
                            formation_obligations,
                        );
                    }
                    StatementNode::Transition(transition) => {
                        if let TransitionGuardNode::When(guard) = &transition.guard {
                            scan_expression(
                                program,
                                machine,
                                state,
                                observations,
                                *guard,
                                diagnostics,
                                &mut stored_integer_writes,
                                statement_index,
                                formation_obligations,
                            );
                        }
                        for target in transition.targets() {
                            scan_transition_target(
                                program,
                                machine,
                                state,
                                observations,
                                program.statement_table.transition_target(target),
                                diagnostics,
                                &mut stored_integer_writes,
                                statement_index,
                                formation_obligations,
                            );
                        }
                    }
                }
            }
        }
    }
    stored_integer_writes
}

fn scan_transition_target(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    observations: &ConstructionObservations<'_>,
    target: &TransitionTargetNode,
    diagnostics: &mut Vec<Diagnostic>,
    stored_integer_writes: &mut Vec<checked_trees::CheckedStoredIntegerWrite>,
    statement_index: usize,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    match target {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                scan_expression(
                    program,
                    machine,
                    state,
                    observations,
                    *argument,
                    diagnostics,
                    stored_integer_writes,
                    statement_index,
                    formation_obligations,
                );
            }
        }
        TransitionTargetNode::Value(expression) => {
            scan_expression(
                program,
                machine,
                state,
                observations,
                *expression,
                diagnostics,
                stored_integer_writes,
                statement_index,
                formation_obligations,
            );
        }
        TransitionTargetNode::Terminal => {}
    }
}

fn scan_expression(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    observations: &ConstructionObservations<'_>,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
    stored_integer_writes: &mut Vec<checked_trees::CheckedStoredIntegerWrite>,
    statement_index: usize,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    if !expression.is_valid() {
        return;
    }

    // The child roster is the shared expression traversal's, so this scan
    // cannot drift from the checks that walk the same tree. Only three node
    // kinds keep an arm of their own, because their judgment rather than their
    // shape is special: a struct literal is judged before its fields, a proven
    // case membership never scans its right operand, and an atomic access
    // scans its value but never its result. Each of those walks itself.
    let node = program.expression_table.expression(expression);
    match node {
        ExpressionNode::StructLiteral(literal) => {
            validate_literal_field_names(
                program,
                machine,
                state,
                expression,
                statement_index,
                literal,
                diagnostics,
            );
            enforce_construction_field_obligations(
                program,
                machine,
                state,
                observations,
                expression,
                literal,
                diagnostics,
                stored_integer_writes,
                statement_index,
                formation_obligations,
            );
        }
        ExpressionNode::Binary(binary) => {
            scan_expression(
                program,
                machine,
                state,
                observations,
                binary.left,
                diagnostics,
                stored_integer_writes,
                statement_index,
                formation_obligations,
            );
            let membership = matches!(
                binary.operator,
                typed_trees::expression::BinaryOperator::Equal
                    | typed_trees::expression::BinaryOperator::CaseMembership
            ) && crate::proof_contracts::bound_expression_meaning::has_exact_case_membership_meaning(
                program,
                machine,
                Some(state),
                expression,
                binary,
            );
            if binary.operator == typed_trees::expression::BinaryOperator::CaseMembership
                && !membership
            {
                diagnostics.push(Diagnostic::error(
                    "case-membership operation lost its exact subject, carrier or case identity",
                ));
                return;
            }
            if !membership {
                // A failed nominal membership check is not ordinary equality.
                // Payload-free cases can share a tag and leaf spelling across
                // packages; neither is evidence that the subject has this owner.
                if matches!(
                    binary.operator,
                    typed_trees::expression::BinaryOperator::Equal
                        | typed_trees::expression::BinaryOperator::CaseMembership
                ) && program.expression_table.authored_selection_occurrences(expression).any(|occurrence| {
                    program.authored_declaration_selections().get(occurrence).is_some_and(|selection| {
                        selection.kind() == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::CaseMembership
                    })
                }) {
                    diagnostics.push(Diagnostic::error(format!(
                        "case membership must test a value of the exact declaring data type (machine `{}`, state `{}`)",
                        machine.name, state.name,
                    )));
                }
                scan_expression(
                    program,
                    machine,
                    state,
                    observations,
                    binary.right,
                    diagnostics,
                    stored_integer_writes,
                    statement_index,
                    formation_obligations,
                );
            }
            return;
        }
        ExpressionNode::Atomic(atomic) => {
            scan_expression(
                program,
                machine,
                state,
                observations,
                atomic.value,
                diagnostics,
                stored_integer_writes,
                statement_index,
                formation_obligations,
            );
            return;
        }
        ExpressionNode::Name(path) => {
            let Some(owner) =
                crate::proof_contracts::bound_expression_meaning::exact_case_reference_owner(
                    program, expression,
                )
            else {
                return;
            };
            let Some(variant) =
                program
                    .data_members(owner)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(variant) if variant.symbol == path.symbol => {
                            Some(variant)
                        }
                        _ => None,
                    })
            else {
                return;
            };
            if !variant.payload.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "case `{}::{}` has a payload; construct it with a case literal",
                    owner.name, variant.name,
                )));
                return;
            }
            // A borrowed empty-field view shares the ordinary construction
            // obligations without inserting a synthetic authored expression.
            let literal = TableStructLiteral {
                type_name: owner.name.clone(),
                type_symbol: owner.symbol,
                case_name: Some(variant.name.clone()),
                case_symbol: Some(variant.symbol),
                fields: Default::default(),
            };
            validate_literal_field_names(
                program,
                machine,
                state,
                expression,
                statement_index,
                &literal,
                diagnostics,
            );
        }
        _ => {}
    }
    crate::expression_traversal::children(program, node, |child| {
        scan_expression(
            program,
            machine,
            state,
            observations,
            child,
            diagnostics,
            stored_integer_writes,
            statement_index,
            formation_obligations,
        );
    });
}

/// Check one literal's named fields against the constructed shape's declared
/// members: record literals (current or historical shape) construct FIELD
/// members; case literals construct the named variant's PAYLOAD fields.
fn validate_literal_field_names(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    statement_index: usize,
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

    let Some(data_definition) =
        crate::declarations::data_queries::data_definition_by_symbol(program, literal.type_symbol)
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
    // Constructor authority belongs to the declaration, not to its member
    // specialization. An opaque generic has no authored constructor even
    // when its dependent field checks must wait for concrete arguments.
    if data_definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque {
        diagnostics.push(Diagnostic::error(format!(
            "opaque boundary data `{type_name}` has no public constructor; a boundary provider must establish its values"
        )));
        return;
    }
    // Member and predicate checking share this exact construction's retained
    // application. The authored template above owns constructor authority;
    // its open telescope is not an exemption for a closed occurrence.
    let Some(data_definition) =
        field_obligations::selected_construction_definition(program, expression, literal)
    else {
        diagnostics.push(Diagnostic::error("constructor obligations have no exact prepared application; unresolved generic members cannot waive field or case checking"));
        return;
    };
    for field in program.expression_table.struct_fields(literal.fields) {
        if selected_construction_field_type(program, expression, literal, field.field_symbol)
            .is_none()
        {
            diagnostics.push(Diagnostic::error(format!(
                "construction of `{type_name}` field `{}` lost its exact declared owner",
                field.name,
            )));
        }
    }

    // Construction establishes the default domain from the values observed
    // at each field's evaluation point; omitted fields contribute ZII zero.
    // False predicates and insufficient facts remain distinct refusals.
    validate_literal_default_domain(
        program,
        machine,
        state,
        expression,
        statement_index,
        literal,
        data_definition,
        diagnostics,
    );
    validate_omitted_gated_fields(program, expression, literal, data_definition, diagnostics);

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
            let Some(variant) = field_obligations::selected_construction_case(
                program,
                expression,
                literal,
                data_definition,
            ) else {
                diagnostics.push(Diagnostic::error(
                    "case construction lost its exact active-case correspondence",
                ));
                return;
            };
            // CASE-CONSTRAINTS: the selected case's `where` facts prove at
            // construction through the same literal fold as the default
            // domain; only the selected case's fact set is consulted.
            validate_literal_case_constraints(
                program,
                machine,
                state,
                expression,
                statement_index,
                literal,
                data_definition,
                case_name.as_str(),
                variant,
                diagnostics,
            );
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
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    literal: &TableStructLiteral,
    data_definition: &DataDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let authored = program.expression_table.struct_fields(literal.fields);
    let field_was_authored = |name: &str| authored.iter().any(|field| field.name.as_str() == name);
    let mut candidates: Vec<&typed_trees::data::DataField> = program
        .data_members(data_definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field),
            DataMember::Variant(_) => None,
        })
        .collect();
    if literal.case_name.is_some()
        && let Some(variant) = field_obligations::selected_construction_case(
            program,
            expression,
            literal,
            data_definition,
        )
    {
        candidates.extend(program.data_payload_fields(variant));
    }

    for field in candidates {
        if !field_was_authored(field.name.as_str())
            && crate::value_custody::data::type_requires_establishment(
                program,
                field.type_reference,
            )
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
    program: &dyn crate::CheckingInput,
    data_definition: &DataDefinition,
    field_name: &str,
) -> bool {
    program.data_members(data_definition).iter().any(
        |member| matches!(member, DataMember::Field(field) if field.name.as_str() == field_name),
    )
}
