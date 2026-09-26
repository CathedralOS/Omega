//! Indexed receiver admission stops at checked trees; Terminal production is separate.

use super::reject_source;
use crate::tests::front_end::checked_program_result;

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
        checked_program_result(&source).unwrap_or_else(|diagnostics| {
            panic!("content-independent path `{receiver}` must check: {diagnostics:#?}\n{source}")
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
        checked_program_result(&source).unwrap_or_else(|diagnostics| {
            panic!("non-observing indexed scalar call must check: {diagnostics:#?}\n{source}")
        });
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
            checked_program_result(&source).unwrap_or_else(|diagnostics| {
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
        checked_program_result(&source).unwrap_or_else(|diagnostics| {
            panic!("only the selected indexed target controls access: {diagnostics:#?}\n{source}")
        });
    }
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
            checked_program_result(&source).unwrap_or_else(|diagnostics| {
                panic!("an alias receiver and distinct argument remain disjoint: {diagnostics:#?}\n{source}")
            });
        }
    }
}
