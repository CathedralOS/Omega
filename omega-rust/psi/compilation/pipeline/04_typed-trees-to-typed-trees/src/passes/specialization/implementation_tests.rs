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
use typed_trees::statement::{StatementNode, TableLocalData, TableTransition, TransitionTargetNode};
use typed_trees::static_applications::StaticMachineBindingValue;
use typed_trees::trait_definition::{
    Conformance, ConformanceRow, ConformanceRowSource, ImplementationValueIdentity,
    TraitDefinition,
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
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
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
                SymbolKind::ConformanceParameter,
                SymbolNameRef::Static("Selected"),
            ),
            (SymbolKind::State, SymbolNameRef::Static("entry")),
        ],
    );
    let (selected, advance_state) = (advance_children.handle_at(0), advance_children.handle_at(1));
    let placeholder = child(&mut builder, selected, SymbolKind::State, "apply");
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

    // Hold and Held select `Hold::apply`; Empty selects nothing.
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
            vec![ConformanceRow {
                source: ConformanceRowSource::Reference,
                ..row
            }],
            hold,
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
    let through = TableCallExpression {
        receiver: qualifier,
        authored_call_binding: Some(ResolvedAuthoredCallBinding {
            selected: placeholder,
            original: placeholder,
        }),
        ..call(&mut program, advance, placeholder, "apply", None)
    };
    let through = program
        .expression_table
        .insert(ExpressionNode::Call(through));
    let target = program
        .statement_table
        .insert_transition_target(TransitionTargetNode::Value(through));
    push_machine(
        &mut program,
        advance,
        "advance",
        vec![GenericConformanceBound {
            binder: selected,
            binder_name: Identifier::generated("Selected"),
            trait_symbol: step,
            trait_name: Identifier::generated("Step"),
            lifetime_arguments: Vec::new(),
            arguments: Vec::new(),
        }],
        advance_state,
        vec![StatementNode::Transition(TableTransition {
            target,
            ..TableTransition::default()
        })],
    );

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
    let statements = vec![applied(&mut program, partial, partial_local, "Empty", empty)];
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
    }
}

fn only_state(program: &TypedTrees, machine: SymbolHandle) -> &State {
    let machine = program.machine_by_symbol(machine).expect("a machine");
    let [state] = program.machine_states(machine) else {
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
