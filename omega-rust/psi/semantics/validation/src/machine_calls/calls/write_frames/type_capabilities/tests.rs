use super::{parameter_may_carry_write, type_may_carry_write};
use typed_trees::TypedTrees;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("symbols");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("types")
}

/// Every parameter of `probe`'s entry state, by declared name, with whether
/// the capability law lets it carry a caller-visible write.
fn probe_parameters(source: &str) -> Vec<(String, bool)> {
    let program = typed(source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "probe")
        .expect("probe machine");
    let entry = &program.machine_states(machine)[0];
    program
        .state_parameters(entry)
        .iter()
        .map(|parameter| {
            assert_eq!(
                type_may_carry_write(&program, parameter.type_reference),
                parameter_may_carry_write(&program, parameter),
                "storage capability must not depend on its query site: {}",
                parameter.name.as_str(),
            );
            (
                parameter.name.as_str().to_owned(),
                parameter_may_carry_write(&program, parameter),
            )
        })
        .collect()
}

#[test]
fn boundary_result_ignores_reference_free_value_arguments_but_keeps_referent_writes() {
    for (declaration, value_type) in [
        ("data Token { code: u64; }", "Token"),
        ("data Token { case Empty; case Code(value: u64); }", "Token"),
        ("data Token { code: u64; }", "[Token; 2]"),
        ("data Box<T> { value: T; }", "Box<u64>"),
        ("data Box<T> { value: T; }", "Box<Box<u64>>"),
    ] {
        let program = typed(&format!(
            "{declaration}
             data Main {{ value: u64; untouched: u64; }}
             boundary trait Device {{
                 machine reference(value: &mut u64, metadata: {value_type}) -> &mut u64;
                 machine write(value: &mut u64);
             }}
             machine Main::inspect(&mut self, metadata: {value_type}) {{
                 let alias: &mut u64 = Device::reference(&mut self.value, metadata);
                 Device::write(alias);
             }}"
        ));
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::inspect")
            .expect("caller");
        let frames = crate::CallFrameResolver::new(&program)
            .expect("frame resolver")
            .inferred_machine_state_write_frames(machine);
        assert_eq!(frames.len(), 1);
        assert_eq!(
            frames[0].complete_paths(),
            Some(["self.value".to_owned()].as_slice()),
            "{value_type}: a by-value argument cannot provide another referent",
        );
    }
}

#[test]
fn call_substitution_applies_inside_reference_free_containers() {
    use super::type_may_carry_write_in;
    use typed_trees::types::TypeReferenceNode;

    let program = typed(
        "data Box<T> { value: T; }
         data View { value: &mut u64; }
         boundary data Handle;
         machine probe<T>(formal: T, nested: Box<T>, scalar: u64, view: View, opaque: Handle) {}",
    );
    let machine = &program.machines()[0];
    let parameters = program.state_parameters(&program.machine_states(machine)[0]);
    let TypeReferenceNode::Named { symbol, .. } = program
        .type_reference_table
        .type_reference(parameters[0].type_reference)
    else {
        panic!("formal type binder");
    };
    for (actual, expected) in [(2, false), (3, true), (4, true)] {
        assert_eq!(
            type_may_carry_write_in(
                &program,
                parameters[1].type_reference,
                &[(*symbol, parameters[actual].type_reference)],
            ),
            expected,
            "actual {}",
            parameters[actual].name.as_str(),
        );
    }
    assert!(type_may_carry_write(&program, parameters[1].type_reference));
}

#[test]
fn boundary_result_keeps_opaque_and_reference_bearing_value_arguments_conservative() {
    for (declaration, value_type) in [
        ("boundary data Token;", "Token"),
        ("data Token { value: &mut u64; }", "Token"),
        (
            "data Token { case Empty; case Loan(value: &mut u64); }",
            "Token",
        ),
        (
            "data Box<T> { value: T; } boundary data Handle;",
            "Box<Handle>",
        ),
        ("data Token { value: &mut u64; }", "[Token; 2]"),
        ("data Token { value: &u64; }", "Token"),
    ] {
        let program = typed(&format!(
            "{declaration}
             data Main {{ value: u64; }}
             boundary trait Device {{
                 machine reference(value: &mut u64, metadata: {value_type}) -> &mut u64;
                 machine write(value: &mut u64);
             }}
             machine Main::inspect(&mut self, metadata: {value_type}) {{
                 let alias: &mut u64 = Device::reference(&mut self.value, metadata);
                 Device::write(alias);
             }}"
        ));
        let machine = &program.machines()[0];
        let frames = crate::CallFrameResolver::new(&program)
            .expect("frame resolver")
            .inferred_machine_state_write_frames(machine);
        assert_eq!(frames.len(), 1);
        assert!(
            !frames[0].is_complete(),
            "{value_type} may supply another referent"
        );
    }
}

#[test]
fn generic_boundary_result_keeps_only_proven_exclusive_origins() {
    for (formal, actual, expected) in [
        (
            "value: &mut u64, metadata: Box<T>",
            "&mut self.value, metadata",
            Some(vec!["self.value"]),
        ),
        ("metadata: Box<T>", "metadata", None),
    ] {
        let program = typed(&format!(
            "data Box<T> {{ value: T; }}
             data Main {{ value: u64; }}
             boundary trait Device {{
                 machine reference<T>({formal}) -> &mut u64;
                 machine write(value: &mut u64);
             }}
             machine Main::inspect(&mut self, metadata: Box<u64>) {{
                 let alias: &mut u64 = Device::reference({actual});
                 Device::write(alias);
             }}"
        ));
        let machine = &program.machines()[0];
        let frames = crate::CallFrameResolver::new(&program)
            .expect("frame resolver")
            .inferred_machine_state_write_frames(machine);
        let expected =
            expected.map(|paths| paths.into_iter().map(str::to_owned).collect::<Vec<_>>());
        assert_eq!(frames[0].complete_paths(), expected.as_deref(), "{formal}");
    }
}

// A by-value parameter whose type contains no exclusive reference anywhere in
// its transitive structure is the state's own storage: no write through it
// reaches a caller place (ownership.md, "Borrows and aliases": `&mut T` is
// the only exclusive observation-and-mutation path).
#[test]
fn reference_free_values_cannot_carry_caller_writes() {
    assert_eq!(
        probe_parameters(
            r#"
            data Kind [copy] { case Next; case Stop; }
            data Span [copy] { start: u64; end: u64; }
            data Token [copy] { kind: Kind; span: Span; }
            data Stream { tokens: [Token; 4]; count: u64; flag: bool; }
            data Shape { case Scalar; case Record(first: Span, count: u64); }
            data Counter { seed: u64; calls: UInt; }
            machine probe(
                kind: Kind,
                token: Token,
                stream: Stream,
                shape: Shape,
                words: [u64; 3],
                count: u64,
                fixed: u64 [0..=7],
                counter: Counter,
                total: Int
            ) {}
            "#
        ),
        [
            ("kind", false),
            ("token", false),
            ("stream", false),
            ("shape", false),
            ("words", false),
            ("count", false),
            ("fixed", false),
            ("counter", false),
            ("total", false),
        ]
        .map(|(name, capable)| (name.to_owned(), capable))
    );
}

// Everything that can denote or reach caller storage stays write-capable:
// exclusive references at the root, inside a field, a case payload or a
// nested record;
// unbound generic parameters; opaque boundary data with no inspectable
// members; and generic applications whose actual is unresolved or opaque.
#[test]
fn reference_bearing_unresolved_and_opaque_types_stay_write_capable() {
    assert_eq!(
        probe_parameters(
            r#"
            data Output { count: u64; }
            data View<'a> { target: &'a mut u64; }
            data Slot<'a> { case Empty; case Live(target: &'a mut Output); }
            data Nested<'a> { inner: View<'a>; count: u64; }
            data Box<T> { value: T; }
            boundary data Handle;
            machine probe<T>(
                output: &mut Output,
                view: View,
                slot: Slot,
                nested: Nested,
                generic: T,
                boxed_generic: Box<T>,
                handle: Handle,
                boxed_handle: Box<Handle>,
                boxed_value: Box<u64>,
                shared: &Output
            ) {}
            "#
        ),
        [
            ("output", true),
            ("view", true),
            ("slot", true),
            ("nested", true),
            ("generic", true),
            ("boxed_generic", true),
            ("handle", true),
            ("boxed_handle", true),
            ("boxed_value", false),
            ("shared", false),
        ]
        .map(|(name, capable)| (name.to_owned(), capable))
    );
}
