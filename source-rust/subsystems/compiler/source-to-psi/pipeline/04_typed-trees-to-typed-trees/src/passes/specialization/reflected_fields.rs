//! Expand typed visitors into ordinary calls. Each call gets the exact field
//! type and its description. Runtime visitors borrow each field for its call;
//! declaration inspection also describes erased fields without borrowing them.
//! The callback must satisfy the library's published interface.

use super::callable_binder::{Bound, satisfies_binder};
use super::{Served, Stop, named_type, unserved};
use crate::source_state::TypedTrees;
use crate::source_state::data::{
    DataField, DataMember, MachineParameterContract, TypeParameterKind,
};
use crate::source_state::expression::{
    Expression, ExpressionHandle, ExpressionNode, MemberSelection, NamePath, StaticMachineArgument,
    TableBorrowExpression, TableCallExpression, TableMemberExpression,
};
use crate::source_state::machine::Machine;
use crate::source_state::name::Identifier;
use crate::source_state::signature::StateSignature;
use crate::source_state::state::State;
use crate::source_state::statement::{StatementHandle, StatementNode, TableCall, TableLocalData};
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use language_core::ReferenceAccess;
use language_core::core_vocabulary::{CoreVocabulary, ReflectionDeclaration};
use symbols::{SymbolHandle, SymbolKind};

mod active_cases;

/// Expand reflection visitors into checked calls. A refusal belongs to the
/// machine that names the visitor, so an unused generic body can remain open.
pub(super) fn elaborate(
    program: &mut TypedTrees,
    position: usize,
) -> Result<(), (Stop, source_text::SourceSpan)> {
    let machine = program.machines()[position].clone();
    for state_index in 0..program.machine_states(&machine).len() {
        // Rewriting a state replaces its statement span, so each pass over
        // the state starts again from the new span.
        loop {
            let state = program.machine_states(&machine)[state_index].clone();
            let Some((site, call, operation)) =
                state.statement_nodes.handles().find_map(|handle| {
                    let StatementNode::Call(call) = program.statement_table.statement(handle)
                    else {
                        return None;
                    };
                    let operation = reflection_operation(program, call.target_symbol)?;
                    Some((handle, call.clone(), operation))
                })
            else {
                break;
            };
            let replacement = match operation {
                ReflectionOperation::Record(operation) => {
                    close(program, &machine, &state, site, &call, operation)
                }
                ReflectionOperation::ActiveCases(access) => {
                    active_cases::expand(program, &machine, &state, site, &call, access)
                }
            };
            match replacement {
                Ok(replacement) => {
                    replace(program, &machine, state_index, &state, site, replacement);
                }
                Err(stop) => return Err((stop, call.source_span)),
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum RecordQuery {
    Inspect,
    Visit(ReferenceAccess),
}

#[derive(Clone, Copy)]
enum ReflectionOperation {
    Record(RecordQuery),
    ActiveCases(ReferenceAccess),
}

fn reflection_operation(program: &TypedTrees, target: SymbolHandle) -> Option<ReflectionOperation> {
    use ReflectionOperation::{ActiveCases, Record};
    let machine = declaration(program, target)?;
    match program.core_vocabulary(machine.symbol)? {
        CoreVocabulary::Reflection(ReflectionDeclaration::InspectRecordFields) => {
            Some(Record(RecordQuery::Inspect))
        }
        CoreVocabulary::Reflection(ReflectionDeclaration::VisitRuntimeFields) => {
            Some(Record(RecordQuery::Visit(ReferenceAccess::Shared)))
        }
        CoreVocabulary::Reflection(ReflectionDeclaration::VisitRuntimeFieldsMut) => {
            Some(Record(RecordQuery::Visit(ReferenceAccess::Mutable)))
        }
        CoreVocabulary::Reflection(ReflectionDeclaration::VisitActiveFields) => {
            Some(ActiveCases(ReferenceAccess::Shared))
        }
        CoreVocabulary::Reflection(ReflectionDeclaration::VisitActiveFieldsMut) => {
            Some(ActiveCases(ReferenceAccess::Mutable))
        }
        _ => None,
    }
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
    operation: RecordQuery,
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
    let (subject_operand, operand) = match (operation, operands.as_slice()) {
        (RecordQuery::Inspect, [context]) => (None, *context),
        (RecordQuery::Visit(access), [subject, context]) => (Some((*subject, access)), *context),
        _ => {
            return Err(unserved(
                "a field traversal has an unexpected operand roster",
            ));
        }
    };
    let context = context_type(program, state, operand, written_context)?;
    let visitor = callback(program, visitor)?;
    let included = |field: &&DataField| {
        matches!(operation, RecordQuery::Inspect) || !field.relevance.is_erased()
    };
    let applications = record
        .fields
        .iter()
        .filter(included)
        .map(|field| field.type_reference)
        .collect::<Vec<_>>();
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
        &format!("reflect::{}", program.symbols.name(query.symbol)),
        &[
            (subject_binder.symbol, Bound::Type(record.type_reference)),
            (context_binder.symbol, Bound::Type(context)),
        ],
        Some(&applications),
    )?;
    let field_info = description_type(program, &binder, ReflectionDeclaration::FieldInfo)?;
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
    // Capture operands once, in their authored left-to-right order.
    let subject_local = subject_operand.map(|(operand, access)| {
        let reference = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: record.type_reference,
                access,
                lifetime: None,
            });
        (
            local(
                program,
                state.symbol,
                query.symbol,
                "subject",
                site,
                reference,
                operand,
                &mut replacement,
            ),
            access,
        )
    });
    // Each callback call reborrows the captured context.
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
        operand,
        &mut replacement,
    );
    let emission = CallbackSite {
        state: state.symbol,
        site,
        origin: query.symbol,
        lexical_owner,
        call,
    };
    for field in record.fields.iter().filter(included) {
        let operand = name(
            program,
            vec![record.name.clone(), field.name.clone()],
            vec![record.symbol, field.symbol],
            span,
        );
        let value = subject_local
            .map(|(subject, access)| field_borrow(program, subject, field, None, access, span));
        emission.field(
            program,
            &field_query,
            field_info,
            &visitor,
            context_local,
            field,
            operand,
            value,
            &mut replacement,
        );
    }
    Ok(replacement)
}

struct CallbackSite<'call> {
    state: SymbolHandle,
    site: StatementHandle,
    origin: SymbolHandle,
    lexical_owner: SymbolHandle,
    call: &'call TableCall,
}

impl CallbackSite<'_> {
    #[allow(clippy::too_many_arguments)]
    fn field(
        &self,
        program: &mut TypedTrees,
        query: &FieldQuery,
        info: TypeReferenceHandle,
        visitor: &Callback,
        context: SymbolHandle,
        field: &DataField,
        declaration: ExpressionHandle,
        value: Option<ExpressionHandle>,
        statements: &mut Vec<StatementNode>,
    ) {
        let application = program.expression_table.retain_static_machine_application(
            query.entry,
            query.name.clone(),
            &[],
        );
        let row = program
            .expression_table
            .static_machine_application_mut(application);
        row.lexical_owner = self.lexical_owner;
        row.binding_declaration = query.machine;
        let arguments = program
            .expression_table
            .insert_expression_handles([declaration]);
        let description = expression(
            program,
            ExpressionNode::Call(TableCallExpression {
                authored_machine_application: application,
                target_symbol: query.entry,
                target: query.name.clone(),
                arguments,
                operational_acknowledgement: self.call.operational_acknowledgement,
                ..Default::default()
            }),
            self.call.source_span,
        );
        let kind = StaticMachineArgument {
            type_reference: field.type_reference,
            ..Default::default()
        };
        self.invoke(
            program,
            visitor,
            context,
            info,
            description,
            value,
            &[kind],
            statements,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn invoke(
        &self,
        program: &mut TypedTrees,
        visitor: &Callback,
        context: SymbolHandle,
        info: TypeReferenceHandle,
        description: ExpressionHandle,
        value: Option<ExpressionHandle>,
        machine_arguments: &[StaticMachineArgument],
        statements: &mut Vec<StatementNode>,
    ) {
        let span = self.call.source_span;
        let described = local(
            program,
            self.state,
            self.origin,
            "description",
            self.site,
            info,
            description,
            statements,
        );
        let application = program.expression_table.retain_static_machine_application(
            visitor.entry,
            visitor.name.clone(),
            machine_arguments,
        );
        program
            .expression_table
            .static_machine_application_mut(application)
            .lexical_owner = self.lexical_owner;
        let context = binding_name(program, context, span);
        let described = binding_name(program, described, span);
        let description = expression(
            program,
            ExpressionNode::Borrow(TableBorrowExpression {
                target: described,
                access: ReferenceAccess::Shared,
            }),
            span,
        );
        let arguments = program
            .statement_table
            .insert_expression_handles([context, description].into_iter().chain(value));
        statements.push(StatementNode::Call(TableCall {
            authored_machine_application: application,
            target_symbol: visitor.entry,
            target: visitor.name.clone(),
            machine_arguments: machine_arguments.into(),
            arguments,
            operational_acknowledgement: self.call.operational_acknowledgement,
            discards_result: self.call.discards_result,
            source_span: span,
            ..Default::default()
        }));
    }
}

fn expression(
    program: &mut TypedTrees,
    node: ExpressionNode,
    span: source_text::SourceSpan,
) -> ExpressionHandle {
    let handle = program.expression_table.insert(node);
    program.expression_table.set_source_span(handle, span);
    handle
}

fn binding_name(
    program: &mut TypedTrees,
    symbol: SymbolHandle,
    span: source_text::SourceSpan,
) -> ExpressionHandle {
    let spelling = Identifier::generated(program.symbols.name(symbol));
    name(program, vec![spelling], vec![symbol], span)
}

fn field_borrow(
    program: &mut TypedTrees,
    subject: SymbolHandle,
    field: &DataField,
    case_variant: Option<Identifier>,
    access: ReferenceAccess,
    span: source_text::SourceSpan,
) -> ExpressionHandle {
    let receiver = binding_name(program, subject, span);
    let target = expression(
        program,
        ExpressionNode::Member(TableMemberExpression {
            receiver,
            selection: MemberSelection::Field(field.symbol),
            member: field.name.clone(),
            case_variant,
        }),
        span,
    );
    expression(
        program,
        ExpressionNode::Borrow(TableBorrowExpression { target, access }),
        span,
    )
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
    let type_reference = named_type(program, subject, holder.symbol)?;
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
    // The context can still contain open runtime types. Retain the exact
    // type that typing supplied, rather than requiring a closed static
    // application, and compare it with the borrowed storage below.
    let written = written
        .map(|argument| {
            if argument.type_reference.is_valid() {
                program
                    .type_reference_table
                    .contains_type_reference(argument.type_reference)
                    .then_some(argument.type_reference)
                    .ok_or(unserved("a field context lost its retained runtime type"))
            } else {
                named_type(program, argument, state.symbol)
            }
        })
        .transpose()?;
    let identity = |reference| {
        crate::source_state::type_identity::descriptive_type_identity_in_region_scope(
            program,
            crate::source_state::type_identity::TypeIdentityRequest::ordinary(reference),
            state.symbol,
            state.symbol,
            &[],
            &[],
        )
    };
    match (written, borrowed) {
        (Some(written), Some(borrowed))
            if identity(written).is_some_and(|written| Some(written) == identity(borrowed)) =>
        {
            Ok(written)
        }
        (Some(_), Some(_)) => Err(Stop::Rejected(
            "the visitor context has a different type from its written Context argument".to_owned(),
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

/// The exact toolchain description type borrowed by a callback.
fn description_type(
    program: &TypedTrees,
    binder: &StateSignature,
    vocabulary: ReflectionDeclaration,
) -> Served<TypeReferenceHandle> {
    let parameters = program.state_signature_parameters(binder);
    let Some(field) = parameters.get(1) else {
        return Err(unserved(
            "the callback binder lost its description parameter",
        ));
    };
    let TypeReferenceNode::Reference { referee, .. } = program
        .type_reference_table
        .type_reference(field.type_reference)
    else {
        return Err(unserved(
            "the callback binder lost its borrowed description",
        ));
    };
    let described = program
        .closed_data_definition(*referee)
        .is_some_and(|data| {
            program.core_vocabulary(data.symbol) == Some(CoreVocabulary::Reflection(vocabulary))
        });
    if !described {
        return Err(unserved(
            "the callback binder lost its reflection description type",
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
    span: source_text::SourceSpan,
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
            let arguments = match program.statement_table.statement(original) {
                StatementNode::Call(call) => call.arguments,
                _ => unreachable!("a reflection visitor was selected as a call"),
            };
            for generated in replacement.take().into_iter().flatten() {
                let shares_operands = matches!(&generated, StatementNode::Call(call)
                    if call.arguments == arguments);
                let moved = program.statement_table.push_statement(&mut body, generated);
                if shares_operands {
                    program
                        .expression_table
                        .relocate_statement_operand_captures(original, moved);
                }
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
