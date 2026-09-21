//! Contract consumption at call boundaries: element-level field evidence
//! must survive a mutable call through a sibling element of the same slice
//! view, while a call that corrupts the consumed element still rejects.

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    typed_trees_to_checked_trees::lower_typed_trees(typed(source)?)
}

fn typed(source: &str) -> Result<typed_trees::TypedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add(
            std::path::PathBuf::from("contracts_probe.omg"),
            source.to_owned(),
        )
        .source_id;
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
    let syntax = syntax_trees_to_symbol_resolved_trees::pre_resolution::normalize_generic_data(
        syntax_trees_to_symbol_resolved_trees::pre_resolution::GenericDataRequest::new(syntax),
    )?;
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax: &syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
        },
    )?;
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|diagnostic| vec![diagnostic])
}

const DEFINITIONS: &str = r#"
domain [u8; 4]::Utf8 requires valid_utf8(self);
data Row [copy] { bytes: [u8; 4] in Utf8; tag: u64; }
data Level [copy] { rooms: [Row; 2]; }
"#;

fn assert_accepted(name: &str, source: &str) {
    if let Err(diagnostics) = check(source) {
        panic!(
            "{name}: expected acceptance, got:\n{}",
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

fn assert_rejected(name: &str, source: &str, expected: &[&str]) {
    match check(source) {
        Ok(_) => panic!("{name}: expected rejection containing {expected:?}, got acceptance"),
        Err(diagnostics) => {
            let messages = diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect::<Vec<_>>();
            for fragment in expected {
                assert!(
                    messages.iter().any(|message| message.contains(fragment)),
                    "{name}: no diagnostic contains {fragment:?}; got:\n{}",
                    messages.join("\n")
                );
            }
        }
    }
}

/// The flattened write frame reports `clear(&mut rooms[0])` as a write to
/// `rooms`; the structured mutation set keeps `rooms[0].tag`. The sibling
/// element's index bound and `bytes in Utf8` evidence must survive the call.
#[test]
fn call_through_view_element_keeps_sibling_element_coverage() {
    assert_accepted(
        "call_through_view_element_keeps_sibling_element_coverage",
        &format!(
            r#"{DEFINITIONS}
            machine clear(row: &mut Row) {{ row.tag = 0; }}
            machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
            machine caller(level: &mut Level) {{
                let rooms: &mut [Row] = level.rooms.as_mut_slice();
                clear(&mut rooms[0]);
                consume(&rooms[1]);
            }}
        "#
        ),
    );
}

/// Writing an element's declared field retires that element's evidence only:
/// consuming the same view element after the mutating call must reject.
#[test]
fn call_through_view_element_retires_the_written_element() {
    assert_rejected(
        "call_through_view_element_retires_the_written_element",
        &format!(
            r#"{DEFINITIONS}
            machine poke(row: &mut Row) {{ row.bytes[0] = 255; }}
            machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
            machine caller(level: &mut Level) {{
                let rooms: &mut [Row] = level.rooms.as_mut_slice();
                poke(&mut rooms[1]);
                consume(&rooms[1]);
            }}
        "#
        ),
        &["parameter row.bytes requires"],
    );
}
const PREDICATE_DEFINITIONS: &str = r#"
data Choice [copy] {
    case Empty;
    case Some(value: u32);
}
domain Choice::NonEmpty requires self in Choice::Some;
data Command [copy] {
    case Move;
    case Say(text: u32);
    case Quit;
}
domain Command::Interactive requires self in Command::Move | Command::Say;
data Pair [copy] { a: u32; b: u32; flag: bool; }
domain Pair::ZeroA requires self.a == 0;
domain Pair::Nested requires self.a == 0 && self.flag;
"#;

/// A predicate-only domain annotation on a local is an obligation on its
/// initializer, not a free establishment route: `Choice::Empty` under
/// `requires self in Choice::Some` must reject, not mint a `NonEmpty` fact.
#[test]
fn predicate_domain_initializer_rejects_wrong_case() {
    {
        assert_rejected(
            "predicate_domain_initializer_rejects_wrong_case",
            &format!(
                r#"{PREDICATE_DEFINITIONS}
            machine read() -> u32 {{
                let value: Choice in Choice::NonEmpty = Choice::Empty {{ }};
                value.value
            }}
        "#
            ),
            &[
                "cannot prove initializer of `value`",
                "domain `Choice::NonEmpty`",
            ],
        );
    }
}

/// The same annotation on a construction whose selected case satisfies the
/// predicate — `Choice::Some {{ value: 3 }}`, `Choice::Some {{ .. }}`, or a bare case path
/// — establishes membership and accepts.
#[test]
fn predicate_domain_initializer_accepts_satisfying_case() {
    {
        assert_accepted(
            "predicate_domain_initializer_accepts_satisfying_case",
            &format!(
                r#"{PREDICATE_DEFINITIONS}
            machine read() -> u32 {{
                let first: Choice in Choice::NonEmpty = Choice::Some {{ value: 3 }};
                let second: Choice in Choice::NonEmpty = Choice::Some {{ value: 4 }};
                let union: Command in Command::Interactive = Command::Move {{ }};
                first.value
            }}
        "#
            ),
        );
    }
}

/// A union case predicate accepts any member of the union and rejects a case
/// outside it.
#[test]
fn predicate_domain_initializer_union_membership() {
    {
        assert_accepted(
            "predicate_domain_initializer_union_membership",
            &format!(
                r#"{PREDICATE_DEFINITIONS}
            machine read() -> u32 {{
                let moved: Command in Command::Interactive = Command::Move {{ }};
                let said: Command in Command::Interactive = Command::Say {{ text: 1 }};
                0
            }}
        "#
            ),
        );
        assert_rejected(
            "predicate_domain_initializer_union_rejects_outside_case",
            &format!(
                r#"{PREDICATE_DEFINITIONS}
            machine read() -> u32 {{
                let quit: Command in Command::Interactive = Command::Quit {{ }};
                0
            }}
        "#
            ),
            &["cannot prove initializer of `quit`"],
        );
    }
}

/// Member predicates on a record literal evaluate the field initializers
/// themselves: `self.a == 0` accepts `a: 0` and rejects `a: 1`; a conjunctive
/// predicate needs every conjunct.
#[test]
fn predicate_domain_initializer_member_predicates() {
    {
        assert_accepted(
            "predicate_domain_initializer_member_predicates",
            &format!(
                r#"{PREDICATE_DEFINITIONS}
            machine read() -> u32 {{
                let pair: Pair in Pair::ZeroA = Pair {{ a: 0, b: 9, flag: false }};
                let nested: Pair in Pair::Nested = Pair {{ a: 0, b: 0, flag: true }};
                pair.a
            }}
        "#
            ),
        );
        assert_rejected(
            "predicate_domain_initializer_member_predicate_rejects",
            &format!(
                r#"{PREDICATE_DEFINITIONS}
            machine read() -> u32 {{
                let pair: Pair in Pair::ZeroA = Pair {{ a: 1, b: 0, flag: true }};
                pair.a
            }}
        "#
            ),
            &["cannot prove initializer of `pair`"],
        );
        assert_rejected(
            "predicate_domain_initializer_nested_partially_true_rejects",
            &format!(
                r#"{PREDICATE_DEFINITIONS}
            machine read() -> u32 {{
                let pair: Pair in Pair::Nested = Pair {{ a: 0, b: 0, flag: false }};
                pair.a
            }}
        "#
            ),
            &["cannot prove initializer of `pair`"],
        );
    }
}

/// A non-literal initializer satisfies a case predicate from live evidence:
/// a parameter declared in the domain, a call returning the domain, or a
/// place whose current assigned case is selected.
#[test]
fn predicate_domain_initializer_live_evidence() {
    {
        assert_accepted(
            "predicate_domain_initializer_live_evidence",
            &format!(
                r#"{PREDICATE_DEFINITIONS}
            machine produce() -> Choice in Choice::NonEmpty {{ Choice::Some {{ value: 7 }} }}
            machine forward(input: Choice in Choice::NonEmpty) -> u32 {{
                let value: Choice in Choice::NonEmpty = input;
                let made: Choice in Choice::NonEmpty = produce();
                let selected: Choice = Choice::Some {{ value: 1 }};
                let alias: Choice in Choice::NonEmpty = selected;
                value.value
            }}
        "#
            ),
        );
    }
}

/// Stale case evidence cannot establish membership: a place reassigned to a
/// different case before the qualified `let` no longer satisfies `self in
/// Choice::Some`.
#[test]
fn predicate_domain_initializer_stale_evidence_rejects() {
    {
        assert_rejected(
            "predicate_domain_initializer_stale_evidence_rejects",
            &format!(
                r#"{PREDICATE_DEFINITIONS}
            machine read() -> u32 {{
                let mut value: Choice = Choice::Some {{ value: 1 }};
                value = Choice::Empty {{ }};
                let qualified: Choice in Choice::NonEmpty = value;
                qualified.value
            }}
        "#
            ),
            &["cannot prove initializer of `qualified`"],
        );
    }
}

/// A foreign same-named variant constructs the foreign type: `Other::Some`
/// under `Choice::NonEmpty` fails the owner check before domain discharge,
/// and the same-named `Other::Occupied` domain correctly discharges an
/// `Other` initializer but not the foreign `None` case.
#[test]
fn predicate_domain_initializer_wrong_owner_rejects() {
    {
        assert_rejected(
            "predicate_domain_initializer_wrong_owner_rejects",
            &format!(
                r#"{PREDICATE_DEFINITIONS}
            data Other [copy] {{
                case None;
                case Some(value: u32);
            }}
            domain Other::Occupied requires self in Other::Some;
            machine read() -> u32 {{
                let held: Other in Other::Occupied = Other::Some {{ value: 1 }};
                let value: Other in Other::Occupied = Other::None {{}};
                0
            }}
        "#
            ),
            &["cannot prove initializer of `value`"],
        );
    }
}

/// An equality live at a write's incoming boundary transports through the
/// write: `counter.count`'s exit value is the stored source expression with
/// the entry equality substituted, so `counter.count == before + 1` follows
/// from `counter.count = counter.count + 1` under `counter.count == before`.
/// A wrong update to the same shape still rejects.
#[test]
fn write_transport_carries_entry_equality_across_scalar_write() {
    assert_accepted(
        "write_transport_carries_entry_equality_across_scalar_write",
        r#"
        data Counter { count: u32; }
        machine bump(counter: &mut Counter, before: u32 [0..=4095])
            requires
                counter.count == before
                counter.count <= 4095
            ensures
                counter.count == before + 1
        {
            counter.count = counter.count + 1;
        }
    "#,
    );
    assert_rejected(
        "write_transport_rejects_a_wrong_update",
        r#"
        data Counter { count: u32; }
        machine bump(counter: &mut Counter, before: u32 [0..=4095])
            requires
                counter.count == before
                counter.count <= 4095
            ensures
                counter.count == before + 1
        {
            counter.count = counter.count + 2;
        }
    "#,
        &["cannot prove ensures"],
    );
}
