//! One hand-built program, as typing leaves it:
//!
//! ```omega
//! trait Step { machine apply(); }
//! Hold satisfies Step { machine apply() { } }
//! Held satisfies Step { Step::apply = Hold::apply; }   // the value `Hold` constructs
//! Empty satisfies Step { }                              // gives `apply` no machine
//! machine advance<Selected satisfies Step>() { Selected::apply() }
//! machine main() {
//!     let a = advance<Hold>();
//!     let b = advance<Held>();
//! }
//! machine partial() { let c = advance<Empty>(); }
//! ```
//!
//! `main` applies `advance` at one implementation under two names, which is
//! served once. `partial` applies it at an implementation with no machine
//! for `apply`, which is not served: it is the failing twin.

use super::prepare;
use arena::HandleSpan;
use language_semantics::declaration_selection::ResolvedAuthoredCallBinding;
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::TypedTrees;
use typed_trees::deferred_refusals::DeferredRequest;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, StaticMachineArgument, TableCallExpression, TableNamePath,
};
use typed_trees::machine::{GenericConformanceBound, Machine};
use typed_trees::name::Identifier;
use typed_trees::signature::StateSignature;
use typed_trees::state::State;
use typed_trees::statement::{
    StatementNode, TableLocalData, TableTransition, TransitionTargetNode,
};
use typed_trees::static_applications::StaticMachineBindingValue;
use typed_trees::trait_definition::{
    Conformance, ConformanceRow, ConformanceRowSource, ImplementationValueIdentity, TraitDefinition,
};
use typed_trees::types::TypeReferenceHandle;

struct Source {
    program: TypedTrees,
    step: SymbolHandle,
    requirement: SymbolHandle,
    hold: SymbolHandle,
    hold_apply: SymbolHandle,
    hold_apply_state: SymbolHandle,
    advance: SymbolHandle,
    selected: SymbolHandle,
    placeholder: SymbolHandle,
    main: SymbolHandle,
    partial: SymbolHandle,
    nested: Option<(SymbolHandle, SymbolHandle)>,
}

/// `target<static>()` or `target()`, as an expression held by `holder`.
fn call(
    program: &mut TypedTrees,
    holder: SymbolHandle,
    target: SymbolHandle,
    spelled: &'static str,
    static_argument: Option<StaticMachineArgument>,
) -> TableCallExpression {
    let static_arguments = static_argument.into_iter().collect::<Vec<_>>();
    let application = program.expression_table.retain_static_machine_application(
        target,
        Identifier::generated(spelled),
        &static_arguments,
    );
    program
        .expression_table
        .static_machine_application_mut(application)
        .lexical_owner = holder;
    TableCallExpression {
        runtime_call: None,
        authored_call_selection: None,
        authored_call_binding: None,
        build_selection: arena::Handle::invalid(),
        authored_machine_application: application,
        exact_coercion_target: TypeReferenceHandle::invalid(),
        requirement_application: TypeReferenceHandle::invalid(),
        receiver: ExpressionHandle::invalid(),
        target_symbol: target,
        static_machine_parameter: SymbolHandle::invalid(),
        target: Identifier::generated(spelled),
        static_requirement_dispatch: None,
        machine_arguments: static_arguments.into_boxed_slice(),
        quotient_operation: None,
        private_layout_operation: None,
        installed_interrupt_operation: None,
        layout_query: None,
        native_code_layout: None,
        arguments: HandleSpan::empty(),
        evidence_arguments: Box::new([]),
        operational_acknowledgement: Default::default(),
    }
}

fn push_machine(
    program: &mut TypedTrees,
    symbol: SymbolHandle,
    spelled: &'static str,
    conformance_bounds: Vec<GenericConformanceBound>,
    state: SymbolHandle,
    statements: Vec<StatementNode>,
) {
    let statement_nodes = program.statement_table.insert_statements(statements);
    let states = program.machine_states.insert_many([State {
        symbol: state,
        name: Identifier::generated("entry"),
        statement_nodes,
        ..Default::default()
    }]);
    program.push_machine(Machine {
        symbol,
        name: Identifier::generated(spelled),
        conformance_bounds,
        states,
        body_is_present: true,
        ..Default::default()
    });
}

fn source() -> Source {
    source_with_typed_record(false)
}

fn source_with_typed_record(typed_record: bool) -> Source {
    source_with_record_children(typed_record, false)
}

/// The same authored Const record with two nested declarations, each retaining
/// the enclosing binder. Held selects a different operation instead of an alias.
fn source_with_record_children(typed_record: bool, with_children: bool) -> Source {
    let mut sources = source::SourceMap::default();
    let file = sources.add_with_metadata(
        std::path::PathBuf::from("implementation-values.omg"),
        "root".into(),
        std::path::PathBuf::from("implementation-values"),
        semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]),
        source::SourceOrigin::User,
    );
    let root_name = SymbolNameRef::Source(source::SourceSpan::new(
        file.source_id,
        source::Span::new(0, 4),
    ));
    let mut builder = SymbolTableBuilder::with_sources(Some(std::sync::Arc::new(sources)));
    let root = builder.insert_root(SymbolKind::Root, root_name);
    let top = builder.insert_children(
        root,
        [
            (SymbolKind::Trait, SymbolNameRef::Static("Step")),
            (SymbolKind::Conformance, SymbolNameRef::Static("Hold")),
            (SymbolKind::Conformance, SymbolNameRef::Static("Held")),
            (SymbolKind::Conformance, SymbolNameRef::Static("Empty")),
            (SymbolKind::Machine, SymbolNameRef::Static("Hold::apply")),
            (SymbolKind::Machine, SymbolNameRef::Static("advance")),
            (SymbolKind::Machine, SymbolNameRef::Static("main")),
            (SymbolKind::Machine, SymbolNameRef::Static("partial")),
        ],
    );
    let [step, hold, held, empty, hold_apply, advance, main, partial] =
        std::array::from_fn(|position| top.handle_at(position as u32));
    let child = |builder: &mut SymbolTableBuilder, parent, kind, spelled| {
        builder
            .insert_children(parent, [(kind, SymbolNameRef::Static(spelled))])
            .handle_at(0)
    };
    let requirement = child(&mut builder, step, SymbolKind::State, "apply");
    let hold_apply_state = child(&mut builder, hold_apply, SymbolKind::State, "apply");
    let advance_children = builder.insert_children(
        advance,
        [
            (
                if typed_record {
                    SymbolKind::TypeParameter
                } else {
                    SymbolKind::ConformanceParameter
                },
                SymbolNameRef::Static("Selected"),
            ),
            (SymbolKind::State, SymbolNameRef::Static("entry")),
        ],
    );
    let (selected, advance_state) = (advance_children.handle_at(0), advance_children.handle_at(1));
    let placeholder = child(&mut builder, selected, SymbolKind::State, "apply");
    let nested = with_children.then(|| {
        let machine = child(&mut builder, advance_state, SymbolKind::Machine, "child");
        let state = child(&mut builder, machine, SymbolKind::State, "entry");
        let grandchild = child(&mut builder, state, SymbolKind::Machine, "grandchild");
        let grandchild_state = child(&mut builder, grandchild, SymbolKind::State, "entry");
        (machine, state, grandchild, grandchild_state)
    });
    let held_operation = with_children.then(|| {
        let machine = child(&mut builder, held, SymbolKind::Machine, "apply");
        let state = child(&mut builder, machine, SymbolKind::State, "apply");
        (machine, state)
    });
    let main_state = child(&mut builder, main, SymbolKind::State, "entry");
    let partial_state = child(&mut builder, partial, SymbolKind::State, "entry");
    let locals = builder.insert_children(
        main_state,
        [
            (SymbolKind::Local, SymbolNameRef::Static("a")),
            (SymbolKind::Local, SymbolNameRef::Static("b")),
        ],
    );
    let partial_local = child(&mut builder, partial_state, SymbolKind::Local, "c");
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..Default::default()
    };
    // Source resolution records the owner of every inline member.
    program
        .symbols
        .record_declaration_owner(hold_apply, hold)
        .unwrap();
    if let Some((machine, _)) = held_operation {
        program
            .symbols
            .record_declaration_owner(machine, held)
            .unwrap();
    }

    // trait Step { machine apply(); }
    let machines = program
        .trait_machine_signatures
        .insert_many([StateSignature {
            symbol: requirement,
            name: Identifier::generated("apply"),
            ..Default::default()
        }]);
    program.push_trait_definition(TraitDefinition {
        symbol: step,
        name: Identifier::generated("Step"),
        machines,
        ..Default::default()
    });

    // Ordinarily Held aliases Hold. The nested fixture instead selects its
    // own authored operation, making two distinct resulting record values.
    let row = ConformanceRow {
        declaring_trait: step,
        declaring_trait_name: Identifier::generated("Step"),
        requirement,
        requirement_name: Identifier::generated("apply"),
        realization_machine: hold_apply,
        realization_state: hold_apply_state,
        realization_name: Identifier::generated("Hold::apply"),
        source: ConformanceRowSource::Inline,
    };
    for (symbol, rows, representative) in [
        (hold, vec![row.clone()], SymbolHandle::invalid()),
        (
            held,
            vec![if let Some((machine, state)) = held_operation {
                ConformanceRow {
                    realization_machine: machine,
                    realization_state: state,
                    realization_name: Identifier::generated("Held::apply"),
                    ..row
                }
            } else {
                ConformanceRow {
                    source: ConformanceRowSource::Reference,
                    ..row
                }
            }],
            if with_children {
                SymbolHandle::invalid()
            } else {
                hold
            },
        ),
        (empty, Vec::new(), SymbolHandle::invalid()),
    ] {
        program.push_conformance(Conformance {
            symbol,
            trait_name: Identifier::generated("Step"),
            trait_symbol: step,
            rows,
            value_identity: ImplementationValueIdentity {
                representative,
                ..Default::default()
            },
            ..Default::default()
        });
    }

    push_machine(
        &mut program,
        hold_apply,
        "Hold::apply",
        Vec::new(),
        hold_apply_state,
        Vec::new(),
    );

    if let Some((machine, state)) = held_operation {
        push_machine(
            &mut program,
            machine,
            "Held::apply",
            Vec::new(),
            state,
            Vec::new(),
        );
    }

    // advance<Selected satisfies Step>() { Selected::apply() }
    let mut members = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member(&mut members, Identifier::generated("Selected"));
    let qualifier = program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath {
            members,
            symbol: selected,
            head_symbol: selected,
            ..TableNamePath::default()
        }));
    let written_requirement = if typed_record {
        requirement
    } else {
        placeholder
    };
    let through = TableCallExpression {
        receiver: qualifier,
        authored_call_binding: Some(ResolvedAuthoredCallBinding {
            selected: written_requirement,
            original: written_requirement,
        }),
        ..call(
            &mut program,
            nested.map_or(advance, |(_, _, grandchild, _)| grandchild),
            written_requirement,
            "apply",
            None,
        )
    };
    let through = program
        .expression_table
        .insert(ExpressionNode::Call(through));
    let target = program
        .statement_table
        .insert_transition_target(TransitionTargetNode::Value(through));
    let outer_target = if let Some((_, state, _, _)) = nested {
        let call = call(
            &mut program,
            advance,
            state,
            "child",
            Some(StaticMachineArgument {
                symbol: selected,
                path: Box::new([Identifier::generated("Selected")]),
                ..Default::default()
            }),
        );
        let value = program.expression_table.insert(ExpressionNode::Call(call));
        program
            .statement_table
            .insert_transition_target(TransitionTargetNode::Value(value))
    } else {
        target
    };
    push_machine(
        &mut program,
        advance,
        "advance",
        if typed_record {
            Vec::new()
        } else {
            vec![GenericConformanceBound {
                binder: selected,
                binder_name: Identifier::generated("Selected"),
                trait_symbol: step,
                trait_name: Identifier::generated("Step"),
                lifetime_arguments: Vec::new(),
                arguments: Vec::new(),
            }]
        },
        advance_state,
        vec![StatementNode::Transition(TableTransition {
            target: outer_target,
            ..TableTransition::default()
        })],
    );

    if typed_record {
        let schema =
            program
                .type_reference_table
                .insert(typed_trees::types::TypeReferenceNode::Named {
                    symbol: step,
                    name: Identifier::generated("Step"),
                });
        let parameters =
            program
                .data_type_parameters
                .insert_many([typed_trees::data::TypeParameter {
                    symbol: selected,
                    name: Identifier::generated("Selected"),
                    kind: typed_trees::data::TypeParameterKind::Const {
                        type_reference: schema,
                    },
                    ..Default::default()
                }]);
        let position = program.machine_position_by_symbol(advance).unwrap();
        program.machine_mut(position).type_parameters = parameters;
        if let Some((child, child_state, grandchild, grandchild_state)) = nested {
            let parameter = program
                .machine_type_parameters(program.machine_by_symbol(advance).unwrap())[0]
                .clone();
            let call = call(
                &mut program,
                child,
                grandchild_state,
                "grandchild",
                Some(StaticMachineArgument {
                    symbol: selected,
                    path: Box::new([Identifier::generated("Selected")]),
                    ..Default::default()
                }),
            );
            let value = program.expression_table.insert(ExpressionNode::Call(call));
            let child_target = program
                .statement_table
                .insert_transition_target(TransitionTargetNode::Value(value));
            for (machine, state, name, target) in [
                (child, child_state, "child", child_target),
                (grandchild, grandchild_state, "grandchild", target),
            ] {
                push_machine(
                    &mut program,
                    machine,
                    name,
                    Vec::new(),
                    state,
                    vec![StatementNode::Transition(TableTransition {
                        target,
                        ..Default::default()
                    })],
                );
                let parameters = program
                    .data_type_parameters
                    .insert_many([parameter.clone()]);
                let position = program.machine_position_by_symbol(machine).unwrap();
                program.machine_mut(position).type_parameters = parameters;
            }
        }
    }

    // main and partial: `let _ = advance<Implementation>();`
    let applied =
        |program: &mut TypedTrees, holder, local, spelled: &'static str, implementation| {
            let argument = StaticMachineArgument {
                path: Box::new([Identifier::generated(spelled)]),
                symbol: implementation,
                ..StaticMachineArgument::default()
            };
            let applied = call(program, holder, advance_state, "advance", Some(argument));
            let initial_value = program
                .expression_table
                .insert(ExpressionNode::Call(applied));
            StatementNode::LocalData(TableLocalData {
                symbol: local,
                name: Identifier::generated(spelled),
                initial_value,
                ..Default::default()
            })
        };
    let statements = vec![
        applied(&mut program, main, locals.handle_at(0), "Hold", hold),
        applied(&mut program, main, locals.handle_at(1), "Held", held),
    ];
    push_machine(
        &mut program,
        main,
        "main",
        Vec::new(),
        main_state,
        statements,
    );
    let statements = vec![applied(
        &mut program,
        partial,
        partial_local,
        "Empty",
        empty,
    )];
    push_machine(
        &mut program,
        partial,
        "partial",
        Vec::new(),
        partial_state,
        statements,
    );

    Source {
        program,
        step,
        requirement,
        hold,
        hold_apply,
        hold_apply_state,
        advance,
        selected,
        placeholder,
        main,
        partial,
        nested: nested.map(|(child, _, grandchild, _)| (child, grandchild)),
    }
}

fn only_state(program: &TypedTrees, machine: SymbolHandle) -> &State {
    let machine = program.machine_by_symbol(machine).expect("a machine");
    let Some([state]) = program.machine_states(machine).as_array() else {
        panic!("one state");
    };
    state
}

/// The calls a machine's one state holds as local initializers or as the
/// value it returns, in statement order.
fn calls(program: &TypedTrees, machine: SymbolHandle) -> Vec<&TableCallExpression> {
    only_state(program, machine)
        .statement_nodes
        .handles()
        .filter_map(|handle| {
            let value = match program.statement_table.statement(handle) {
                StatementNode::LocalData(local) => local.initial_value,
                StatementNode::Transition(transition) => {
                    match program.statement_table.transition_target(transition.target) {
                        TransitionTargetNode::Value(value) => *value,
                        _ => return None,
                    }
                }
                _ => return None,
            };
            match program.expression_table.expression(value) {
                ExpressionNode::Call(call) => Some(call),
                _ => None,
            }
        })
        .collect()
}

#[test]
fn a_call_through_an_implementation_parameter_enters_the_selected_operation() {
    let Source {
        mut program,
        step,
        requirement,
        hold,
        hold_apply,
        hold_apply_state,
        advance,
        selected,
        placeholder,
        main,
        ..
    } = source();
    prepare(&mut program).expect("the pass refuses no program");

    let [at_hold, at_held] = calls(&program, main)[..] else {
        panic!("two calls");
    };
    // Two names of one implementation value are one application.
    assert_eq!(at_hold.target_symbol, at_held.target_symbol);
    assert!(at_hold.machine_arguments.is_empty());
    let [binding] = program
        .expression_table
        .static_machine_application_bindings(at_hold.authored_machine_application)
    else {
        panic!("one binding");
    };
    assert_eq!(binding.parameter, selected);
    let StaticMachineBindingValue::Conformance(bound) = &binding.value else {
        panic!("an implementation");
    };
    assert_eq!(bound.symbol, hold);

    let generated = program.symbols.get(at_hold.target_symbol).parent;
    let receipt = program.machine_specialization(generated).unwrap();
    let key = program.specialization_source_identity(receipt).unwrap();
    let mut alias = receipt.clone();
    alias.binding_application = at_held.authored_machine_application;
    assert_eq!(
        key,
        program.specialization_source_identity(&alias).unwrap(),
        "two names of the same implementation value have one source application key"
    );
    assert_eq!(
        program
            .executable_application_template(generated)
            .expect("one template"),
        Some(advance)
    );
    // The generated machine has no parameter left.
    assert!(
        program
            .machine_by_symbol(generated)
            .expect("generated")
            .conformance_bounds
            .is_empty()
    );
    let [through] = calls(&program, generated)[..] else {
        panic!("one call");
    };
    assert_eq!(through.target_symbol, hold_apply_state);
    assert!(!through.receiver.is_valid());
    let dispatch = through
        .static_requirement_dispatch
        .as_ref()
        .expect("the selection");
    assert_eq!(dispatch.evidence_parameter, selected);
    assert_eq!(dispatch.declaring_trait, step);
    assert_eq!(dispatch.requirement, requirement);
    assert_eq!(dispatch.realization_machine, hold_apply);
    assert_eq!(dispatch.realization_state, hold_apply_state);
    // Retained typed trees recover the same exact selection, without a second
    // application or contract signature for the reader to rediscover.
    let limits = compiler_storage::RecordLimits::default();
    let encoded = compiler_storage::encode(dispatch, limits).unwrap();
    let recovered: typed_trees::StaticRequirementDispatch =
        compiler_storage::decode(&encoded, limits).unwrap();
    assert_eq!(&recovered, dispatch);
    // The call still records what the author named: the parameter's requirement.
    assert_eq!(
        through.authored_call_binding.map(|named| named.selected),
        Some(placeholder)
    );

    // The generic machine is left as written.
    let [written] = calls(&program, advance)[..] else {
        panic!("one call");
    };
    assert_eq!(written.target_symbol, placeholder);
    assert!(written.static_requirement_dispatch.is_none());
    assert!(written.receiver.is_valid());
    assert_eq!(program.machine_specializations.len(), 1);
    assert!(program.deferred_refusal(main).is_none());
}

#[test]
fn an_implementation_with_no_machine_for_a_requirement_is_refused_on_its_holder_only() {
    let Source {
        mut program,
        advance,
        main,
        partial,
        ..
    } = source();
    prepare(&mut program).expect("the pass refuses no program");

    let [call] = calls(&program, partial)[..] else {
        panic!("one call");
    };
    // The call is as typing wrote it: nothing was generated for it.
    assert_eq!(call.target_symbol, only_state(&program, advance).symbol);
    assert_eq!(call.machine_arguments.len(), 1);
    assert_eq!(
        program.deferred_refusal(partial).map(|row| row.request),
        Some(DeferredRequest::StaticApplication)
    );
    assert!(program.deferred_refusal(main).is_none());
}

#[test]
fn a_typed_const_record_keeps_its_value_binding_and_exact_named_call() {
    let Source {
        mut program,
        step,
        requirement,
        selected,
        advance,
        hold,
        hold_apply,
        hold_apply_state,
        main,
        partial,
        ..
    } = source_with_typed_record(true);
    // Source typing has selected the requirement, but has not retained an
    // application for this typed receiver's empty static tuple yet.
    let authored_member = program
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| {
            matches!(node, ExpressionNode::Call(call) if call.target_symbol == requirement)
                .then_some(expression)
        })
        .expect("the authored requirement call");
    let ExpressionNode::Call(call) = program.expression_table.expression_mut(authored_member)
    else {
        panic!("the authored requirement call");
    };
    assert!(call.machine_arguments.is_empty());
    call.authored_machine_application = Default::default();
    program
        .retain_static_argument_contexts()
        .expect("retain the authored member application");
    let authored = program.expression_table.call_node(authored_member).unwrap();
    assert!(authored.authored_machine_application.is_valid());
    let application = program
        .expression_table
        .static_machine_application(authored.authored_machine_application);
    assert_eq!(application.registration_operation, requirement);
    assert_eq!(application.lexical_owner, advance);
    assert!(application.arguments.is_empty());

    let held = program
        .conformances()
        .iter()
        .find(|declaration| program.symbols.name(declaration.symbol) == "Held")
        .unwrap()
        .symbol;
    let record = program
        .canonical_operation_record_value(hold, &[])
        .expect("the selected record has a complete canonical value");
    let alias = program
        .canonical_operation_record_value(held, &[])
        .expect("the record alias keeps the same fields");
    assert_eq!(record.identity(), alias.identity());
    let language_semantics::const_value::DecodedCanonicalConstValue::Record { fields, .. } =
        record.decode_encoding().unwrap()
    else {
        panic!("an operation record")
    };
    assert_eq!(fields.len(), 1);
    assert!(matches!(
        &fields[0].1,
        language_semantics::const_value::DecodedCanonicalConstValue::StaticCallable { .. }
    ));
    let signature = program
        .trait_machine_signature(requirement)
        .unwrap()
        .1
        .clone();
    let contract = program.runtime_callable_contracts.insert(
        typed_trees::callable_values::RuntimeCallableContract {
            signature,
            lexical_owner: step,
            resolved_source: None,
        },
    );
    let callable_type = program
        .type_reference_table
        .insert(typed_trees::types::TypeReferenceNode::Callable { contract });
    let application = program.expression_table.retain_static_machine_application(
        hold_apply_state,
        Identifier::generated("apply"),
        &[],
    );
    let source = program
        .expression_table
        .static_machine_application_mut(application);
    source.binding_declaration = hold_apply;
    source.lexical_owner = advance;
    let retained = typed_trees::callable_values::MachineValue {
        type_reference: callable_type,
        selected: StaticMachineArgument {
            symbol: hold_apply,
            ..Default::default()
        },
        application,
        conformance: hold,
        requirement,
    };
    assert_eq!(
        program
            .canonical_static_callable_value(&retained, &[])
            .unwrap(),
        fields[0].1,
        "a retained selection and a named record field are the same value production"
    );

    prepare(&mut program).expect("typed record preparation");
    for machine in program.machines() {
        if program
            .machine_instantiation(machine.symbol)
            .unwrap()
            .is_none()
        {
            continue;
        }
        for call in calls(&program, machine.symbol) {
            if !program
                .expression_table
                .static_machine_application_is_valid(call.authored_machine_application)
            {
                continue;
            }
            if let Some(parameter) = program
                .expression_table
                .static_machine_application(call.authored_machine_application)
                .source_record_receiver
            {
                assert_eq!(
                    parameter, selected,
                    "copied calls keep the original Const binder"
                );
                assert!(
                    program
                        .static_record_source_receiver(call.authored_machine_application)
                        .unwrap()
                        .is_some()
                );
            }
        }
    }

    assert!(
        program.deferred_refusal(main).is_none(),
        "{:?}",
        program.deferred_refusal(main)
    );
    assert_eq!(
        program.deferred_refusal(partial).map(|row| row.request),
        Some(DeferredRequest::StaticApplication)
    );
    let authored = program.machine_by_symbol(advance).unwrap();
    assert!(matches!(
        program.machine_type_parameters(authored)[0].kind,
        typed_trees::data::TypeParameterKind::Const { .. }
    ));
    let [first, alias] = calls(&program, main)[..] else {
        panic!("two value aliases");
    };
    assert_eq!(first.target_symbol, alias.target_symbol);
    let [binding] = program
        .expression_table
        .static_machine_application_bindings(first.authored_machine_application)
    else {
        panic!("one original const binding");
    };
    assert_eq!(binding.parameter, selected);
    let StaticMachineBindingValue::Const(reference) = binding.value else {
        panic!("the binder remains Const");
    };
    assert!(
        matches!(program.type_reference_table.type_reference(reference),
        typed_trees::types::TypeReferenceNode::Named { symbol, .. } if *symbol == hold)
    );
    let generated = program.symbols.get(first.target_symbol).parent;
    let explicit = StaticMachineArgument {
        symbol: advance,
        ..Default::default()
    };
    let generated_selection = StaticMachineArgument {
        symbol: generated,
        ..Default::default()
    };
    assert_eq!(
        typed_trees::type_identity::portable_machine_selection_identity(
            &program,
            &explicit,
            first.authored_machine_application,
            &[],
        )
        .unwrap(),
        typed_trees::type_identity::portable_machine_selection_identity(
            &program,
            &generated_selection,
            Default::default(),
            &[],
        )
        .unwrap(),
        "supplied source arguments and the generated body's retained bindings normalize alike",
    );
    let [member] = calls(&program, generated)[..] else {
        panic!("one member call");
    };
    assert_eq!(
        program
            .expression_table
            .static_machine_application(member.authored_machine_application)
            .source_record_receiver,
        Some(selected),
        "the closed named call retains its originating Const receiver"
    );
    assert!(member.static_requirement_dispatch.is_none());
    assert_eq!(member.target_symbol, requirement);
    let receiver = program
        .expression_table
        .name_path_node(member.receiver)
        .unwrap();
    assert_eq!(receiver.symbol, hold);
    let (_, row, signature) = program
        .named_conformance_call_member(member)
        .unwrap()
        .unwrap();
    assert_eq!(row.declaring_trait, step);
    assert_eq!(row.requirement, requirement);
    assert_eq!(signature.symbol, requirement);
    let (operand, _) = program
        .source_call_selected_operand(
            generated,
            typed_trees::references::StaticArgumentCall::Expression(
                program
                    .expression_table
                    .iter_expressions()
                    .find_map(|(handle, node)| {
                        matches!(node, ExpressionNode::Call(call) if std::ptr::eq(call, member))
                            .then_some(handle)
                    })
                    .unwrap(),
                member,
            ),
        )
        .unwrap();
    assert_eq!(operand.symbol, hold);
    assert!(
        program
            .specialization_source_identity(program.machine_specialization(generated).unwrap())
            .is_ok()
    );

    // A malformed value that selects code statically applied to itself is not
    // a finite canonical record. Reject through the same active query instead
    // of repeatedly starting a new callable projector.
    let cycle = program
        .symbols
        .append_generated_children_from(step, [(hold, SymbolKind::Conformance, "Cycle")])
        .start();
    let generated_entry = only_state(&program, generated).symbol;
    program.push_conformance(Conformance {
        symbol: cycle,
        trait_symbol: step,
        trait_name: Identifier::generated("Step"),
        rows: vec![ConformanceRow {
            declaring_trait: step,
            declaring_trait_name: Identifier::generated("Step"),
            requirement,
            requirement_name: Identifier::generated("apply"),
            realization_machine: generated,
            realization_state: generated_entry,
            realization_name: Identifier::generated("advance"),
            source: ConformanceRowSource::Reference,
        }],
        ..Default::default()
    });
    let cycle_reference =
        program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Named {
                symbol: cycle,
                name: Identifier::generated("Cycle"),
            });
    let application = program
        .machine_instantiation(generated)
        .unwrap()
        .unwrap()
        .binding_application();
    let mut bindings = program
        .expression_table
        .static_machine_application_bindings(application)
        .to_vec();
    bindings[0].value = StaticMachineBindingValue::Const(cycle_reference);
    program
        .expression_table
        .set_static_machine_application_bindings(application, &bindings);
    assert!(
        program
            .canonical_operation_record_value(cycle, &[])
            .is_err()
    );
}

#[test]
fn typed_const_records_forward_the_declared_value_and_reject_another_schema() {
    let Source {
        mut program,
        advance,
        selected,
        step,
        ..
    } = source_with_typed_record(true);
    let parameter =
        program.machine_type_parameters(program.machine_by_symbol(advance).unwrap())[0].clone();
    let forwarded = StaticMachineArgument {
        symbol: selected,
        path: Box::new([Identifier::generated("Selected")]),
        ..Default::default()
    };
    let operand = super::Operand::read(
        &mut program,
        &typed_trees::data::TypeParameter {
            symbol: selected,
            ..parameter.clone()
        },
        &forwarded,
    )
    .unwrap_or_else(|_| {
        panic!("forward the original Const value without changing its binder kind")
    });
    assert!(operand.is_open(&program));
    assert!(matches!(operand.value, StaticMachineBindingValue::Const(_)));
    assert_eq!(operand.argument, forwarded);

    let other = program
        .symbols
        .insert_generated_root_from(step, SymbolKind::Trait, "OtherStep");
    program.push_trait_definition(TraitDefinition {
        symbol: other,
        name: Identifier::generated("OtherStep"),
        ..Default::default()
    });
    let reference =
        program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Named {
                symbol: other,
                name: Identifier::generated("OtherStep"),
            });
    let parameters = program.machine_by_symbol(advance).unwrap().type_parameters;
    program
        .data_type_parameters
        .get_mut(parameters.start())
        .kind = typed_trees::data::TypeParameterKind::Const {
        type_reference: reference,
    };
    let errors =
        prepare(&mut program).expect_err("another exact schema is not the declared record type");
    assert!(errors[0].message.contains("another declared schema"));
}

#[test]
fn nested_const_records_keep_the_root_binding_owner_and_distinct_callable_identities() {
    let Source {
        mut program,
        advance,
        selected,
        hold,
        main,
        nested,
        ..
    } = source_with_record_children(true, true);
    let (child_template, grandchild_template) = nested.expect("authored child and grandchild");
    prepare(&mut program).expect("prepare real nested record applications");
    assert!(
        program.deferred_refusal(main).is_none(),
        "{:?}",
        program.deferred_refusal(main)
    );
    let entered: Vec<_> = calls(&program, main)
        .iter()
        .map(|call| call.target_symbol)
        .collect();
    assert_eq!(entered.len(), 2);
    assert_ne!(
        entered[0], entered[1],
        "different record values make different outer applications"
    );
    let held = program
        .conformances()
        .iter()
        .find(|record| program.symbols.name(record.symbol) == "Held")
        .unwrap()
        .symbol;
    let mut identities = Vec::new();
    for (entry, actual) in [(entered[0], hold), (entered[1], held)] {
        let outer = program.symbols.get(entry).parent;
        let application = program
            .machine_specialization(outer)
            .expect("outer receipt")
            .binding_application;
        let child_entry = calls(&program, outer)[0].target_symbol;
        let child = program.symbols.get(child_entry).parent;
        let grandchild_entry = calls(&program, child)[0].target_symbol;
        let grandchild = program.symbols.get(grandchild_entry).parent;
        assert_eq!(program.symbols.get(child).parent, entry);
        assert_eq!(program.symbols.get(grandchild).parent, child_entry);
        let mut application_identities = Vec::new();
        for (instance, template) in [(child, child_template), (grandchild, grandchild_template)] {
            assert!(
                program.deferred_refusal(instance).is_none(),
                "{:?}",
                program.deferred_refusal(instance)
            );
            let receipt = program
                .machine_specialization(instance)
                .expect("real copied receipt");
            assert_eq!(
                receipt.template, template,
                "executable identity remains the child declaration"
            );
            assert_eq!(receipt.binding_application, application);
            assert_eq!(
                program.specialization_binding_declaration(receipt).unwrap(),
                advance
            );
            assert_eq!(
                program.executable_application_template(instance).unwrap(),
                Some(template)
            );
            assert_eq!(
                program
                    .data_type_parameters
                    .span_or_empty(receipt.template_parameters),
                program.machine_type_parameters(program.machine_by_symbol(advance).unwrap()),
                "the producer retains the complete original binder prefix",
            );
            let frames = program
                .enclosing_invocation_frames(instance)
                .expect("read the outer binding frame");
            let [frame] = frames.as_slice() else {
                panic!("one enclosing application")
            };
            assert_eq!(frame.declaration, advance);
            assert_eq!(frame.argument_owner, main);
            let [binding] = frame.bindings else {
                panic!("one retained Const binding")
            };
            assert_eq!(binding.parameter, selected);
            let StaticMachineBindingValue::Const(reference) = binding.value else {
                panic!("Const remains a value")
            };
            assert!(
                matches!(program.type_reference_table.type_reference(reference),
                typed_trees::types::TypeReferenceNode::Named { symbol, .. } if *symbol == actual)
            );
            let source = program
                .specialization_source_operand(receipt)
                .expect("retain each child's source application");
            assert_eq!(source.symbol, template);
            assert_eq!(
                source.application.as_ref().unwrap().arguments[0].symbol,
                actual
            );
            let callable = typed_trees::type_identity::portable_machine_selection_identity(
                &program,
                &StaticMachineArgument {
                    symbol: instance,
                    ..Default::default()
                },
                Default::default(),
                &[],
            )
            .expect("canonical generated callable selection");
            assert_eq!(callable.0, template);
            application_identities.push((
                program.specialization_source_identity(receipt).unwrap(),
                callable,
            ));
        }
        assert_ne!(
            application_identities[0], application_identities[1],
            "child and grandchild have separate callable declarations"
        );
        identities.push(application_identities);
    }
    for level in 0..2 {
        assert_ne!(
            identities[0][level].0, identities[1][level].0,
            "source identity retains the actual outer record"
        );
        assert_ne!(
            identities[0][level].1, identities[1][level].1,
            "callable identity retains the actual outer record"
        );
    }
}

#[test]
fn nested_const_record_frames_refuse_foreign_owners_prefixes_and_suppliers() {
    let Source {
        mut program,
        advance,
        main,
        hold_apply,
        ..
    } = source_with_record_children(true, true);
    prepare(&mut program).expect("prepare real nested record applications");
    let outer = program
        .symbols
        .get(calls(&program, main)[0].target_symbol)
        .parent;
    let child = program
        .symbols
        .get(calls(&program, outer)[0].target_symbol)
        .parent;
    let original = program
        .machine_specialization(child)
        .expect("real copied child")
        .clone();
    let application = original.binding_application;
    assert_eq!(
        program
            .specialization_binding_declaration(&original)
            .unwrap(),
        advance
    );
    let refused = |program: &TypedTrees, expected: &str| {
        let receipt = program.machine_specialization(child).unwrap();
        let owner = program
            .specialization_binding_declaration(receipt)
            .expect_err("corrupt ownership must refuse");
        assert!(owner.contains(expected), "{owner}");
        let frames = program
            .enclosing_invocation_frames(child)
            .err()
            .expect("corrupt frames must refuse");
        assert!(frames.contains(expected), "{frames}");
        let identity = program
            .specialization_source_identity(receipt)
            .expect_err("corrupt source identity must refuse");
        assert!(identity.contains(expected), "{identity}");
    };
    let replace = |program: &mut TypedTrees, receipt: typed_trees::MachineSpecialization| {
        program.machine_specializations.edit(|rows| {
            *rows.iter_mut().find(|row| row.instance == child).unwrap() = receipt;
        });
    };

    // A valid, unrelated source machine is not an enclosing declaration.
    let mut foreign_parent = original.clone();
    foreign_parent.template = hold_apply;
    replace(&mut program, foreign_parent);
    refused(&program, "lexical ancestry");
    replace(&mut program, original.clone());

    program
        .expression_table
        .static_machine_application_mut(application)
        .binding_declaration = main;
    refused(&program, "declaration");
    program
        .expression_table
        .static_machine_application_mut(application)
        .binding_declaration = advance;

    let mut missing = original.clone();
    missing.template_parameters = HandleSpan::empty();
    replace(&mut program, missing);
    refused(&program, "prefix");
    replace(&mut program, original.clone());

    // Keep the child prefix present, but cross its binder kind against the
    // untouched outer declaration. Restore the source row before the next check.
    let prefix = original.template_parameters.start();
    let original_parameter = program.data_type_parameters.get(prefix).clone();
    program.data_type_parameters.get_mut(prefix).kind = typed_trees::data::TypeParameterKind::Type;
    refused(&program, "prefix");
    *program.data_type_parameters.get_mut(prefix) = original_parameter;

    program
        .expression_table
        .static_machine_application_mut(application)
        .lexical_owner = SymbolHandle::invalid();
    refused(&program, "supplier");
    program
        .expression_table
        .static_machine_application_mut(application)
        .lexical_owner = main;
    assert_eq!(
        program
            .specialization_binding_declaration(&original)
            .unwrap(),
        advance
    );
    assert!(program.enclosing_invocation_frames(child).is_ok());
    assert!(program.specialization_source_identity(&original).is_ok());
}
