//! Domain-qualified field writes through mutable carriers: declared-zeroed
//! record locals, member-path reference field stores, pinned runtime index
//! selectors, and owned-array to constrained-view arguments.
use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

fn check(source: &str, accepted: bool) {
    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled());
    match diagnostics {
        Ok(_) => assert!(accepted, "expected the write to reject:\n{source}"),
        Err(diagnostics) => {
            assert!(
                !accepted,
                "the write should satisfy its domain: {diagnostics:#?}\n{source}"
            );
        }
    }
}

/// `[u8; N] in Utf8` viewed as `&[u8] in Utf8`: the owned carrier's declared
/// domain satisfies the view's identical constraint. The same transition with
/// an unconstrained array must still fail — the shared-view shape cannot
/// invent the callee's domain claim.
#[test]
fn constrained_owned_array_satisfies_matching_constrained_view_argument() {
    for (field_type, accepted) in [("[u8; 16] in Utf8", true), ("[u8; 16]", false)] {
        let source = format!(
            r#"
            domain [u8]::Utf8 requires valid_utf8(self);
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            domain [u8; 16]::Utf8 requires valid_utf8(self);
            data Room {{ label: [u8; 4] in Utf8; }}
            data Line {{ text: {field_type}; }}
            data Main {{ room: Room; line: Line; }}
            machine Main::main(&mut self) {{
                self.room.label = "A1";
                self.line.text = "Room " + self.room.label;
                transition {{ _ -> write_line(self.line.text) }}
                state write_line(&mut self, text: &[u8] in Utf8) {{}}
            }}"#
        );
        check(&source, accepted);
    }
}

/// `let room: Room;` has no initializer to walk, but its declaration frontier
/// is still the type-derived symbolic may-set. A later member store
/// (`room.label = source.label`) must splice the value's proven leaves into
/// that frontier instead of going opaque and losing the record's other
/// obligations — the write-frame path that made fixture rows like
/// `runtime_local_struct_string_field_concat_exit` reject.
#[test]
fn declared_zeroed_record_local_accepts_reference_field_write() {
    let source = r#"
        domain [u8]::Utf8 requires valid_utf8(self);
        data Room [copy] { label: &[u8] in Utf8; }
        data Main { source: Room; }
        machine Main::main(&mut self) {
            self.source.label = "Gate";
            transition { _ -> render(self.source) }
            state render(&mut self, source: Room) {
                let room: Room;
                let mut out_line: &[u8] in Utf8;
                room.label = source.label;
                out_line = "== " + room.label + " ==";
                transition out_line == "== Gate ==" {
                    true -> good()
                    false -> bad()
                }
            }
            state good(&mut self) {}
            state bad(&mut self) {}
        }
    "#;
    check(source, true);
}

/// A runtime `Index { expression }` whose selector the scalar facts pin to a
/// value names exactly one element. Establishment and invalidation share the
/// same narrowing, so `room_slice[match_index]` writes retire only the pinned
/// element and sibling reads keep their proven domains.
#[test]
fn pinned_runtime_index_writes_keep_sibling_field_domains() {
    let source = r#"
        domain [u8]::Utf8 requires valid_utf8(self);
        data Room { name: &[u8] in Utf8; }
        data Main { rooms: [Room; 3]; }
        machine Main::main(&mut self) {
            let match_index: u64 = 1;
            let differ_index: u64 = 2;
            let empty_index: u64 = 0;
            let room_slice: &mut [Room] = self.rooms.as_mut_slice();
            room_slice[match_index].name = "expected";
            room_slice[differ_index].name = "expectes";
            transition room_slice[empty_index].name == "expected" {
                true -> fail()
                false -> check_match()
            }
            state check_match(&mut self) {}
            state fail(&mut self) {}
        }
    "#;
    check(source, true);
}

/// The conservative edge of the same narrowing: an unpinned selector cannot
/// be attributed to one element, so the write still retires every candidate
/// and the later read cannot prove the domain.
#[test]
fn unpinned_runtime_index_write_still_retires_sibling_domains() {
    let source = r#"
        domain [u8]::Utf8 requires valid_utf8(self);
        data Room { name: &[u8] in Utf8; }
        data Main { rooms: [Room; 3]; }
        machine Main::main(&mut self, slot: u64 [0..=2]) {
            let room_slice: &mut [Room] = self.rooms.as_mut_slice();
            room_slice[0].name = "expected";
            room_slice[slot].name = "clobbered";
            transition room_slice[0].name == "expected" {
                true -> done()
                false -> fail()
            }
            state done(&mut self) {}
            state fail(&mut self) {}
        }
    "#;
    check(source, false);
}
