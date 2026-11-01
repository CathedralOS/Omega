//! Structural formation uses retained selectors and exact declarations. Diagnostic
//! local names carry no coverage or custody evidence.
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::{
    data::{DataDefinition, DataMember},
    expression::{MatchFieldAction, TableMatchField},
};

/// Validate the semantic component sequence. Case patterns contain payload only;
/// data patterns contain common fields followed by one anonymous case component.
pub(crate) fn validate_component_roster(
    program: &dyn crate::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
    data: &DataDefinition,
    case: SymbolHandle,
    fields: &[TableMatchField],
    has_rest: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use language_core::DataComponentSelector;
    let has_cases = program
        .data_members(data)
        .iter()
        .any(|member| matches!(member, DataMember::Variant(_)));
    let declared = if case.is_valid() {
        program
            .data_members(data)
            .iter()
            .filter_map(DataMember::as_variant)
            .find(|variant| variant.symbol == case)
            .map(|variant| {
                program
                    .data_payload_fields(variant)
                    .iter()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    } else {
        program
            .data_members(data)
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field) => Some(field),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let case_component = !case.is_valid() && has_cases;
    let arity = declared.len() + usize::from(case_component);
    let mut selected = Vec::new();
    for field in fields {
        let index = match field.selector {
            DataComponentSelector::Case => case_component.then_some(declared.len()),
            DataComponentSelector::Positional(ordinal) => {
                ((ordinal as usize) < arity).then_some(ordinal as usize)
            }
            DataComponentSelector::Named => {
                let selected = declared
                    .iter()
                    .find(|declared| declared.symbol == field.field_symbol)
                    .map(|field| field.symbol)
                    .or_else(|| {
                        program
                            .data_constructor_field(
                                program
                                    .type_reference_table
                                    .peeled_type_reference(reference),
                                data.symbol,
                                field.field_symbol,
                            )
                            .map(|field| field.symbol)
                    });
                selected.and_then(|symbol| declared.iter().position(|field| field.symbol == symbol))
            }
        };
        if let Some(index) = index {
            if selected.contains(&index) {
                diagnostics.push(Diagnostic::error(format!(
                    "data pattern selects component `{}` more than once",
                    field.member
                )));
            }
            selected.push(index);
            if index == declared.len() && matches!(field.action, MatchFieldAction::Equal(_)) {
                diagnostics.push(Diagnostic::error(
                    "the anonymous case component requires `case as binding` or `case as _`",
                ));
            }
        } else {
            diagnostics.push(Diagnostic::error(format!("data pattern selects `{}`, which is not a component of this exact data or case declaration", field.member)));
        }
    }
    if !has_rest && (0..arity).any(|index| !selected.contains(&index)) {
        diagnostics.push(Diagnostic::error(
            "data pattern must mention every declared component or explicitly use `..`",
        ));
    }
}

pub(crate) fn validate_destructure_exhaustiveness(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let Some(local) = statement.as_local_data() else {
                    continue;
                };
                let Some(pattern) = local.pattern else {
                    continue;
                };
                let expressions = crate::frozen_program::expression_table(program);
                let declared = crate::expression_result_type_reference(
                    program,
                    machine,
                    state,
                    local.initial_value,
                );
                let Some(reference) = declared else {
                    diagnostics.push(Diagnostic::error(
                        "data pattern requires its initializer's exact declared carrier",
                    ));
                    continue;
                };
                let Some(data) =
                    crate::value_custody::places::data_definition_for_type(program, reference)
                else {
                    diagnostics.push(Diagnostic::error(
                        "data pattern requires a data declaration",
                    ));
                    continue;
                };
                if data.quotient.is_some() {
                    diagnostics.push(Diagnostic::error(
                        "data patterns cannot expose an opaque quotient representative",
                    ));
                    continue;
                }
                let Some(carrier) =
                    crate::value_custody::places::unwrapped_type_reference(program, reference)
                else {
                    continue;
                };
                let mut case = SymbolHandle::invalid();
                if pattern.head.is_valid() {
                    let head = expressions
                        .name_path_node(pattern.head)
                        .map(|head| head.symbol)
                        .or_else(|| match expressions.expression(pattern.head) {
                            typed_trees::expression::ExpressionNode::StructLiteral(literal)
                                if expressions.struct_fields(literal.fields).is_empty() =>
                            {
                                literal.case_symbol
                            }
                            _ => None,
                        })
                        .unwrap_or_default();
                    if program.data_constructor_definition(carrier, head).is_none() {
                        if let Some(variant) = head
                            .is_valid()
                            .then(|| program.symbols.get(head).parent)
                            .and_then(|owner| program.data_constructor_case(carrier, owner, head))
                        {
                            case = variant.symbol;
                        } else {
                            diagnostics.push(Diagnostic::error("data pattern does not name its initializer's exact carrier or case").with_source_span(expressions.source_span(pattern.head)));
                            continue;
                        }
                    }
                }
                if program.type_reference_table.has_case_view(reference) && !case.is_valid() {
                    diagnostics.push(Diagnostic::error("a case-only view decomposes through its selected case payload, not the original common fields"));
                    continue;
                }
                let fields = expressions.match_fields(pattern.fields);
                validate_component_roster(
                    program,
                    reference,
                    data,
                    case,
                    fields,
                    pattern.has_rest,
                    diagnostics,
                );
                // A binding is not a dispatch: tests require a prior proof. Keep
                // refutable patterns out of the runtime path; proof admission is
                // performed by the ordinary live-condition checking pass.
                if local.role != language_core::LocalBindingRole::ArmPattern
                    && ((case.is_valid()
                        && !(program.data_members(data).iter().filter_map(DataMember::as_variant)
                            .map(|variant| variant.symbol).eq(std::iter::once(case))
                            || crate::proof_contracts::contract_entailment::completed_value_case_at_occurrence(
                                program, machine, state, statement_index, local.initial_value, reference,
                            ) == Some(case)))
                        || (fields.iter().any(|field| matches!(field.action, MatchFieldAction::Equal(_)))
                            && !crate::proof_contracts::contract_entailment::completed_pattern_values_at_occurrence(
                                program, machine, state, statement_index, local.initial_value,
                                reference, case, fields,
                            )))
                {
                    diagnostics.push(Diagnostic::error("data binding pattern must be proved irrefutable by the subject's checked type and live facts"));
                }
            }
        }
    }
}
