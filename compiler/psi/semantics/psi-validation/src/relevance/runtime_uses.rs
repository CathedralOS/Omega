use super::{Context, call_targets_proof_machine, erased_fields};
use psi_diagnostics::Diagnostic;
use psi_language_core::BindingRelevance;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataDefinition, DataField, DataMember};
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;

pub(super) fn validate_expression(
    program: &TypedTrees,
    proof_only: &psi_typed_trees::proof_only::ProofOnlyClassification,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    context: Context,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            if context == Context::Runtime {
                let mut reported = false;
                for symbol in program
                    .expression_table
                    .name_path_member_symbols(path.member_symbols)
                {
                    reported |= report_runtime_erased_field(program, *symbol, diagnostics);
                }
                let members = program.expression_table.name_path_members(path.members);
                if !reported && members.len() == 2 && members[0].as_str() == "self" {
                    report_runtime_erased_attached_field(
                        program,
                        machine,
                        members[1].as_str(),
                        diagnostics,
                    );
                }
            }
        }
        ExpressionNode::Member(member) => {
            if context == Context::Runtime {
                if !report_runtime_erased_field(program, member.member_symbol, diagnostics) {
                    if crate::places::direct_self_field_member(program, expression)
                        == Some(member.member.as_str())
                    {
                        report_runtime_erased_attached_field(
                            program,
                            machine,
                            member.member.as_str(),
                            diagnostics,
                        );
                    } else {
                        report_runtime_erased_member_by_receiver(
                            program,
                            machine,
                            state,
                            member.receiver,
                            member.member.as_str(),
                            member.case_variant.as_ref().map(|variant| variant.as_str()),
                            diagnostics,
                        );
                    }
                }
            }
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                member.receiver,
                context,
                diagnostics,
            );
        }
        ExpressionNode::StructLiteral(literal) => validate_struct_literal(
            program,
            proof_only,
            machine,
            state,
            literal,
            context,
            diagnostics,
        ),
        ExpressionNode::Call(call) => {
            if context == Context::Runtime
                && call_targets_proof_machine(program, proof_only, call.target_symbol)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "call to proof machine `{}` has no runtime result; use it only in a proof or erased context, or as a statement citation",
                    call.target
                )));
            }
            if context == Context::ErasedInitializer
                && !call_targets_proof_machine(program, proof_only, call.target_symbol)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "erased field initializer calls runtime machine `{}`; erased initialization cannot perform runtime effects or computation",
                    call.target
                )));
            }
            if call.receiver.is_valid() {
                validate_expression(
                    program,
                    proof_only,
                    machine,
                    state,
                    call.receiver,
                    context,
                    diagnostics,
                );
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                validate_expression(
                    program,
                    proof_only,
                    machine,
                    state,
                    *argument,
                    context,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Atomic(atomic) => {
            if context == Context::ErasedInitializer {
                diagnostics.push(Diagnostic::error(
                    "erased field initializer cannot perform an atomic runtime operation",
                ));
            }
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                atomic.value,
                context,
                diagnostics,
            );
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                atomic.result,
                context,
                diagnostics,
            );
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                validate_expression(
                    program,
                    proof_only,
                    machine,
                    state,
                    *value,
                    context,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                binary.left,
                context,
                diagnostics,
            );
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                binary.right,
                context,
                diagnostics,
            );
        }
        ExpressionNode::Cast(cast) => validate_expression(
            program,
            proof_only,
            machine,
            state,
            cast.value,
            context,
            diagnostics,
        ),
        ExpressionNode::Indexed(indexed) => {
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                indexed.collection,
                context,
                diagnostics,
            );
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                indexed.index,
                context,
                diagnostics,
            );
        }
        ExpressionNode::Borrow(inner) => validate_expression(
            program,
            proof_only,
            machine,
            state,
            inner.target,
            context,
            diagnostics,
        ),
        ExpressionNode::Range(range) => {
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                range.start,
                context,
                diagnostics,
            );
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                range.end,
                context,
                diagnostics,
            );
        }
        ExpressionNode::Unary(unary) => validate_expression(
            program,
            proof_only,
            machine,
            state,
            unary.operand,
            context,
            diagnostics,
        ),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn validate_struct_literal(
    program: &TypedTrees,
    proof_only: &psi_typed_trees::proof_only::ProofOnlyClassification,
    machine: &Machine,
    state: &State,
    literal: &TableStructLiteral,
    context: Context,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name == literal.type_name)
    else {
        for field in program.expression_table.struct_fields(literal.fields) {
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                field.value,
                context,
                diagnostics,
            );
        }
        return;
    };
    let unsupported_bare_generic =
        !definition.type_parameters.is_empty() && !erased_fields(program, definition).is_empty();
    if unsupported_bare_generic {
        diagnostics.push(Diagnostic::error(format!(
            "construction of erased generic data `{}` is unsupported in this context; use a closed generic record in an explicitly typed local initializer",
            definition.name
        )));
    }
    let declared_erased = literal_fields(program, definition, literal.case_name.as_ref())
        .filter(|field| field.relevance.is_erased())
        .collect::<Vec<_>>();
    let authored = program.expression_table.struct_fields(literal.fields);
    for erased in &declared_erased {
        if !unsupported_bare_generic && !authored.iter().any(|field| field.name == erased.name) {
            diagnostics.push(Diagnostic::error(format!(
                "construction of `{}` omits erased field `{}`; supply an explicit proof term because no unique accessible nullary constructor determines this binding",
                definition.name, erased.name
            )));
        }
    }
    for field in authored {
        let field_context = if declared_erased
            .iter()
            .any(|declared| declared.name == field.name)
        {
            Context::ErasedInitializer
        } else {
            context
        };
        validate_expression(
            program,
            proof_only,
            machine,
            state,
            field.value,
            field_context,
            diagnostics,
        );
    }
}

fn report_runtime_erased_field(
    program: &TypedTrees,
    symbol: SymbolHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if !symbol.is_valid() {
        return false;
    }
    if let Some(field) = field_by_symbol(program, symbol)
        && field.relevance == BindingRelevance::Erased
    {
        diagnostics.push(Diagnostic::error(format!(
            "erased field `{}` has no runtime value, address, read, write, or cleanup; it may be used only by proofs or another erased binding",
            field.name
        )));
        return true;
    }
    false
}

fn report_runtime_erased_attached_field(
    program: &TypedTrees,
    machine: &Machine,
    name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(attached) = machine.attached_data.as_ref() else {
        return;
    };
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name == *attached)
    else {
        return;
    };
    if let Some(field) = program.data_members(definition).iter().find_map(|member| {
        let DataMember::Field(field) = member else {
            return None;
        };
        (field.name.as_str() == name).then_some(field)
    }) {
        report_runtime_erased_field(program, field.symbol, diagnostics);
    }
}

fn report_runtime_erased_member_by_receiver(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    receiver: ExpressionHandle,
    name: &str,
    case_variant: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let receiver_type = crate::places::declared_place_type(program, machine, Some(state), receiver)
        .or_else(|| {
            crate::places::declared_indexed_projection_type(program, machine, Some(state), receiver)
        });
    let Some(definition) = receiver_type.and_then(|type_reference| {
        crate::places::data_definition_for_type(program, type_reference)
    }) else {
        return;
    };
    let field = if let Some(case_variant) = case_variant {
        program.data_members(definition).iter().find_map(|member| {
            let DataMember::Variant(variant) = member else {
                return None;
            };
            (variant.name.as_str() == case_variant).then(|| {
                program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.name.as_str() == name)
            })?
        })
    } else {
        let common = program.data_members(definition).iter().find_map(|member| {
            let DataMember::Field(field) = member else {
                return None;
            };
            (field.name.as_str() == name).then_some(field)
        });
        common.or_else(|| {
            program.data_members(definition).iter().find_map(|member| {
                let DataMember::Variant(variant) = member else {
                    return None;
                };
                program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.name.as_str() == name && field.relevance.is_erased())
            })
        })
    };
    if let Some(field) = field {
        report_runtime_erased_field(program, field.symbol, diagnostics);
    }
}

fn literal_fields<'program>(
    program: &'program TypedTrees,
    definition: &'program DataDefinition,
    case_name: Option<&psi_typed_trees::name::Identifier>,
) -> impl Iterator<Item = &'program DataField> {
    let mut fields = Vec::new();
    for member in program.data_members(definition) {
        match member {
            DataMember::Field(field) => fields.push(field),
            DataMember::Variant(variant)
                if case_name.is_some_and(|case_name| *case_name == variant.name) =>
            {
                fields.extend(program.data_payload_fields(variant));
            }
            DataMember::Variant(_) => {}
        }
    }
    fields.into_iter()
}

fn field_by_symbol(program: &TypedTrees, symbol: SymbolHandle) -> Option<&DataField> {
    program.data_definitions().iter().find_map(|definition| {
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.symbol == symbol => Some(field),
                DataMember::Variant(variant) => program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.symbol == symbol),
                _ => None,
            })
    })
}
