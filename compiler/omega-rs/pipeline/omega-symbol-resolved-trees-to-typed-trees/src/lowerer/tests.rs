use super::lower_symbol_resolved_trees;
use omega_source_files_to_tokens::Lexer;
use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use omega_tokens_to_syntax_trees::parse_syntax_trees;

#[test]
fn lowers_dungeon_style_machine_program() {
    let source = r#"
    data Inventory {
        gold: u32[exact];
    }

    machine Inventory::clear {
        pub entry(&mut self, inventory: &mut Inventory) {
            inventory.gold = 0;
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");

    assert_eq!(typed_trees.data_definitions().len(), 1);
    assert_eq!(typed_trees.machines().len(), 1);
    assert_eq!(
        typed_trees.machine_states(&typed_trees.machines()[0]).len(),
        1
    );
    assert!(
        typed_trees
            .symbols
            .find_child_by_name(typed_trees.symbols.root(), "u32")
            .is_some()
    );
}

#[test]
fn lowers_slice_range_surface_into_typed_trees() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) -> usize {
        let values: [usize; 4] = [1, 2, 3, 4];
        let view: &[usize] = values.as_slice();
        let tail: &[usize] = view[1..];
        tail.len
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("typed lowering should succeed");

    assert!(
        typed_trees
            .machines()
            .first()
            .is_some_and(|machine| !typed_trees.machine_states(machine).is_empty())
    );
}

#[test]
fn preserves_structural_recast_targets_through_typed_lowering() {
    let source = r#"
    machine inspect(bytes: [u8; 4]) {
        let fixed: &[u8; 4] = &bytes as &[u8; 4];
        let slice: &[u8] = &bytes as &[u8];
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("typed lowering should succeed");
    let machine = &typed_trees.machines()[0];
    let state = &typed_trees.machine_states(machine)[0];
    let locals = typed_trees
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            omega_typed_trees::statement::StatementNode::LocalData(local) => Some(local),
            _ => None,
        })
        .collect::<Vec<_>>();

    let omega_typed_trees::expression::ExpressionNode::Cast(fixed) = typed_trees
        .expression_table
        .expression(locals[0].initial_value)
    else {
        panic!("fixed-array initializer should remain a cast");
    };
    assert!(matches!(
        typed_trees
            .type_reference_table
            .type_reference(fixed.target_type),
        omega_typed_trees::types::TypeReferenceNode::FixedArray {
            length: omega_typed_trees::types::FixedArrayLength::Literal(4),
            ..
        }
    ));

    let omega_typed_trees::expression::ExpressionNode::Cast(slice) = typed_trees
        .expression_table
        .expression(locals[1].initial_value)
    else {
        panic!("slice initializer should remain a cast");
    };
    assert!(matches!(
        typed_trees
            .type_reference_table
            .type_reference(slice.target_type),
        omega_typed_trees::types::TypeReferenceNode::Slice { .. }
    ));
}

#[test]
fn lowers_domain_definitions() {
    let source = r#"
    domain Player::Valid {
        self.health >= 0
    }

    domain Player::Alive {
        self in Player::Valid;
        self.health > 0
    }

    domain Player::Tagged {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");

    assert_eq!(typed_trees.domain_definitions().len(), 3);
    let domain = typed_trees
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Player::Alive")
        .expect("alive domain should lower");
    assert!(domain.symbol.is_valid());
    assert_eq!(domain.name.as_str(), "Player::Alive");
    let facts = typed_trees.proof_facts(domain);
    assert_eq!(facts.len(), 2);
    let omega_typed_trees::domain::ProofFact::Membership(membership) = &facts[0] else {
        panic!("first domain fact should be membership")
    };
    assert!(membership.domain_symbol.is_valid());
    assert!(domain.body_token_count >= 3);
    assert!(domain.target_type.is_valid());
    let resolved_domain = resolved_program
        .domain_definitions
        .iter()
        .find(|candidate| candidate.name.as_str() == "Player::Alive")
        .expect("resolved alive domain");
    assert_eq!(domain.facets, resolved_domain.facets);
    assert!(domain.facets.predicate);
    assert_eq!(domain.facets.semantic, Some(domain.semantic_id));
    let tagged = typed_trees
        .domain_definitions()
        .iter()
        .find(|candidate| candidate.name.as_str() == "Player::Tagged")
        .expect("typed tagged domain");
    assert!(!tagged.facets.predicate);
    assert_eq!(tagged.facets.semantic, Some(tagged.semantic_id));
}

#[test]
fn normalizes_domain_constraints_by_short_name_and_carrier() {
    let source = r#"
    data Box<T> {
        value: T;
    }

    data Holder {
        signed: i64 in Tagged;
        unsigned: u64 in Tagged;
        boxed_signed: Box<i64 in Tagged>;
    }

    domain i64::Tagged {
    }

    domain u64::Tagged {
        self >= 0;
    }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");

    let signed_domain = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "i64::Tagged")
        .expect("signed domain");
    let unsigned_domain = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "u64::Tagged")
        .expect("unsigned domain");
    assert!(!signed_domain.facets.predicate);
    assert!(unsigned_domain.facets.predicate);

    let holder = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Holder")
        .expect("Holder");
    let fields = typed
        .data_members(holder)
        .iter()
        .filter_map(|member| match member {
            omega_typed_trees::data::DataMember::Field(field) => {
                Some((field.name.as_str(), field.type_reference))
            }
            omega_typed_trees::data::DataMember::Variant(_) => None,
        })
        .collect::<std::collections::HashMap<_, _>>();

    let constraint_for = |type_reference| {
        let omega_typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } =
            typed.type_reference_table.type_reference(type_reference)
        else {
            panic!("constrained field")
        };
        let [omega_typed_trees::types::TypeConstraintNode::Domain(domain)] =
            typed.type_reference_table.constraints(*constraints)
        else {
            panic!("one domain constraint")
        };
        domain
    };

    let signed = constraint_for(fields["signed"]);
    assert_eq!(signed.symbol, signed_domain.symbol);
    assert_eq!(signed.semantic_id, signed_domain.semantic_id);
    assert_eq!(signed.facets, signed_domain.facets);

    let unsigned = constraint_for(fields["unsigned"]);
    assert_eq!(unsigned.symbol, unsigned_domain.symbol);
    assert_eq!(unsigned.semantic_id, unsigned_domain.semantic_id);
    assert_eq!(unsigned.facets, unsigned_domain.facets);

    let omega_typed_trees::types::TypeReferenceNode::Generic { arguments, .. } = typed
        .type_reference_table
        .type_reference(fields["boxed_signed"])
    else {
        panic!("generic field")
    };
    let [argument] = typed
        .type_reference_table
        .type_reference_handles(*arguments)
    else {
        panic!("one generic argument")
    };
    let boxed_signed = constraint_for(*argument);
    assert_eq!(boxed_signed.symbol, signed_domain.symbol);
    assert_eq!(boxed_signed.facets, signed_domain.facets);
}

#[test]
fn parameter_domain_conjunction_synthesizes_each_predicate_contract_only() {
    let source = r#"
    domain [u8]::Meaning {}
    domain [u8]::Utf8 { valid_utf8(self); }
    domain [u8]::NoNul { no_nul(self); }

    machine inspect(bytes: &[u8] in Meaning & Utf8 & NoNul) {
    }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = typed.machines().first().expect("inspect machine");
    let names: Vec<_> = typed
        .machine_contracts(machine)
        .iter()
        .map(|contract| {
            let [omega_typed_trees::domain::ProofFact::Membership(membership)] =
                typed.proof_facts.span_or_empty(contract.facts)
            else {
                panic!("one synthesized membership fact")
            };
            typed
                .domain_definitions()
                .iter()
                .find(|domain| domain.symbol == membership.domain_symbol)
                .expect("normalized predicate domain")
                .name
                .as_str()
                .to_owned()
        })
        .collect();

    assert_eq!(names, ["[u8]::Utf8", "[u8]::NoNul"]);
}

#[test]
fn preserves_operator_declarations() {
    let source = r#"
    operator Slice::index<T>(items: &[T], index: usize) -> T
    requires
        index < items.len;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");

    assert_eq!(typed_trees.operators().len(), 1);
    let operator = &typed_trees.operators()[0];
    assert_eq!(
        typed_trees
            .operator_path_members(operator.name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["Slice", "index"]
    );
    assert_eq!(
        typed_trees
            .data_type_parameters
            .span_or_empty(operator.type_parameters)
            .len(),
        1
    );
    assert_eq!(
        typed_trees
            .state_parameters
            .span_or_empty(operator.parameters)
            .len(),
        2
    );
    assert!(operator.symbol.is_valid());
    assert!(operator.return_type.is_valid());
    assert_eq!(
        typed_trees
            .signature_contracts
            .span_or_empty(operator.contracts)
            .len(),
        1
    );
    assert!(operator.token_count > 0);
}

#[test]
fn preserves_domain_operator_declarations() {
    let source = r#"
    data Quantity {
        value: i32;
    }

    domain Quantity::Additive {
        self.value >= 0;

        operator add(left: Quantity, right: Quantity) -> Quantity;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");
    let domain = typed_trees
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Quantity::Additive")
        .expect("domain should lower");
    let operators = typed_trees.domain_operators(domain);

    assert_eq!(operators.len(), 1);
    assert!(operators[0].symbol.is_valid());
    assert_eq!(
        typed_trees
            .operator_path_members(operators[0].name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["add"]
    );
    assert_eq!(typed_trees.proof_facts(domain).len(), 1);
}

#[test]
fn lowers_machine_contract_clauses() {
    let source = r#"
    machine distinct_indices(i: usize, j: usize)
    requires
        i < j
    ensures
        i != j
    {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");
    let machine = typed_trees.machines().first().expect("machine");
    let contracts = typed_trees.machine_contracts(machine);

    assert_eq!(contracts.len(), 2);
    assert!(contracts[0].token_count >= 3);
    assert!(contracts[1].token_count >= 3);
    assert_eq!(
        typed_trees
            .proof_facts
            .span_or_empty(contracts[0].facts)
            .len(),
        1
    );
    assert_eq!(
        typed_trees
            .proof_facts
            .span_or_empty(contracts[1].facts)
            .len(),
        1
    );
}

#[test]
fn lowers_statement_argument_spans_from_statement_table() {
    let source = r#"
    data Parser {}

    machine Parser::start(&mut self, level: i32, cell: i32, line: i32) -> i32 {
        transition {
            _ -> self.resolve_exit(level, cell, line)
        }

        state resolve_exit(&mut self, level: i32, cell: i32, line: i32) -> i32 {
            0
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");
    let machine = &typed_trees.machines()[0];
    let entry = &typed_trees.machine_states(machine)[0];
    let statements = typed_trees
        .statement_table
        .statements(entry.statement_nodes);

    let omega_typed_trees::statement::StatementNode::Transition(transition) = &statements[0] else {
        panic!("entry should lower to transition statement");
    };
    let omega_typed_trees::statement::TransitionTargetNode::Named { arguments, .. } = typed_trees
        .statement_table
        .transition_target(transition.target)
    else {
        panic!("transition target should be named");
    };
    let arguments = typed_trees.statement_table.expression_handles(*arguments);
    let argument_names = arguments
        .iter()
        .map(|argument| typed_trees.expression_table.display_name(*argument))
        .collect::<Vec<_>>();

    assert_eq!(argument_names, ["level", "cell", "line"]);
}

#[test]
fn preserves_linear_multiplicity_through_typed_lowering() {
    let source = r#"
        data Token [linear] {}
        data Holder<T [linear]> [linear] { token: T; }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");

    for definition in typed_trees.data_definitions() {
        assert_eq!(
            definition.properties.multiplicity,
            omega_core::semantics::Multiplicity::Linear
        );
        assert!(!definition.properties.copy);
    }
    let holder = &typed_trees.data_definitions()[1];
    assert_eq!(
        typed_trees.data_type_parameters(holder)[0]
            .bounds
            .multiplicity,
        omega_core::semantics::Multiplicity::Linear
    );
}
