use super::{Lexer, lower_syntax_trees, parse_syntax_trees};
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::domain::ProofFact;
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
use symbol_resolved_trees::signature::SignatureContract;
use symbol_resolved_trees::state::State;
use symbol_resolved_trees::statement::{StatementNode, TableLocalData};
use symbols::SymbolHandle;

fn resolve(source: &str) -> SymbolResolvedTrees {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize const value names");
    let syntax = parse_syntax_trees(&tokens).expect("parse const value names");
    lower_syntax_trees(&syntax).expect("resolve const value names")
}

fn local<'program>(
    program: &'program SymbolResolvedTrees,
    state: &State,
    name: &str,
) -> &'program TableLocalData {
    program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == name => Some(local),
            _ => None,
        })
        .expect("named local")
}

fn assert_name_symbol(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
    expected: SymbolHandle,
) {
    let expressions = &program.tables.bodies.expressions;
    let ExpressionNode::Name(path) = expressions.expression(expression) else {
        panic!("ordinary value name")
    };
    assert_eq!(path.head_symbol, expected);
    assert_eq!(path.symbol, expected);
    assert_eq!(
        expressions.name_path_member_symbols(path.member_symbols),
        [expected]
    );
}

fn assert_contract_const_symbols(
    program: &SymbolResolvedTrees,
    contracts: &[SignatureContract],
    expected: SymbolHandle,
) {
    let expressions = &program.tables.bodies.expressions;
    let mut observed = 0;
    for contract in contracts {
        for fact in program.proof_facts(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                panic!("expression contract")
            };
            let ExpressionNode::Binary(binary) = expressions.expression(*expression) else {
                panic!("comparison contract")
            };
            for operand in [binary.left, binary.right] {
                if let ExpressionNode::Name(path) = expressions.expression(operand)
                    && expressions.name_path_members(path.members)[0].as_str() == "N"
                {
                    assert_name_symbol(program, operand, expected);
                    observed += 1;
                }
            }
        }
    }
    assert!(observed > 0, "contract must exercise its const binder");
}

#[test]
fn ordinary_const_values_retain_each_free_and_attached_machines_exact_binder() {
    let program = resolve(
        "data Main {}
         data Config { count: u8; }
         machine first<const N: u64>() -> u64 requires N <= 3; ensures result == N {
             let observed: u64 = N;
             observed
         }
         machine Main::second<const N: bool>(&self) -> bool ensures result == N {
             let observed: bool = N;
             observed
         }
         machine structured<const N: Config>() -> Config {
             let observed: Config = N;
             observed
         }",
    );
    let mut binders = Vec::new();
    for machine in program.machines.iter() {
        let [binder] = program.machine_type_parameters(machine) else {
            panic!("one const parameter")
        };
        assert!(binder.symbol.is_valid());
        assert_eq!(program.symbols.get(binder.symbol).parent, machine.symbol);
        assert!(!binders.contains(&binder.symbol));
        binders.push(binder.symbol);
        let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
        assert_name_symbol(
            &program,
            local(&program, state, "observed").initial_value,
            binder.symbol,
        );
        let contracts = program.machine_contracts(machine);
        if !contracts.is_empty() {
            assert_contract_const_symbols(&program, contracts, binder.symbol);
        }
    }
    assert_eq!(binders.len(), 3);
}

#[test]
fn a_local_shadows_a_const_only_after_its_initializer_and_outside_contracts() {
    let program = resolve(
        "machine value<const N: u64>() -> u64 ensures result == N {
             let before: u64 = N;
             let N: u64 = N;
             let after: u64 = N;
             after
         }",
    );
    let machine = &program.machines[0];
    let binder = program.machine_type_parameters(machine)[0].symbol;
    let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
    let shadow = local(&program, state, "N");
    assert!(binder.is_valid());
    assert!(shadow.symbol.is_valid());
    assert_ne!(shadow.symbol, binder);
    assert_name_symbol(
        &program,
        local(&program, state, "before").initial_value,
        binder,
    );
    assert_name_symbol(&program, shadow.initial_value, binder);
    assert_name_symbol(
        &program,
        local(&program, state, "after").initial_value,
        shadow.symbol,
    );
    assert_contract_const_symbols(&program, program.machine_contracts(machine), binder);
}

#[test]
fn named_states_share_const_binders_but_keep_their_own_parameter_shadowing() {
    let program = resolve(
        "machine value<const N: u64>() -> u64 {
             let N: u64 = 7;
             transition { _ -> generic() }
             state generic() -> u64 requires N <= 3 {
                 let observed: u64 = N;
                 observed
             }
             state shadow(N: u64) -> u64 requires N <= 3 {
                 let observed: u64 = N;
                 observed
             }
         }",
    );
    let machine = &program.machines[0];
    let binder = program.machine_type_parameters(machine)[0].symbol;
    let states = program.machine_state_handles(machine.states);
    let generic = program.machine_state(states[1]);
    let shadow = program.machine_state(states[2]);
    let parameter = program.state_parameters(shadow.parameters)[0].symbol;
    assert!(binder.is_valid());
    assert!(parameter.is_valid());
    assert_ne!(parameter, binder);
    for (state, expected) in [(generic, binder), (shadow, parameter)] {
        assert_name_symbol(
            &program,
            local(&program, state, "observed").initial_value,
            expected,
        );
        assert_contract_const_symbols(
            &program,
            program.signature_contracts(state.contracts),
            expected,
        );
    }
}

#[test]
fn ordinary_value_lookup_does_not_admit_type_or_machine_generic_parameters() {
    let program = resolve(
        "machine value<Element, machine Selected>() -> u64
         where machine Selected() -> u64;
         {
             let type_value: u64 = Element;
             let machine_value: u64 = Selected;
             0
         }",
    );
    let machine = &program.machines[0];
    assert!(
        program
            .machine_type_parameters(machine)
            .iter()
            .all(|binder| binder.symbol.is_valid())
    );
    let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
    for name in ["type_value", "machine_value"] {
        assert_name_symbol(
            &program,
            local(&program, state, name).initial_value,
            SymbolHandle::invalid(),
        );
    }
}
