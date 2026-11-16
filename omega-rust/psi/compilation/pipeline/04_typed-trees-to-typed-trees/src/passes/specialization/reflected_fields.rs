//! `reflect::inspect_record_fields<Packet, describe>(&mut capture)` has an
//! empty library body: the compiler closes it, in the caller, into one
//! ordinary call of the selected callback per field of `Packet`, in declared
//! order, each given the field's exact type and its `reflect::field` query
//! (reflection.md, "Typed runtime visitation": "It checks every selected
//! application and generates ordinary calls before Terminal Psi").
//!
//! ```text
//! reflect::inspect_record_fields<Packet, describe>(&mut capture);
//!     let context: &mut Capture = &mut capture;
//!     let field: reflect::FieldInfo = reflect::field(Packet::id);
//!     describe<u32>(context, &field);
//!     ...one query and one call for each further field
//! ```
//!
//! The callback is checked against the binder the library publishes,
//! `requires machine Visitor<Kind>(context: &mut Context, field: &FieldInfo)`,
//! before any call is written. generics.md, "Requirements and conformance
//! evidence": selected implementations must refine the requirement, and a
//! concrete selection cannot silently strengthen the generic API. So a
//! callback that adds a precondition the binder does not publish is refused
//! even where every generated call would meet it. The calls themselves are
//! ordinary: the specialization pass then serves each one, and the
//! lowered-form checker judges them like any other call.
//!
//! Served today: a record with no parameters of its own declared in the
//! caller's package, a callback with exactly the binder's one type parameter,
//! and a context written or taken from a borrowed binding. Runtime and sum
//! visitation, and binder contracts that would need a refinement proof, stay
//! refused.

use super::callable_binder::{Bound, satisfies_binder};
use super::{Served, Stop, named_type, unserved};
use language_core::ReferenceAccess;
use language_core::core_vocabulary::{CoreVocabulary, ReflectionDeclaration};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::{DataField, DataMember, MachineParameterContract, TypeParameterKind};
use typed_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, NamePath, StaticMachineArgument,
    TableBorrowExpression, TableCallExpression,
};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::signature::StateSignature;
use typed_trees::state::State;
use typed_trees::statement::{StatementHandle, StatementNode, TableCall, TableLocalData};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Close every `inspect_record_fields` call statement in the machine at
/// `position`. The first call that cannot be closed is left as typing wrote
/// it, and the reason is returned for the caller to record against the
/// machine holding it, which is then refused wherever a program demands it.
pub(super) fn elaborate(
    program: &mut TypedTrees,
    position: usize,
) -> Result<(), (Stop, source::SourceSpan)> {
    let machine = program.machines()[position].clone();
    for state_index in 0..program.machine_states(&machine).len() {
        // Rewriting a state replaces its statement span, so each pass over
        // the state starts again from the new span.
        loop {
            let state = program.machine_states(&machine)[state_index].clone();
            let Some((site, call)) = state.statement_nodes.handles().find_map(|handle| {
                let StatementNode::Call(call) = program.statement_table.statement(handle) else {
                    return None;
                };
                inspects_record_fields(program, call.target_symbol).then(|| (handle, call.clone()))
            }) else {
                break;
            };
            match close(program, &machine, &state, site, &call) {
                Ok(replacement) => {
                    replace(program, &machine, state_index, &state, site, replacement);
                }
                Err(stop) => return Err((stop, call.source_span)),
            }
        }
    }
    Ok(())
}

fn inspects_record_fields(program: &TypedTrees, target: SymbolHandle) -> bool {
    declaration(program, target).is_some_and(|machine| {
        program.core_vocabulary(machine.symbol)
            == Some(CoreVocabulary::Reflection(
                ReflectionDeclaration::InspectRecordFields,
            ))
    })
}

/// The machine a call target names: the machine or its entry state.
fn declaration(program: &TypedTrees, target: SymbolHandle) -> Option<&Machine> {
    if !target.is_valid() || !program.symbols.symbols().nodes().is_valid(target) {
        return None;
    }
    program
        .machine_by_symbol(target)
        .or_else(|| program.machine_holding_state(target))
}

/// The statements that replace one `inspect_record_fields` call.
fn close(
    program: &mut TypedTrees,
    holder: &Machine,
    state: &State,
    site: StatementHandle,
    call: &TableCall,
) -> Served<Vec<StatementNode>> {
    let query = declaration(program, call.target_symbol)
        .cloned()
        .ok_or(unserved("a field inspection lost its declaration"))?;
    let parameters = program.machine_callable_parameters(&query);
    let [subject_binder, visitor_binder, context_binder] = parameters.as_slice() else {
        return Err(unserved(
            "a field inspection has an unexpected binder roster",
        ));
    };
    let TypeParameterKind::Machine {
        contract: MachineParameterContract::Structural(binder),
    } = &visitor_binder.kind
    else {
        return Err(unserved(
            "a field inspection needs its structural callback binder",
        ));
    };
    let binder = binder.clone();
    if subject_binder.kind != TypeParameterKind::Type
        || context_binder.kind != TypeParameterKind::Type
    {
        return Err(unserved(
            "a field inspection has an unexpected binder roster",
        ));
    }
    let arguments = if call.authored_machine_application.is_valid() {
        program
            .expression_table
            .static_machine_application_arguments(call.authored_machine_application)
            .to_vec()
    } else {
        call.machine_arguments.to_vec()
    };
    let (subject, visitor, written_context) = match arguments.as_slice() {
        [subject, visitor] => (subject, visitor, None),
        [subject, visitor, context] => (subject, visitor, Some(context)),
        _ => {
            return Err(unserved(
                "a field inspection needs its subject and callback",
            ));
        }
    };
    let record = record_fields(program, holder, subject)?;
    let operands = program
        .statement_table
        .expression_handles(call.arguments)
        .to_vec();
    let [operand] = operands.as_slice() else {
        return Err(unserved(
            "a field inspection takes exactly its context operand",
        ));
    };
    let context = context_type(program, state, *operand, written_context)?;
    let visitor = callback(program, visitor)?;
    if program
        .authored_service_reach_rows_for(binder.symbol)
        .next()
        .is_some()
    {
        return Err(unserved(
            "a callback binder needs a refinement proof for its full interface",
        ));
    }
    satisfies_binder(
        program,
        &binder,
        &visitor.machine,
        "reflect::inspect_record_fields",
        &[
            (subject_binder.symbol, Bound::Type(record.type_reference)),
            (context_binder.symbol, Bound::Type(context)),
        ],
    )?;
    let field_info = field_info_type(program, &binder)?;
    let field_query = field_query_entry(program, query.symbol)?;
    let lexical_owner = if call.authored_machine_application.is_valid() {
        program
            .expression_table
            .static_machine_application(call.authored_machine_application)
            .lexical_owner
    } else {
        SymbolHandle::invalid()
    };
    if !lexical_owner.is_valid() {
        return Err(unserved("a field inspection lost its lexical author"));
    }

    let span = call.source_span;
    let mut replacement = Vec::with_capacity(1 + 2 * record.fields.len());
    // The context is evaluated once; each callback call reborrows it.
    let context_reference = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: context,
            access: ReferenceAccess::Mutable,
            lifetime: None,
        });
    let context_local = local(
        program,
        state.symbol,
        query.symbol,
        "context",
        site,
        context_reference,
        *operand,
        &mut replacement,
    );
    for field in &record.fields {
        let operand = name(
            program,
            vec![record.name.clone(), field.name.clone()],
            vec![record.symbol, field.symbol],
            span,
        );
        let application = program.expression_table.retain_static_machine_application(
            field_query.entry,
            field_query.name.clone(),
            &[],
        );
        let application_row = program
            .expression_table
            .static_machine_application_mut(application);
        application_row.lexical_owner = lexical_owner;
        application_row.binding_declaration = field_query.machine;
        let arguments = program
            .expression_table
            .insert_expression_handles([operand]);
        let description =
            program
                .expression_table
                .insert(ExpressionNode::Call(TableCallExpression {
                    authored_call_selection: None,
                    authored_call_binding: None,
                    build_selection: Default::default(),
                    authored_machine_application: application,
                    exact_coercion_target: TypeReferenceHandle::invalid(),
                    requirement_application: TypeReferenceHandle::invalid(),
                    receiver: ExpressionHandle::invalid(),
                    target_symbol: field_query.entry,
                    static_machine_parameter: SymbolHandle::invalid(),
                    target: field_query.name.clone(),
                    static_requirement_dispatch: None,
                    machine_arguments: Box::default(),
                    quotient_operation: None,
                    private_layout_operation: None,
                    installed_interrupt_operation: None,
                    layout_query: None,
                    native_code_layout: None,
                    arguments,
                    evidence_arguments: Box::default(),
                    operational_acknowledgement: call.operational_acknowledgement,
                }));
        program.expression_table.set_source_span(description, span);
        let described = local(
            program,
            state.symbol,
            query.symbol,
            "field",
            site,
            field_info,
            description,
            &mut replacement,
        );
        let kind = StaticMachineArgument {
            type_reference: field.type_reference,
            ..Default::default()
        };
        let application = program.expression_table.retain_static_machine_application(
            visitor.entry,
            visitor.name.clone(),
            std::slice::from_ref(&kind),
        );
        program
            .expression_table
            .static_machine_application_mut(application)
            .lexical_owner = lexical_owner;
        let context_argument = name(
            program,
            vec![Identifier::generated(program.symbols.name(context_local))],
            vec![context_local],
            span,
        );
        let description_name = name(
            program,
            vec![Identifier::generated(program.symbols.name(described))],
            vec![described],
            span,
        );
        let borrowed =
            program
                .expression_table
                .insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target: description_name,
                    access: ReferenceAccess::Shared,
                }));
        program.expression_table.set_source_span(borrowed, span);
        let arguments = program
            .statement_table
            .insert_expression_handles([context_argument, borrowed]);
        replacement.push(StatementNode::Call(TableCall {
            authored_machine_application: application,
            target_symbol: visitor.entry,
            target: visitor.name.clone(),
            machine_arguments: Box::new([kind]),
            arguments,
            operational_acknowledgement: call.operational_acknowledgement,
            discards_result: call.discards_result,
            source_span: span,
            ..Default::default()
        }));
    }
    Ok(replacement)
}

/// The subject's fields, in declared order, erased ones included:
/// declaration inspection describes erased members too (reflection.md).
struct Record {
    symbol: SymbolHandle,
    name: Identifier,
    type_reference: TypeReferenceHandle,
    fields: Vec<DataField>,
}

fn record_fields(
    program: &mut TypedTrees,
    holder: &Machine,
    subject: &StaticMachineArgument,
) -> Served<Record> {
    let symbol = subject.symbol;
    if subject.application.is_some()
        || subject.const_literal.is_some()
        || !symbol.is_valid()
        || !program.symbols.symbols().nodes().is_valid(symbol)
        || program.symbols.get(symbol).kind != SymbolKind::Data
    {
        return Err(unserved(
            "a field inspection needs a record named by its declaration",
        ));
    }
    let data = program
        .unique_data_definition_by_symbol(symbol)
        .ok_or(unserved(
            "a field inspection needs a unique record declaration",
        ))?;
    if !program.data_type_parameters(data).is_empty()
        || !data.lifetime_parameters.is_empty()
        || data.quotient.is_some()
    {
        return Err(unserved(
            "a field inspection of an applied or quotient record is not served yet",
        ));
    }
    let mut fields = Vec::new();
    for member in program.data_members(data) {
        let DataMember::Field(field) = member else {
            return Err(unserved(
                "a sum is inspected with its active-case visitor, not inspect_record_fields",
            ));
        };
        fields.push(field.clone());
    }
    if !program
        .symbols
        .same_symbol_source_package(holder.symbol, symbol)
    {
        return Err(unserved(
            "a field inspection of a record declared in another package needs an owning operation",
        ));
    }
    let name = data.name.clone();
    let type_reference = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol,
            name: name.clone(),
        });
    Ok(Record {
        symbol,
        name,
        type_reference,
        fields,
    })
}

/// The context type: the written argument, or the type of the borrowed
/// binding the call passes. When both exist they must be one type.
fn context_type(
    program: &mut TypedTrees,
    state: &State,
    operand: ExpressionHandle,
    written: Option<&StaticMachineArgument>,
) -> Served<TypeReferenceHandle> {
    let borrowed = match program.expression_table.expression(operand) {
        ExpressionNode::Borrow(TableBorrowExpression {
            target,
            access: ReferenceAccess::Mutable,
        }) => program.declared_binding_type(state, *target),
        _ => program
            .declared_binding_type(state, operand)
            .and_then(
                |declared| match program.type_reference_table.type_reference(declared) {
                    TypeReferenceNode::Reference {
                        referee,
                        access: ReferenceAccess::Mutable,
                        ..
                    } => Some(*referee),
                    _ => None,
                },
            ),
    };
    let written = written
        .map(|argument| named_type(program, argument))
        .transpose()?;
    match (written, borrowed) {
        (Some(written), Some(borrowed))
            if program.normalized_type_identity(written)
                == program.normalized_type_identity(borrowed) =>
        {
            Ok(written)
        }
        (Some(_), Some(_)) => Err(Stop::Rejected(
            "the context passed to reflect::inspect_record_fields is not of its written Context type"
                .to_owned(),
        )),
        (Some(context), None) | (None, Some(context)) => Ok(context),
        (None, None) => Err(unserved(
            "a field inspection needs its context as a borrow of a declared binding",
        )),
    }
}

/// The selected callback: a machine named without arguments of its own.
struct Callback {
    machine: Machine,
    entry: SymbolHandle,
    name: Identifier,
}

fn callback(program: &TypedTrees, argument: &StaticMachineArgument) -> Served<Callback> {
    if argument.application.is_some()
        || argument.const_literal.is_some()
        || argument.evidence_projection.is_some()
    {
        return Err(unserved(
            "a field inspection callback must be named without its own arguments",
        ));
    }
    let machine = declaration(program, argument.symbol)
        .cloned()
        .ok_or(unserved("a field inspection callback must name a machine"))?;
    let entry = program
        .machine_states(&machine)
        .first()
        .map(|state| state.symbol)
        .ok_or(unserved("a field inspection callback has no entry"))?;
    let name = argument
        .path
        .last()
        .cloned()
        .unwrap_or_else(|| machine.name.clone());
    Ok(Callback {
        machine,
        entry,
        name,
    })
}

/// The `FieldInfo` the binder's second parameter borrows: the exact
/// vocabulary type each generated query local is declared with.
fn field_info_type(program: &TypedTrees, binder: &StateSignature) -> Served<TypeReferenceHandle> {
    let parameters = program.state_signature_parameters(binder);
    let Some([_, field]) = parameters.as_array() else {
        return Err(unserved("the callback binder lost its field parameter"));
    };
    let TypeReferenceNode::Reference { referee, .. } = program
        .type_reference_table
        .type_reference(field.type_reference)
    else {
        return Err(unserved(
            "the callback binder lost its borrowed field description",
        ));
    };
    let described = program
        .closed_data_definition(*referee)
        .is_some_and(|data| {
            program.core_vocabulary(data.symbol)
                == Some(CoreVocabulary::Reflection(ReflectionDeclaration::FieldInfo))
        });
    if !described {
        return Err(unserved(
            "the callback binder lost its FieldInfo vocabulary",
        ));
    }
    Ok(*referee)
}

/// `reflect::field`, the declaration query each generated local calls: the
/// one declared in the same file as the inspection the call names, since a program may
/// hold more than one copy of the toolchain's `reflect` module.
struct FieldQuery {
    machine: SymbolHandle,
    entry: SymbolHandle,
    name: Identifier,
}

fn field_query_entry(program: &TypedTrees, inspection: SymbolHandle) -> Served<FieldQuery> {
    let file = |symbol| {
        program
            .symbols
            .symbol_provenance_source_span(symbol)
            .map(|span| span.source_id)
    };
    let module = file(inspection).ok_or(unserved("a field inspection lost its declaring file"))?;
    let mut queries = program.machines().iter().filter(|machine| {
        file(machine.symbol) == Some(module)
            && program.core_vocabulary(machine.symbol)
                == Some(CoreVocabulary::Reflection(ReflectionDeclaration::Field))
    });
    let (Some(machine), None) = (queries.next(), queries.next()) else {
        return Err(unserved(
            "the reflect::field query is not uniquely declared",
        ));
    };
    let entry = program
        .machine_states(machine)
        .first()
        .map(|state| state.symbol)
        .ok_or(unserved("the reflect::field query has no entry"))?;
    Ok(FieldQuery {
        machine: machine.symbol,
        entry,
        name: Identifier::generated("reflect::field"),
    })
}

/// A name expression resolved to `symbols`' last entry.
fn name(
    program: &mut TypedTrees,
    members: Vec<Identifier>,
    symbols: Vec<SymbolHandle>,
    span: source::SourceSpan,
) -> ExpressionHandle {
    let head = symbols[0];
    let symbol = *symbols.last().expect("a name has a member");
    let handle = program.expression_table.insert_tree(&Expression::Name(
        NamePath::resolved_with_member_symbols(members, symbols, head, symbol),
    ));
    program.expression_table.set_source_span(handle, span);
    handle
}

/// A generated immutable local of the state, derived from the query
/// declaration, holding `value`.
#[allow(clippy::too_many_arguments)]
fn local(
    program: &mut TypedTrees,
    state: SymbolHandle,
    origin: SymbolHandle,
    role: &str,
    site: StatementHandle,
    type_reference: TypeReferenceHandle,
    initial_value: ExpressionHandle,
    statements: &mut Vec<StatementNode>,
) -> SymbolHandle {
    let name = format!(
        "__reflect_{role}#{}.{}",
        site.arena_index(),
        statements.len()
    );
    let children = program
        .symbols
        .append_generated_children_from(state, [(origin, SymbolKind::Local, name.as_str())]);
    let symbol = children.handle_at(0);
    statements.push(StatementNode::LocalData(TableLocalData {
        symbol,
        name: Identifier::generated(name),
        type_reference,
        initial_value,
        ..Default::default()
    }));
    symbol
}

/// Put `replacement` where the call at `site` was, keeping every other
/// statement and the operand captures recorded against it.
fn replace(
    program: &mut TypedTrees,
    machine: &Machine,
    state_index: usize,
    state: &State,
    site: StatementHandle,
    replacement: Vec<StatementNode>,
) {
    let mut replacement = Some(replacement);
    let mut body = arena::HandleSpan::empty();
    for original in state.statement_nodes.handles() {
        if original == site {
            for generated in replacement.take().into_iter().flatten() {
                program.statement_table.push_statement(&mut body, generated);
            }
            continue;
        }
        let statement = program.statement_table.statement(original).clone();
        let moved = program.statement_table.push_statement(&mut body, statement);
        program
            .expression_table
            .relocate_statement_operand_captures(original, moved);
    }
    program
        .machine_states
        .get_mut(machine.states.handle_at(state_index as u32))
        .statement_nodes = body;
}
