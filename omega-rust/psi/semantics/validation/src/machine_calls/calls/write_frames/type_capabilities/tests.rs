use super::parameter_may_carry_write;
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
            (
                parameter.name.as_str().to_owned(),
                parameter_may_carry_write(&program, parameter),
            )
        })
        .collect()
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
