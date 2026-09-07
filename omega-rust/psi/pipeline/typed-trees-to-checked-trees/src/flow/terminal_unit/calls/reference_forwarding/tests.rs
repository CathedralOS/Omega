use super::*;

struct ForwardingFixture {
    program: TypedTrees,
    facts: CheckFacts,
    call: checked_trees::FlowCallFact,
    machine: SymbolHandle,
    state: SymbolHandle,
    parameter: SymbolHandle,
    argument: typed_trees::expression::ExpressionHandle,
    borrow_state: arena::Handle<checked_trees::StateBorrowFact>,
    borrow_call: arena::Handle<checked_trees::BorrowCallFact>,
}

impl ForwardingFixture {
    fn new(source_access: &str, target_access: &str) -> Self {
        Self::with_prefix(source_access, target_access, "")
    }

    fn with_prefix(source_access: &str, target_access: &str, prefix: &str) -> Self {
        let source = format!(
            "data Flag {{ enabled: bool; }}
             data Main {{}}
             data Helper {{}}
             machine Helper::forward(record: {target_access} Flag) {{}}
             machine Main::value(record: {source_access} Flag) {{ {prefix} Helper::forward(record); }}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .expect("tokenize forwarding fixture");
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse forwarding fixture");
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve forwarding fixture");
        let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type forwarding fixture");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str().ends_with("value"))
            .expect("caller machine");
        let state = program
            .machine_states(machine)
            .first()
            .expect("caller entry");
        let parameter = program.state_parameters(state)[0].symbol;
        let (statement_index, authored) = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
            .find_map(|(statement_index, statement)| match statement {
                StatementNode::Call(authored) => Some((statement_index, authored)),
                _ => None,
            })
            .expect("forwarding statement call");
        let argument = program
            .statement_table
            .expression_handles(authored.arguments)[0];
        let machine = machine.symbol;
        let state = state.symbol;
        let call = checked_trees::FlowCallFact {
            statement_index,
            target_symbol: authored.target_symbol,
            ..Default::default()
        };
        let mut facts = CheckFacts::default();
        let accesses =
            facts
                .borrow
                .argument_accesses
                .insert_many([checked_trees::BorrowArgumentAccessFact {
                    root_symbol: parameter,
                    kind: checked_trees::BorrowAccessKind::Read,
                    ..Default::default()
                }]);
        let mut calls = arena::HandleSpan::default();
        let borrow_call = facts.borrow.calls.append_to_span(
            &mut calls,
            checked_trees::BorrowCallFact {
                statement_index,
                target_symbol: call.target_symbol,
                accesses,
                ..Default::default()
            },
        );
        let borrow_state = facts.borrow.states.append(checked_trees::StateBorrowFact {
            machine_symbol: machine,
            state_symbol: state,
            calls,
            ..Default::default()
        });
        Self {
            program,
            facts,
            call,
            machine,
            state,
            parameter,
            argument,
            borrow_state,
            borrow_call,
        }
    }

    fn access(&self) -> Option<CheckedStructuralAccess> {
        let place = crate::flow::canonical_place_from_symbol(self.parameter)
            .expect("exact parameter place");
        exact_structural_argument_access(
            &self.program,
            &self.facts,
            self.machine,
            self.state,
            &self.call,
            &place,
            CheckedStructuralAccess::MutableBorrow,
        )
    }
}

#[test]
fn a_bare_mutable_reference_preserves_its_exact_referent_access() {
    let fixture = ForwardingFixture::new("&mut", "&mut");
    let state = crate::find_state_in_machine(&fixture.program, fixture.machine, fixture.state)
        .expect("exact caller state");
    let site = crate::find_call_site(&fixture.program, fixture.machine, fixture.state, 0, 0)
        .expect("exact forwarding call occurrence");
    assert!(
        matches!(site, crate::CallSite::Statement(_)),
        "statement call occurrence"
    );
    let parameters = crate::call_target_parameters(&fixture.program, fixture.call.target_symbol)
        .expect("exact target parameters");
    assert!(
        fixture.call.target_symbol.is_valid(),
        "valid authored target"
    );
    assert_eq!(
        fixture.access(),
        Some(CheckedStructuralAccess::MutableBorrow),
        "source={:?}, target={parameters:?}, argument={:?}",
        fixture.program.state_parameters(state),
        fixture
            .program
            .expression_table
            .expression(fixture.argument)
    );
}

#[test]
fn shared_source_or_target_cannot_grant_mutable_forwarding() {
    for (source, target) in [("&", "&"), ("&mut", "&"), ("&", "&mut")] {
        let fixture = ForwardingFixture::new(source, target);
        assert_eq!(
            fixture.access(),
            Some(CheckedStructuralAccess::SharedBorrow)
        );
    }
}

#[test]
fn forwarding_rejoins_actual_prefix_writes_instead_of_the_mutable_parameter_bit() {
    for (prefix, preserved) in [
        ("let count: u32 = 0u32;", true),
        ("record.enabled = false;", false),
    ] {
        let fixture = ForwardingFixture::with_prefix("&mut", "&mut", prefix);
        assert_eq!(
            fixture.access() == Some(CheckedStructuralAccess::MutableBorrow),
            preserved,
            "prefix {prefix}"
        );
    }
}

#[test]
fn forwarding_requires_exact_call_and_parameter_custody() {
    for mutation in 0..5 {
        let mut fixture = ForwardingFixture::new("&mut", "&mut");
        match mutation {
            0 => fixture.call.target_symbol = fixture.machine,
            1 => {
                fixture
                    .facts
                    .borrow
                    .calls
                    .get_mut(fixture.borrow_call)
                    .target_symbol = fixture.machine
            }
            2 => {
                let ExpressionNode::Name(path) = fixture
                    .program
                    .expression_table
                    .expression_mut(fixture.argument)
                else {
                    panic!("bare carrier argument");
                };
                path.head_symbol = SymbolHandle::invalid();
            }
            3 => {
                let ExpressionNode::Name(path) = fixture
                    .program
                    .expression_table
                    .expression_mut(fixture.argument)
                else {
                    panic!("bare carrier argument");
                };
                path.symbol = fixture.machine;
                path.head_symbol = fixture.machine;
            }
            4 => {
                let handle = fixture
                    .program
                    .state_parameters
                    .iter()
                    .find(|(_, parameter)| parameter.symbol == fixture.parameter)
                    .expect("source parameter row")
                    .0;
                fixture.program.state_parameters.get_mut(handle).is_const = true;
            }
            _ => unreachable!(),
        }
        assert_ne!(
            fixture.access(),
            Some(CheckedStructuralAccess::MutableBorrow),
            "mutation {mutation}"
        );
    }
}

#[test]
fn forwarding_does_not_invent_loan_restoration_or_duplicate_access_authority() {
    for mutation in 0..4 {
        let mut fixture = ForwardingFixture::new("&mut", "&mut");
        if mutation == 0 {
            let access = checked_trees::BorrowArgumentAccessFact {
                root_symbol: fixture.parameter,
                kind: checked_trees::BorrowAccessKind::Read,
                ..Default::default()
            };
            fixture
                .facts
                .borrow
                .calls
                .get_mut(fixture.borrow_call)
                .accesses = fixture
                .facts
                .borrow
                .argument_accesses
                .insert_many([access.clone(), access]);
        } else {
            let loan = checked_trees::BorrowLoanFact {
                root_symbol: if mutation == 3 {
                    fixture.machine
                } else {
                    fixture.parameter
                },
                statement_index: 0,
                last_use_statement_index: 0,
                ..Default::default()
            };
            fixture
                .facts
                .borrow
                .states
                .get_mut(fixture.borrow_state)
                .loans = fixture.facts.borrow.loans.insert_many([loan]);
            if mutation == 2 {
                // A future loan is not live at this call occurrence.
                let handle = fixture
                    .facts
                    .borrow
                    .loans
                    .iter()
                    .next()
                    .expect("loan row")
                    .0;
                fixture.facts.borrow.loans.get_mut(handle).statement_index = 1;
                fixture
                    .facts
                    .borrow
                    .loans
                    .get_mut(handle)
                    .last_use_statement_index = 1;
            }
        }
        assert_eq!(
            fixture.access() == Some(CheckedStructuralAccess::MutableBorrow),
            mutation >= 2,
            "mutation {mutation}"
        );
    }
}
