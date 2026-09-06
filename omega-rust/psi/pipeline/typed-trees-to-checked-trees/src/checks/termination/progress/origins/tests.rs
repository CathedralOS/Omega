use super::*;
use arena::HandleSpan;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use symbols::SymbolHandle;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::data::DataMember;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

struct Fixture {
    program: TypedTrees,
    flow: FlowFacts,
    state: FlowStateFact,
}

impl Fixture {
    fn new(statements: &str, argument: &str) -> Self {
        let source = format!(
            r#"
            data Main {{}}
            machine Main::run(&mut self) {{}}
            data SchedulerHandle {{}}
            data Context {{ scheduler: SchedulerHandle; }}
            data Holder {{ view: Context; }}
            machine observe_scheduler(value: SchedulerHandle) -> u64 {{ 0 }}
            machine probe(context: &mut Context, replacement: &Context, holder: Holder) -> u64 {{
                {statements}
                transition {{ _ -> observe_scheduler({argument}) }}
            }}
            "#
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize origins");
        let syntax = parse_syntax_trees(&tokens).expect("parse origins");
        let resolved = lower_syntax_trees(&syntax).expect("resolve origins");
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
        // The prefix is call-free. Retain the sole real call occurrence in a
        // minimal flow row so at_call exercises its ordinary pointer lookup,
        // backward stores, declaration capture, and shared frame adapter.
        let mut flow = FlowFacts::default();
        let mut calls = HandleSpan::empty();
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
        let call = &self.flow.control.calls.span_or_empty(self.state.calls)[0];
        at_call(
            &self.program,
            &self.flow,
            machine,
            &self.state,
            call,
            subject,
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
