//! Retired write-only-borrow probe pins: semantic rejections that must keep
//! rejecting during checking, and the shapes that still stop at terminal
//! production's missing checked control plan
//! (`checked-trees-to-lowered-psi`'s dispatch hole — see WRITE-ONLY-BORROW's
//! board rows). A production pin that starts producing an artifact has closed
//! its slice of the hole and belongs in a topical sibling file with
//! caller-storage observation instead.

fn checked_error(source: &str) -> String {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    match typed_trees_to_checked_trees::lower_typed_trees(typed) {
        Ok(_) => panic!("expected a checking rejection"),
        Err(error) => format!("{error:?}"),
    }
}

fn production_error(source: &str, entry: &str) -> String {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    match terminal_production::TerminalProductionRequest::new(&checked, entry).produce_artifact() {
        Ok(_) => panic!("{entry}: expected the production frontier, got an artifact"),
        Err(error) => format!("{error:?}"),
    }
}

/// `&write` forwarding stays explicit: passing a write-only binding bare both
/// observes the binding and skips the required `&write` attenuation marker.
#[test]
fn bare_write_only_forwarding_rejects_during_checking() {
    let rendered = checked_error(
        "machine stamp(slot: &write u64, value: u64) { slot = value; }
        machine forward(root: &mut u64, value: u64) {
            let held: &write u64 = &write root;
            stamp(held, value);
        }",
    );
    assert!(
        rendered.contains("never observation"),
        "write-only observation diagnostic: {rendered}"
    );
    assert!(
        rendered.contains("explicit write-only attenuation"),
        "explicit-attenuation diagnostic: {rendered}"
    );
}

/// A runtime index with no provable bound cannot store through a borrow at
/// any index scalar width.
#[test]
fn unbounded_dynamic_index_stores_reject_during_checking() {
    for index_type in ["u8", "u64"] {
        let rendered = checked_error(&format!(
            "machine forward(values: &mut [u16; 4], index: {index_type}) {{
                values[index] = 17;
            }}"
        ));
        assert!(
            rendered.contains("cannot prove index `index` is within length 4"),
            "{index_type} index: {rendered}"
        );
    }
}

/// `&write` on a shared `&` binding asks for write authority the parent loan
/// never held — in transient call-argument position, in a `let`-bound local
/// reborrow, and in a `let`-bound parameter reborrow alike.
#[test]
fn shared_loan_write_reborrows_reject_during_checking() {
    for (name, source) in [
        (
            "transient argument",
            "machine stamp(slot: &write u64) { slot = 7; }
            machine forward(source: &u64) {
                let r: &u64 = source;
                stamp(&write r);
            }",
        ),
        (
            "let-bound local reborrow",
            "machine forward(source: &u64) {
                let r: &u64 = source;
                let w: &write u64 = &write r;
            }",
        ),
        (
            "let-bound parameter reborrow",
            "machine forward(source: &u64) {
                let w: &write u64 = &write source;
            }",
        ),
    ] {
        let rendered = checked_error(source);
        assert!(
            rendered.contains(
                "cannot derive WriteOnly reborrow authority from an exact Read parent loan"
            ),
            "{name}: {rendered}"
        );
    }
}

/// Whole-aggregate stores through a borrow, an owned record's field lent as a
/// call argument, and domain-qualified field stores meet the same hole.
#[test]
fn aggregate_and_owned_borrow_stores_still_miss_the_checked_control_plan() {
    for (name, entry, source) in [
        (
            "owned record field borrow argument",
            "enter",
            "data Pair [copy] { left: u64; right: u64; }
            machine stamp(left: &write u64, value: u64) { left = value; }
            machine enter(value: u64) -> u64 {
                let mut pair: Pair = Pair { left: 1, right: 2 };
                stamp(&write pair.left, value);
                pair.left
            }",
        ),
        (
            "whole aggregate store through borrow",
            "forward",
            "data Pair [copy] { left: u64; right: u64; }
            machine forward(pair: &write Pair, left: u64, right: u64) {
                pair = Pair { left: left, right: right };
            }",
        ),
        (
            "domain-qualified record field store",
            "forward",
            "domain [u8; 8]::Utf8 requires valid_utf8(self);
            data Limited { label: [u8; 8] in Utf8; }
            machine forward(limited: &write Limited, next: [u8; 8] in Utf8) {
                limited.label = next;
            }",
        ),
    ] {
        let rendered = production_error(source, entry);
        assert!(
            rendered.contains("no source-independent checked scalar control plan"),
            "{name}: {rendered}"
        );
    }
}

/// Guarded runtime-index stores and receiver calls, and computed IEEE stores
/// through a borrow, meet the same hole.
#[test]
fn guarded_index_and_computed_stores_still_miss_the_checked_control_plan() {
    for (name, source) in [
        (
            "guarded dynamic index store",
            "machine forward(values: &mut [u16; 4], index: u64) {
                transition index < 4 { true -> store(values, index) _ -> done() }
                state store(&mut self, values: &mut [u16; 4], index: u64) { values[index] = 17; }
                state done(&mut self) {}
            }",
        ),
        (
            "guarded dynamic index receiver call",
            "data Record [copy] { value: u16; }
            machine Record::replace(&write self, value: u16) { self.value = value; }
            machine forward(records: &write [Record; 4], index: u64, value: u16) {
                transition index < 4 { true -> stamp(&write records, index, value) _ -> done() }
                state stamp(&mut self, records: &write [Record; 4], index: u64, value: u16) { records[index].replace(value); }
                state done(&mut self) {}
            }",
        ),
        (
            "computed ieee store",
            "machine forward(values: &mut [f64; 4], left: f64, right: f64) {
                values[2] = left + right;
            }",
        ),
    ] {
        let rendered = production_error(source, "forward");
        assert!(
            rendered.contains("no source-independent checked scalar control plan"),
            "{name}: {rendered}"
        );
    }
}
