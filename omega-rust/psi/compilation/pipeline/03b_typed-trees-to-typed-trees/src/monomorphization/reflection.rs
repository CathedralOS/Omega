//! Elaborate authorized reflection occurrences into ordinary checked calls.
//!
//! This runs inside generic closure so fields retain their complete applied
//! types. Projection occurs in the lexical author's scope, never under ambient
//! library authority. No wire policy or source text is interpreted here.

use super::{CallSelection, CallSite, Candidate, Diagnostic, SymbolHandle, TypedTrees};
use crate::tables::{expressions_of, expressions_of_mut};
use arena::HandleSpan;
use language_core::ReferenceAccess;
use symbols::{SymbolKind, SymbolTableBuilder};
use typed_trees::data::{DataDefinition, DataField, DataMember};
mod active_case;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableBorrowExpression, TableMemberExpression, TableNamePath,
    TableStructLiteral, TableStructLiteralField,
};
use typed_trees::name::Identifier;
use typed_trees::statement::{StatementNode, TableCall, TableLocalData};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

fn error(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message)]
}

fn is_reflection_declaration(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    program
        .symbols
        .symbol_source_span(symbol)
        .and_then(|span| program.symbols.source_file(span))
        .is_some_and(|source| {
            source.origin == source::SourceOrigin::Toolchain
                && source.path.strip_prefix(&source.package_root).ok()
                    == Some(std::path::Path::new("reflect.omg"))
        })
}

/// Expand one complete occurrence, then recollect handles for the next round.
/// Replacing a statement span invalidates the old round's statement positions.
pub(super) fn elaborate_calls(
    program: &mut TypedTrees,
    candidates: &[Candidate],
    selections: &[CallSelection],
    changed_owners: &mut Vec<SymbolHandle>,
) -> Result<bool, Vec<Diagnostic>> {
    for selection in selections
        .iter()
        .filter(|selection| selection.is_complete())
    {
        let candidate = super::candidate_for_selection(
            program,
            &candidates[selection.candidate_index],
            selection,
        );
        let declaration = candidate.template.template_symbol;
        if !is_reflection_declaration(program, declaration) {
            continue;
        }
        let inspection = program.symbols.name(declaration) == "inspect_record_fields";
        let active = matches!(
            program.symbols.name(declaration),
            "visit_active_fields" | "visit_active_fields_mut"
        );
        let field_access = match program.symbols.name(declaration) {
            "visit_runtime_fields" | "visit_active_fields" | "inspect_record_fields" => {
                ReferenceAccess::Shared
            }
            "visit_runtime_fields_mut" | "visit_active_fields_mut" => ReferenceAccess::Mutable,
            _ => continue,
        };
        let CallSite::Statement(site) = selection.site else {
            return Err(error(
                "typed field visitation requires a unit statement invocation",
            ));
        };
        let StatementNode::Call(call) = program.statement_table.statement(site).clone() else {
            return Err(error("typed field visitation lost its call occurrence"));
        };
        let [Some(subject), Some(context)] = candidate.type_bindings.as_slice() else {
            return Err(error(
                "typed field visitation requires its exact subject and context types",
            ));
        };
        let (case_callback, callback) = match candidate.machine_bindings.as_slice() {
            [Some(case_callback), Some(callback)] if active => (Some(case_callback), callback),
            [Some(callback)] if !active => (None, callback),
            _ => {
                return Err(error(
                    "typed field visitation requires its explicitly selected callbacks",
                ));
            }
        };
        let Some(subject_data) = program.closed_data_definition(*subject).cloned() else {
            return Err(error(
                "typed field visitation requires a closed semantic data application",
            ));
        };
        let author_span = program
            .symbols
            .symbol_provenance_source_span(selection.lexical_owner)
            .ok_or_else(|| error("typed field visitation requires its lexical source authority"))?;
        if !program
            .symbols
            .source_reference_may_select_symbol(author_span, subject_data.symbol)
            || (!program
                .symbols
                .same_symbol_source_package(selection.lexical_owner, subject_data.symbol)
                && !subject_data.is_public)
        {
            return Err(error(
                "typed field visitation cannot select the subject from its lexical author's scope",
            ));
        }
        // Public type visibility describes a carrier; it does not delegate
        // enumeration of that carrier's fields. A foreign library invokes an
        // explicitly supplied owner operation, whose own lexical occurrence
        // is elaborated here under its original package authority.
        if !program
            .symbols
            .same_symbol_source_package(selection.lexical_owner, subject_data.symbol)
        {
            return Err(error(
                "typed field visitation requires its subject owner; delegate an owning operation instead of reflecting a foreign carrier",
            ));
        }
        let owner_identity = program
            .normalized_hermetic_symbol_identity(subject_data.symbol)
            .map_err(|message| {
                error(format!(
                    "typed field visitation subject identity: {message}"
                ))
            })?;
        let requester_identity = program
            .normalized_hermetic_symbol_identity(selection.lexical_owner)
            .map_err(|message| {
                error(format!("typed field visitation author identity: {message}"))
            })?;
        let authority = if program
            .symbols
            .same_symbol_source_package(selection.lexical_owner, subject_data.symbol)
        {
            validation::reflection::SchemaQueryAuthority::OwningScope { requester_identity }
        } else {
            validation::reflection::SchemaQueryAuthority::ForeignScope { requester_identity }
        };
        let graph = validation::reflection::construct_semantic_schema_graph(
            program,
            subject_data.symbol,
            authority,
        )
        .map_err(error)?;
        if !active && graph.cases().next().is_some() {
            return Err(error(
                "sum visitation requires an explicitly selected active-case callback",
            ));
        }
        let owner_application = program.package_qualified_type_identity_text(*subject);
        let members = program.data_members(&subject_data).to_vec();
        // The exact catalog signature owns its metadata vocabulary. Rejoining
        // that declared type handles independently loaded package scopes without
        // choosing another same-named FieldInfo from the program roster.
        let signature = &candidate.template.machine_parameters[usize::from(active)].2;
        let metadata = program
            .state_signature_parameters(signature)
            .get(1)
            .ok_or_else(|| error("typed field visitation lost its metadata parameter"))?;
        let metadata_type = program
            .type_reference_table
            .dereferenced_type_reference(metadata.type_reference);
        let field_info = program
            .closed_data_definition(metadata_type)
            .filter(|definition| {
                definition.name.as_str() == "FieldInfo"
                    && is_reflection_declaration(program, definition.symbol)
            })
            .cloned()
            .ok_or_else(|| {
                error("typed field visitation requires its exact FieldInfo vocabulary")
            })?;
        let owner = program
            .machine_by_symbol(selection.lexical_owner)
            .cloned()
            .ok_or_else(|| error("typed field visitation lost its lexical machine"))?;
        let state_index = program
            .machine_states(&owner)
            .iter()
            .position(|state| state.symbol == selection.lexical_state)
            .ok_or_else(|| error("typed field visitation lost its lexical state"))?;
        let state = program.machine_states(&owner)[state_index].clone();
        let operands = program
            .statement_table
            .expression_handles(call.arguments)
            .to_vec();
        let (subject_operand, context_operand) = match operands.as_slice() {
            [context] if inspection => (None, *context),
            [subject, context] if !inspection => (Some(*subject), *context),
            _ => return Err(error("typed field query has invalid operands")),
        };
        let mut replacement = Vec::new();
        let captured_subject = if let Some(subject_operand) = subject_operand {
            let subject_type = reference(program, *subject, field_access);
            let subject_operand = reborrow_operand(program, subject_operand, field_access);
            Some(local(
                program,
                state.symbol,
                declaration,
                "subject",
                subject_type,
                subject_operand,
                &mut replacement,
            )?)
        } else {
            None
        };
        let context_type = reference(program, *context, ReferenceAccess::Mutable);
        let context_operand = reborrow_operand(program, context_operand, ReferenceAccess::Mutable);
        let captured_context = local(
            program,
            state.symbol,
            declaration,
            "context",
            context_type,
            context_operand,
            &mut replacement,
        )?;
        let Some(StatementNode::LocalData(context_capture)) = replacement.last_mut() else {
            return Err(error("typed field query lost its context capture"));
        };
        context_capture.reflected_query_origin = Some(site);
        let info_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: field_info.symbol,
                name: field_info.name.clone(),
            });
        let fields = members
            .iter()
            .enumerate()
            .filter_map(|(position, member)| {
                let DataMember::Field(field) = member else {
                    return None;
                };
                let description = graph
                    .fields()
                    .find(|(_, description)| description.declaration_position as usize == position)
                    .map(|(_, description)| description.clone());
                Some(
                    description.map(|description| (position, field.clone(), description, None, "")),
                )
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| error("typed field visitation lost its exact schema member"))?;
        if let Some(case_callback) = case_callback {
            active_case::emit(
                program,
                &owner,
                state.symbol,
                declaration,
                *subject,
                *context,
                &subject_data,
                &graph,
                &field_info,
                info_type,
                &owner_identity,
                &owner_application,
                field_access,
                captured_subject.ok_or_else(|| error("active visitation needs its subject"))?,
                captured_context,
                callback,
                case_callback,
                &candidate.template.machine_parameters[0].2,
                site,
                call.operational_acknowledgement,
                &mut replacement,
            )?;
        } else {
            emit_fields(
                program,
                fields,
                state.symbol,
                declaration,
                &field_info,
                info_type,
                &owner_identity,
                &owner_application,
                inspection,
                field_access,
                captured_subject,
                captured_context,
                callback,
                site,
                call.operational_acknowledgement,
                &mut replacement,
            )?;
        }
        let statements = program
            .statement_table
            .statements(state.statement_nodes)
            .to_vec();
        let mut body = HandleSpan::empty();
        for (position, statement) in statements.into_iter().enumerate() {
            let original = state.statement_nodes.handle_at(position as u32);
            if original == site {
                for generated in replacement.drain(..) {
                    program.statement_table.push_statement(&mut body, generated);
                }
            } else {
                program.statement_table.push_statement(&mut body, statement);
                let target = body.handle_at(body.count() - 1);
                expressions_of_mut(program).relocate_statement_operand_captures(original, target);
            }
        }
        program.machine_states_mut(&owner)[state_index].statement_nodes = body;
        changed_owners.push(owner.symbol);
        return Ok(true);
    }
    Ok(false)
}

/// Reference arguments lend their referents for this operation. Capturing a
/// named exclusive descriptor by value would consume the caller's binding.
/// Preserve authored access, except for ordinary mutable-to-shared attenuation.
fn reborrow_operand(
    program: &mut TypedTrees,
    operand: ExpressionHandle,
    access: ReferenceAccess,
) -> ExpressionHandle {
    if let ExpressionNode::Borrow(loan) = expressions_of(program).expression(operand) {
        return if loan.access == ReferenceAccess::Mutable && access == ReferenceAccess::Shared {
            borrow(program, loan.target, ReferenceAccess::Shared)
        } else {
            operand
        };
    }
    // Existing named descriptors lend their referents. A computed reference
    // value already owns the call/selection's declared loan; receive that
    // value once rather than trying to borrow an unnamed temporary. The
    // ordinary local initializer still checks its access and provenance.
    if validation::canonical_place_from_expression(program, operand)
        .is_some_and(|place| place.root.as_symbol().is_some())
    {
        borrow(program, operand, access)
    } else {
        operand
    }
}

fn reference(
    program: &mut TypedTrees,
    referee: TypeReferenceHandle,
    access: ReferenceAccess,
) -> TypeReferenceHandle {
    program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee,
            access,
            lifetime: None,
        })
}

fn name(program: &mut TypedTrees, symbol: SymbolHandle) -> ExpressionHandle {
    let identifier = Identifier::generated(program.symbols.name(symbol));
    let mut members = HandleSpan::empty();
    expressions_of_mut(program).push_name_path_member(&mut members, identifier);
    let mut member_symbols = HandleSpan::empty();
    expressions_of_mut(program).push_name_path_member_symbol(&mut member_symbols, symbol);
    expressions_of_mut(program).insert(ExpressionNode::Name(TableNamePath::new(
        members,
        member_symbols,
        symbol,
        symbol,
    )))
}

fn borrow(
    program: &mut TypedTrees,
    target: ExpressionHandle,
    access: ReferenceAccess,
) -> ExpressionHandle {
    expressions_of_mut(program).insert(ExpressionNode::Borrow(TableBorrowExpression {
        target,
        access,
    }))
}

fn local(
    program: &mut TypedTrees,
    state: SymbolHandle,
    origin: SymbolHandle,
    role: &str,
    type_reference: TypeReferenceHandle,
    initial_value: ExpressionHandle,
    statements: &mut Vec<StatementNode>,
) -> Result<SymbolHandle, Vec<Diagnostic>> {
    let local_name = format!(
        "__reflect_{role}#{}",
        expressions_of(program).expression_count()
    );
    let children = program
        .symbols
        .append_generated_children_from(state, [(origin, SymbolKind::Local, local_name.as_str())]);
    let symbol = SymbolTableBuilder::child_handles(children)
        .next()
        .ok_or_else(|| error("typed field visitation could not allocate its operand capture"))?;
    statements.push(StatementNode::LocalData(TableLocalData {
        symbol,
        name: Identifier::generated(local_name),
        type_reference,
        initial_value,
        ..Default::default()
    }));
    Ok(symbol)
}

fn info_field(
    program: &TypedTrees,
    definition: &typed_trees::data::DataDefinition,
    field_name: &str,
    value: ExpressionHandle,
) -> Result<TableStructLiteralField, Vec<Diagnostic>> {
    let field = program
        .data_members(definition)
        .iter()
        .find_map(|member| {
            member
                .as_field()
                .filter(|field| field.name.as_str() == field_name)
        })
        .ok_or_else(|| error("typed field visitation has an incomplete FieldInfo vocabulary"))?;
    Ok(TableStructLiteralField {
        name: field.name.clone(),
        field_symbol: field.symbol,
        value,
    })
}

fn integer(program: &mut TypedTrees, value: u64) -> Result<ExpressionHandle, Vec<Diagnostic>> {
    let literal = numerics::literals::IntegerLiteral::from_parts(
        false,
        numerics::literals::IntegerRadix::Decimal,
        &value.to_string(),
    )
    .map_err(error)?;
    Ok(expressions_of_mut(program).insert(ExpressionNode::Integer(literal)))
}

#[allow(clippy::too_many_arguments)]
fn emit_fields(
    program: &mut TypedTrees,
    fields: Vec<(
        usize,
        DataField,
        validation::reflection::FieldDescription,
        Option<Identifier>,
        &str,
    )>,
    state: SymbolHandle,
    declaration: SymbolHandle,
    field_info: &DataDefinition,
    info_type: TypeReferenceHandle,
    owner_identity: &str,
    owner_application: &str,
    inspection: bool,
    field_access: ReferenceAccess,
    captured_subject: Option<SymbolHandle>,
    captured_context: SymbolHandle,
    callback: &typed_trees::expression::StaticMachineArgument,
    site: typed_trees::statement::StatementHandle,
    operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
    replacement: &mut Vec<StatementNode>,
) -> Result<(), Vec<Diagnostic>> {
    for (position, field, description, case_variant, case_identity) in fields {
        if field.relevance.is_erased() && !inspection {
            continue;
        }
        let text_values = [
            ("member_identity", description.member_identity.as_str()),
            ("owner_identity", owner_identity),
            ("owner_application", owner_application),
            ("name", description.name.as_str()),
            ("qualified_type", description.qualified_type.as_str()),
            ("case_identity", case_identity),
        ];
        let mut info_fields = Vec::new();
        for (field_name, text) in text_values {
            let value =
                expressions_of_mut(program).insert(ExpressionNode::String(text.as_bytes().into()));
            info_fields.push(info_field(program, &field_info, field_name, value)?);
        }
        let position_value = integer(program, position as u64)?;
        info_fields.push(info_field(
            program,
            &field_info,
            "declaration_position",
            position_value,
        )?);
        let has_number =
            expressions_of_mut(program).insert(ExpressionNode::Boolean(field.identity.is_some()));
        info_fields.push(info_field(
            program,
            &field_info,
            "has_stable_number",
            has_number,
        )?);
        let erased = expressions_of_mut(program)
            .insert(ExpressionNode::Boolean(field.relevance.is_erased()));
        info_fields.push(info_field(program, &field_info, "is_erased", erased)?);
        let number = integer(program, field.identity.unwrap_or(0))?;
        info_fields.push(info_field(program, &field_info, "stable_number", number)?);
        let fields = expressions_of_mut(program).insert_struct_fields(info_fields);
        let info =
            expressions_of_mut(program).insert(ExpressionNode::StructLiteral(TableStructLiteral {
                type_name: field_info.name.clone(),
                type_symbol: field_info.symbol,
                fields,
                ..Default::default()
            }));
        let info = local(
            program,
            state,
            declaration,
            "field_info",
            info_type,
            info,
            replacement,
        )?;
        let context_value = name(program, captured_context);
        let context_value = borrow(program, context_value, ReferenceAccess::Mutable);
        let info_value = name(program, info);
        let info_value = borrow(program, info_value, ReferenceAccess::Shared);
        let mut values = vec![context_value, info_value];
        if let Some(captured_subject) = captured_subject {
            let subject_value = name(program, captured_subject);
            let field_value =
                expressions_of_mut(program).insert(ExpressionNode::Member(TableMemberExpression {
                    receiver: subject_value,
                    member_symbol: field.symbol,
                    member: field.name.clone(),
                    case_variant,
                }));
            let field_loan_type = reference(program, field.type_reference, field_access);
            let field_loan = borrow(program, field_value, field_access);
            let field_loan = local(
                program,
                state,
                declaration,
                "field_loan",
                field_loan_type,
                field_loan,
                replacement,
            )?;
            values.push(name(program, field_loan));
        }
        let arguments = program.statement_table.insert_expression_handles(values);
        let mut machine_arguments = callback
            .application
            .as_ref()
            .map_or_else(Vec::new, |application| application.arguments.to_vec());
        machine_arguments.push(
            program
                .static_operand_from_type_reference(field.type_reference)
                .map_err(error)?,
        );
        replacement.push(StatementNode::Call(TableCall {
            target_symbol: callback.symbol,
            target: callback
                .path
                .last()
                .cloned()
                .ok_or_else(|| error("typed field callback has no selected name"))?,
            machine_arguments: machine_arguments.into_boxed_slice(),
            arguments,
            operational_acknowledgement: operational_acknowledgement,
            reflected_field_origin: Some(typed_trees::statement::ReflectedFieldCallOrigin {
                query: site,
                callback: callback.clone(),
            }),
            ..Default::default()
        }));
    }
    Ok(())
}
