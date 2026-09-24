use super::{BoundsCheckResult, RangeFacts, check_indexed_access};
use crate::tests::front_end::{checked_program_result, typed_program};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableIndexedExpression};

mod length_endpoints;
mod lower_bounds;
mod selected;

fn fixture(
    collection_type: &str,
    access: &str,
) -> (
    typed_trees::TypedTrees,
    ExpressionHandle,
    TableIndexedExpression,
) {
    let source =
        format!("machine inspect(items: &{collection_type}, index: u64) {{ items{access}; }}");
    let program = typed_program(&source);
    let (expression, indexed) = program
        .expression_table
        .iter_expressions()
        .filter_map(|(expression, node)| match node {
            ExpressionNode::Indexed(indexed) => Some((expression, *indexed)),
            _ => None,
        })
        .last()
        .expect("indexed expression");
    (program, expression, indexed)
}

/// Lowers a whole source through the checked-tree pass so range proofs see
/// the same facts the real pipeline seeds (requires floors, guard bounds,
/// alias transfers). `Ok` means every bounds obligation was discharged.
fn check_source(source: &str) -> Result<(), Vec<String>> {
    checked_program_result(source)
        .map(|_| ())
        .map_err(|diagnostics| {
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect()
        })
}

fn result(collection_type: &str, access: &str, prove_index: bool) -> BoundsCheckResult {
    let (program, expression, indexed) = fixture(collection_type, access);
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let mut facts = RangeFacts::new(&[]);
    for parameter in program.state_parameters(state) {
        facts.define_local(
            parameter.symbol,
            parameter.name.to_string(),
            super::super::super::arrays::fixed_array_type_length(
                &program,
                parameter.type_reference,
            ),
            None,
        );
    }
    if prove_index {
        facts.prove_index("items".into(), "index".into());
    }
    // An unrelated diagnostic must not change this occurrence's proof result.
    let mut diagnostics = vec![diagnostics::Diagnostic::error("earlier error")];
    let result = check_indexed_access(
        &program,
        machine,
        state,
        &facts,
        expression,
        &indexed,
        &mut diagnostics,
    );
    assert_eq!(diagnostics.len() > 1, result == BoundsCheckResult::Rejected);
    result
}

#[test]
fn fixed_bounds_distinguish_scalar_elements_from_range_windows() {
    for (access, expected) in [
        ("[0]", BoundsCheckResult::ProvenScalar),
        ("[3]", BoundsCheckResult::ProvenScalar),
        ("[4]", BoundsCheckResult::Rejected),
        ("[-1]", BoundsCheckResult::Rejected),
        ("[0..4]", BoundsCheckResult::ProvenRange),
        ("[4..4]", BoundsCheckResult::ProvenRange),
        ("[0..5]", BoundsCheckResult::Rejected),
        ("[3..2]", BoundsCheckResult::Rejected),
    ] {
        assert_eq!(result("[u8; 4]", access, false), expected, "{access}");
    }
}

#[test]
fn dynamic_bounds_require_the_collection_relative_fact() {
    for collection in ["[u8; 4]", "[u8]"] {
        assert_eq!(
            result(collection, "[index]", false),
            BoundsCheckResult::Rejected
        );
        assert_eq!(
            result(collection, "[index]", true),
            BoundsCheckResult::ProvenScalar
        );
    }
}

#[test]
fn nested_collection_bounds_use_the_selected_element_type() {
    for (access, expected) in [
        ("[0][0]", BoundsCheckResult::ProvenScalar),
        ("[0][1]", BoundsCheckResult::ProvenScalar),
        ("[0][2]", BoundsCheckResult::Rejected),
        ("[0][0..2]", BoundsCheckResult::ProvenRange),
        ("[0][0..3]", BoundsCheckResult::Rejected),
    ] {
        assert_eq!(result("[[u8; 2]; 4]", access, false), expected, "{access}");
    }
}

#[test]
fn unrecognized_collection_is_not_a_bounds_admission() {
    assert_eq!(result("u64", "[0]", false), BoundsCheckResult::Unsupported);
}

/// A call used as an index proves through the callee's own `ensures` result
/// contract: `result < K` / `result <= K` / `result == K` (with `>=`/`>`
/// covering the non-negative half a signed result still owes) bound THIS
/// occurrence's return value exactly like a declared return range — the
/// contract is discharged at every callee exit, so the caller may rely on it.
/// Unbounded spellings, insufficient bounds, and `result`-shadowed signatures
/// keep the ordinary rejection; nothing here replays the callee's body.
#[test]
fn call_index_proves_through_ensured_result_bounds() {
    fn check(source: &str) -> Result<(), Vec<String>> {
        checked_program_result(source)
            .map(|_| ())
            .map_err(|diagnostics| {
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.clone())
                    .collect()
            })
    }
    for (callee, accepted) in [
        // `ensures result` bounds discharge the call-index obligation.
        ("machine idx() -> u64 ensures result < 4 { 1 }", true),
        ("machine idx() -> u64 ensures result <= 3 { 1 }", true),
        ("machine idx() -> u64 ensures result == 1 { 1 }", true),
        (
            "machine idx() -> u64 ensures result <= 3 && result >= 0 { 1 }",
            true,
        ),
        // A signed result needs its non-negative half from the contract too.
        (
            "machine idx() -> i64 ensures result >= 0 && result <= 3 { 1 }",
            true,
        ),
        ("machine idx() -> i64 ensures result <= 3 { 1 }", false),
        // A bound at the collection length is still out of range.
        ("machine idx() -> u64 ensures result <= 4 { 1 }", false),
        // No contract bound at all keeps the ordinary rejection.
        ("machine idx() -> u64 { 1 }", false),
        // A `result`-named parameter shadows the binder: the conjunct then
        // bounds that parameter, never the return value.
        (
            "machine pick(result: u64 [0..=3]) -> u64 ensures result <= 3 { result }",
            false,
        ),
    ] {
        let call = if callee.starts_with("machine pick") {
            "pick(2)"
        } else {
            "idx()"
        };
        let source =
            format!("{callee} machine write(output: &mut [u8; 4]) {{ output[{call}] = 65; }}");
        match (check(&source), accepted) {
            (Ok(()), true) => {}
            (Err(messages), false) => assert!(
                messages
                    .iter()
                    .any(|message| message.contains("cannot prove index")),
                "{callee}: {messages:?}"
            ),
            (result, _) => panic!("{callee}: {result:?}"),
        }
    }
}

/// The byte-write leg: a statement can carry two call occurrences — the index
/// selector and the assigned source call. The selector's bound comes from its
/// callee's `ensures`; the written byte's bound comes from the captured source
/// call. Neither occurrence disturbs the other's evidence.
#[test]
fn call_index_alongside_the_source_call_keeps_occurrence_custody() {
    fn check(source: &str) -> Result<(), Vec<String>> {
        checked_program_result(source)
            .map(|_| ())
            .map_err(|diagnostics| {
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.clone())
                    .collect()
            })
    }
    let digit = "machine digit(value: u64) -> u8 { ((value % 10 + 48) as u8 in Wrapping) as u8 }";
    // The ensured selector and the captured byte source compose into one
    // proved write; the domain contract mirrors the indexed-text writer case.
    let source = format!(
        "domain [u8; 4]::Ascii requires ascii_only(self); \
         machine idx() -> u64 ensures result <= 3 {{ 1 }} {digit} \
         machine write(output: &mut [u8; 4], unknown: u64) \
         requires output in Ascii ensures output in Ascii \
         {{ output[idx()] = digit(unknown); }}"
    );
    check(&source).unwrap_or_else(|messages| {
        panic!("ensured call index + captured byte source must compose: {messages:?}")
    });
    // An unbounded selector still rejects — the byte evidence never implies
    // the index proof.
    let source = format!(
        "domain [u8; 4]::Ascii requires ascii_only(self); \
         machine idx() -> u64 {{ 1 }} {digit} \
         machine write(output: &mut [u8; 4], unknown: u64) \
         requires output in Ascii ensures output in Ascii \
         {{ output[idx()] = digit(unknown); }}"
    );
    let Err(messages) = check(&source) else {
        panic!("an unbounded call index must still reject");
    };
    assert!(
        messages
            .iter()
            .any(|message| message.contains("cannot prove index")),
        "{messages:?}"
    );
}

/// A receiver call's `ensures` contract bounds the result the same way — the
/// receiver supplies `self`, never the return value.
#[test]
fn receiver_call_index_reads_the_callee_ensures() {
    fn check(source: &str) -> Result<(), Vec<String>> {
        checked_program_result(source)
            .map(|_| ())
            .map_err(|diagnostics| {
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.clone())
                    .collect()
            })
    }
    for (callee, accepted) in [
        (
            "data Pad { cell: u64 [0..=9]; } \
             machine Pad::idx(&mut self) -> u64 ensures result <= 3 { 1 }",
            true,
        ),
        (
            "data Pad { cell: u64 [0..=9]; } \
             machine Pad::idx(&mut self) -> u64 { 1 }",
            false,
        ),
    ] {
        let source = format!(
            "{callee} machine write(output: &mut [u8; 4], pad: &mut Pad) {{ output[pad.idx()] = 65; }}"
        );
        let result = check(&source);
        assert_eq!(result.is_ok(), accepted, "{callee}: {result:?}");
    }
}

/// `let i = idx()` binds THIS call's result, so the callee's ensured literal
/// bounds follow the local into the index position — seeded on the local's
/// label at the binding point and retired by reassignment like any other
/// label-keyed bound. A chained `let j = i` inherits them through the ordinary
/// alias transfer; a `result`-unbounded or out-of-range contract still rejects.
#[test]
fn call_result_alias_carries_the_ensured_result_bounds() {
    fn check(source: &str) -> Result<(), Vec<String>> {
        checked_program_result(source)
            .map(|_| ())
            .map_err(|diagnostics| {
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.clone())
                    .collect()
            })
    }
    for (callee, binding, accepted) in [
        // The ensured upper bound transfers through the binding.
        (
            "machine idx() -> u64 ensures result <= 3 { 1 }",
            "let i: u64 = idx(); output[i] = 65;",
            true,
        ),
        (
            "machine idx() -> u64 ensures result < 4 { 1 }",
            "let i: u64 = idx(); output[i] = 65;",
            true,
        ),
        // A signed result still owes its non-negative half to the contract.
        (
            "machine idx() -> i64 ensures result >= 0 && result <= 3 { 1 }",
            "let i: i64 = idx(); output[i] = 65;",
            true,
        ),
        (
            "machine idx() -> i64 ensures result <= 3 { 1 }",
            "let i: i64 = idx(); output[i] = 65;",
            false,
        ),
        // The ensured bound at the collection length is still out of range.
        (
            "machine idx() -> u64 ensures result <= 4 { 1 }",
            "let i: u64 = idx(); output[i] = 65;",
            false,
        ),
        // No contract bound keeps the ordinary rejection.
        (
            "machine idx() -> u64 { 1 }",
            "let i: u64 = idx(); output[i] = 65;",
            false,
        ),
        // A chained copy inherits the seeded bound through alias_index.
        (
            "machine idx() -> u64 ensures result <= 3 { 1 }",
            "let i: u64 = idx(); let j: u64 = i; output[j] = 65;",
            true,
        ),
        // Rebinding the name to a new ensured call re-seeds the bound.
        (
            "machine idx() -> u64 ensures result <= 3 { 1 }",
            "let mut i: u64 = idx(); i = idx(); output[i] = 65;",
            true,
        ),
        // Rebinding the name retires the stale bound: an unknown store must
        // not keep the initializer's contract.
        (
            "machine idx() -> u64 ensures result <= 3 { 1 }",
            "let mut i: u64 = idx(); i = unknown; output[i] = 65;",
            false,
        ),
    ] {
        let source =
            format!("{callee} machine write(output: &mut [u8; 4], unknown: u64) {{ {binding} }}");
        match (check(&source), accepted) {
            (Ok(()), true) => {}
            (Err(messages), false) => assert!(
                messages
                    .iter()
                    .any(|message| message.contains("cannot prove index")),
                "{callee} | {binding}: {messages:?}"
            ),
            (result, _) => panic!("{callee} | {binding}: {result:?}"),
        }
    }
    // A field assignment binds the call result the same way — the ensured
    // bound keys on the member's display label.
    for (binding, accepted) in [
        ("self.slot = idx(); output[self.slot] = 65;", true),
        ("self.slot = unknown; output[self.slot] = 65;", false),
    ] {
        let source = format!(
            "data Holder {{ slot: u64 }} \
             machine idx() -> u64 ensures result <= 3 {{ 1 }} \
             machine Holder::write(&mut self, output: &mut [u8; 4], unknown: u64) {{ {binding} }}"
        );
        let result = check(&source);
        assert_eq!(result.is_ok(), accepted, "{binding}: {result:?}");
    }
}

/// An unknown-length slice consults the same ensured result contract: the
/// call's inclusive high is met against the collection's own length evidence —
/// a `requires`-seeded `minimum_length` floor or a shrunk window's derived
/// floor — exactly like a folded literal bound, and a signed result's `>= 0`
/// conjunct supplies the non-negativity the collection-relative vocabulary
/// never implies. Without a floor fact the contract alone cannot prove
/// `result < len`; a bound at the floor is still out of range.
#[test]
fn call_index_on_unknown_slice_meets_ensured_bounds_against_length_facts() {
    fn check(source: &str) -> Result<(), Vec<String>> {
        checked_program_result(source)
            .map(|_| ())
            .map_err(|diagnostics| {
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.clone())
                    .collect()
            })
    }
    for (callee, contract, body, accepted) in [
        // The ensured inclusive high meets the `requires` length floor.
        (
            "machine idx() -> u64 ensures result <= 3 { 1 }",
            "requires output.len >= 4",
            "output[idx()] = 65;",
            true,
        ),
        (
            "machine idx() -> u64 ensures result < 4 { 1 }",
            "requires output.len >= 4",
            "output[idx()] = 65;",
            true,
        ),
        (
            "machine idx() -> u64 ensures result == 1 { 1 }",
            "requires output.len >= 4",
            "output[idx()] = 65;",
            true,
        ),
        // A signed result owes both halves to the contract: `>= 0` supplies
        // the non-negativity the slice vocabulary cannot imply.
        (
            "machine idx() -> i64 ensures result >= 0 && result <= 3 { 1 }",
            "requires output.len >= 4",
            "output[idx()] = 65;",
            true,
        ),
        (
            "machine idx() -> i64 ensures result <= 3 { 1 }",
            "requires output.len >= 4",
            "output[idx()] = 65;",
            false,
        ),
        // The ensured bound AT the floor is still out of range.
        (
            "machine idx() -> u64 ensures result <= 4 { 1 }",
            "requires output.len >= 4",
            "output[idx()] = 65;",
            false,
        ),
        // No length floor: the contract alone cannot prove `result < len`.
        (
            "machine idx() -> u64 ensures result <= 3 { 1 }",
            "",
            "output[idx()] = 65;",
            false,
        ),
        // No contract bound keeps the ordinary rejection.
        (
            "machine idx() -> u64 { 1 }",
            "requires output.len >= 4",
            "output[idx()] = 65;",
            false,
        ),
        // A shrunk tail window's derived floor (`4 - 2`) meets the bound too.
        (
            "machine idx() -> u64 ensures result <= 1 { 1 }",
            "requires output.len >= 4",
            "let tail: &[u8] = output[2..]; let picked: u8 = tail[idx()];",
            true,
        ),
        (
            "machine idx() -> u64 ensures result <= 2 { 1 }",
            "requires output.len >= 4",
            "let tail: &[u8] = output[2..]; let picked: u8 = tail[idx()];",
            false,
        ),
    ] {
        let source = format!("{callee} machine write(output: &mut [u8]) {contract} {{ {body} }}");
        match (check(&source), accepted) {
            (Ok(()), true) => {}
            (Err(messages), false) => assert!(
                messages
                    .iter()
                    .any(|message| message.contains("cannot prove")),
                "{callee} | {body}: {messages:?}"
            ),
            (result, _) => panic!("{callee} | {body}: {result:?}"),
        }
    }
}

/// A label-keyed exclusive upper bound — seeded by a `let` alias of an ensured
/// call (`i < 4` from `ensures result <= 3`) — meets an unknown slice's
/// `minimum_length`/`exact_length` floor exactly like a folded literal:
/// `i < u` and `u <= floor` give `i < len`. The same bound at `u - 1 <= floor`
/// discharges an exclusive range end. A bound past the floor and a missing
/// floor keep the ordinary rejection; rebinding the name retires it.
#[test]
fn unknown_slice_index_meets_label_upper_bounds_against_length_facts() {
    for (callee, contract, body, accepted) in [
        // The ensured inclusive high seeds `i < 4`, which meets the
        // `requires`-seeded `minimum_length` floor.
        (
            "machine idx() -> u64 ensures result <= 3 { 1 }",
            "requires output.len >= 4",
            "let i: u64 = idx(); output[i] = 65;",
            true,
        ),
        (
            "machine idx() -> u64 ensures result < 4 { 1 }",
            "requires output.len >= 4",
            "let i: u64 = idx(); output[i] = 65;",
            true,
        ),
        (
            "machine idx() -> u64 ensures result == 1 { 1 }",
            "requires output.len >= 4",
            "let i: u64 = idx(); output[i] = 65;",
            true,
        ),
        // A signed alias owes its lower half to the ensured `>= 0` conjunct
        // the alias seeding already publishes as a non-negative fact.
        (
            "machine idx() -> i64 ensures result >= 0 && result <= 3 { 1 }",
            "requires output.len >= 4",
            "let i: i64 = idx(); output[i] = 65;",
            true,
        ),
        (
            "machine idx() -> i64 ensures result <= 3 { 1 }",
            "requires output.len >= 4",
            "let i: i64 = idx(); output[i] = 65;",
            false,
        ),
        // `i < 5` against a floor of 4 is still out of range.
        (
            "machine idx() -> u64 ensures result <= 4 { 1 }",
            "requires output.len >= 4",
            "let i: u64 = idx(); output[i] = 65;",
            false,
        ),
        // No length floor: the bound alone cannot prove `i < len`.
        (
            "machine idx() -> u64 ensures result <= 3 { 1 }",
            "",
            "let i: u64 = idx(); output[i] = 65;",
            false,
        ),
        // A chained copy inherits the seeded bound through alias_index.
        (
            "machine idx() -> u64 ensures result <= 3 { 1 }",
            "requires output.len >= 4",
            "let i: u64 = idx(); let j: u64 = i; output[j] = 65;",
            true,
        ),
        // Rebinding the name retires the stale bound: an unbounded call must
        // not keep the initializer's contract.
        (
            "machine raw() -> u64 { 7 } machine idx() -> u64 ensures result <= 3 { 1 }",
            "requires output.len >= 4",
            "let mut i: u64 = idx(); i = raw(); output[i] = 65;",
            false,
        ),
        // A constant-window `exact_length` meets the bound the same way.
        (
            "machine idx() -> u64 ensures result <= 3 { 1 }",
            "requires output.len >= 4",
            "let tail: &[u8] = output[0..4]; let i: u64 = idx(); let picked: u8 = tail[i];",
            true,
        ),
        (
            "machine idx() -> u64 ensures result <= 4 { 1 }",
            "requires output.len >= 4",
            "let tail: &[u8] = output[0..4]; let i: u64 = idx(); let picked: u8 = tail[i];",
            false,
        ),
        // The exclusive range end discharges `i <= len` at `u - 1 <= floor`:
        // `i < 5` gives `i <= 4`, exactly the floor.
        (
            "machine idx() -> u64 ensures result <= 4 { 1 }",
            "requires output.len >= 4",
            "let i: u64 = idx(); let window: &[u8] = output[..i];",
            true,
        ),
        (
            "machine idx() -> u64 ensures result <= 5 { 1 }",
            "requires output.len >= 4",
            "let i: u64 = idx(); let window: &[u8] = output[..i];",
            false,
        ),
    ] {
        let source = format!("{callee} machine write(output: &mut [u8]) {contract} {{ {body} }}");
        match (check_source(&source), accepted) {
            (Ok(()), true) => {}
            (Err(messages), false) => assert!(
                messages
                    .iter()
                    .any(|message| message.contains("cannot prove")),
                "{callee} | {body}: {messages:?}"
            ),
            (result, _) => panic!("{callee} | {body}: {result:?}"),
        }
    }
}

/// The `i < K` guard seeds the same label-keyed bound: a guarded transition
/// arm meets it against the collection's floor, an `i <= pivot` ordering chain
/// reaches the pivot's bound one hop out, and a signed index still owes its
/// `>= 0` half. The value-target `items[i]` is checked under the guard's facts.
#[test]
fn unknown_slice_index_meets_guard_seeded_upper_bounds_against_length_facts() {
    for (parameters, contract, guard, accepted) in [
        ("", "requires items.len >= 4", "i < 4", true),
        ("", "requires items.len >= 4", "i <= 3", true),
        // `i < 5` against a floor of 4 is still out of range.
        ("", "requires items.len >= 4", "i < 5", false),
        // No length floor: the guard bound alone cannot prove `i < len`.
        ("", "", "i < 4", false),
        // `i <= pivot` plus the pivot's `j < 4` bound chains one hop.
        (
            ", j: u64",
            "requires items.len >= 4",
            "i <= j && j < 4",
            true,
        ),
        (
            ", j: u64",
            "requires items.len >= 4",
            "i <= j && j < 5",
            false,
        ),
    ] {
        let source = format!(
            "machine read(items: &[u8], i: u64{parameters}) -> u8 {contract} {{
                transition {guard} {{ true -> (items[i]) false -> (0) }}
            }}"
        );
        match (check_source(&source), accepted) {
            (Ok(()), true) => {}
            (Err(messages), false) => assert!(
                messages
                    .iter()
                    .any(|message| message.contains("cannot prove")),
                "{parameters} | {guard}: {messages:?}"
            ),
            (result, _) => panic!("{parameters} | {guard}: {result:?}"),
        }
    }
    // The signed lane needs its own parameter spelling.
    for (guard, accepted) in [("i >= 0 && i < 4", true), ("i < 4", false)] {
        let source = format!(
            "machine read(items: &[u8], i: i64) -> u8 requires items.len >= 4 {{
                transition {guard} {{ true -> (items[i]) false -> (0) }}
            }}"
        );
        let result = check_source(&source);
        assert_eq!(result.is_ok(), accepted, "{guard}: {result:?}");
    }
}

/// Indexed-member guards: `self.<array-field>[index]` inside transitions and
/// state bodies discharges through the same bound lanes as parameters — a
/// literal index against the fixed extent, a prior transition's `<`/`<=`
/// bound on a field or transported state argument, a member slice's `.len`
/// bound, and a `requires` clause. The collection lives on `self` rather
/// than in a parameter position, but the label-keyed proof facts are the
/// same ones the parameter fixtures exercise.
#[test]
fn member_index_guards_meet_guard_and_state_bounds() {
    for (fields, machine, accepted) in [
        // Constant index against the member's fixed extent.
        (
            "control: [u8; 64];",
            "machine Queryer::probe(&mut self) -> u64 {
                transition self.control[3] >= 48 { true -> (1) false -> (0) }
            }",
            true,
        ),
        // A constant index past the extent stays rejected.
        (
            "control: [u8; 64];",
            "machine Queryer::probe(&mut self) -> u64 {
                transition self.control[64] >= 48 { true -> (1) false -> (0) }
            }",
            false,
        ),
        // An unbounded index stays rejected.
        (
            "control: [u8; 64];",
            "machine Queryer::probe(&mut self, i: u64) -> u64 {
                transition self.control[i] >= 48 { true -> (1) false -> (0) }
            }",
            false,
        ),
        // A `pos < N` transition bound carries the field index into the
        // guarded state's reads and stores (squalr's `put_leaf` shape).
        (
            "control: [u8; 64]; pos: u64;",
            "machine Queryer::probe(&mut self) -> u64 {
                transition self.pos < 64 { true -> store() false -> out() }
                state store(&mut self) -> u64 {
                    self.control[self.pos] = 1;
                    self.control[self.pos]
                }
                state out(&mut self) -> u64 { 0 }
            }",
            true,
        ),
        // The inclusive spelling `pos <= N - 1` reaches the same bound.
        (
            "control: [u8; 64]; pos: u64;",
            "machine Queryer::probe(&mut self) -> u64 {
                transition self.pos <= 63 { true -> read() false -> out() }
                state read(&mut self) -> u64 { self.control[self.pos] }
                state out(&mut self) -> u64 { 0 }
            }",
            true,
        ),
        // The same bound survives an unconditional hop through an
        // intermediate state (the facts transport).
        (
            "control: [u8; 64]; pos: u64;",
            "machine Queryer::probe(&mut self) -> u64 {
                transition self.pos < 64 { true -> mid() false -> out() }
                state mid(&mut self) {
                    transition { _ -> read() }
                }
                state read(&mut self) -> u64 {
                    transition self.control[self.pos] >= 48 { true -> (1) false -> (0) }
                }
                state out(&mut self) -> u64 { 0 }
            }",
            true,
        ),
        // A `pos < len` bound on a borrowed member slice discharges the
        // member-slice index (boyer-moore `self.table[i]` shape).
        (
            "pos: u64;",
            "machine Queryer::probe(&mut self, table: &[u8]) -> u64 {
                transition self.pos < table.len { true -> read(table) false -> out() }
                state read(&mut self, table: &[u8]) -> u64 { table[self.pos] }
                state out(&mut self) -> u64 { 0 }
            }",
            true,
        ),
        // A machine-level `requires` bound discharges the member index.
        (
            "control: [u8; 64];",
            "machine Queryer::probe(&mut self, i: u64) -> u64
            requires
                i < 64
            {
                transition self.control[i] >= 48 { true -> (1) false -> (0) }
            }",
            true,
        ),
        // A state-parameter declared range discharges the member index (the
        // caller hands a literal already inside it).
        (
            "control: [u8; 64];",
            "machine Queryer::probe(&mut self) -> u64 {
                transition { _ -> read(12) }
                state read(&mut self, i: u64 [0..64]) -> u64 { self.control[i] }
            }",
            true,
        ),
        // A scalar statement binding established before the transition.
        (
            "control: [u8; 64]; pos: u64;",
            "machine Queryer::probe(&mut self) -> u64 {
                self.pos = 12;
                transition self.control[self.pos] >= 48 { true -> (1) false -> (0) }
            }",
            true,
        ),
    ] {
        let source = format!("data Queryer {{ {fields} }} {machine}");
        match (check_source(&source), accepted) {
            (Ok(()), true) => {}
            (Err(messages), false) => assert!(
                messages
                    .iter()
                    .any(|message| message.contains("cannot prove index")),
                "{fields} | {machine}: {messages:?}"
            ),
            (result, _) => panic!("{fields} | {machine}: {result:?}"),
        }
    }
    // `index < collection.len` seeded on a transition carries the proven-index
    // pair across the state's argument transport (squalr's needle_copy ->
    // needle_store -> needle_store_raw shape): the transported slice parameter
    // keeps its proven element index whether the index is a transported
    // parameter or a bounded field.
    for source in [
        "machine Queryer::probe(&mut self, arr: &[u8], i: u64) -> u64 {
            transition i < arr.len { true -> read(arr, i) false -> out() }
            state read(&mut self, arr: &[u8], i: u64) -> u64 { arr[i] }
            state out(&mut self) -> u64 { 0 }
        }",
        "data Queryer { pos: u64; }
        machine Queryer::probe(&mut self, arr: &[u8]) -> u64 {
            transition self.pos < arr.len { true -> read(arr) false -> out() }
            state read(&mut self, arr: &[u8]) -> u64 { arr[self.pos] }
            state out(&mut self) -> u64 { 0 }
        }",
    ] {
        let result = check_source(source);
        assert!(result.is_ok(), "{source}: {result:?}");
    }
}

/// A `len - offset` subtrahend under an exclusive range end reads the offset's
/// ensured call bounds too: the ensured `>= 0` conjunct supplies the
/// non-negativity a signed offset still owes, and the ensured inclusive high
/// met against the floor supplies `offset <= len`.
#[test]
fn length_difference_offset_reads_ensured_result_bounds() {
    for (callee, contract, body, accepted) in [
        (
            "machine idx() -> u64 ensures result <= 3 { 1 }",
            "requires output.len >= 4",
            "let window: &[u8] = output[..output.len - idx()];",
            true,
        ),
        // `result <= 4` still fits: the offset can equal the length floor.
        (
            "machine idx() -> u64 ensures result <= 4 { 1 }",
            "requires output.len >= 4",
            "let window: &[u8] = output[..output.len - idx()];",
            true,
        ),
        (
            "machine idx() -> u64 ensures result <= 5 { 1 }",
            "requires output.len >= 4",
            "let window: &[u8] = output[..output.len - idx()];",
            false,
        ),
        // No contract bound keeps the ordinary rejection.
        (
            "machine idx() -> u64 { 1 }",
            "requires output.len >= 4",
            "let window: &[u8] = output[..output.len - idx()];",
            false,
        ),
    ] {
        let source = format!("{callee} machine write(output: &mut [u8]) {contract} {{ {body} }}");
        match (check_source(&source), accepted) {
            (Ok(()), true) => {}
            (Err(messages), false) => assert!(
                messages
                    .iter()
                    .any(|message| message.contains("cannot prove")),
                "{callee} | {body}: {messages:?}"
            ),
            (result, _) => panic!("{callee} | {body}: {result:?}"),
        }
    }
}

#[test]
fn nested_index_traversal_checks_each_collection_extent() {
    for (access, accepted) in [("[3][1]", true), ("[4][1]", false), ("[3][2]", false)] {
        let (program, _, _) = fixture("[[u8; 2]; 4]", access);
        let frames = validation::CallFrameResolver::new(&program);
        let incoming = crate::checks::ranges::incoming_guards::IncomingGuardIndex::build(
            &program,
            frames.as_ref(),
        );
        let checked = crate::checks::ranges::check_indexed_accesses(
            &program,
            &checked_trees::CheckedOperatorFacts::default(),
            &checked_trees::BorrowFacts::default(),
            &checked_trees::FlowFacts::default(),
            frames.as_ref(),
            &incoming,
            &crate::flow::StateMutationSummaryCache::default(),
        );
        assert_eq!(checked.is_ok(), accepted, "{access}: {checked:?}");
    }
}

// CONST-GENERIC-EXTENT-RANGE-DISCHARGE: indexing a `[T; N]` const-extent
// collection discharges `index < N` / `end <= N` against the binder itself —
// its declared floor, the index's `u64[..N]` declared range, and the ordinary
// collection-keyed facts. An unproven obligation rejects like every other
// extent lane; it may not fall through to silent accept.
#[test]
fn const_generic_extent_index_discharge() {
    for (source, accepted) in [
        // Unproven scalar index is a real obligation, not a silent pass.
        (
            "machine inspect<const N: u64>(items: &[u8; N], index: u64) { let x: u8 = items[index]; }",
            false,
        ),
        // `u64[0..=N]` declares `index <= N` — the strict form `index < N`
        // stays unproven and must reject.
        (
            "machine inspect<const N: u64>(items: &[u8; N], index: u64[0..=N]) { let x: u8 = items[index]; }",
            false,
        ),
        // `u64[0..N]` is the language's "index of `[T; N]`" spelling.
        (
            "machine inspect<const N: u64>(items: &[u8; N], index: u64[0..N]) { let x: u8 = items[index]; }",
            true,
        ),
        // A literal index is provable exactly when it lies below the binder's
        // declared floor: `N: u64` admits `N == 0`, so `items[0]` cannot hold.
        (
            "machine inspect<const N: u64>(items: &[u8; N]) { let x: u8 = items[0]; }",
            false,
        ),
        (
            "machine inspect<const N: u64[1..=18446744073709551615]>(items: &[u8; N]) { let x: u8 = items[0]; }",
            true,
        ),
        // A closed declared high below the binder's floor composes:
        // `index <= 3` and `3 < 5 <= N`.
        (
            "machine inspect<const N: u64[5..=18446744073709551615]>(items: &[u8; N], index: u64[0..=3]) { let x: u8 = items[index]; }",
            true,
        ),
        // Attached-data fields on a generic record take the same lane.
        (
            "data FixedBuffer<const N: u64> { items: [i32 in Wrapping; N]; } machine FixedBuffer::poke(&mut self, i: u64, value: i32 in Wrapping) { self.items[i] = value; }",
            false,
        ),
        (
            "data FixedBuffer<const N: u64[1..=18446744073709551615]> { items: [i32 in Wrapping; N]; } machine FixedBuffer::set_first(&mut self, value: i32 in Wrapping) { self.items[0] = value; }",
            true,
        ),
    ] {
        let result = check_source(source);
        assert_eq!(result.is_ok(), accepted, "{source}: {result:?}");
        if accepted {
            continue;
        }
        assert!(
            result
                .unwrap_err()
                .iter()
                .any(|message| message.contains("const extent")),
            "{source}"
        );
    }
}

// Range windows over a const-generic extent discharge `end <= N` — and the
// strict `end < N` for an inclusive end.
#[test]
fn const_generic_extent_range_discharge() {
    for (source, accepted) in [
        // `items[..N]` names the extent itself — `N <= N` holds trivially.
        (
            "machine inspect<const N: u64>(items: &[u8; N]) { let x: &[u8] = &items[..N]; }",
            true,
        ),
        (
            "machine inspect<const N: u64>(items: &[u8; N], start: u64[0..=N]) { let x: &[u8] = &items[start..N]; }",
            true,
        ),
        // The inclusive end is itself an index: `items[..=N]` reads `N`, out
        // of bounds for every admissible `N`.
        (
            "machine inspect<const N: u64>(items: &[u8; N]) { let x: &[u8] = &items[..=N]; }",
            false,
        ),
        // An unproven window end owes `end <= N`.
        (
            "machine inspect<const N: u64>(items: &[u8; N], end: u64) { let x: &[u8] = &items[..end]; }",
            false,
        ),
        // `end: u64[0..=N]` asserts `end <= N` — sufficient for the window end.
        (
            "machine inspect<const N: u64>(items: &[u8; N], end: u64[0..=N]) { let x: &[u8] = &items[..end]; }",
            true,
        ),
        // The builtin length spelling of the same place discharges the end.
        (
            "machine inspect<const N: u64>(items: &[u8; N]) { let x: &[u8] = &items[..items.len]; }",
            true,
        ),
        // A guard-minted collection/index fact carries the pair regardless
        // of whether the extent is symbolic or concrete.
        (
            "machine inspect<const N: u64>(items: &[u8; N], index: u64, end: u64) { transition index < items.len && end <= items.len { true -> ok() _ -> fail() } state ok(&mut self) { } state fail(&mut self) { } }",
            true,
        ),
    ] {
        let result = check_source(source);
        assert_eq!(result.is_ok(), accepted, "{source}: {result:?}");
        if accepted {
            continue;
        }
        assert!(
            result
                .unwrap_err()
                .iter()
                .any(|message| message.contains("const extent")),
            "{source}"
        );
    }
}

/// Guards whose scalar reads bottom out in a borrowed/member root rather
/// than a parameter — borrowed member slices (`data<'a> { &'a mut [u8] }`),
/// member arrays, nested member paths, and member `.len` comparisons —
/// discharge through the same bound lanes as parameter-rooted guards:
/// `index < self.slice.len` mints the proven-index pair and floors the
/// slice length at the index's own lower bound, so a later literal-`0`
/// element read or a transported state's member read validates.
#[test]
fn borrowed_root_guards_meet_member_slice_bounds() {
    for (source, accepted) in [
        // Member-slice field, param index, `i < self.table.len` conjunct.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, i: u64) -> u8 {
            transition i < self.table.len && self.table[i] >= 48 { true -> (1) false -> (0) }
        }", true),
        // Member-slice field, param index, requires-bound.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, i: u64) -> u8
            requires i < self.table.len {
            transition self.table[i] >= 48 { true -> (1) false -> (0) }
        }", true),
        // Member-slice field, field index, `self.pos < self.table.len` conjunct.
        ("data Main<'a> { table: &'a mut [u8]; pos: u64; } machine Main::run(&mut self) -> u8 {
            transition self.pos < self.table.len && self.table[self.pos] >= 48 { true -> (1) false -> (0) }
        }", true),
        // Member-slice read inside a named state (transported param index).
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, i: u64) -> u8 {
            transition i < self.table.len { true -> read(i) false -> (0) }
            state read(&mut self, i: u64) -> u8 { self.table[i] }
        }", true),
        // Comparison side = member read (non-param) vs literal.
        ("data Main { count: u64; } machine Main::run(&mut self) -> u8 {
            transition self.count >= 48 { true -> (1) false -> (0) }
        }", true),
        // Nested member path read.
        ("data Inner { count: u64; } data Main { inner: Inner; } machine Main::run(&mut self) -> u8 {
            transition self.inner.count >= 48 { true -> (1) false -> (0) }
        }", true),
        // Member-array index through `&&` conjunct on param index.
        ("data Main { items: [u8; 64]; } machine Main::run(&mut self, i: u64) -> u8 {
            transition i < self.items.len && self.items[i] >= 48 { true -> (1) false -> (0) }
        }", true),
        // Borrowed member-slice read inside a state, index = self field.
        ("data Main<'a> { table: &'a mut [u8]; pos: u64; } machine Main::run(&mut self) -> u8 {
            transition self.pos < self.table.len { true -> read() false -> (0) }
            state read(&mut self) -> u8 { self.table[self.pos] }
        }", true),
        // Member-slice `.len` itself as the guard.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self) -> u8 {
            transition self.table.len >= 48 { true -> (1) false -> (0) }
        }", true),
        // `pos < len` floors the slice at len >= 1: literal-0 element read.
        ("data Main<'a> { table: &'a mut [u8]; pos: u64; } machine Main::run(&mut self) -> u8 {
            transition self.pos < self.table.len && self.table[0] >= 48 { true -> (1) false -> (0) }
        }", true),
        // Element-vs-element, param indexes both bounded.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, i: u64, j: u64) -> u8 {
            transition i < self.table.len && j < self.table.len && self.table[i] >= self.table[j] { true -> (1) false -> (0) }
        }", true),
        // The floor rides the state-edge pair transport (self.field side).
        ("data Main<'a> { table: &'a mut [u8]; pos: u64; } machine Main::run(&mut self) -> u8 {
            transition self.pos < self.table.len { true -> read() false -> (0) }
            state read(&mut self) -> u8 { self.table[0] }
        }", true),
        // Member-slice element vs member-array element.
        ("data Main<'a> { table: &'a mut [u8]; pos: u64; items: [u8; 64]; } machine Main::run(&mut self) -> u8 {
            transition self.pos < self.table.len && self.table[self.pos] >= self.items[0] { true -> (1) false -> (0) }
        }", true),
        // Member-slice element vs element where one index is `self.pos`.
        ("data Main<'a> { table: &'a mut [u8]; pos: u64; } machine Main::run(&mut self) -> u8 {
            transition self.pos < self.table.len && self.table[self.pos] >= self.table[0] { true -> (1) false -> (0) }
        }", true),
        // A bare literal-0 read on an unbounded member slice still rejects.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self) -> u8 {
            transition self.table[0] >= 48 { true -> (1) false -> (0) }
        }", false),
        // A signed unproven index does not floor the slice: `pos` could be
        // negative, so `pos < len` cannot pin len >= 1.
        ("data Main<'a> { table: &'a mut [u8]; pos: i64; } machine Main::run(&mut self) -> u8 {
            transition self.pos < self.table.len && self.table[0] >= 48 { true -> (1) false -> (0) }
        }", false),
        // A literal index pins the floor exactly: `3 < len` ⇒ `len >= 4`,
        // so element reads below the floor validate.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self) -> u8 {
            transition 3 < self.table.len && self.table[2] >= 48 { true -> (1) false -> (0) }
        }", true),
        // `3 < len` floors the length at 4 exactly: `table[4]` still
        // out-of-reach — the bound is exclusive.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self) -> u8 {
            transition 3 < self.table.len && self.table[4] >= 48 { true -> (1) false -> (0) }
        }", false),
    ] {
        match (check_source(source), accepted) {
            (Ok(()), true) => {}
            (Err(messages), false) => assert!(
                messages
                    .iter()
                    .any(|message| message.contains("cannot prove index")),
                "{source}: {messages:?}"
            ),
            (result, _) => panic!("{source}: {result:?}"),
        }
    }
}

/// Strict-ordering guards keep their strictness: `index < pivot` mints a
/// `strictly_less` ordering, which lets the chained bound proofs accept a
/// pivot bound one element higher (`index < pivot <= len` proves
/// `index < len`). `>`/`>=` seed the mirrored ordering, and `==` unfolds to
/// `<=` in both directions plus the integer bound.
#[test]
fn strict_orderings_and_equality_chain_through_bounds() {
    for (source, accepted) in [
        // `pos < max` (strict) + `max <= len` proves `pos < len`.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64, max: u64) -> u8 {
            transition pos < max && max <= self.table.len && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", true),
        // Non-strict decay still cannot close it: `pos <= max && max <= len`
        // allows `pos == len` — correctly rejected.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64, max: u64) -> u8 {
            transition pos <= max && max <= self.table.len && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", false),
        // Mirrored `>` seeds the same strict ordering.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64, max: u64) -> u8 {
            transition max > pos && max <= self.table.len && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", true),
        // Mirrored `>=` is non-strict — still cannot close it.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64, max: u64) -> u8 {
            transition max >= pos && max <= self.table.len && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", false),
        // `pos < max` where max's bound comes from `max < len` directly —
        // the classic two-pointer shape stays green.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64, max: u64) -> u8 {
            transition pos < max && max < self.table.len && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", true),
        // `pos == K` unfolds to the exclusive bound: `pos <= K` gives
        // `pos < K + 1`, and `K < len` then proves `pos < len`... but only
        // when `K` itself is strictly below the length.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64) -> u8 {
            transition pos == 3 && 3 < self.table.len && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", true),
        // `pos == K` where `K <= len` only gives `pos <= len` — the edge
        // case `pos == len` is real, so the read must reject.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64) -> u8 {
            transition pos == 3 && self.table.len >= 3 && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", false),
        // Equality against a bounded companion: `pos == max` and
        // `max < len` chain pos <= max < len.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64, max: u64) -> u8 {
            transition pos == max && max < self.table.len && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", true),
        // A strict ordering across a state edge: the callee guard seeds
        // the same fact through the argument transport.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64, max: u64) -> u8 {
            transition pos < max && max <= self.table.len { true -> read(pos) false -> (0) }
            state read(&mut self, pos: u64) -> u8 { self.table[pos] }
        }", true),
        // Signed strict index: `pos < max` with `pos >= 0` proven — the
        // lower-bound half is still owed on a signed index. (`max <= 4` and
        // `len >= 4` close the upper half through the strict ordering.)
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: i64, max: i64) -> u8 {
            transition self.table.len >= 4 && pos >= 0 && pos < max && max <= 4 && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", true),
        // A strict ordering does not manufacture non-negativity: signed
        // `pos` without `pos >= 0` still rejects.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: i64, max: i64) -> u8 {
            transition self.table.len >= 4 && pos < max && max <= 4 && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", false),
    ] {
        match (check_source(source), accepted) {
            (Ok(()), true) => {}
            (Err(messages), false) => assert!(
                messages
                    .iter()
                    .any(|message| message.contains("cannot prove index")),
                "{source}: {messages:?}"
            ),
            (result, _) => panic!("{source}: {result:?}"),
        }
    }
}

/// A subtraction bound decomposes: `bound <= coll.len - k` (k >= 1) or
/// `bound < coll.len - k` (k >= 0) proves `bound` an index — the offset
/// pays the one element the bare `<=` cannot. The same decomposition on a
/// label RHS (`pos == max - 1`) relaxes to the strict ordering
/// `pos < max` and chains through `max`'s own bounds. `==`/`<=` against
/// `len` itself still correctly reject at the boundary.
#[test]
fn len_offset_bounds_decompose_through_index_proofs() {
    for (source, accepted) in [
        // `pos == len - 1` under the len > 0 premise the subtraction owes.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64) -> u8 {
            transition self.table.len > 0 && pos == self.table.len - 1 && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", true),
        // `pos <= len - 1` — the at-most offset pays the element.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64) -> u8 {
            transition self.table.len > 0 && pos <= self.table.len - 1 && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", true),
        // `pos < len - k` needs no offset help — strictness alone proves it.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64) -> u8 {
            transition self.table.len > 1 && pos < self.table.len - 1 && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", true),
        // Mirrored `len - k > pos` — the Greater arm's mirrored len seeder.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64) -> u8 {
            transition self.table.len > 1 && self.table.len - 1 > pos && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", true),
        // Mirrored `len - k >= pos` — non-strict, k >= 1 pays the element.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64) -> u8 {
            transition self.table.len > 1 && self.table.len - 1 >= pos && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", true),
        // A wider offset: `pos == len - 2` under `len >= 2`.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64) -> u8 {
            transition self.table.len >= 2 && pos == self.table.len - 2 && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", true),
        // The companion-label subtraction: `pos == max - 1` relaxes to the
        // strict ordering `pos < max` and chains `max <= len`.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64, max: u64) -> u8 {
            transition max > 0 && max <= self.table.len && pos == max - 1 && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", true),
        // Same decomposition from `pos <= max - 1`.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64, max: u64) -> u8 {
            transition max > 0 && max <= self.table.len && pos <= max - 1 && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", true),
        // `pos < max` itself stays the classic shape — nothing regresses.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64, max: u64) -> u8 {
            transition pos < max && max <= self.table.len && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", true),
        // The minted index pair rides the state-edge transport.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64) -> u8 {
            transition self.table.len > 0 && pos <= self.table.len - 1 { true -> read(pos) false -> (0) }
            state read(&mut self, pos: u64) -> u8 { self.table[pos] }
        }", true),
        // Boundary: `pos == len` is out of bounds — correctly rejected.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64) -> u8 {
            transition pos == self.table.len && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", false),
        // Boundary: `pos <= len` admits `pos == len` — correctly rejected.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64) -> u8 {
            transition pos <= self.table.len && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", false),
        // `pos == len - 0` is `pos == len` verbatim — correctly rejected.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64) -> u8 {
            transition pos == self.table.len - 0 && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", false),
        // A bound on a DIFFERENT collection's len never crosses over.
        ("data Main<'a> { table: &'a mut [u8]; other: &'a mut [u8]; } machine Main::run(&mut self, pos: u64) -> u8 {
            transition self.other.len > 0 && pos <= self.other.len - 1 && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", false),
        // A companion subtraction with no companion bound proves nothing.
        ("data Main<'a> { table: &'a mut [u8]; } machine Main::run(&mut self, pos: u64, max: u64) -> u8 {
            transition max > 0 && pos == max - 1 && self.table[pos] >= 48 { true -> (1) false -> (0) }
        }", false),
    ] {
        match (check_source(source), accepted) {
            (Ok(()), true) => {}
            (Err(messages), false) => assert!(
                messages
                    .iter()
                    .any(|message| message.contains("cannot prove index")),
                "{source}: {messages:?}"
            ),
            (result, _) => panic!("{source}: {result:?}"),
        }
    }
}
