use super::typed_source;
use crate::CheckingRequest;
use crate::tests::{Lexer, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees};
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};

#[test]
fn specialization_identity_ignores_selected_machine_body_edits() {
    fn report_fingerprint(body: &str) -> u64 {
        let source = format!(
            r#"
                data Main {{}}
                machine selected(value: &i32) {{ {body} }}
                machine apply<T, machine F>(value: &T)
                where machine F(item: &T)
                {{ F(value); }}
                machine caller(value: &i32) {{ apply<selected>(value); }}
                machine Main::run(&mut self) {{}}
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved =
            resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        lower_typed_trees(typed, &CheckingRequest::settled())
            .expect("specialization should check")
            .machine_specializations[0]
            .report_fingerprint
    }

    assert_eq!(
        report_fingerprint(""),
        report_fingerprint("let one: i32 = 1;")
    );
}

#[test]
fn generic_template_identity_is_positional_across_parameter_renames() {
    fn identity(
        machine_parameter: &str,
        value: &str,
        item: &str,
    ) -> (u64, typed_trees::typed_trees::MachineTemplateCommitment) {
        let source = format!(
            r#"
                boundary machine admitted<machine {machine_parameter}>({value}: i32)
                where machine {machine_parameter}({item}: i32)
                    requires {item} > 0;
                requires {value} > 0;
                ensures true;
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved =
            resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        let admitted = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "admitted")
            .expect("accepted template should exist");
        (
            crate::monomorphization::generic_machine_template_report_fingerprint(
                &typed,
                admitted.symbol,
            )
            .expect("generic template should have a report identity"),
            crate::monomorphization::generic_machine_template_commitment(&typed, admitted.symbol)
                .expect("generic template should have a strong identity"),
        )
    }

    assert_eq!(
        identity("F", "value", "item"),
        identity("Operation", "input", "candidate")
    );
}

#[test]
fn generic_template_identity_normalizes_crash_route_buckets() {
    fn report_fingerprint(crash_clauses: &str) -> u64 {
        let source = format!(
            r#"
                boundary machine admitted<T>(first: bool, second: bool)
                {crash_clauses}
                ensures true;
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved =
            resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        let admitted = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "admitted")
            .expect("generic template should exist");
        crate::monomorphization::generic_machine_template_report_fingerprint(
            &typed,
            admitted.symbol,
        )
        .expect("generic template should have an identity")
    }

    let grouped = r#"
        crashes Trap
            first
            second
    "#;
    let split = r#"
        crashes Trap
            second
        crashes Trap
            first
    "#;
    let duplicated = r#"
        crashes Trap
            first
            second
            first
    "#;
    let unconditional = r#"
        crashes Trap
    "#;
    let explicit_true = r#"
        crashes Trap
            true
    "#;
    let unconditional_with_guard = r#"
        crashes Trap
            first
        crashes Trap
    "#;

    assert_eq!(report_fingerprint(grouped), report_fingerprint(split));
    assert_eq!(report_fingerprint(grouped), report_fingerprint(duplicated));
    assert_eq!(
        report_fingerprint(unconditional),
        report_fingerprint(explicit_true)
    );
    assert_eq!(
        report_fingerprint(unconditional),
        report_fingerprint(unconditional_with_guard)
    );
    assert_ne!(
        report_fingerprint(grouped),
        report_fingerprint(unconditional)
    );

    fn slot_report_fingerprint(crash_clauses: &str) -> u64 {
        let source = format!(
            r#"
                boundary machine admitted<T, machine Operation>()
                where machine Operation(first: bool, second: bool)
                    {crash_clauses};
                ensures true;
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved =
            resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        let admitted = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "admitted")
            .expect("generic template should exist");
        crate::monomorphization::generic_machine_template_report_fingerprint(
            &typed,
            admitted.symbol,
        )
        .expect("generic template should have an identity")
    }

    assert_eq!(
        slot_report_fingerprint(grouped),
        slot_report_fingerprint(split)
    );
    assert_eq!(
        slot_report_fingerprint(grouped),
        slot_report_fingerprint(duplicated)
    );
    assert_eq!(
        slot_report_fingerprint(unconditional),
        slot_report_fingerprint(explicit_true)
    );
    assert_eq!(
        slot_report_fingerprint(unconditional),
        slot_report_fingerprint(unconditional_with_guard)
    );
}

#[test]
fn generic_template_identity_pins_conformance_bounds_positionally() {
    fn report_fingerprint(parameter: &str, trait_name: &str) -> u64 {
        let source = format!(
            r#"
                trait First {{ machine inspect(value: &Self); }}
                trait Second {{ machine inspect(value: &Self); }}
                machine admitted<{parameter}>(value: &{parameter})
                where {parameter} satisfies {trait_name}
                {{}}
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved =
            resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        let admitted = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "admitted")
            .expect("generic template should exist");
        crate::monomorphization::generic_machine_template_report_fingerprint(
            &typed,
            admitted.symbol,
        )
        .expect("generic template should have an identity")
    }

    assert_eq!(
        report_fingerprint("T", "First"),
        report_fingerprint("Item", "First")
    );
    assert_ne!(
        report_fingerprint("T", "First"),
        report_fingerprint("T", "Second")
    );
}

#[test]
fn generic_template_identity_pins_selected_open_index_operation_authority() {
    fn report_fingerprint(operator_namespace: &str) -> u64 {
        let source = format!(
            r#"
                domain<T, const I: u64> T::Indexed<I>;

                trait {operator_namespace}<Operand> {{
                    operator + add(a: Operand, b: Operand) -> Operand;
                    machine add_comm(a: Operand, b: Operand)
                    ensures add(a, b) == add(b, a);
                    machine add_assoc(a: Operand, b: Operand, c: Operand)
                    ensures add(add(a, b), c) == add(a, add(b, c));
                }}

                machine plus_index(a: u64, b: u64) -> u64
                satisfies {operator_namespace}<u64>::add as Canonical
                {{ 0 }}

                machine plus_index_comm(a: u64, b: u64) -> u64
                satisfies {operator_namespace}<u64>::add_comm as Canonical
                requires plus_index(a, b) == plus_index(b, a)
                ensures plus_index(a, b) == plus_index(b, a)
                {{ 0 }}

                machine plus_index_assoc(a: u64, b: u64, c: u64) -> u64
                satisfies {operator_namespace}<u64>::add_assoc as Canonical
                requires plus_index(plus_index(a, b), c) == plus_index(a, plus_index(b, c))
                ensures plus_index(plus_index(a, b), c) == plus_index(a, plus_index(b, c))
                {{ 0 }}

                boundary machine admitted<T, const A: u64, const B: u64, Canonical: T satisfies {operator_namespace}<u64>>()
                    -> i64 in Indexed<A + B>;
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved =
            resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
        let mut typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        validation::normalize_open_index_expressions(&mut typed)
            .expect("exact proved index algebra should normalize");
        crate::monomorphization::refresh_closed_domain_instance_identities(&mut typed)
            .expect("selected authority should refresh indexed instance identity");
        let admitted = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "admitted")
            .expect("generic template should exist");
        crate::monomorphization::generic_machine_template_report_fingerprint(
            &typed,
            admitted.symbol,
        )
        .expect("generic template should have an identity")
    }

    assert_ne!(
        report_fingerprint("IndexAlgebra"),
        report_fingerprint("AlternateIndexAlgebra")
    );
}

#[test]
fn generic_template_identity_pins_independent_operational_interfaces() {
    fn report_fingerprint(source: String) -> u64 {
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens)
            .unwrap_or_else(|error| panic!("parse should succeed for `{source}`: {error:?}"));
        let resolved =
            resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        let admitted = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "admitted")
            .expect("generic template should exist");
        crate::monomorphization::generic_machine_template_report_fingerprint(
            &typed,
            admitted.symbol,
        )
        .expect("generic template should have an identity")
    }

    fn template_report_fingerprint(template_clause: &str) -> u64 {
        let source = format!(
            r#"
                boundary machine admitted<T>(value: &T) {template_clause}
                ensures true;
            "#
        );
        report_fingerprint(source)
    }

    fn slot_report_fingerprint(slot_clause: &str) -> u64 {
        report_fingerprint(format!(
            r#"
                boundary machine admitted<T, machine F>(value: &T)
                where machine F(item: &T) {slot_clause}
                ensures true;
            "#
        ))
    }

    let template_base = template_report_fingerprint("");
    assert_ne!(template_base, template_report_fingerprint("suspends;"));
    assert_ne!(template_base, template_report_fingerprint("blocks;"));

    let slot_base = slot_report_fingerprint(";");
    assert_ne!(slot_base, slot_report_fingerprint("suspends;"));
    assert_ne!(slot_base, slot_report_fingerprint("blocks;"));
    assert_ne!(
        slot_report_fingerprint("suspends;"),
        slot_report_fingerprint("blocks;")
    );

    fn reach_report_fingerprint(reach: &str) -> u64 {
        report_fingerprint(format!(
            r#"
                boundary trait Readable {{}}
                boundary trait Filesystem: Readable {{}}

                boundary machine admitted<T>(value: &T)
                reaches {reach}
                ensures true;
            "#
        ))
    }

    assert_eq!(
        reach_report_fingerprint("Filesystem"),
        reach_report_fingerprint("Filesystem + Readable"),
        "template identity must consume the normalized service row, including parent closure"
    );

    fn slot_reach_report_fingerprint(reach: &str) -> u64 {
        report_fingerprint(format!(
            r#"
                boundary trait Readable {{}}
                boundary trait Filesystem: Readable {{}}

                boundary machine admitted<T, machine F>(value: &T)
                where machine F(item: &T) reaches {reach};
                ensures true;
            "#
        ))
    }

    assert_eq!(
        slot_reach_report_fingerprint("Filesystem"),
        slot_reach_report_fingerprint("Filesystem + Readable"),
        "machine-parameter identity must consume the normalized service row"
    );
}

#[test]
fn generic_template_identity_distinguishes_structural_and_nominal_machine_contracts() {
    fn report_fingerprint(contract: &str) -> u64 {
        let source = format!(
            r#"
                trait Handler {{
                    machine call(value: i32) -> i32;
                }}

                machine admitted<machine F>(value: i32) -> i32
                {contract}
                {{
                    0
                }}
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved =
            resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        let admitted = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "admitted")
            .expect("generic template should exist");
        crate::monomorphization::generic_machine_template_report_fingerprint(
            &typed,
            admitted.symbol,
        )
        .expect("generic template should have an identity")
    }

    assert_ne!(
        report_fingerprint("where machine F(value: i32) -> i32;"),
        report_fingerprint("where machine F satisfies Handler::call;")
    );
}

#[test]
fn consuming_seq_map_specializes_recursive_machine_parameter_calls() {
    let source = r#"
        data Seq<T> {
            case Empty;
            case Cons(head: T, tail: Seq<T>);
        }
        data Main {}

        // This fixture checks generic specialization, not an input upper bound.
        machine increment(value: u64) -> u64 {
            ((value as u64 in Wrapping) + 1) as u64
        }

        machine map<T, U, machine F>(items: Seq<T>) -> Seq<U>
        where machine F(value: T) -> U;
        terminates by items;
        {
            transition items {
                Seq::Empty -> Seq::Empty
                Seq::Cons { head, tail } -> Seq::Cons {
                    head: F(head),
                    tail: map<F>(tail)
                }
            }
        }

        machine caller(items: Seq<u64>) -> Seq<u64> {
            map<increment>(items)
        }
        machine Main::run(&mut self) {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("consuming Seq map should specialize");

    let map_instances: Vec<_> = checked
        .machine_specializations
        .iter()
        .filter(|specialization| {
            checked.machines().iter().any(|machine| {
                machine.symbol == specialization.template && machine.name.as_str() == "map"
            })
        })
        .collect();
    assert_eq!(map_instances.len(), 1);
    assert_eq!(map_instances[0].type_arguments, ["u64", "u64"]);
    assert_eq!(map_instances[0].machine_arguments.len(), 1);
}

#[test]
fn unused_recursive_generic_value_template_is_not_emitted_or_fenced() {
    let source = r#"
        data Seq<T> {
            case Empty;
            case Cons(head: T, tail: Seq<T>);
        }
        data Main {}

        machine map<T, U, machine F>(items: Seq<T>) -> Seq<U>
        where machine F(value: T) -> U;
        terminates by items;
        {
            transition items {
                Seq::Empty -> Seq::Empty
                Seq::Cons { head, tail } -> Seq::Cons {
                    head: F(head),
                    tail: map<F>(tail)
                }
            }
        }

        machine Main::run(&mut self) {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("unused generic template should remain legal");
    let map = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "map")
        .expect("generic template should remain in checked semantic data");
    assert!(!checked.machine_type_parameters(map).is_empty());
}

#[test]
fn const_generic_template_is_not_consumed_by_machine_specialization() {
    let source = r#"
        data Unit {}
        domain<T, const U: Unit> T::Quantity<U>;

        machine retag<const To: Unit>(value: i64) -> i64 in Quantity<To> {
            transition { _ -> (value as i64 in Quantity<To>) }
        }

        data Main {}
        machine Main::run(&mut self) {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("const-generic template should validate");
    let retag = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "retag")
        .expect("retag template should remain in checked semantic data");
    let [parameter] = checked.machine_type_parameters(retag) else {
        panic!("retag should retain its const binder");
    };
    assert!(matches!(
        parameter.kind,
        typed_trees::data::TypeParameterKind::Const { .. }
    ));
}

#[test]
fn const_generic_result_indices_produce_distinct_concrete_machine_instances() {
    let source = r#"
        domain<T, const U: u64> T::Quantity<U>;

        machine retag<const To: u64>(value: i64) -> i64 in Quantity<To> {
            transition { _ -> (value as i64 in Quantity<To>) }
        }

        data Main {}
        machine Main::run(&mut self) {
            let first: i64 in Quantity<1> = retag(70);
            let second: i64 in Quantity<2> = retag(70);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let mut typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    crate::specialize_static_machine_calls(&mut typed)
        .expect("const result indices should specialize before validation");
    let concrete_return_domains = typed
        .machine_specializations
        .iter()
        .map(|specialization| {
            let machine = typed
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.instance)
                .expect("specialized machine");
            let return_type = typed
                .machine_states(machine)
                .first()
                .expect("entry state")
                .return_type;
            let typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } =
                typed.type_reference_table.type_reference(return_type)
            else {
                panic!("specialized return should be constrained");
            };
            let [typed_trees::types::TypeConstraintNode::Domain(domain)] =
                typed.type_reference_table.constraints(*constraints)
            else {
                panic!("specialized return should carry Quantity");
            };
            typed
                .semantic_domains
                .name(domain.semantic_id)
                .expect("indexed semantic identity")
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        concrete_return_domains,
        ["Quantity<integer:u64:1>", "Quantity<integer:u64:2>"]
    );
    let cast_domains = typed
        .expression_table
        .iter_expressions()
        .filter_map(|(_, expression)| {
            let typed_trees::expression::ExpressionNode::Cast(cast) = expression else {
                return None;
            };
            typed
                .semantic_domains
                .name(cast.semantic_domain_id)
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        cast_domains,
        [
            "Quantity<binder:retag::To:u64>",
            "Quantity<integer:u64:1>",
            "Quantity<integer:u64:2>"
        ]
    );
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("const result indices should specialize");

    let retag = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "retag")
        .expect("retag template instance");
    assert_eq!(checked.machine_type_parameters(retag).len(), 1);
    let specializations = checked
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.template == retag.symbol)
        .collect::<Vec<_>>();
    assert_eq!(specializations.len(), 2);
    assert_ne!(specializations[0].instance, specializations[1].instance);
    assert_eq!(specializations[0].const_arguments, ["1"]);
    assert_eq!(specializations[1].const_arguments, ["2"]);
    assert_eq!(
        specializations[0].const_argument_identities,
        ["named(integer-const(1))"]
    );
    assert_eq!(
        specializations[1].const_argument_identities,
        ["named(integer-const(2))"]
    );
    assert_ne!(
        specializations[0].report_fingerprint,
        specializations[1].report_fingerprint
    );

    for specialization in specializations {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == specialization.instance)
            .expect("concrete retag instance");
        let state = checked
            .machine_states(machine)
            .first()
            .expect("retag entry state");
        let typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } = checked
            .type_reference_table
            .type_reference(state.return_type)
        else {
            panic!("specialized retag result should remain constrained");
        };
        let [typed_trees::types::TypeConstraintNode::Domain(result_domain)] =
            checked.type_reference_table.constraints(*constraints)
        else {
            panic!("specialized retag result should carry Quantity");
        };
        let cast_id = checked
            .expression_table
            .iter_expressions()
            .filter_map(|(_, expression)| {
                let typed_trees::expression::ExpressionNode::Cast(cast) = expression else {
                    return None;
                };
                (cast.semantic_domain_symbol == result_domain.symbol
                    && cast.semantic_domain_id == result_domain.semantic_id)
                    .then_some(cast.semantic_domain_id)
            })
            .find(|identity| *identity == result_domain.semantic_id);
        assert_eq!(cast_id, Some(result_domain.semantic_id));
    }
}

#[test]
fn contract_only_static_selections_do_not_consume_generic_machine_schema() {
    let source = r#"
        pub data Index { case Zero; }
        data Stream<machine S>
        where machine S(index: Index) -> Index;
        { case Empty; case More(tail: Stream<S>); }
        data Main {}

        boundary machine source(index: Index) -> Index;

        machine equivalent<machine A, machine B>(a: Stream<A>, b: Stream<B>) -> bool
        where machine A(index: Index) -> Index;
        where machine B(index: Index) -> Index;
        {
            transition { _ -> (true) }
        }

        machine reflexive<machine A>(a: Stream<A>)
        where machine A(index: Index) -> Index;
        ensures equivalent<A, A>(a, a)
        {
        }

        machine concrete_premise(a: Stream<source>)
        requires equivalent<source, source>(a, a)
        {
        }

        machine Main::run(&mut self) {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("contract schemas should remain generic");

    let equivalent = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "equivalent")
        .expect("generic relation schema");
    assert_eq!(checked.machine_type_parameters(equivalent).len(), 2);
    assert!(
        checked
            .machine_specializations
            .iter()
            .all(|specialization| specialization.template != equivalent.symbol)
    );
}

#[test]
fn generic_member_borrows_use_the_receivers_exact_type_arguments() {
    for (member, accepted) in [("first", true), ("second", false)] {
        let source = format!(
            r#"
            data Pair<Left, Right> {{ first: Left; second: Right; }}
            machine inspect<Left, Right, machine Visit>(pair: Pair<Left, Right>)
            where machine Visit(value: &Left);
            {{ Visit(pair.{member}); }}
        "#
        );
        let typed = typed_source(&source).expect("generic member source types");
        let result = lower_typed_trees(typed, &CheckingRequest::settled());
        assert_eq!(result.is_ok(), accepted, "{source}: {result:?}");
    }
}

#[test]
fn consuming_seq_filter_borrows_each_value_before_preserving_or_dropping_it() {
    let source = r#"
        data Seq<T> {
            case Empty;
            case Cons(head: T, tail: Seq<T>);
        }
        data Main {}

        machine positive(value: &i32) -> bool { value > 0 }

        machine filter<T, machine Predicate>(items: Seq<T>) -> Seq<T>
        where machine Predicate(value: &T) -> bool;
        terminates by items;
        {
            transition items {
                Seq::Empty -> Seq::Empty
                Seq::Cons { head, tail } -> choose(
                    Predicate(head),
                    head,
                    filter<Predicate>(tail)
                )
            }

            state choose(keep: bool, head: T, tail: Seq<T>) -> Seq<T> {
                transition keep {
                    true -> Seq::Cons { head: head, tail: tail }
                    false -> tail
                }
            }
        }

        machine caller(items: Seq<i32>) -> Seq<i32> {
            filter<positive>(items)
        }
        machine Main::run(&mut self) {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("consuming Seq filter should specialize");
}
