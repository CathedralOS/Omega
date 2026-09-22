use super::{
    Context, argument_position_context, call_targets_proof_machine, callee_parameters,
    erased_fields,
};
use diagnostics::Diagnostic;
use language_core::BindingRelevance;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableCall};
use typed_trees::types::TypeReferenceHandle;

pub(super) fn validate_expression(
    program: &TypedTrees,
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
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
        ExpressionNode::Match(dispatch) => {
            for child in crate::value_custody::expression_types::match_children(program, *dispatch)
            {
                validate_expression(
                    program,
                    proof_only,
                    machine,
                    state,
                    child,
                    context,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Name(path) => {
            if context == Context::Runtime {
                let mut reported = false;
                for symbol in program
                    .expression_table
                    .name_path_member_symbols(path.member_symbols)
                {
                    reported |= report_runtime_erased_field(program, *symbol, diagnostics)
                        || report_runtime_erased_binding(program, state, *symbol, diagnostics);
                }
                let members = program.expression_table.name_path_members(path.members);
                if !reported && members.len() == 2 && members[0].is_self_receiver() {
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
            if context == Context::Runtime
                && !report_runtime_erased_field(program, member.member_symbol, diagnostics)
            {
                if crate::value_custody::places::direct_self_field_member(program, expression)
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
                    "erased binding initializer calls runtime machine `{}`; erased initialization cannot perform runtime effects or computation",
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
            let parameters = callee_parameters(program, call.target_symbol);
            for (position, argument) in program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .enumerate()
            {
                validate_expression(
                    program,
                    proof_only,
                    machine,
                    state,
                    *argument,
                    argument_position_context(context, parameters, position),
                    diagnostics,
                );
            }
        }
        ExpressionNode::Atomic(atomic) => {
            if context == Context::ErasedInitializer {
                diagnostics.push(Diagnostic::error(
                    "erased binding initializer cannot perform an atomic runtime operation",
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
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
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

/// A statement call's receiver is the callee's `self` operand: an address of
/// runtime storage, plus a read for `&self`/`self` targets. The receiver is a
/// name path with independently resolved root and leaf symbols, not an
/// expression child of the call, so the expression walker never sees it. When
/// the path resolves to an erased binding or projects through an erased field,
/// the call is a runtime use of a proof-side binding and rejects exactly like
/// a direct read. The caller runs this only when the call itself is a runtime
/// call; a proof-machine receiver may cite erased bindings as proof material.
pub(super) fn validate_statement_call_receiver(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &TableCall,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Resolution binds the path's root and leaf independently
    // (symbols/statements/routing.rs), so a one- or two-member receiver's
    // erased endpoint is answered directly by symbol.
    if report_runtime_erased_binding(program, state, call.receiver_root_symbol, diagnostics)
        || report_runtime_erased_binding(program, state, call.receiver_symbol, diagnostics)
        || report_runtime_erased_field(program, call.receiver_root_symbol, diagnostics)
        || report_runtime_erased_field(program, call.receiver_symbol, diagnostics)
    {
        return;
    }
    // A projected member can await type-aware resolution, and a middle
    // projection retains no symbol at all: `a.b.c.tick()` where only `b` is
    // erased still binds just `a` and `c`. Descend the declared member types
    // instead, checking every projected field's relevance; when a segment has
    // no closed record owner (a generic, boundary, or namespace root) the walk
    // stops rather than guessing by name.
    let members = program.statement_table.name_path_members(call.receiver);
    let Some((first, projected)) = members.split_first() else {
        return;
    };
    let mut owner = if first.is_self_receiver() {
        // `self` resolves to the machine itself, never an erased binding;
        // its fields live on the attached data.
        program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == machine.attached_data_symbol)
    } else {
        receiver_root_type(program, state, call.receiver_root_symbol).and_then(|type_reference| {
            crate::value_custody::places::data_definition_for_type(program, type_reference)
        })
    };
    for member in projected {
        let Some(definition) = owner else {
            return;
        };
        let Some(field) = crate::value_custody::places::exact_data_member_field(
            program,
            definition,
            SymbolHandle::invalid(),
            member.as_str(),
            None,
        ) else {
            return;
        };
        if field.relevance.is_erased() {
            diagnostics.push(Diagnostic::error(format!(
                "erased field `{}` has no runtime value, address, read, write, or cleanup; it may be used only by proofs or another erased binding",
                field.name
            )));
            return;
        }
        owner =
            crate::value_custody::places::data_definition_for_type(program, field.type_reference);
    }
}

/// The declared type of a receiver path's root binding: a parameter or `let`
/// local of the current state, or an attached field named bare. Anything else
/// (a namespace root, an unresolved projection) yields no owner to descend.
fn receiver_root_type(
    program: &TypedTrees,
    state: &State,
    root: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !root.is_valid() {
        return None;
    }
    if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == root)
    {
        return Some(parameter.type_reference);
    }
    if let Some(local) = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == root => Some(local),
            _ => None,
        })
    {
        return Some(local.type_reference);
    }
    field_by_symbol(program, root).map(|field| field.type_reference)
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

/// An erased parameter or `let` local of the current state has no runtime
/// value; a runtime read of its symbol rejects exactly like an erased field.
fn report_runtime_erased_binding(
    program: &TypedTrees,
    state: &State,
    symbol: SymbolHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if !symbol.is_valid() {
        return false;
    }
    let erased_parameter = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol && parameter.relevance.is_erased());
    if let Some(parameter) = erased_parameter {
        diagnostics.push(Diagnostic::error(format!(
            "erased parameter `{}` has no runtime value, address, read, write, or cleanup; it may be used only by proofs, contracts, or another erased binding",
            parameter.name
        )));
        return true;
    }
    let erased_local = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local)
                if local.symbol == symbol && local.relevance.is_erased() =>
            {
                Some(local)
            }
            _ => None,
        });
    if let Some(local) = erased_local {
        diagnostics.push(Diagnostic::error(format!(
            "erased local `{}` has no runtime value, address, read, write, or cleanup; it may be used only by proofs, contracts, or another erased binding",
            local.name
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
    let receiver_type =
        crate::value_custody::places::declared_place_type(program, machine, Some(state), receiver)
            .or_else(|| {
                crate::value_custody::places::declared_indexed_projection_type(
                    program,
                    machine,
                    Some(state),
                    receiver,
                )
            });
    let Some(definition) = receiver_type.and_then(|type_reference| {
        crate::value_custody::places::data_definition_for_type(program, type_reference)
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
    case_name: Option<&typed_trees::name::Identifier>,
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
