/*
These queries read retained declaration identities and already formed types.
They establish no contract, value inhabitation, or body judgment. A missing
required type is unavailable rather than reconstructed with the retired checker.
*/
use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::{
    TypedTrees,
    data::DataField,
    expression::{ExpressionHandle, TableStructLiteral},
    signature::StateParameter,
    types::{TypeReferenceHandle, TypeReferenceNode},
};

pub(crate) fn exact_declared_field(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&DataField> {
    if !symbol.is_valid() || program.symbols.get(symbol).kind != SymbolKind::Field {
        return None;
    }
    let mut fields = program
        .data_definitions()
        .iter()
        .flat_map(|data| program.data_member_fields(program.data_members(data)))
        .filter(|field| field.symbol == symbol);
    let field = fields.next()?;
    fields.next().is_none().then_some(field)
}

pub(crate) fn call_target_parameters(
    program: &TypedTrees,
    target: SymbolHandle,
) -> Result<&[StateParameter], Diagnostic> {
    if let Some(slot) = program.dynamic_slot_by_symbol(target) {
        return Ok(&slot.parameters);
    }
    if let Some(state) = program.state_by_symbol(target) {
        return Ok(program.state_parameters(state));
    }
    if let Some(machine) = program.machine_by_symbol(target) {
        return program
            .machine_states(machine)
            .first()
            .map(|state| program.state_parameters(state))
            .ok_or_else(|| crate::unavailable_judgment("call entry parameter types"));
    }
    if let Some((_, signature)) = program.machine_parameter_signature(target) {
        return Ok(program.state_signature_parameters(signature));
    }
    if let Some(operator) = program.operator_definition_by_symbol(target) {
        return Ok(program.operator_parameters(operator));
    }
    let mut signatures = program
        .traits()
        .iter()
        .flat_map(|declaration| program.trait_machine_signatures(declaration))
        .filter(|signature| signature.symbol == target);
    if let Some(signature) = signatures.next()
        && signatures.next().is_none()
    {
        return Ok(program.state_signature_parameters(signature));
    }
    Err(crate::unavailable_judgment("selected call parameter types"))
}

pub(crate) fn selected_construction_field_type(
    program: &TypedTrees,
    expression: ExpressionHandle,
    literal: &TableStructLiteral,
    symbol: SymbolHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let table = &program.expression_table;
    let application = [
        table.construction_required_type_reference(expression),
        table.constructed_value_type_reference(expression),
        literal.application_type,
    ]
    .into_iter()
    .find(|reference| reference.is_valid());
    if let Some(application) = application {
        if let Some(field) =
            program.data_constructor_field(application, literal.type_symbol, symbol)
        {
            let parent = program.symbols.get(field.symbol).parent;
            if program.symbols.get(parent).kind == SymbolKind::Variant {
                let case = literal.case_symbol.and_then(|case| {
                    program.data_constructor_case(application, literal.type_symbol, case)
                });
                if case.is_none_or(|case| case.symbol != parent) {
                    return Err(crate::unavailable_judgment(
                        "selected constructor case field type",
                    ));
                }
            }
            return Ok(field.type_reference);
        }
        return Err(crate::unavailable_judgment(
            "selected constructor field type",
        ));
    }
    let definition = program
        .unique_data_definition_by_symbol(literal.type_symbol)
        .ok_or_else(|| crate::unavailable_judgment("constructor declaration types"))?;
    if !program.data_type_parameters(definition).is_empty() || definition.generic_instance.is_some()
    {
        return Err(crate::unavailable_judgment(
            "applied constructor field type",
        ));
    }
    let field = exact_declared_field(program, symbol)
        .ok_or_else(|| crate::unavailable_judgment("constructor field type"))?;
    let parent = program.symbols.get(symbol).parent;
    let owner = if program.symbols.get(parent).kind == SymbolKind::Variant {
        if literal.case_symbol != Some(parent) {
            return Err(crate::unavailable_judgment("constructor case identity"));
        }
        program.symbols.get(parent).parent
    } else {
        parent
    };
    if owner != literal.type_symbol {
        return Err(crate::unavailable_judgment("constructor field owner"));
    }
    Ok(field.type_reference)
}

pub(crate) fn collection_element_type(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let reference = program
        .type_reference_table
        .peeled_type_reference(reference);
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => Ok(*element_type),
        _ => Err(crate::unavailable_judgment(
            "collection element type projection",
        )),
    }
}

pub(crate) fn collection_field_type(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    symbol: SymbolHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let reference = program
        .type_reference_table
        .peeled_type_reference(reference);
    let definition = program
        .closed_data_definition(reference)
        .ok_or_else(|| crate::unavailable_judgment("applied collection field type"))?;
    let field = program
        .data_constructor_field(reference, definition.symbol, symbol)
        .ok_or_else(|| crate::unavailable_judgment("collection field owner/type"))?;
    Ok(field.type_reference)
}

use typed_trees::{expression::ExpressionNode, types::PrimitiveType};
pub(crate) fn land_owned_fixed_byte_literal(
    program: &mut TypedTrees,
    value: ExpressionHandle,
    destination: TypeReferenceHandle,
) -> Result<(), Diagnostic> {
    let ExpressionNode::String(bytes) = program.expression_table.expression(value).clone() else {
        return Ok(());
    };
    let mut carrier = destination;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(carrier)
    {
        carrier = *base_type;
    }
    let TypeReferenceNode::FixedArray {
        element_type,
        length: _,
    } = program.type_reference_table.type_reference(carrier).clone()
    else {
        return Ok(());
    };

    if program.primitive_type_reference(element_type) != Some(PrimitiveType::U8) {
        return Err(Diagnostic::error(format!(
            "quoted byte literal cannot initialize owned fixed array `{}`: its element type must be `u8`",
            program.display_type_reference(destination),
        )));
    }
    let Some(width) = program.closed_fixed_array_length(carrier) else {
        return Err(Diagnostic::error(format!(
            "quoted byte literal cannot initialize owned fixed byte array `{}`: its width must be a compile-known resolved integer literal",
            program.display_type_reference(destination),
        )));
    };
    let actual = bytes.len();
    if actual != width {
        return Err(Diagnostic::error(format!(
            "quoted byte literal has {actual} source byte(s), but owned fixed byte array `{}` requires exactly {width}; fixed-array literal copying neither truncates nor pads",
            program.display_type_reference(destination),
        )));
    }

    let elements = bytes
        .iter()
        .map(|byte| {
            program.expression_table.insert(ExpressionNode::Integer(
                numerics::literals::IntegerLiteral::from_value(i64::from(*byte)),
            ))
        })
        .collect::<Vec<_>>();
    let elements = program.expression_table.insert_expression_handles(elements);
    *program.expression_table.expression_mut(value) = ExpressionNode::ArrayLiteral(elements);
    Ok(())
}

fn static_namespace(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    symbol.is_valid()
        && matches!(
            program.symbols.get(symbol).kind,
            SymbolKind::Module
                | SymbolKind::BuiltinType
                | SymbolKind::Data
                | SymbolKind::Domain
                | SymbolKind::TypeAlias
                | SymbolKind::Machine
                | SymbolKind::Trait
                | SymbolKind::Conformance
        )
}

fn declared_call_target(
    program: &TypedTrees,
    symbol: SymbolHandle,
    receiver: Option<SymbolHandle>,
) -> bool {
    // A requirement has a declaration before it has a supplied body. Retain
    // its exact signature for lowering; availability and call obligations do
    // not become facts merely because preparation can identify the target.
    if !symbol.is_valid() || !program.symbols.symbols().nodes().is_valid(symbol) {
        return false;
    }
    if program.machine_by_own_or_state_symbol(symbol).is_some()
        || program.operator_definition_by_symbol(symbol).is_some()
        || program.trait_machine_signature(symbol).is_some()
    {
        return receiver.is_none_or(|receiver| static_namespace(program, receiver));
    }
    // An evidence child belongs to its actual bound namespace. Reuse the
    // applied-requirement query; a similarly named binder or a value receiver
    // cannot supply this declaration, and ambiguity remains unavailable.
    let binder = program.symbols.get(symbol).parent;
    binder.is_valid()
        && receiver.is_none_or(|receiver| receiver == binder)
        && matches!(
            program.evidence_bound_requirement(program.symbols.get(binder).parent, binder, symbol,),
            Ok(Some(_))
        )
}

/// Read a value's retained type in the exact formal scope. This operation
/// projects declarations and applied data views; it supplies no call verdict.
pub(crate) fn scoped_expression_type_reference(
    program: &TypedTrees,
    parameters: &[StateParameter],
    result: TypeReferenceHandle,
    expression: ExpressionHandle,
) -> Result<Option<TypeReferenceHandle>, Diagnostic> {
    fn visit(
        program: &TypedTrees,
        parameters: &[StateParameter],
        result: TypeReferenceHandle,
        expression: ExpressionHandle,
        active: &mut Vec<ExpressionHandle>,
    ) -> Result<Option<TypeReferenceHandle>, Diagnostic> {
        if !program.expression_table.expression_is_valid(expression) || active.contains(&expression)
        {
            return Err(crate::unavailable_judgment(
                "stale or cyclic value type query",
            ));
        }
        if let Some(reference) = program
            .declared_callable_expression_type_reference(expression)
            .map_err(Diagnostic::error)?
        {
            return Ok(Some(reference));
        }
        active.push(expression);
        let reference = match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                if program.expression_table.names_reserved_result(expression) {
                    result.is_valid().then_some(result)
                } else if let Some(parameter) = parameters.iter().find(|parameter| {
                    parameter.symbol == path.symbol || parameter.symbol == path.head_symbol
                }) {
                    Some(parameter.type_reference)
                } else {
                    program.first_named_value_declared_type(path.symbol, path.head_symbol)
                }
            }
            ExpressionNode::Member(member) => {
                let Some(receiver) = visit(program, parameters, result, member.receiver, active)?
                else {
                    active.pop();
                    return Ok(None);
                };
                let receiver = program.type_reference_table.peeled_type_reference(receiver);
                program
                    .closed_data_definition(receiver)
                    .and_then(|definition| {
                        program
                            .data_constructor_field(
                                receiver,
                                definition.symbol,
                                member.member_symbol,
                            )
                            .map(|field| field.type_reference)
                    })
            }
            ExpressionNode::Indexed(indexed) => {
                if matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                    None
                } else {
                    match visit(program, parameters, result, indexed.collection, active)? {
                        Some(collection) => Some(collection_element_type(program, collection)?),
                        None => None,
                    }
                }
            }
            ExpressionNode::Cast(cast) => Some(cast.target_type),
            ExpressionNode::SavedSubject(saved) => match program.saved_value_source(saved) {
                Some(value) => visit(program, parameters, result, value, active)?,
                None => return Err(crate::unavailable_judgment("saved value source type")),
            },
            ExpressionNode::Atomic(atomic) => {
                visit(program, parameters, result, atomic.value, active)?
            }
            ExpressionNode::Borrow(borrow) => {
                visit(program, parameters, result, borrow.target, active)?
            }
            ExpressionNode::Match(dispatch) => {
                let arms = program.expression_table.match_arms(dispatch.arms);
                let mut joined = None;
                for arm in arms {
                    let Some(candidate) = visit(program, parameters, result, arm.value, active)?
                    else {
                        active.pop();
                        return Ok(None);
                    };
                    if joined.is_some_and(|previous| {
                        program.normalized_type_identity(previous)
                            != program.normalized_type_identity(candidate)
                    }) {
                        return Err(crate::unavailable_judgment(
                            "incompatible value type at match join",
                        ));
                    }
                    joined = Some(candidate);
                }
                joined
            }
            ExpressionNode::Call(call) => {
                if let Some(state) = program.state_by_symbol(call.target_symbol) {
                    Some(state.return_type)
                } else if let Some(machine) = program.machine_by_symbol(call.target_symbol) {
                    program
                        .machine_states(machine)
                        .first()
                        .map(|state| state.return_type)
                } else if let Some(operator) =
                    program.operator_definition_by_symbol(call.target_symbol)
                {
                    Some(operator.return_type)
                } else {
                    program
                        .trait_machine_signature(call.target_symbol)
                        .map(|(_, signature)| signature.return_type)
                }
            }
            _ => None,
        };
        active.pop();
        if let Some(reference) = reference
            && reference.is_valid()
            && !program
                .type_reference_table
                .contains_type_reference(reference)
        {
            return Err(crate::unavailable_judgment(
                "value type with stale source custody",
            ));
        }
        Ok(reference)
    }
    visit(program, parameters, result, expression, &mut Vec::new())
}

pub(crate) fn require_prebound_calls(program: &TypedTrees) -> Result<(), Diagnostic> {
    use typed_trees::expression::ExpressionNode;
    for (handle, expression) in program.expression_table.expression_entries() {
        let ExpressionNode::Call(call) = expression else {
            continue;
        };
        if let Some(binding) = call.runtime_call {
            program
                .declared_callable_contract(binding.contract)
                .map_err(Diagnostic::error)?;
            if !program.expression_table.expression_is_valid(binding.callee)
                || call.target_symbol.is_valid()
                || call.receiver.is_valid()
                || call.static_machine_parameter.is_valid()
                || call.static_requirement_dispatch.is_some()
                || !call.machine_arguments.is_empty()
            {
                return Err(crate::unavailable_judgment(
                    "conflicting runtime call target metadata",
                ));
            }
            continue;
        }
        let receiver = if !call.receiver.is_valid() {
            None
        } else if let ExpressionNode::Name(path) =
            program.expression_table.expression(call.receiver)
        {
            Some(path.symbol)
        } else {
            return Err(
                crate::unavailable_judgment("static or dynamic callable target binding")
                    .with_source_span(program.expression_table.source_span(handle)),
            );
        };
        if !declared_call_target(program, call.target_symbol, receiver) {
            return Err(
                crate::unavailable_judgment("static or dynamic callable target binding")
                    .with_source_span(program.expression_table.source_span(handle)),
            );
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some(call) = statement.as_call() else {
                    continue;
                };
                if let Some(binding) = call.runtime_call {
                    program
                        .declared_callable_contract(binding.contract)
                        .map_err(Diagnostic::error)?;
                    if !program.expression_table.expression_is_valid(binding.callee)
                        || call.target_symbol.is_valid()
                        || call.receiver_symbol.is_valid()
                        || call.static_machine_parameter.is_valid()
                        || call.static_requirement_dispatch.is_some()
                        || !call.machine_arguments.is_empty()
                    {
                        return Err(crate::unavailable_judgment(
                            "conflicting runtime statement-call metadata",
                        ));
                    }
                    continue;
                }
                let receiver = (!program
                    .statement_table
                    .name_path_members(call.receiver)
                    .is_empty())
                .then_some(call.receiver_symbol);
                if !declared_call_target(program, call.target_symbol, receiver) {
                    return Err(crate::unavailable_judgment(
                        "projected or dynamic statement-call target binding",
                    )
                    .with_source_span(call.source_span));
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn require_prebound_transitions(
    program: &TypedTrees,
    frontier: usize,
    completed: &[SymbolHandle],
) -> Result<(), Diagnostic> {
    use typed_trees::statement::TransitionTargetNode;
    for (ordinal, machine) in program.machines().iter().enumerate() {
        if ordinal < frontier && !completed.contains(&machine.symbol) {
            continue;
        }
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some(transition) = statement.as_transition() else {
                    continue;
                };
                for target in transition.targets() {
                    let TransitionTargetNode::Named {
                        path, source_span, ..
                    } = program.statement_table.transition_target(target)
                    else {
                        continue;
                    };
                    // Keep the exact named call for the lowered checker. It
                    // binds the arguments/result and emits every callee
                    // requirement at this terminal call. Unbound or value-based
                    // dispatch still needs its real selected correspondence.
                    if !declared_call_target(
                        program,
                        path.symbol,
                        path.receiver_symbol
                            .is_valid()
                            .then_some(path.receiver_symbol),
                    ) {
                        return Err(crate::unavailable_judgment(
                            "terminal callable transition target binding",
                        )
                        .with_source_span(*source_span));
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_call_and_transition_rosters_need_no_unavailable_binding() {
        let program = TypedTrees::default();
        require_prebound_calls(&program).unwrap();
        require_prebound_transitions(&program, 0, &[]).unwrap();
    }

    #[test]
    fn an_unresolved_call_refuses_instead_of_claiming_a_bound_target() {
        use typed_trees::expression::TableCallExpression;
        let mut program = TypedTrees::default();
        program
            .expression_table
            .insert(ExpressionNode::Call(TableCallExpression {
                authored_call_selection: None,
                authored_call_binding: None,
                build_selection: Default::default(),
                authored_machine_application: Default::default(),
                exact_coercion_target: Default::default(),
                requirement_application: Default::default(),
                receiver: Default::default(),
                target_symbol: SymbolHandle::invalid(),
                static_machine_parameter: SymbolHandle::invalid(),
                target: typed_trees::name::Identifier::generated("unresolved"),
                static_requirement_dispatch: None,
                machine_arguments: Box::new([]),
                quotient_operation: None,
                private_layout_operation: None,
                installed_interrupt_operation: None,
                layout_query: None,
                native_code_layout: None,
                arguments: Default::default(),
                evidence_arguments: Box::new([]),
                operational_acknowledgement: Default::default(),
            }));
        let error = require_prebound_calls(&program).unwrap_err();
        assert!(error.message.starts_with("no product route yet:"));
        assert!(error.message.contains("callable target binding"));
        assert_eq!(program.expression_table.expression_count(), 1);
    }

    #[test]
    fn a_retained_requirement_needs_its_exact_signature_and_a_namespace_receiver() {
        use typed_trees::expression::{TableCallExpression, TableNamePath};
        use typed_trees::signature::StateSignature;
        use typed_trees::trait_definition::TraitDefinition;

        let mut symbols = symbols::SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, symbols::SymbolNameRef::Static("root"));
        let owners = symbols.insert_children(
            root,
            [
                (SymbolKind::Trait, symbols::SymbolNameRef::Static("Api")),
                (
                    SymbolKind::Machine,
                    symbols::SymbolNameRef::Static("caller"),
                ),
            ],
        );
        let owner = owners.handle_at(0);
        let caller = owners.handle_at(1);
        let members = symbols.insert_children(
            owner,
            [
                (SymbolKind::State, symbols::SymbolNameRef::Static("call")),
                (SymbolKind::State, symbols::SymbolNameRef::Static("missing")),
            ],
        );
        let requirement = members.handle_at(0);
        let missing = members.handle_at(1);
        let receiver_value = symbols
            .insert_children(
                caller,
                [(SymbolKind::Local, symbols::SymbolNameRef::Static("value"))],
            )
            .handle_at(0);
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let signatures = program
            .trait_machine_signatures
            .insert_many([StateSignature {
                symbol: requirement,
                ..Default::default()
            }]);
        program.push_trait_definition(TraitDefinition {
            symbol: owner,
            is_boundary: true,
            machines: signatures,
            ..Default::default()
        });
        let receiver = program
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath::new(
                Default::default(),
                Default::default(),
                owner,
                owner,
            )));
        let expression =
            program
                .expression_table
                .insert(ExpressionNode::Call(TableCallExpression {
                    authored_call_selection: None,
                    authored_call_binding: None,
                    build_selection: Default::default(),
                    authored_machine_application: Default::default(),
                    exact_coercion_target: Default::default(),
                    requirement_application: Default::default(),
                    receiver,
                    target_symbol: requirement,
                    static_machine_parameter: Default::default(),
                    target: typed_trees::name::Identifier::generated("call"),
                    static_requirement_dispatch: None,
                    machine_arguments: Box::new([]),
                    quotient_operation: None,
                    private_layout_operation: None,
                    installed_interrupt_operation: None,
                    layout_query: None,
                    native_code_layout: None,
                    arguments: Default::default(),
                    evidence_arguments: Box::new([]),
                    operational_acknowledgement: Default::default(),
                }));
        let before = program.clone();
        require_prebound_calls(&program).expect("the retained requirement is a declared target");
        assert!(program == before);
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
            panic!("call expression")
        };
        call.target_symbol = missing;
        assert!(require_prebound_calls(&program).is_err());
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
            panic!("call expression")
        };
        call.target_symbol = requirement;
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(receiver) else {
            panic!("receiver name")
        };
        path.symbol = receiver_value;
        path.head_symbol = receiver_value;
        assert!(require_prebound_calls(&program).is_err());
    }
    #[test]
    fn bound_terminal_calls_keep_their_target_and_unbound_receivers_refuse() {
        use typed_trees::statement::{
            StatementNode, TableNamePath, TableTransition, TransitionTargetNode,
        };
        use typed_trees::{
            machine::{Machine, MachineBodyStatus},
            state::State,
        };
        let mut symbols = symbols::SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, symbols::SymbolNameRef::Static("root"));
        let machines = symbols.insert_children(
            root,
            [
                (
                    SymbolKind::Machine,
                    symbols::SymbolNameRef::Static("caller"),
                ),
                (
                    SymbolKind::Machine,
                    symbols::SymbolNameRef::Static("callee"),
                ),
            ],
        );
        let caller = machines.handle_at(0);
        let callee = machines.handle_at(1);
        let caller_children = symbols.insert_children(
            caller,
            [
                (SymbolKind::State, symbols::SymbolNameRef::Static("entry")),
                (SymbolKind::Local, symbols::SymbolNameRef::Static("context")),
            ],
        );
        let state = caller_children.handle_at(0);
        let receiver = caller_children.handle_at(1);
        let target_state = symbols
            .insert_children(
                callee,
                [(SymbolKind::State, symbols::SymbolNameRef::Static("entry"))],
            )
            .handle_at(0);
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let mut members = Default::default();
        program.statement_table.push_name_path_member(
            &mut members,
            typed_trees::name::Identifier::generated("callee"),
        );
        let target =
            program
                .statement_table
                .insert_transition_target(TransitionTargetNode::Named {
                    path: TableNamePath {
                        symbol: callee,
                        head_symbol: callee,
                        members,
                        ..Default::default()
                    },
                    arguments: Default::default(),
                    evidence_arguments: Box::new([]),
                    source_span: Default::default(),
                    authored_call_selection: None,
                    operational_acknowledgement: Default::default(),
                    static_machine_parameter: Default::default(),
                });
        let statements = program
            .statement_table
            .insert_statements([StatementNode::Transition(TableTransition {
                target,
                ..Default::default()
            })]);
        let states = program.machine_states.insert_many([State {
            symbol: state,
            statement_nodes: statements,
            ..Default::default()
        }]);
        let target_states = program.machine_states.insert_many([State {
            symbol: target_state,
            ..Default::default()
        }]);
        for (symbol, states) in [(caller, states), (callee, target_states)] {
            program.push_machine(Machine {
                symbol,
                states,
                body_is_present: true,
                body_status: MachineBodyStatus::Complete,
                ..Default::default()
            });
        }
        let before = program.clone();
        require_prebound_transitions(&program, 0, &[])
            .expect("an exact declared terminal call is retained for judgment");
        assert!(program == before);
        let mut unbound = program.clone();
        let TransitionTargetNode::Named { path, .. } =
            unbound.statement_table.transition_target_mut(target)
        else {
            panic!("named target")
        };
        path.symbol = SymbolHandle::invalid();
        assert!(require_prebound_transitions(&unbound, 0, &[]).is_err());
        let TransitionTargetNode::Named { path, .. } =
            program.statement_table.transition_target_mut(target)
        else {
            panic!("named target")
        };
        path.receiver_symbol = receiver;
        assert!(require_prebound_transitions(&program, 0, &[]).is_err());
    }
}
