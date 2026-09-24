use source::SourceMap;
use source_files_to_tokens::Lexer;
use std::{path::PathBuf, sync::Arc};
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::data::DataMember;
use symbol_resolved_trees::expression::ExpressionNode;
use symbol_resolved_trees::types::TypeReference;
use syntax_trees::SyntaxTrees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::{parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};

#[test]
fn declared_module_retains_namespace_for_local_references() {
    assert_module_reference_symbols(
        r#"
        module dungeon::combat;

        data Damage { amount: u64; }
        data Attack { local: Damage; }

        machine damage() -> u64 { 1 }
        machine local_attack() -> u64 { damage() }
        "#,
        1,
        1,
    );
}

#[test]
fn declared_module_qualified_and_local_references_share_exact_symbols() {
    assert_module_reference_symbols(
        r#"
        module dungeon::combat;

        data Damage { amount: u64; }
        data Attack {
            local: Damage;
            qualified: dungeon::combat::Damage;
        }

        machine damage() -> u64 { 1 }
        machine local_attack() -> u64 { damage() }
        machine qualified_attack() -> u64 { dungeon::combat::damage() }
        "#,
        2,
        2,
    );
}

fn assert_module_reference_symbols(
    source: &str,
    expected_field_count: usize,
    expected_call_count: usize,
) {
    let mut sources = SourceMap::default();
    let source_id = sources
        .add(PathBuf::from("combat.omg"), source.to_owned())
        .source_id;
    let tokens = Lexer::new(source).tokenize().expect("tokenize module");
    let syntax = parse_syntax_trees_with_id(source_id, &tokens).expect("parse module");
    let program = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve declared module references");

    let damage = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Damage")
        .expect("Damage declaration");
    assert_eq!(
        program.symbols.display_path(damage.symbol, "::"),
        "dungeon::combat::Damage"
    );
    let attack = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Attack")
        .expect("Attack declaration");
    let fields = program.data_members(attack.members);
    assert_eq!(fields.len(), expected_field_count);
    for field in fields {
        let DataMember::Field(field) = field else {
            panic!("Attack contains only fields");
        };
        let TypeReference::Named { symbol, .. } = &field.type_reference else {
            panic!("Damage field retains nominal identity");
        };
        assert_eq!(*symbol, damage.symbol);
    }

    let damage_machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "damage")
        .expect("damage machine");
    assert_eq!(
        program.symbols.display_path(damage_machine.symbol, "::"),
        "dungeon::combat::damage"
    );
    let [entry] = program
        .tables
        .declarations
        .machine_state_handles
        .span_or_empty(damage_machine.states)
    else {
        panic!("one damage entry state");
    };
    let entry_symbol = program
        .tables
        .declarations
        .machine_states
        .get(*entry)
        .symbol;
    let mut call_count = 0;
    for (_, expression) in program.tables.bodies.expressions.iter_expressions() {
        if let ExpressionNode::Call(call) = expression {
            assert_eq!(call.target_symbol, entry_symbol);
            call_count += 1;
        }
    }
    assert_eq!(call_count, expected_call_count);
}

fn lower_multi(sources: &[(&str, &str)]) -> SymbolResolvedTrees {
    let mut map = SourceMap::default();
    let mut syntax = SyntaxTrees::default();
    for (name, text) in sources {
        let id = map.add(PathBuf::from(name), (*text).to_owned()).source_id;
        let tokens = Lexer::new(text).tokenize().expect("tokenize");
        parse_syntax_trees_into_with_id(&mut syntax, id, &tokens).expect("parse");
    }
    resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(map)),
        top_level_bindings: Vec::new(),
    })
    .expect("lower")
}

fn membership_domain_symbols(program: &SymbolResolvedTrees) -> Vec<symbols::SymbolHandle> {
    program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            ExpressionNode::Membership(membership) => Some(membership.domain_symbol),
            _ => None,
        })
        .collect()
}

#[test]
fn module_owned_domain_and_operator_home_retain_exact_namespace() {
    let mut program = lower_multi(&[(
        "units.omg",
        r#"
        module units;
        pub domain u64::Distance requires self > 0;
        pub operator + u64::Distance::add(left: u64 in u64::Distance, right: u64 in u64::Distance) -> u64 in u64::Distance;
        "#,
    )]);
    let qualified_id = program.semantic_domains.intern("units::u64::Distance");
    let domain = program
        .domain_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "u64::Distance")
        .expect("module domain");
    assert_eq!(
        program.symbols.display_path(domain.symbol, "::"),
        "units::u64::Distance"
    );
    assert_eq!(
        domain.semantic_id, qualified_id,
        "module-owned domains intern under their complete logical path"
    );
    let [operator] = program.operator_definitions(domain.operators) else {
        panic!("the operator homed into the module domain")
    };
    assert!(
        program.operators.is_empty(),
        "no root operator remains after domain homing"
    );
    assert_eq!(
        program.symbols.display_path(operator.symbol, "::"),
        "units::u64::Distance::add"
    );
}

/// Module-qualified operator calls select the operator declaration itself:
/// `units::copy<u64>` and `units::add` keep `SymbolKind::Operator` identity
/// for typed named-operator validation instead of rewriting to a machine
/// entry state, the receiver path retains each module segment's exact
/// symbol, and a same-spelled unmoduled operator keeps bare spellings under
/// the module name law. A missing member stays unresolved for downstream
/// diagnostics rather than silently selecting a sibling.
#[test]
fn module_qualified_operator_calls_select_the_operator_symbol() {
    let program = lower_multi(&[
        (
            "units.omg",
            "module units; pub operator copy<T>(value: T) -> T; pub operator add(left: u64, right: u64) -> u64;",
        ),
        (
            "main.omg",
            r#"
            use units;
            machine Main::main(&mut self) {
                let total: u64 = units::add(1u64, 2u64);
                let again: u64 = units::copy<u64>(total);
                let bare: u64 = copy<u64>(again);
                let missing: u64 = units::missing(total);
            }
            data Main {}
            pub operator copy<T>(value: T) -> T;
            "#,
        ),
    ]);

    let module_copy = program
        .operators
        .iter()
        .find(|operator| program.symbols.display_path(operator.symbol, "::") == "units::copy")
        .expect("module copy operator");
    let module_add = program
        .operators
        .iter()
        .find(|operator| program.symbols.display_path(operator.symbol, "::") == "units::add")
        .expect("module add operator");
    let root_copy = program
        .operators
        .iter()
        .find(|operator| program.symbols.display_path(operator.symbol, "::") == "copy")
        .expect("root copy operator");
    for operator in [module_copy, module_add, root_copy] {
        assert_eq!(
            program.symbols.get(operator.symbol).kind,
            symbols::SymbolKind::Operator,
            "module members keep their operator declaration identity"
        );
    }

    let calls = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            ExpressionNode::Call(call) => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    let selected = |target: &str| {
        calls
            .iter()
            .find(|call| call.target.as_str() == target)
            .unwrap_or_else(|| panic!("call {target}"))
            .target_symbol
    };
    assert_eq!(selected("units::add"), module_add.symbol);
    assert_eq!(selected("units::copy"), module_copy.symbol);
    assert_eq!(
        selected("copy"),
        root_copy.symbol,
        "a bare spelling selects the unmoduled operator, not the module sibling"
    );
    assert!(
        !selected("units::missing").is_valid(),
        "a missing module member keeps its checked obligation unresolved"
    );
    for call in &calls {
        if call.target.as_str() != "units::missing" {
            assert!(
                call.target_symbol.is_valid(),
                "resolved module operator call keeps an exact target symbol"
            );
        }
    }
}

#[test]
fn module_domain_membership_selects_relative_and_qualified_names() {
    let program = lower_multi(&[(
        "units.omg",
        r#"
        module units;
        pub domain u64::Distance requires self > 0;
        machine near(value: u64) -> bool { value in u64::Distance }
        machine far(value: u64) -> bool { value in units::u64::Distance }
        "#,
    )]);
    let domain = program
        .domain_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "u64::Distance")
        .expect("module domain");
    let selections = membership_domain_symbols(&program);
    assert_eq!(selections.len(), 2, "both memberships resolve");
    for selected in selections {
        assert_eq!(selected, domain.symbol);
    }
}

#[test]
fn foreign_module_domain_needs_exact_or_imported_spelling() {
    let unimported = lower_multi(&[
        (
            "units.omg",
            "module units; pub domain u64::Distance requires self > 0;",
        ),
        (
            "check.omg",
            "machine check(value: u64) -> bool { value in u64::Distance }",
        ),
    ]);
    let [selection] = membership_domain_symbols(&unimported)
        .try_into()
        .expect("one membership");
    assert!(
        !selection.is_valid(),
        "a relative spelling must not reach a foreign module's domain"
    );

    let qualified = lower_multi(&[
        (
            "units.omg",
            "module units; pub domain u64::Distance requires self > 0;",
        ),
        (
            "check.omg",
            "machine check(value: u64) -> bool { value in units::u64::Distance }",
        ),
    ]);
    let domain = qualified
        .domain_definitions
        .iter()
        .next()
        .expect("module domain");
    let [selection] = membership_domain_symbols(&qualified)
        .try_into()
        .expect("one membership");
    assert_eq!(
        selection, domain.symbol,
        "a fully qualified path selects the exact module domain"
    );
}

#[test]
fn module_domain_proof_facts_select_their_own_domain() {
    let program = lower_multi(&[(
        "units.omg",
        r#"
        module units;
        domain u64::Long requires self > 0;
        domain u64::Distance requires self in u64::Long;
        "#,
    )]);
    let long = program
        .domain_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "u64::Long")
        .expect("u64::Long domain");
    let domain = program
        .domain_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "u64::Distance")
        .expect("u64::Distance domain");
    let [fact] = program.proof_facts(domain.facts) else {
        panic!("one proof fact")
    };
    let symbol_resolved_trees::domain::ProofFact::Membership(membership) = fact else {
        panic!("membership fact")
    };
    assert_eq!(
        membership.domain_symbol, long.symbol,
        "a relative proof-fact path selects the same-module domain"
    );
}

fn fact_membership_domain_symbols(
    program: &SymbolResolvedTrees,
    facts: arena::HandleSpan<symbol_resolved_trees::domain::ProofFact>,
) -> Vec<symbols::SymbolHandle> {
    program
        .proof_facts(facts)
        .iter()
        .filter_map(|fact| match fact {
            symbol_resolved_trees::domain::ProofFact::Membership(membership) => {
                Some(membership.domain_symbol)
            }
            _ => None,
        })
        .collect()
}

fn domain_named<'a>(
    program: &'a SymbolResolvedTrees,
    name: &str,
) -> &'a symbol_resolved_trees::domain::DomainDefinition {
    program
        .domain_definitions
        .iter()
        .find(|definition| definition.name.as_str() == name)
        .unwrap_or_else(|| panic!("{name} domain"))
}

/// `use units;` imports the declaring module and exposes its directly
/// declared domains to the carrier-qualified spelling in both proof-fact and
/// executable membership positions.
#[test]
fn imported_module_exposes_carrier_qualified_domain_spelling() {
    let program = lower_multi(&[
        (
            "units.omg",
            "module units; pub domain u64::Long requires self > 0;",
        ),
        (
            "check.omg",
            r#"
            use units;
            domain u64::Near requires self in u64::Long;
            machine check(value: u64) -> bool { value in u64::Long }
            "#,
        ),
    ]);
    let long = domain_named(&program, "u64::Long");
    assert_eq!(
        program.symbols.display_path(long.symbol, "::"),
        "units::u64::Long"
    );
    let near = domain_named(&program, "u64::Near");
    let [fact] = fact_membership_domain_symbols(&program, near.facts)
        .try_into()
        .expect("one fact membership");
    assert_eq!(
        fact, long.symbol,
        "a declaring-module import exposes the carrier-qualified proof fact"
    );
    let [expression] = membership_domain_symbols(&program)
        .try_into()
        .expect("one expression membership");
    assert_eq!(
        expression, long.symbol,
        "a declaring-module import exposes the carrier-qualified expression"
    );
}

/// `use units::u64::Long;` exposes the exact declaration to both its leaf and
/// its declared carrier-qualified spelling.
#[test]
fn narrow_domain_import_exposes_leaf_and_carrier_qualified_spellings() {
    let program = lower_multi(&[
        (
            "units.omg",
            "module units; pub domain u64::Long requires self > 0;",
        ),
        (
            "check.omg",
            r#"
            use units::u64::Long;
            machine carrier(value: u64) -> bool { value in u64::Long }
            machine leaf(value: u64) -> bool { value in Long }
            "#,
        ),
    ]);
    let long = domain_named(&program, "u64::Long");
    let selections = membership_domain_symbols(&program);
    assert_eq!(selections.len(), 2, "both spellings resolve");
    for selected in selections {
        assert_eq!(selected, long.symbol);
    }
}

/// Spellings that lack an exposing import — or import the wrong declaration
/// kind — must never select a foreign attached domain. Resolution retains
/// the checked obligation (an invalid domain symbol) for downstream
/// diagnostics instead of reaching through the carrier or a sibling.
#[test]
fn unexposed_foreign_domain_spellings_keep_checked_obligations() {
    let units = "module units; pub domain u64::Long requires self > 0;";
    let units_with_machine = "module units; pub domain u64::Long requires self > 0; pub machine helper() -> bool { true }";
    let geometry = "module geometry; pub data Point { x: u64; }";
    let screen = "module ui_policy::screen; use geometry::Point; pub domain Point::OnScreen requires self.x > 0;";
    for (tag, declaring, check) in [
        (
            "no import",
            units,
            "domain u64::Near requires self in u64::Long;",
        ),
        // A broad module import exposes the carrier-qualified form only; the
        // leaf is not a bare local name.
        (
            "broad import, leaf spelling",
            units,
            "use units; domain u64::Near requires self in Long;",
        ),
        // Importing an ordinary sibling machine exposes no sibling domains.
        (
            "sibling machine import",
            units_with_machine,
            "use units::helper; domain u64::Near requires self in u64::Long;",
        ),
    ] {
        let program = lower_multi(&[("units.omg", declaring), ("check.omg", check)]);
        let near = domain_named(&program, "u64::Near");
        let [fact] = fact_membership_domain_symbols(&program, near.facts)
            .try_into()
            .expect("one fact membership");
        assert!(!fact.is_valid(), "{tag} must not select a foreign domain");
    }
    for (tag, check) in [
        // Importing the carrier type exposes nothing attached to it.
        (
            "carrier type import",
            "use geometry::Point; domain Point::Near requires self in Point::OnScreen;",
        ),
        // An ancestor module exposes no descendant-module declarations.
        (
            "ancestor module import",
            "use ui_policy; use geometry::Point; domain Point::Near requires self in Point::OnScreen;",
        ),
        // A caller's same-spelled carrier cannot redirect the attachment:
        // the local `Point` is not the carrier `Point::OnScreen` was
        // declared against.
        (
            "same-spelled local carrier",
            "use ui_policy::screen; data Point { x: u64; } domain Point::Near requires self in Point::OnScreen;",
        ),
    ] {
        let program = lower_multi(&[
            ("geom.omg", geometry),
            ("screen.omg", screen),
            ("check.omg", check),
        ]);
        let near = domain_named(&program, "Point::Near");
        let [fact] = fact_membership_domain_symbols(&program, near.facts)
            .try_into()
            .expect("one fact membership");
        assert!(!fact.is_valid(), "{tag} must not select a foreign domain");
    }
}

/// Two visible declarations of the same carrier-qualified domain contest the
/// spelling; neither import order nor traversal order selects a winner. The
/// membership keeps its checked obligation for downstream diagnostics.
#[test]
fn distinct_exposed_domain_owners_contest_the_spelling() {
    let program = lower_multi(&[
        (
            "units.omg",
            "module units; pub domain u64::Long requires self > 0;",
        ),
        (
            "more.omg",
            "module more_units; pub domain u64::Long requires self > 0;",
        ),
        (
            "check.omg",
            r#"
            use units;
            use more_units;
            domain u64::Near requires self in u64::Long;
            machine check(value: u64) -> bool { value in u64::Long }
            "#,
        ),
    ]);
    let near = domain_named(&program, "u64::Near");
    let [fact] = fact_membership_domain_symbols(&program, near.facts)
        .try_into()
        .expect("one fact membership");
    assert!(
        !fact.is_valid(),
        "two exposed owners must not let either win the fact"
    );
    let [expression] = membership_domain_symbols(&program)
        .try_into()
        .expect("one expression membership");
    assert!(
        !expression.is_valid(),
        "two exposed owners must not let either win the expression"
    );
}

/// A domain declared in the reference's own module outranks an exposed
/// foreign declaration of the same carrier-qualified spelling.
#[test]
fn module_local_domain_outranks_exposed_foreign_domain() {
    let program = lower_multi(&[
        (
            "units.omg",
            "module units; pub domain u64::Long requires self > 0;",
        ),
        (
            "check.omg",
            r#"
            module local_units;
            use units;
            domain u64::Long requires self > 1;
            domain u64::Near requires self in u64::Long;
            "#,
        ),
    ]);
    let local = program
        .domain_definitions
        .iter()
        .find(|definition| {
            program.symbols.display_path(definition.symbol, "::") == "local_units::u64::Long"
        })
        .expect("the module-local u64::Long domain");
    let near = domain_named(&program, "u64::Near");
    let [fact] = fact_membership_domain_symbols(&program, near.facts)
        .try_into()
        .expect("one fact membership");
    assert_eq!(fact, local.symbol, "the module-local owner wins");
}

/// A qualified case path in a declared-domain proof fact resolves to the
/// exact case owner and case symbol, same-module or through imports.
#[test]
fn proof_fact_case_membership_retains_case_identity() {
    // Same-module case membership on a domain requirement.
    let program = lower_multi(&[(
        "choice.omg",
        "data Choice { case Empty; case Some(v: u32); } domain Choice::NonEmpty requires self in Choice::Some;",
    )]);
    let choice = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Choice")
        .expect("Choice data");
    let some_case = program
        .data_members(choice.members)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant) if variant.name.as_str() == "Some" => Some(variant.symbol),
            _ => None,
        })
        .expect("Choice::Some case");
    let non_empty = domain_named(&program, "Choice::NonEmpty");
    let [fact] = program.proof_facts(non_empty.facts) else {
        panic!("one proof fact")
    };
    assert_case_membership_fact(&program, fact, choice.symbol, some_case);

    // Foreign qualified and imported case paths keep the same identities.
    for (tag, check) in [
        (
            "fully qualified",
            "data Holder where c in shapes::Choice::Some, { c: shapes::Choice; }",
        ),
        (
            "imported carrier",
            "use shapes::Choice; data Holder where c in Choice::Some, { c: Choice; }",
        ),
    ] {
        let program = lower_multi(&[
            (
                "shapes.omg",
                "module shapes; pub data Choice { case Empty; case Some(v: u32); }",
            ),
            ("check.omg", check),
        ]);
        let choice = program
            .data_definitions
            .iter()
            .find(|definition| definition.name.as_str() == "Choice")
            .expect("Choice data");
        let some_case = program
            .data_members(choice.members)
            .iter()
            .find_map(|member| match member {
                DataMember::Variant(variant) if variant.name.as_str() == "Some" => {
                    Some(variant.symbol)
                }
                _ => None,
            })
            .expect("Choice::Some case");
        let holder = program
            .data_definitions
            .iter()
            .find(|definition| definition.name.as_str() == "Holder")
            .expect("Holder data");
        let [fact] = program.proof_facts(holder.where_facts) else {
            panic!("{tag}: one where fact")
        };
        assert_case_membership_fact(&program, fact, choice.symbol, some_case);
    }
}

fn assert_case_membership_fact(
    program: &SymbolResolvedTrees,
    fact: &symbol_resolved_trees::domain::ProofFact,
    expected_owner: symbols::SymbolHandle,
    expected_case: symbols::SymbolHandle,
) {
    // Case membership rewrites the proof fact to a membership expression
    // that retains the exact case owner and case symbol.
    let symbol_resolved_trees::domain::ProofFact::Expression(expression) = fact else {
        panic!("a case-membership fact becomes a membership expression")
    };
    let ExpressionNode::Membership(membership) =
        program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("a case-membership fact holds a membership expression")
    };
    assert!(!membership.domain_symbol.is_valid());
    assert_eq!(membership.case_type_symbol, expected_owner);
    assert_eq!(membership.case_symbol, expected_case);
}
