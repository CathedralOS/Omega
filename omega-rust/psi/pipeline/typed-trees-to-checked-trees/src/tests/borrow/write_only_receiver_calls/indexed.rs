//! Indexed receiver admission stops at checked trees; Terminal production is separate.

use super::{check_source, reject_source};

fn source(signature: &str, body: &str, methods: &str) -> String {
    format!(
        "data Record [copy] {{ value: u16; }}
         data Entry [copy] {{ record: Record; }}
         data Container {{ records: [Record; 2]; entries: [Entry; 2]; }}
         {methods}
         machine {signature} {{ {body} }}"
    )
}

const REPLACE: &str =
    "machine Record::replace(&write self, replacement: u16) { self.value = replacement; }";

#[test]
fn literal_indexed_receiver_paths_check() {
    for (signature, receiver) in [
        ("exercise(records: &write [Record; 2])", "records[1]"),
        (
            "exercise(records: &write [[Record; 2]; 2])",
            "records[1][0]",
        ),
        (
            "exercise(container: &write Container)",
            "container.records[0]",
        ),
        ("exercise(entries: &write [Entry; 2])", "entries[0].record"),
        ("Container::exercise(&write self)", "self.entries[1].record"),
        ("Container::exercise(&write self)", "records[0]"),
    ] {
        let source = source(signature, &format!("{receiver}.replace(17);"), REPLACE);
        check_source(&source).unwrap_or_else(|diagnostics| {
            panic!("content-independent path `{receiver}` must check: {diagnostics:#?}\n{source}")
        });
    }
}

#[test]
fn proven_dynamic_indexed_receiver_paths_check() {
    for (signature, receiver) in [
        (
            "exercise(records: &write [Record; 2], index: u64 [0..=1])",
            "records[index]",
        ),
        (
            "exercise(records: &write [[Record; 2]; 2], row: u64 [0..=1], column: u64 [0..=1])",
            "records[row][column]",
        ),
        (
            "Container::exercise(&write self, index: u64 [0..=1])",
            "self.entries[index].record",
        ),
    ] {
        let source = source(signature, &format!("{receiver}.replace(17);"), REPLACE);
        check_source(&source).unwrap_or_else(|diagnostics| {
            panic!("caller-supplied bounds admit `{receiver}`: {diagnostics:#?}\n{source}")
        });
    }
}

#[test]
fn indexed_receiver_scalar_result_depends_only_on_written_input() {
    for (signature, receiver) in [
        (
            "exercise(records: &write [Record; 2], replacement: u16)",
            "records[0]",
        ),
        (
            "exercise(container: &write Container, index: u64 [0..=1], replacement: u16)",
            "container.entries[index].record",
        ),
        (
            "Container::exercise(&write self, replacement: u16)",
            "self.records[1]",
        ),
    ] {
        let source = source(
            signature,
            &format!("let written: u16 = {receiver}.replace(replacement);"),
            "machine Record::replace(&write self, replacement: u16) -> u16 {
                 self.value = replacement;
                 replacement
             }",
        );
        check_source(&source).unwrap_or_else(|diagnostics| {
            panic!("non-observing indexed scalar call must check: {diagnostics:#?}\n{source}")
        });
    }
}

#[test]
fn indexed_receiver_cannot_widen_to_shared_mutable_or_owned() {
    for access in ["&self", "&mut self", "self"] {
        for (binding, result, returned) in [("", "", ""), ("let written: u16 = ", "-> u16", "17")] {
            let source = source(
                "exercise(records: &write [Record; 2])",
                &format!("{binding}records[0].replace();"),
                &format!("machine Record::replace({access}) {result} {{ {returned} }}"),
            );
            // Even a constant-result or empty body requires its declared access.
            reject_source(&source, &["write-only parameter `records`", "observation"]);
        }
    }
}

#[test]
fn indexed_receiver_callee_cannot_read_even_after_replacement() {
    for replacement in ["", "self.value = 17;"] {
        let source = source(
            "exercise(records: &write [Record; 2])",
            "let written: u16 = records[0].replace();",
            &format!("machine Record::replace(&write self) -> u16 {{ {replacement} self.value }}"),
        );
        reject_source(
            &source,
            &[
                "reads field `value` from write-only parameter `self`",
                "never grants observation",
            ],
        );
    }
}

#[test]
fn indexed_receiver_selector_cannot_observe_write_only_data() {
    for (signature, body, expected) in [
        (
            "exercise(records: &write [Record; 2], index: &write u64)",
            "records[index].replace(17);",
            "reads write-only parameter `index`",
        ),
        (
            "exercise(records: &write [Record; 2], indexes: &write [u64; 2])",
            "records[indexes[0]].replace(17);",
            "reads through index projection of write-only parameter `indexes`",
        ),
        (
            "Container::exercise(&write self)",
            "self.records[self.records[0].value].replace(17);",
            "reads field `value` from write-only parameter `self`",
        ),
    ] {
        reject_source(&source(signature, body, REPLACE), &[expected]);
    }
}

#[test]
fn indexed_receiver_requires_an_ordinary_bounds_proof() {
    for (signature, receiver, expected) in [
        (
            "exercise(records: &write [Record; 2])",
            "records[2]",
            "cannot prove index `2` is within length 2",
        ),
        (
            "exercise(records: &write [Record; 2], index: u64)",
            "records[index]",
            "cannot prove index `index` is within length 2",
        ),
        (
            "exercise(records: &write [[Record; 2]; 2])",
            "records[0][2]",
            "cannot prove index `2` is within length 2",
        ),
    ] {
        reject_source(
            &source(signature, &format!("{receiver}.replace(17);"), REPLACE),
            &[expected],
        );
    }
}

#[test]
fn indexed_receiver_requires_builtin_index_operator_meaning() {
    for (declaration, accepted) in [
        (
            "boundary operator [] Collection::read(items: &[Record], index: u64) -> Record;",
            false,
        ),
        (
            "boundary operator [] Collection::read(items: &[u8], index: u64) -> u8;",
            true,
        ),
    ] {
        let source = source(
            "exercise(records: &write [Record; 2])",
            "records[0].replace(17);",
            &format!("{declaration}\n{REPLACE}"),
        );
        if accepted {
            check_source(&source).unwrap_or_else(|diagnostics| {
                panic!("unrelated indexing overload must not block admission: {diagnostics:#?}\n{source}")
            });
        } else {
            reject_source(
                &source,
                &[
                    "reads through index projection of write-only parameter `records`",
                    "never observation",
                ],
            );
        }
    }
}

#[test]
fn indexed_receiver_requires_builtin_selector_addition_meaning() {
    for (declaration, accepted) in [
        (
            "operator + u64::custom(left: u64, right: u64) -> u64;",
            false,
        ),
        (
            "operator + f64::custom(left: f64, right: f64) -> f64;",
            true,
        ),
    ] {
        let source = source(
            "exercise(records: &write [Record; 2])",
            "records[0u64 + 1u64].replace(17);",
            &format!("{declaration}\n{REPLACE}"),
        );
        if accepted {
            check_source(&source).unwrap_or_else(|diagnostics| {
                panic!("unrelated addition overload must not block admission: {diagnostics:#?}\n{source}")
            });
        } else {
            reject_source(
                &source,
                &[
                    "reads through index projection of write-only parameter `records`",
                    "never observation",
                ],
            );
        }
    }
}

#[test]
fn indexed_receiver_reference_sum_and_generic_paths_remain_fenced() {
    for (declarations, signature, receiver, root) in [
        (
            "data Holder<'data> { records: &'data [Record; 2]; }",
            "exercise<'data>(holder: &write Holder<'data>)",
            "holder.records[0]",
            "holder",
        ),
        (
            "data Choice [copy] { case Empty; case Value(value: u16); }
             machine Choice::replace(&write self, replacement: u16) {}",
            "exercise(choices: &write [Choice; 2])",
            "choices[0]",
            "choices",
        ),
        (
            "data Holder<T> { records: [Record; 2]; marker: T; }",
            "exercise<T>(holder: &write Holder<T>)",
            "holder.records[0]",
            "holder",
        ),
    ] {
        let source = source(
            signature,
            &format!("{receiver}.replace(17);"),
            &format!("{declarations}\n{REPLACE}"),
        );
        reject_source(
            &source,
            &[&format!("write-only parameter `{root}`"), "observation"],
        );
    }
}

#[test]
fn foreign_write_only_method_cannot_authorize_indexed_observing_target() {
    for foreign_first in [false, true] {
        let foreign = "data Foreign [copy] { value: u16; }
                       machine Foreign::replace(&write self) { self.value = 17; }";
        let selected = "machine Record::replace(&self) {}";
        let methods = if foreign_first {
            format!("{foreign}\n{selected}")
        } else {
            format!("{selected}\n{foreign}")
        };
        reject_source(
            &source(
                "exercise(records: &write [Record; 2])",
                "records[0].replace();",
                &methods,
            ),
            &["write-only parameter `records`", "observation"],
        );
    }
}

#[test]
fn foreign_observing_method_does_not_block_indexed_write_only_target() {
    for foreign_first in [false, true] {
        let foreign = "data Foreign [copy] { value: u16; }
                       machine Foreign::replace(&self, replacement: u16) -> u16 { self.value }";
        let selected = "machine Record::replace(&write self, replacement: u16) -> u16 {
                            self.value = replacement;
                            replacement
                        }";
        let methods = if foreign_first {
            format!("{foreign}\n{selected}")
        } else {
            format!("{selected}\n{foreign}")
        };
        let source = source(
            "exercise(records: &write [Record; 2])",
            "let written: u16 = records[0].replace(17);",
            &methods,
        );
        check_source(&source).unwrap_or_else(|diagnostics| {
            panic!("only the selected indexed target controls access: {diagnostics:#?}\n{source}")
        });
    }
}

#[test]
fn indexed_receiver_conflicts_with_its_whole_root_argument_only() {
    for argument in ["records", "other"] {
        let source = source(
            "exercise(records: &write [Record; 2], other: &write [Record; 2])",
            &format!("records[0].noop(&write {argument});"),
            "machine Record::noop(&write self, other: &write [Record; 2]) {}",
        );
        // Whole-root forwarding is independently supported; no aggregate
        // element subloan is needed to witness receiver exclusion.
        if argument == "records" {
            reject_source(
                &source,
                &[
                    "receives write-only",
                    "overlapping another argument in the same call",
                ],
            );
        } else {
            check_source(&source).unwrap_or_else(|diagnostics| {
                panic!("distinct whole roots remain disjoint: {diagnostics:#?}\n{source}")
            });
        }
    }
}

#[test]
fn indexed_receiver_conflicts_with_a_live_whole_root_loan() {
    let source = source(
        "Container::exercise(&write self)",
        "let held: &write Container = &write self;
         self.records[0].noop();
         held.records[0].value = 17;",
        "machine Record::noop(&write self) {}",
    );
    reject_source(
        &source,
        &["write-only receiver", "local borrow `held` is still active"],
    );
}

#[test]
fn indexed_receiver_alias_chain_preserves_overlap_and_distinct_root_access() {
    for (receiver, argument) in [
        ("child[0]", "other"),
        ("child[0]", "parent"),
        ("child[0]", "records"),
        ("child[index]", "other"),
        ("child[index]", "parent"),
    ] {
        let source = source(
            "exercise(records: &write [Record; 2], other: &write [Record; 2], index: u64 [0..=1])",
            &format!(
                "let parent: &write [Record; 2] = &write records;
                 let child: &write [Record; 2] = &write parent;
                 {receiver}.noop(&write {argument});"
            ),
            "machine Record::noop(&write self, other: &write [Record; 2]) {}",
        );
        // Exact whole-root reborrows preserve storage identity across both
        // aliases. Exclusion applies even when the selected method has no writes.
        if argument != "other" {
            reject_source(
                &source,
                &[
                    "receives write-only",
                    "overlapping another argument in the same call",
                ],
            );
        } else {
            check_source(&source).unwrap_or_else(|diagnostics| {
                panic!("an alias receiver and distinct argument remain disjoint: {diagnostics:#?}\n{source}")
            });
        }
    }
}

#[test]
fn indexed_alias_receiver_distinguishes_ancestor_authority_from_a_live_child() {
    for (body, accepted) in [
        (
            "child[0].noop(&write other); parent[1].noop(&write other);",
            true,
        ),
        (
            "parent[0].noop(&write other); child[1].noop(&write other);",
            false,
        ),
    ] {
        let source = source(
            "exercise(records: &write [Record; 2], other: &write [Record; 2])",
            &format!(
                "let parent: &write [Record; 2] = &write records;
                 let child: &write [Record; 2] = &write parent;
                 {body}"
            ),
            "machine Record::noop(&write self, other: &write [Record; 2]) {}",
        );
        // The parent's spelling remains live in both controls. A call through
        // the child uses its ancestry; a call through the suspended parent
        // would introduce a competing branch while the whole-array child lives.
        if accepted {
            check_source(&source).unwrap_or_else(|diagnostics| {
                panic!("a receiver may use its exact retained ancestor: {diagnostics:#?}\n{source}")
            });
        } else {
            reject_source(
                &source,
                &[
                    "write-only receiver",
                    "local borrow `child` is still active",
                ],
            );
        }
    }
}
