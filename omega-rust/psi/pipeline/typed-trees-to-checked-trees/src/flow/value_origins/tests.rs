use super::value_origin_before_statement;
use crate::flow::CanonicalPlace;
use crate::tests::front_end::typed_program;
use checked_trees::FlowStateFact;
use facts::PlaceRoot;
use symbols::SymbolHandle;
use typed_trees::data::DataMember;
use typed_trees::statement::StatementNode;
use typed_trees::{TypedTrees, machine::Machine};

struct Fixture {
    program: TypedTrees,
    machine: Machine,
    state: FlowStateFact,
    bound: usize,
}

impl Fixture {
    /// `machines` is additional top-level source inserted before `probe`:
    /// data declarations a probe needs beyond the shared set.
    fn new(statements: &str, machines: &str) -> Self {
        let source = format!(
            r#"
            data SchedulerHandle {{}}
            data Context {{ scheduler: SchedulerHandle; }}
            data Holder {{ view: Context; }}
            {machines}
            machine probe(context: &mut Context, replacement: &Context, holder: Holder) -> u64 {{
                {statements}
                transition {{ _ -> 0 }}
            }}
            "#
        );
        let program = typed_program(&source);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "probe")
            .expect("probe machine")
            .clone();
        let typed_state = &program.machine_states(&machine)[0];
        let bound = program
            .statement_table
            .statements(typed_state.statement_nodes)
            .len()
            - 1;
        let state = FlowStateFact {
            machine_symbol: machine.symbol,
            state_symbol: typed_state.symbol,
            ..FlowStateFact::default()
        };
        Self {
            program,
            machine,
            state,
            bound,
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

    fn place(&self, root: &str, projections: &[(&str, &str)]) -> CanonicalPlace {
        let mut segments = Vec::new();
        for (owner, name) in projections {
            crate::flow::push_field_place_segments(
                &self.program,
                &mut segments,
                self.field(owner, name),
            );
        }
        CanonicalPlace {
            root: PlaceRoot::Symbol(self.root(root)),
            segments,
        }
    }

    fn query(&self, place: CanonicalPlace) -> Option<CanonicalPlace> {
        value_origin_before_statement(
            &self.program,
            &self.machine,
            &self.state,
            self.bound,
            place,
            None,
        )
    }
}

/// A stored projection off a record literal has no storage of its own: the
/// demanded value arrives from the operand bound to that exact field.
#[test]
fn constructor_field_projection_derives_the_bound_operand() {
    let fixture = Fixture::new(
        "let saved: SchedulerHandle = Context { scheduler: context.scheduler }.scheduler;",
        "",
    );
    assert_eq!(
        fixture.query(fixture.place("saved", &[])),
        Some(fixture.place("context", &[("Context", "scheduler")]))
    );
}

/// A stored projection off an array literal selects one element, whose own
/// operand place is the demanded value's origin.
#[test]
fn constructor_element_projection_derives_the_bound_operand() {
    let fixture = Fixture::new(
        "let saved: SchedulerHandle = [context.scheduler, replacement.scheduler][1];",
        "",
    );
    assert_eq!(
        fixture.query(fixture.place("saved", &[])),
        Some(fixture.place("replacement", &[("Context", "scheduler")]))
    );
}

/// The bound operand may itself be a reference: the captured origin continues
/// through the leaf's referent rather than the literal's spelling.
#[test]
fn constructor_projection_through_a_reference_leaf_derives_the_referent() {
    let fixture = Fixture::new(
        "let saved: SchedulerHandle = RefBox { view: &context }.view.scheduler;",
        "data RefBox { view: &Context; }",
    );
    assert_eq!(
        fixture.query(fixture.place("saved", &[])),
        Some(fixture.place("context", &[("Context", "scheduler")]))
    );
}
