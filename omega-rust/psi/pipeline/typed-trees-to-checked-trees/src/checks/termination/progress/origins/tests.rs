use super::{FlowCallFact, FlowFacts, FlowStateFact, ProgressSubject, TypedTrees};
use crate::checks::termination::progress::origins::at_call;
use arena::HandleSpan;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use symbols::SymbolHandle;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::data::DataMember;
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

mod references;

struct Fixture {
    program: TypedTrees,
    flow: FlowFacts,
    state: FlowStateFact,
}

impl Fixture {
    fn new(statements: &str, argument: &str) -> Self {
        Self::with_helper_calls(statements, argument, &[])
    }

    /// `helper_call_statements` are statement indexes containing one
    /// value-position helper call each; each call's flow row is retained in
    /// execution order before the terminal demand call, matching how the
    /// real pipeline publishes authored call occurrences.
    fn with_helper_calls(
        statements: &str,
        argument: &str,
        helper_call_statements: &[usize],
    ) -> Self {
        let source = format!(
            r#"
            data Main {{}}
            machine Main::run(&mut self) {{}}
            data SchedulerHandle {{}}
            data Context {{ scheduler: SchedulerHandle; }}
            data Holder {{ view: Context; }}
            machine observe_scheduler(value: SchedulerHandle) -> u64 {{ 0 }}
            machine pick(context: &Context) -> SchedulerHandle {{ context.scheduler }}
            machine pick_second(former: &Context, latter: &Context) -> SchedulerHandle {{ latter.scheduler }}
            machine pick_cached(context: &Context) -> SchedulerHandle {{ let s: SchedulerHandle = context.scheduler; s }}
            machine pick_mutated(context: &Context) -> SchedulerHandle {{ let mut s: SchedulerHandle = context.scheduler; s = s; s }}
            machine pick_mut(context: &mut Context) -> SchedulerHandle {{ context.scheduler }}
            machine forward(context: &Context) -> SchedulerHandle {{ pick(context) }}
            machine forward_cached(context: &Context) -> SchedulerHandle {{ let s: SchedulerHandle = pick(context); s }}
            machine forward_mut(context: &mut Context) -> SchedulerHandle {{ pick_mut(context) }}
            machine rebuild(context: &Context) -> Context {{ Context {{ scheduler: context.scheduler }} }}
            machine rebuild_second(former: &Context, latter: &Context) -> Context {{ Context {{ scheduler: latter.scheduler }} }}
            machine rebuild_fresh(context: &Context) -> Context {{ Context {{ scheduler: SchedulerHandle {{}} }} }}
            machine rebuild_mut(context: &mut Context) -> Context {{ Context {{ scheduler: context.scheduler }} }}
            machine rebuild_cached(context: &Context) -> Context {{ let c: Context = Context {{ scheduler: context.scheduler }}; c }}
            machine wrap_pick(context: &Context) -> Holder {{ Holder {{ view: Context {{ scheduler: pick(context) }} }} }}
            machine rebuild_pair(context: &Context) -> [SchedulerHandle; 2] {{ [context.scheduler, context.scheduler] }}
            machine probe(context: &mut Context, replacement: &Context, holder: Holder) -> u64 {{
                {statements}
                transition {{ _ -> observe_scheduler({argument}) }}
            }}
            "#
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize origins");
        let syntax = parse_syntax_trees(&tokens).expect("parse origins");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve origins");
        let program = lower_symbol_resolved_trees(&resolved).expect("type origins");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "probe")
            .expect("probe machine");
        let typed_state = &program.machine_states(machine)[0];
        let statements = program
            .statement_table
            .statements(typed_state.statement_nodes);
        assert!(matches!(
            statements.last(),
            Some(StatementNode::Transition(_))
        ));
        let target_symbol = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "observe_scheduler")
            .expect("observed call")
            .symbol;
        // Retain each authored call occurrence in execution order so at_call
        // exercises its ordinary pointer lookup, backward stores, declaration
        // capture, helper-result resolution, and shared frame adapter.
        let mut flow = FlowFacts::default();
        let mut calls = HandleSpan::empty();
        for &index in helper_call_statements {
            let statement = statements.get(index).expect("helper call statement");
            let expression =
                helper_call_expression(&program, statement).expect("helper call expression");
            let typed_trees::expression::ExpressionNode::Call(call) =
                program.expression_table.expression(expression)
            else {
                unreachable!("helper call node")
            };
            flow.control.calls.append_to_span(
                &mut calls,
                FlowCallFact {
                    statement_index: index,
                    authored_expression: expression,
                    target_symbol: call.target_symbol,
                    ..FlowCallFact::default()
                },
            );
        }
        flow.control.calls.append_to_span(
            &mut calls,
            FlowCallFact {
                statement_index: statements.len() - 1,
                target_symbol,
                ..FlowCallFact::default()
            },
        );
        let state = FlowStateFact {
            machine_symbol: machine.symbol,
            state_symbol: typed_state.symbol,
            calls,
            ..FlowStateFact::default()
        };
        Self {
            program,
            flow,
            state,
        }
    }

    fn root(&self, name: &str) -> SymbolHandle {
        let state = crate::semantic_calls::find_state(&self.program, self.state.state_symbol)
            .expect("fixture state");
        self.program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.name.as_str() == name)
            .map(|parameter| parameter.symbol)
            .or_else(|| {
                self.program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .find_map(|statement| match statement {
                        StatementNode::LocalData(local) if local.name.as_str() == name => {
                            Some(local.symbol)
                        }
                        _ => None,
                    })
            })
            .expect("fixture root")
    }

    fn field(&self, owner: &str, name: &str) -> SymbolHandle {
        let definition = self
            .program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == owner)
            .expect("field owner");
        self.program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == name => Some(field.symbol),
                _ => None,
            })
            .expect("fixture field")
    }

    fn subject(&self, root: &str, projections: &[(&str, &str)]) -> ProgressSubject {
        ProgressSubject {
            root: self.root(root),
            projections: projections
                .iter()
                .map(|(owner, name)| self.field(owner, name))
                .collect(),
        }
    }

    fn query(&self, subject: ProgressSubject) -> Option<ProgressSubject> {
        let machine = self
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == self.state.machine_symbol)
            .expect("fixture machine");
        // The terminal demand call is always the last retained row; helper
        // calls occupy earlier rows in execution order.
        let call = self
            .flow
            .control
            .calls
            .span_or_empty(self.state.calls)
            .last()
            .expect("demand call row");
        at_call(
            &self.program,
            &self.flow,
            machine,
            &self.state,
            call,
            subject,
            None,
        )
    }

    fn readonly(&mut self, referee: TypeReferenceHandle) -> TypeReferenceHandle {
        self.program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee,
                access: language_semantics::ReferenceAccess::Shared,
                lifetime: None,
            })
    }

    fn make_local_reference(&mut self, name: &str, constrained: bool) {
        let state = crate::semantic_calls::find_state(&self.program, self.state.state_symbol)
            .expect("fixture state");
        let statements = state.statement_nodes;
        let (index, stored_type) = self
            .program
            .statement_table
            .statements(statements)
            .iter()
            .enumerate()
            .find_map(|(index, statement)| match statement {
                StatementNode::LocalData(local) if local.name.as_str() == name => {
                    Some((index, local.type_reference))
                }
                _ => None,
            })
            .expect("local declaration");
        let reference = self.readonly(stored_type);
        let reference = if constrained {
            self.program
                .type_reference_table
                .insert(TypeReferenceNode::Constrained {
                    base_type: reference,
                    constraints: HandleSpan::empty(),
                })
        } else {
            reference
        };
        let StatementNode::LocalData(local) =
            &mut self.program.statement_table.statements_mut(statements)[index]
        else {
            unreachable!("retained local declaration")
        };
        local.type_reference = reference;
    }

    fn make_field_reference(&mut self, owner: &str, name: &str) {
        let symbol = self.field(owner, name);
        let (handle, stored_type) = self
            .program
            .data_members
            .iter()
            .find_map(|(handle, member)| match member {
                DataMember::Field(field) if field.symbol == symbol => {
                    Some((handle, field.type_reference))
                }
                _ => None,
            })
            .expect("retained field");
        let reference = self.readonly(stored_type);
        let DataMember::Field(field) = self.program.data_members.get_mut(handle) else {
            unreachable!("retained field declaration")
        };
        field.type_reference = reference;
    }
}

// These mutations test the resolver's own copy fence, not whether the full
// frontend admits an alias-bearing source program. Each unchanged typed graph
// first proves the exact owned origin, preventing unrelated lookup failure from
// making the negative assertion vacuous.

#[test]
fn reference_assignment_cannot_bypass_the_local_declaration_fence() {
    let mut fixture = Fixture::new(
        "let mut saved: SchedulerHandle = context.scheduler; saved = replacement.scheduler;",
        "saved",
    );
    let subject = fixture.subject("saved", &[]);
    let expected = fixture.subject("replacement", &[("Context", "scheduler")]);
    assert_eq!(fixture.query(subject.clone()), Some(expected));
    fixture.make_local_reference("saved", false);
    assert_eq!(fixture.query(subject), None);
}

#[test]
fn constrained_reference_declaration_is_not_an_owned_capture() {
    let mut fixture = Fixture::new("let saved: SchedulerHandle = context.scheduler;", "saved");
    let subject = fixture.subject("saved", &[]);
    let expected = fixture.subject("context", &[("Context", "scheduler")]);
    assert_eq!(fixture.query(subject.clone()), Some(expected));
    fixture.make_local_reference("saved", true);
    assert_eq!(fixture.query(subject), None);
}

#[test]
fn holder_copy_does_not_capture_a_readonly_reference_field_referent() {
    let mut fixture = Fixture::new("let saved: Holder = holder;", "saved.view.scheduler");
    let fields = &[("Holder", "view"), ("Context", "scheduler")];
    let subject = fixture.subject("saved", fields);
    let expected = fixture.subject("holder", fields);
    assert_eq!(fixture.query(subject.clone()), Some(expected));
    fixture.make_field_reference("Holder", "view");
    assert_eq!(fixture.query(subject), None);
}

#[test]
fn field_assignment_checks_the_stored_field_not_the_reference_root() {
    let mut fixture = Fixture::new(
        "context.scheduler = replacement.scheduler;",
        "context.scheduler",
    );
    let subject = fixture.subject("context", &[("Context", "scheduler")]);
    let expected = fixture.subject("replacement", &[("Context", "scheduler")]);
    assert_eq!(fixture.query(subject.clone()), Some(expected));
    fixture.make_field_reference("Context", "scheduler");
    assert_eq!(fixture.query(subject), None);
}

/// The value-position helper call inside a store or declaration, reached
/// through any member or index projections around it.
fn helper_call_expression(
    program: &TypedTrees,
    statement: &StatementNode,
) -> Option<typed_trees::expression::ExpressionHandle> {
    let mut expression = match statement {
        StatementNode::Assignment(assignment) => assignment.value,
        StatementNode::LocalData(local) => local.initial_value,
        _ => return None,
    };
    loop {
        match program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Call(_) => return Some(expression),
            typed_trees::expression::ExpressionNode::Member(member) => {
                expression = member.receiver;
            }
            typed_trees::expression::ExpressionNode::Indexed(indexed) => {
                expression = indexed.collection;
            }
            typed_trees::expression::ExpressionNode::Borrow(borrow) => {
                expression = borrow.target;
            }
            _ => return None,
        }
    }
}

#[test]
fn helper_result_store_derives_the_exact_input_projection() {
    let fixture = Fixture::with_helper_calls(
        "context.scheduler = pick(replacement);",
        "context.scheduler",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("context", &[("Context", "scheduler")])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn helper_result_local_capture_derives_the_exact_input_projection() {
    let fixture = Fixture::with_helper_calls(
        "let saved: SchedulerHandle = pick(replacement);",
        "saved",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("saved", &[])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn helper_result_follows_the_matching_parameter_not_the_first() {
    let fixture = Fixture::with_helper_calls(
        "context.scheduler = pick_second(&holder.view, replacement);",
        "context.scheduler",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("context", &[("Context", "scheduler")])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn helper_result_traces_an_immutable_local_capture_in_the_body() {
    let fixture = Fixture::with_helper_calls(
        "context.scheduler = pick_cached(replacement);",
        "context.scheduler",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("context", &[("Context", "scheduler")])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn helper_result_stays_exact_through_a_later_owned_store() {
    let fixture = Fixture::with_helper_calls(
        "let saved: SchedulerHandle = pick(replacement); context.scheduler = saved;",
        "context.scheduler",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("context", &[("Context", "scheduler")])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn helper_result_survives_writes_after_the_capture() {
    let fixture = Fixture::with_helper_calls(
        "context.scheduler = pick(replacement); replacement.scheduler = pick(replacement);",
        "context.scheduler",
        &[0, 1],
    );
    assert_eq!(
        fixture.query(fixture.subject("context", &[("Context", "scheduler")])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn mutable_body_capture_has_no_exact_origin() {
    let fixture = Fixture::with_helper_calls(
        "context.scheduler = pick_mutated(replacement);",
        "context.scheduler",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("context", &[("Context", "scheduler")])),
        None
    );
}

#[test]
fn mutable_input_helper_result_has_no_exact_origin() {
    let fixture = Fixture::with_helper_calls(
        "context.scheduler = pick_mut(context);",
        "context.scheduler",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("context", &[("Context", "scheduler")])),
        None
    );
}

#[test]
fn helper_result_through_a_nested_call_derives_the_exact_input_projection() {
    let fixture = Fixture::with_helper_calls(
        "context.scheduler = forward(replacement);",
        "context.scheduler",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("context", &[("Context", "scheduler")])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn helper_result_through_a_nested_call_capture_derives_the_exact_input_projection() {
    let fixture = Fixture::with_helper_calls(
        "context.scheduler = forward_cached(replacement);",
        "context.scheduler",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("context", &[("Context", "scheduler")])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn nested_call_result_keeps_a_per_field_projection() {
    let fixture = Fixture::with_helper_calls(
        "let saved: SchedulerHandle = forward(replacement);",
        "saved",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("saved", &[])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn nested_call_through_a_mutable_input_has_no_exact_origin() {
    let fixture = Fixture::with_helper_calls(
        "context.scheduler = forward_mut(context);",
        "context.scheduler",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("context", &[("Context", "scheduler")])),
        None
    );
}

#[test]
fn owned_load_through_a_reference_derives_the_referent() {
    let fixture = Fixture::new(
        "let borrowed: &Context = &context; let saved: SchedulerHandle = borrowed.scheduler;",
        "saved",
    );
    assert_eq!(
        fixture.query(fixture.subject("saved", &[])),
        Some(fixture.subject("context", &[("Context", "scheduler")]))
    );
}

#[test]
fn owned_load_through_reference_after_a_disjoint_store() {
    let fixture = Fixture::new(
        "let borrowed: &Context = &context; let mut tmp: SchedulerHandle = context.scheduler; tmp = replacement.scheduler; let saved: SchedulerHandle = borrowed.scheduler;",
        "saved",
    );
    assert_eq!(
        fixture.query(fixture.subject("saved", &[])),
        Some(fixture.subject("context", &[("Context", "scheduler")]))
    );
}

#[test]
fn owned_load_through_reference_observes_the_current_value() {
    let fixture = Fixture::new(
        "let borrowed: &Context = &context; context.scheduler = replacement.scheduler; let saved: SchedulerHandle = borrowed.scheduler;",
        "saved",
    );
    assert_eq!(
        fixture.query(fixture.subject("saved", &[])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn owned_load_through_a_reference_bound_field() {
    let fixture = Fixture::new(
        "let borrowed: &Context = &holder.view; let saved: SchedulerHandle = borrowed.scheduler;",
        "saved",
    );
    assert_eq!(
        fixture.query(fixture.subject("saved", &[])),
        Some(fixture.subject("holder", &[("Holder", "view"), ("Context", "scheduler")]))
    );
}

#[test]
fn owned_load_through_chained_reference_locals() {
    let fixture = Fixture::new(
        "let first: &Context = &context; let second: &Context = first; let saved: SchedulerHandle = second.scheduler;",
        "saved",
    );
    assert_eq!(
        fixture.query(fixture.subject("saved", &[])),
        Some(fixture.subject("context", &[("Context", "scheduler")]))
    );
}

#[test]
fn constructed_result_field_derives_the_input_projection() {
    let fixture = Fixture::with_helper_calls(
        "context.scheduler = rebuild(replacement).scheduler;",
        "context.scheduler",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("context", &[("Context", "scheduler")])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn constructed_result_local_capture_derives_the_input_projection() {
    let fixture = Fixture::with_helper_calls(
        "let rebuilt: Context = rebuild(replacement); context.scheduler = rebuilt.scheduler;",
        "context.scheduler",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("context", &[("Context", "scheduler")])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn constructed_result_follows_the_selected_operand() {
    let fixture = Fixture::with_helper_calls(
        "context.scheduler = rebuild_second(context, replacement).scheduler;",
        "context.scheduler",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("context", &[("Context", "scheduler")])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn constructed_result_through_a_local_initializer() {
    let fixture = Fixture::with_helper_calls(
        "context.scheduler = rebuild_cached(replacement).scheduler;",
        "context.scheduler",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("context", &[("Context", "scheduler")])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn nested_call_inside_a_constructed_result() {
    let fixture = Fixture::with_helper_calls(
        "let wrapped: Holder = wrap_pick(replacement); context.scheduler = wrapped.view.scheduler;",
        "context.scheduler",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("context", &[("Context", "scheduler")])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn constructed_array_result_derives_the_element_origin() {
    let fixture = Fixture::with_helper_calls(
        "context.scheduler = rebuild_pair(replacement)[0];",
        "context.scheduler",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("context", &[("Context", "scheduler")])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn fresh_constructor_result_has_no_input_origin() {
    let fixture = Fixture::with_helper_calls(
        "context.scheduler = rebuild_fresh(replacement).scheduler;",
        "context.scheduler",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("context", &[("Context", "scheduler")])),
        None
    );
}

#[test]
fn mutable_input_constructor_result_has_no_exact_origin() {
    let fixture = Fixture::with_helper_calls(
        "context.scheduler = rebuild_mut(context).scheduler;",
        "context.scheduler",
        &[0],
    );
    assert_eq!(
        fixture.query(fixture.subject("context", &[("Context", "scheduler")])),
        None
    );
}
