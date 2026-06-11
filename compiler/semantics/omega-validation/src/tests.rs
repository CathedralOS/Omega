use super::{validate_effect_plan, validate_program};
use omega_source_files_to_tokens::Lexer;
use omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use omega_tokens_to_syntax_trees::parse_syntax_trees;

#[test]
fn validates_main_entry_surface_from_source_pipeline() {
    let source = r#"
    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    assert_eq!(typed.machines().len(), 1);
    assert_eq!(typed.machines()[0].name.as_str(), "Main::main");
    assert_eq!(typed.machine_states(&typed.machines()[0]).len(), 1);
    assert_eq!(
        typed.machine_states(&typed.machines()[0])[0].name.as_str(),
        "main"
    );
    validate_program(&typed).expect("validation should succeed");
}

#[test]
fn validates_local_state_call_arguments_from_source_pipeline() {
    let source = r#"
    data Main {
    }

    machine Main::main(&mut self) {
        take_non_negative(0);

        state take_non_negative(
            &mut self,
            value: u32[exact, non_negative]
        ) {
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let entry = typed
        .machine_states(&typed.machines()[0])
        .iter()
        .find(|state| state.name.as_str() == "main")
        .expect("entry state");
    let call_argument_count = typed
        .statement_table
        .statements(entry.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            omega_typed_trees::statement::StatementNode::Call(call) => Some(call.arguments.len()),
            omega_typed_trees::statement::StatementNode::Expression(expression) => {
                let omega_typed_trees::expression::ExpressionNode::Call(call) =
                    typed.expression_table.expression(*expression)
                else {
                    return None;
                };
                Some(call.arguments.len())
            }
            _ => None,
        })
        .expect("expected call statement");
    assert_eq!(call_argument_count, 1);
    validate_program(&typed).expect("validation should succeed");
}

#[test]
fn rejects_unknown_trait_machine_effects() {
    let source = r#"
    boundary trait Console {
        machine write_line(text: String)
        effects
            stdoutish;
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject effect");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unknown effect `stdoutish`")),
        "expected unknown effect diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_unknown_domain_membership_in_domain_body() {
    let source = r#"
    data Player {
    }

    domain Player::Alive {
        self in Player::Valid
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject domain");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("domain `Player::Alive` references unknown domain `Player::Valid`")),
        "expected unknown domain diagnostic, got {diagnostics:#?}"
    );
}

fn validate_contract_source(source: &str) -> Result<(), Vec<omega_core::diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");
    validate_program(&typed)
}

#[test]
fn refutes_constant_false_ensures_on_empty_proof_machine() {
    let diagnostics = validate_contract_source(
        r#"
    machine false_constant()
    ensures
        1nat + 1nat == 3nat
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect_err("constant-false ensures should be refuted");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("ensures contract proof fact `1 + 1 == 3` is disproved by constant arithmetic")),
        "expected constant refutation diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn refutes_strict_order_asymmetry_between_requires_and_ensures() {
    let diagnostics = validate_contract_source(
        r#"
    machine false_asymmetry(i: usize, j: usize)
    requires
        i < j
    ensures
        j < i
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect_err("asymmetric strict-order ensures should be refuted");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "ensures contract proof fact `j < i` is disproved: the requires contract entails its negation"
        )),
        "expected asymmetry refutation diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn proves_entailment_ladder_on_empty_proof_machines() {
    validate_contract_source(
        r#"
    machine sum_in_range(a: i32, b: i32)
    requires
        a in 1..=10
        b in 1..=10
    ensures
        a + b in 2..=20
    {
    }

    machine congruence_add(a: usize, b: usize)
    requires
        a == b
    ensures
        a + 1 == b + 1
    {
    }

    machine square_in_range(a: i32)
    requires
        a in 0..=10
    ensures
        a * a in 0..=100
    {
    }

    machine antisymmetry(a: usize, b: usize)
    requires
        a <= b
        b <= a
    ensures
        a == b
    {
    }

    machine remainder_bound(a: usize)
    ensures
        a % 2 in 0..=1
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect("the entailment ladder's true theorems must prove");
}

#[test]
fn rejects_unprovable_in_language_ensures() {
    let diagnostics = validate_contract_source(
        r#"
    machine sum_corner_too_tight(a: i32, b: i32)
    requires
        a in 1..=10
        b in 1..=10
    ensures
        a + b in 2..=19
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect_err("an unprovable in-language ensures must be rejected");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove ensures contract proof fact")),
        "expected cannot-prove diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn stands_down_on_out_of_language_contracts() {
    // `min(a, b)` is an unknown call shape: the engine cannot read it, so an
    // unprovable ensures must be ACCEPTED unchecked rather than rejected.
    validate_contract_source(
        r#"
    machine uses_unknown_call(a: usize, b: usize)
    requires
        min(a, b) >= 1
    ensures
        a >= 1
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect("contracts outside the engine's language stand down");
}

#[test]
fn accepts_vacuous_ensures_under_contradictory_requires() {
    // Contradictory hypotheses entail anything (the `absurd` case), so the
    // refutation pass must stand down rather than reject a vacuous truth.
    validate_contract_source(
        r#"
    machine vacuous_truth(i: usize, j: usize)
    requires
        i < j
        j < i
    ensures
        j < i
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect("vacuously true contract should not be refuted");
}

#[test]
fn refutation_skips_machines_with_body_statements() {
    // A body could establish facts beyond the requires set; the refutation
    // pass only reasons about empty-body proof artifacts. The constant-false
    // ensures here is still wrong, but rejecting it is the entailment
    // engine's job, not this pass's.
    validate_contract_source(
        r#"
    data Counter {
        value: i32;
    }

    machine Counter::touch(&mut self)
    ensures
        1nat + 1nat == 3nat
    {
        self.value = 1;
    }

    data Main {
        counter: Counter;
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect("non-empty bodies are outside the refutation pass");
}

#[test]
fn accepts_true_theorems_on_empty_proof_machines() {
    validate_contract_source(
        r#"
    machine pythagorean_three_four_five()
    ensures
        3nat * 3nat + 4nat * 4nat == 5nat * 5nat
    {
    }

    machine less_than_transitive(a: usize, b: usize, c: usize)
    requires
        a < b
        b < c
    ensures
        a < c
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#,
    )
    .expect("true theorems should pass the refutation pass");
}

#[test]
fn rejects_unknown_domain_membership_in_contract() {
    let source = r#"
    data Player {
    }

    boundary trait Renderer {
        machine draw(player: Player)
        requires
            player in Player::Drawable;
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject domain");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "trait `Renderer` state `draw` requires contract references unknown domain `Player::Drawable`"
        )),
        "expected unknown contract domain diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_non_boolean_shaped_proof_fact() {
    let source = r#"
    data Player {
    }

    domain Player::Weird {
        1 + 2
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject fact");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("domain `Player::Weird` proof fact `1 + 2` is not boolean-shaped")),
        "expected non-boolean proof fact diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_domain_import_with_different_target_type() {
    let source = r#"
    data Player {
    }

    data Enemy {
    }

    domain Enemy::Valid {
        true
    }

    domain Player::Alive {
        self in Enemy::Valid
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject import");
    let has_target_mismatch = diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "domain `Player::Alive` imports `Enemy::Valid` but they classify different types",
        )
    });
    assert!(
        has_target_mismatch,
        "expected domain target mismatch diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_domain_import_cycles() {
    let source = r#"
    data Player {
    }

    domain Player::Alive {
        self in Player::Valid
    }

    domain Player::Valid {
        self in Player::Alive
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject cycle");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "domain membership cycle: Player::Alive -> Player::Valid -> Player::Alive",
            )
        }),
        "expected domain cycle diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_machine_effects_outside_trait_ceiling() {
    let source = r#"
    boundary trait Console {
        machine write_line(text: String)
        effects
            stdout_io;
    }

    data ConsoleImpl {
    }

    machine ConsoleImpl::write_line(text: String) satisfies Console
    effects
        stdout_io, filesystem_io
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    let diagnostics = validate_program(&typed).expect_err("validation should reject extra effect");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("effect `filesystem_io` is not allowed by the trait requirement")),
        "expected effect ceiling diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn accepts_machine_effects_within_trait_ceiling() {
    let source = r#"
    boundary trait Console {
        machine write_line(text: String)
        effects
            stdout_io;
    }

    data ConsoleImpl {
    }

    machine ConsoleImpl::write_line(text: String) satisfies Console
    effects
        stdout_io
    {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    validate_program(&typed).expect("validation should succeed");
}

#[test]
fn accepts_machine_effects_below_trait_ceiling() {
    let source = r#"
    boundary trait Console {
        machine write_line(text: String)
        effects
            stdout_io;
    }

    data TestConsole {
    }

    machine TestConsole::write_line(text: String) satisfies Console {
    }

    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    validate_program(&typed).expect("validation should succeed");
}

#[test]
fn rejects_declared_machine_effects_below_reached_effects() {
    let source = r#"
    boundary trait Console {
        machine read_line(out: &mut String)
        effects
            stdin_io;
    }

    data ConsoleImpl {
    }

    machine ConsoleImpl::read_line(out: &mut String) satisfies Console
    effects
        stdin_io
    {
    }

    data Main {
        console: ConsoleImpl;
    }

    machine Main::main(&mut self)
    effects
        stdout_io
    {
        let line: String;
        self.console.read_line(&mut line);
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

    validate_program(&typed).expect("direct effect validation should pass");
    let effect_plan = omega_effects::infer_effects(&typed);
    let diagnostics =
        validate_effect_plan(&typed, &effect_plan).expect_err("effect ceiling should fail");

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("reaches undeclared effects `stdin_io`")
            && diagnostic.message.contains("call path for `stdin_io`")
            && diagnostic.message.contains("Main::main statement")
            && diagnostic
                .message
                .contains("source: machine `ConsoleImpl::read_line` directly declares the effect")),
        "expected transitive effect ceiling diagnostic, got {diagnostics:#?}"
    );
}

/// Tests relocated from `omega-effects` when that crate moved into the
/// `representations` layer. They exercise the effect/capability *analysis*
/// passes, which require building a `TypedTrees` through the front-of-pipeline
/// lowering crates. Those pipeline crates are dev-dependencies here (a
/// semantics crate may depend up into pipeline for tests), but must not be
/// dev-dependencies of `omega-effects` itself, since that would re-introduce a
/// `representations -> pipeline` upward edge.
mod effects_analysis {
    use omega_effects::{
        EffectSet, audit_host_calls, build_host_authority_registry, infer_effects,
    };
    use omega_typed_trees::TypedTrees;

    use omega_source_files_to_tokens::Lexer;
    use omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;

    fn lower(source: &str) -> TypedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve");
        lower_symbol_resolved_trees(&resolved).expect("type")
    }

    #[test]
    fn effect_sets_are_bitsets_with_named_edges() {
        let mut effects = EffectSet::empty();
        assert!(effects.insert_name("stdout_io"));
        assert!(effects.insert_name("process_exit"));
        assert!(!effects.insert_name("nope"));
        assert!(effects.contains_all(EffectSet::from_name("stdout_io").unwrap()));
        assert_eq!(
            effects.names().collect::<Vec<_>>(),
            ["stdout_io", "process_exit"]
        );
    }

    #[test]
    fn propagates_machine_effects_to_call_sites() {
        let source = r#"
        boundary trait Console {
            machine write_line(text: String)
            effects
                stdout_io;
        }

        data ConsoleImpl {
        }

        machine ConsoleImpl::write_line(text: String) satisfies Console
        effects
            stdout_io
        {
        }

        data Main {
            console: ConsoleImpl;
        }

        machine Main::main(&mut self) {
            self.console.write_line("hello");
        }
        "#;

        let typed = lower(source);
        let plan = infer_effects(&typed);

        let main_machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("main machine");
        let main_effects = plan
            .machines()
            .iter()
            .find(|effects| effects.symbol == main_machine.symbol)
            .expect("main effects");
        assert!(
            main_effects
                .transitive
                .contains_all(EffectSet::from_name("stdout_io").unwrap())
        );

        let main_state = plan
            .states
            .span_or_empty(main_effects.states)
            .first()
            .expect("state");
        let call = plan
            .calls
            .span_or_empty(main_state.calls)
            .first()
            .expect("call");
        assert!(
            call.transitive
                .contains_all(EffectSet::from_name("stdout_io").unwrap())
        );
    }

    #[test]
    fn abstract_boundary_provider_is_authorized() {
        let program = lower(
            r#"
            boundary trait Console {
                machine write_line(text: String)
                effects
                    stdout_io;
            }

            data Main {
                console: Console;
            }

            machine Main::main(&mut self) {
                self.console.write_line("hello");
            }
            "#,
        );
        let effects = infer_effects(&program);
        let registry = build_host_authority_registry(&program);
        let unapproved = audit_host_calls(&program, &effects, &registry);
        assert!(
            unapproved.is_empty(),
            "abstract provider should be approved"
        );
    }

    #[test]
    fn in_package_host_provider_is_unapproved() {
        let program = lower(
            r#"
            boundary trait LocalFiles {
                machine write_bytes(path: String)
                effects
                    filesystem_io;
            }

            data Disk {
            }

            machine Disk::write_bytes(path: String) satisfies LocalFiles
            effects
                filesystem_io
            {
            }

            data Main {
                disk: Disk;
            }

            machine Main::main(&mut self) {
                self.disk.write_bytes("/etc/passwd");
            }
            "#,
        );
        let effects = infer_effects(&program);
        let registry = build_host_authority_registry(&program);
        let unapproved = audit_host_calls(&program, &effects, &registry);
        assert_eq!(
            unapproved.len(),
            1,
            "in-package host provider should be flagged as unapproved"
        );
        assert_eq!(
            unapproved[0].missing_authority.names().collect::<Vec<_>>(),
            ["filesystem_io"]
        );
    }
}

/// Tests relocated from `omega-effects::capabilities::providers` when that
/// crate moved into the `representations` layer. They parse source into
/// `SyntaxTrees` to exercise the boundary-provider registry, which needs the
/// front-of-pipeline lexer/parser (dev-dependencies allowed here, but not on
/// `omega-effects` itself - see `effects_analysis` above for the rationale).
mod provider_registry {
    use omega_core::operator_spelling::ProviderCategory;
    use omega_effects::{build_provider_registry, host_authority_effects};
    use omega_syntax_trees::SyntaxTrees;

    use omega_source_files_to_tokens::Lexer;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;

    fn parse(source: &str) -> SyntaxTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        parse_syntax_trees(&tokens).expect("parse")
    }

    #[test]
    fn host_abi_provider_metadata_comes_from_bound_operator() {
        let syntax = parse(
            r#"
            provider omega::host::WriteBytes : HostAbiCall;

            boundary operator omega::host::write(handle: i64, length: usize) -> usize
            provider omega::host::WriteBytes
            requires
                length > 0;
            "#,
        );

        let mut diagnostics = Vec::new();
        let registry = build_provider_registry(&syntax, &mut diagnostics);
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );

        let provider = registry
            .resolve("omega::host::WriteBytes")
            .expect("provider should be registered");
        assert_eq!(provider.category, ProviderCategory::HostAbiCall);

        // contract_ref: sourced from the bound operator's `requires` clause,
        // referenced by the operator's qualified name.
        assert_eq!(provider.contract_ref.as_deref(), Some("omega::host::write"));

        // effect_set: a host-ABI provider carries host authority.
        assert!(
            !provider.effect_set.is_empty(),
            "host-ABI provider should carry host authority effects"
        );
        assert_eq!(
            provider.effect_set,
            host_authority_effects(),
            "host-ABI provider should carry the full host authority set"
        );
    }

    #[test]
    fn compute_provider_carries_no_authority() {
        let syntax = parse(
            r#"
            provider omega::language::core::Slice : SliceIndexing;

            boundary operator omega::language::core::index<T>(items: &[T], index: usize) -> T
            spelling []
            provider omega::language::core::Slice
            requires
                index < items.len;
            "#,
        );

        let mut diagnostics = Vec::new();
        let registry = build_provider_registry(&syntax, &mut diagnostics);
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );

        let provider = registry
            .resolve("omega::language::core::Slice")
            .expect("provider should be registered");
        assert_eq!(provider.category, ProviderCategory::SliceIndexing);
        assert_eq!(
            provider.contract_ref.as_deref(),
            Some("omega::language::core::index")
        );
        // Pure-compute primitive: no host authority.
        assert!(
            provider.effect_set.is_empty(),
            "slice indexing provider should not carry host authority"
        );
    }

    #[test]
    fn unbound_provider_keeps_empty_metadata() {
        let syntax = parse("provider omega::host::Unbound : HostAbiCall;");

        let mut diagnostics = Vec::new();
        let registry = build_provider_registry(&syntax, &mut diagnostics);
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );

        let provider = registry
            .resolve("omega::host::Unbound")
            .expect("provider should be registered");
        // No operator binds it, so metadata stays at empty defaults.
        assert_eq!(provider.contract_ref, None);
        assert!(provider.effect_set.is_empty());
        assert!(provider.target_applicability.is_empty());
    }
}
