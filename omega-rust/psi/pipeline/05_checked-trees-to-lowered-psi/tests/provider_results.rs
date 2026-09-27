use checked_trees_to_lowered_psi::TerminalMachineSelection;

const SOURCE: &str = r#"
    pub data Token { value: u64; }
    pub data Pair { left: Token; right: Token; }
    boundary trait Factory {
        machine forward(value: Pair) -> Pair reaches Factory;
    }
    data Provider {}
    machine Provider::forward(value: Pair) -> Pair satisfies Factory::forward { value }
    data Alternative {}
    machine Alternative::forward(value: Pair) -> Pair satisfies Factory::forward { value }
    data Sink {}
    machine Sink::take(token: Token) {}
    data Root {}
    machine Root::enter(value: Pair) reaches Factory {
        let result: Pair = Factory::forward(value);
        Sink::take(result.right);
    }
"#;

#[test]
fn authored_affine_provider_requires_one_exact_checked_return_plan() {
    let checked = crate::front_end::checked_program(SOURCE);
    let plans = &checked
        .facts
        .flow
        .terminal_structural_returns
        .claim_free_affine_machines;
    assert_eq!(plans.len(), 2);
    for mutation in 0..3 {
        let mut malformed = checked.clone();
        let plans = &mut malformed
            .facts
            .flow
            .terminal_structural_returns
            .claim_free_affine_machines;
        match mutation {
            0 => {
                plans.remove(0);
            }
            1 => {
                plans.push(plans[0].clone());
            }
            2 => {
                plans[0].result.type_identity = "Token".into();
            }
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &malformed,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn retained_affine_provider_rejoins_its_authored_return_source() {
    let checked = crate::front_end::checked_program(SOURCE);
    let plan = &checked
        .facts
        .flow
        .terminal_structural_returns
        .claim_free_affine_machines[0];
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == plan.machine)
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let [checked_trees::statement::StatementNode::Expression(expression)] =
        checked.statement_table.statements(state.statement_nodes)
    else {
        panic!("identity body")
    };
    for replace_head in [false, true] {
        let mut changed = checked.clone();
        let checked_trees::expression::ExpressionNode::Name(path) =
            changed.typed.expression_table.expression_mut(*expression)
        else {
            panic!("returned name")
        };
        if replace_head {
            path.head_symbol = machine.symbol;
        } else {
            path.symbol = machine.symbol;
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err()
        );
    }
}
