use super::rendered_rejection;
use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;

#[test]
fn direct_root_write_only_subloan_keeps_wider_index_shapes_fenced() {
    for (name, source) in [
        (
            "runtime index",
            r#"
                machine replace(value: &write u16) {}
                machine forward(values: &write [u16; 2], index: u64) {
                    replace(&write values[index]);
                }
            "#,
        ),
        (
            "range",
            r#"
                machine replace(value: &write [u16; 1]) {}
                machine forward(values: &write [u16; 2]) {
                    replace(&write values[0..1]);
                }
            "#,
        ),
        (
            "out of bounds",
            r#"
                machine replace(value: &write u16) {}
                machine forward(values: &write [u16; 2]) {
                    replace(&write values[2]);
                }
            "#,
        ),
        (
            "nested runtime index",
            r#"
                machine replace(value: &write u16) {}
                machine forward(values: &write [[u16; 2]; 2], index: u64) {
                    replace(&write values[0][index]);
                }
            "#,
        ),
        (
            "nested range",
            r#"
                machine replace(value: &write [u16; 1]) {}
                machine forward(values: &write [[u16; 2]; 2]) {
                    replace(&write values[0][0..1]);
                }
            "#,
        ),
        (
            "nested array element",
            r#"
                machine replace(value: &write [u16; 2]) {}
                machine forward(values: &write [[u16; 2]; 2]) {
                    replace(&write values[0]);
                }
            "#,
        ),
        (
            "record element",
            r#"
                data Leaf [copy] { value: u16; }
                machine replace(value: &write Leaf) {}
                machine forward(values: &write [Leaf; 2]) {
                    replace(&write values[0]);
                }
            "#,
        ),
        (
            "nested record element",
            r#"
                data Leaf [copy] { value: u16; }
                machine replace(value: &write Leaf) {}
                machine forward(values: &write [[Leaf; 2]; 2]) {
                    replace(&write values[0][0]);
                }
            "#,
        ),
        (
            "zero-length array",
            r#"
                machine replace(value: &write u16) {}
                machine forward(values: &write [u16; 0]) {
                    replace(&write values[0]);
                }
            "#,
        ),
        (
            "atomic element",
            r#"
                machine replace(value: &write AtomicU32) {}
                machine forward(values: &write [AtomicU32; 2]) {
                    replace(&write values[0]);
                }
            "#,
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains("forms `&write` from an unsupported projection"),
            "{name} unexpectedly crossed the direct-root literal-index subloan gate: {rendered}"
        );
    }
}

#[test]
fn qualified_field_write_only_subloans_carry_exact_atoms() {
    // The projected `&write` carries the field's declared atoms verbatim into
    // the callee's identically-declared referee, so every store the callee
    // performs re-derives the same obligations the caller's field declared.
    for (name, source) in [
        (
            "closed ranged leaf",
            r#"
                data Outer { value: u8 [0..=10]; }

                machine replace(value: &write u8 [0..=10]) {
                    value = 7;
                }

                machine forward(outer: &write Outer) {
                    replace(&write outer.value);
                }
            "#,
        ),
        (
            "arithmetic policy leaf",
            r#"
                data Outer { value: u32 in Wrapping; }

                machine replace(value: &write u32 in Wrapping) {
                    value = 7;
                }

                machine forward(outer: &write Outer) {
                    replace(&write outer.value);
                }
            "#,
        ),
        (
            "plain domain leaf",
            r#"
                domain [u8; 8]::Utf8
                requires
                    valid_utf8(self);

                data Tagged { label: [u8; 8] in Utf8; }
                data Outer { tagged: Tagged; }

                machine rename(label: &write [u8; 8] in Utf8, next: [u8; 8] in Utf8) {
                    label = next;
                }

                machine forward(outer: &write Outer, next: [u8; 8] in Utf8) {
                    rename(&write outer.tagged.label, next);
                }
            "#,
        ),
        (
            "domain-qualified record leaf",
            r#"
                data Leaf [copy] { value: u16; }
                domain Leaf::Valid requires self.value <= 10;
                data Outer { leaf: Leaf in Valid; }

                machine replace(value: &write Leaf in Valid, next: Leaf in Valid) {
                    value = next;
                }

                machine forward(outer: &write Outer, next: Leaf in Valid) {
                    replace(&write outer.leaf, next);
                }
            "#,
        ),
        (
            "conjunction leaf",
            r#"
                domain u8::Low requires self < 250;
                domain u8::Small requires self < 10;
                data Outer { value: u8 in Low & Small; }

                machine replace(value: &write u8 in Low & Small, next: u8 in Low & Small) {
                    value = next;
                }

                machine forward(outer: &write Outer, next: u8 in Low & Small) {
                    replace(&write outer.value, next);
                }
            "#,
        ),
    ] {
        lower_typed_trees(typed_program(source), &CheckingRequest::settled()).unwrap_or_else(
            |errors| panic!("{name}: exact-atom `&write` subloan should lower: {errors:?}"),
        );
    }
}

#[test]
fn qualified_field_subloans_reject_every_non_exact_referee() {
    // Exact-atom admission pins the boundary: a weaker referee sheds atoms,
    // a stronger or different one asserts obligations the place never owned.
    for (name, source) in [
        (
            "domain dropped",
            r#"
                domain u8::Low requires self < 250;
                data Outer { value: u8 in Low; }
                machine replace(value: &write u8) {}
                machine forward(outer: &write Outer) {
                    replace(&write outer.value);
                }
            "#,
        ),
        (
            "domain subset",
            r#"
                domain u8::Low requires self < 250;
                domain u8::Small requires self < 10;
                data Outer { value: u8 in Low & Small; }
                machine replace(value: &write u8 in Low) {}
                machine forward(outer: &write Outer) {
                    replace(&write outer.value);
                }
            "#,
        ),
        (
            "domain superset",
            r#"
                domain u8::Low requires self < 250;
                domain u8::Small requires self < 10;
                data Outer { value: u8 in Low; }
                machine replace(value: &write u8 in Low & Small) {}
                machine forward(outer: &write Outer) {
                    replace(&write outer.value);
                }
            "#,
        ),
        (
            "domain identity mismatch",
            r#"
                domain u8::Low requires self < 250;
                domain u8::Small requires self < 10;
                data Outer { value: u8 in Low; }
                machine replace(value: &write u8 in Small) {}
                machine forward(outer: &write Outer) {
                    replace(&write outer.value);
                }
            "#,
        ),
        (
            "carrier mismatch beneath equal domains",
            r#"
                domain u8::Low requires self < 250;
                domain u16::Wide requires self < 60000;
                data Outer { value: u8 in Low; }
                machine replace(value: &write u16 in Wide) {}
                machine forward(outer: &write Outer) {
                    replace(&write outer.value);
                }
            "#,
        ),
        (
            "whole root widened",
            r#"
                machine replace(value: &write u8) {}
                machine forward(limited: &write u8 [0..=10]) {
                    replace(&write limited);
                }
            "#,
        ),
        (
            "whole root narrowed",
            r#"
                machine replace(value: &write u8 [0..=10]) {}
                machine forward(limited: &write u8) {
                    replace(&write limited);
                }
            "#,
        ),
        (
            "literal-indexed element widened",
            r#"
                machine replace(value: &write u8) {}
                machine forward(values: &write [u16; 2]) {
                    replace(&write values[1]);
                }
            "#,
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains("must preserve the callee-declared constraint atoms exactly"),
            "{name}: a non-exact `&write` referee must stay rejected: {rendered}"
        );
    }
}

#[test]
fn write_only_subloan_remains_checked_body_only() {
    let rendered = rendered_rejection(
        r#"
            data Leaf [copy] { value: u16; }
            boundary trait Sink {
                machine fill(destination: &write Leaf) reaches Sink;
            }
            data Outer { leaf: Leaf; }
            machine forward(outer: &write Outer) reaches Sink {
                Sink::fill(&write outer.leaf);
            }
        "#,
    );
    assert!(
        rendered.contains("uses `&write` outside the checked whole-scalar parameter slice")
            && rendered.contains("traits"),
        "a bodyless provider boundary unexpectedly accepted write-only authority: {rendered}"
    );
}

#[test]
fn projected_write_only_local_requires_builtin_indexing() {
    for (element, accepted) in [("Record", false), ("u8", true)] {
        let source = format!(
            "data Record [copy] {{ value: u16; }}
             boundary operator [] Collection::read(items: &[{element}], index: u64) -> {element};
             machine forward(records: &write [Record; 2]) {{
                 let held: &write Record = &write records[1]; held.value = 17;
             }}"
        );
        let result = lower_typed_trees(typed_program(&source), &CheckingRequest::settled());
        assert_eq!(result.is_ok(), accepted, "{source}");
    }
}

#[test]
fn mut_rooted_write_only_subloans_keep_the_projection_envelope() {
    // The wider sources only name where a `&write` formation may begin; the
    // lent place still walks the same content-independent projection envelope,
    // so a computed expression, a runtime index, or a field beneath a
    // non-eligible record keeps the generic rejection.
    for (name, source) in [
        (
            "computed expression",
            r#"
                machine replace(value: &write u8) {}

                machine forward(outer: &mut u8) {
                    replace(&write outer.clone());
                }
            "#,
        ),
        (
            "runtime index",
            r#"
                machine replace(value: &write u16) {}

                machine forward(values: &mut [u16; 2], index: u64) {
                    replace(&write values[index]);
                }
            "#,
        ),
        (
            "affine record field",
            r#"
                data Leaf { value: u16; }
                data Outer { leaf: Leaf; }

                machine replace(value: &write Leaf) {}

                machine forward(outer: &mut Outer) {
                    replace(&write outer.leaf);
                }
            "#,
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains("forms `&write` from an unsupported projection"),
            "{name}: an `&mut`-rooted `&write` outside the projection envelope must stay fenced: {rendered}"
        );
    }
}

#[test]
fn mut_value_binding_exact_atom_write_only_subloans_are_forwardable() {
    // A `mut` value parameter or a `let mut` local owns writable storage, so
    // it may source a `&write` formation on the same terms as an `&mut`
    // place: the lent place carries the callee's declared referee
    // atom-for-atom. The gate runs whether or not the state declares a
    // `&write` root — the same calls that once walked past the gate with
    // their atoms dropped are admitted on their exact atoms.
    for (name, source) in [
        (
            "mutable local",
            r#"
                machine replace(value: &write u8 [0..=10]) {
                    value = 7;
                }

                machine forward() {
                    let mut x: u8 [0..=10] = 0;
                    replace(&write x);
                }
            "#,
        ),
        (
            "mutable value parameter",
            r#"
                machine replace(value: &write u8 [0..=10]) {
                    value = 7;
                }

                machine forward(mut x: u8 [0..=10]) {
                    replace(&write x);
                }
            "#,
        ),
        (
            "mutable value parameter beside a write-only root",
            r#"
                machine replace(value: &write u8 [0..=10]) {
                    value = 7;
                }

                machine forward(mut x: u8 [0..=10], pad: &write u16) {
                    pad = 3;
                    replace(&write x);
                }
            "#,
        ),
        (
            "mutable local record field",
            r#"
                data Outer { value: u8 [0..=10]; }

                machine replace(value: &write u8 [0..=10]) {
                    value = 7;
                }

                machine forward() {
                    let mut outer: Outer = Outer { value: 1 };
                    replace(&write outer.value);
                }
            "#,
        ),
        (
            "mutable value parameter record field",
            r#"
                data Outer { value: u8 [0..=10]; }

                machine replace(value: &write u8 [0..=10]) {
                    value = 7;
                }

                machine forward(mut outer: Outer) {
                    replace(&write outer.value);
                }
            "#,
        ),
        (
            "attached field under a mutable consuming receiver",
            r#"
                data Boxed { value: u8 [0..=10]; }

                machine replace(value: &write u8 [0..=10]) {
                    value = 7;
                }

                machine Boxed::forward(mut self) {
                    replace(&write self.value);
                }
            "#,
        ),
        (
            "write-only local formed from a mutable value binding",
            r#"
                machine forward() {
                    let mut x: u8 [0..=10] = 0;
                    let held: &write u8 [0..=10] = &write x;
                    held = 7;
                }
            "#,
        ),
    ] {
        lower_typed_trees(typed_program(source), &CheckingRequest::settled()).unwrap_or_else(|errors| {
            panic!(
                "{name}: an exact-atom `&write` subloan lent from a `mut` value binding should lower: {errors:?}"
            )
        });
    }
}

#[test]
fn immutable_value_bindings_cannot_form_write_only_borrows() {
    // `&write` formation needs a place with mutable authority: a declared
    // `&write` root, a `&mut` place, or a `mut` value binding. A plain `let`
    // or an immutable parameter holds none — before this gate the same
    // formation walked past every source check and compiled with the
    // write-only contract invented out of thin air. A shared-reference
    // binding is a different rejection: `&write` there is a reborrow of the
    // referent, and the writability/borrow lattice — not this gate — answers
    // its authority question.
    for (name, expected, source) in [
        (
            "immutable local",
            "a binding without mutable authority",
            r#"
                machine replace(value: &write u8 [0..=10]) {}

                machine forward() {
                    let x: u8 [0..=10] = 0;
                    replace(&write x);
                }
            "#,
        ),
        (
            "immutable value parameter",
            "a binding without mutable authority",
            r#"
                machine replace(value: &write u8 [0..=10]) {}

                machine forward(x: u8 [0..=10]) {
                    replace(&write x);
                }
            "#,
        ),
        (
            "immutable local beside a write-only root",
            "a binding without mutable authority",
            r#"
                machine replace(value: &write u8 [0..=10]) {}

                machine forward(pad: &write u16) {
                    let x: u8 [0..=10] = 0;
                    pad = 3;
                    replace(&write x);
                }
            "#,
        ),
        (
            "immutable local behind a write-only local formation",
            "a binding without mutable authority",
            r#"
                machine forward() {
                    let x: u8 = 0;
                    let held: &write u8 = &write x;
                }
            "#,
        ),
        (
            "shared reference parameter",
            "is not writable in this state",
            r#"
                machine replace(value: &write u8) {}

                machine forward(x: &u8) {
                    replace(&write x);
                }
            "#,
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains(expected),
            "{name}: `&write` on this binding must be rejected: {rendered}"
        );
    }
}

#[test]
fn transient_reborrow_arguments_face_the_access_pair_rule() {
    // `&write`/`&mut` formed directly in a call argument is a referent
    // reborrow when the target names a reference-typed binding: the transient
    // formation builds no local loan to rebase through, so the binding's
    // declared access is the parent authority and the lattice's access-pair
    // rule decides — the same edge a `let`-bound reborrow faces. Before this
    // rung the transient argument skipped the lattice entirely and `&write r`
    // on `r: &u8` compiled with write authority invented from a shared
    // referent.
    for (name, expected, source) in [
        (
            "shared local into a helper argument",
            "cannot derive WriteOnly reborrow authority from an exact Read parent loan",
            r#"
                machine stamp(slot: &write u8) {}

                machine forward(source: &u8) {
                    let r: &u8 = source;
                    stamp(&write r);
                }
            "#,
        ),
        (
            "shared local into a transition argument",
            "cannot derive WriteOnly reborrow authority from an exact Read parent loan",
            r#"
                machine forward(source: &u8) {
                    let r: &u8 = source;
                    transition true { true -> stamp(&write r) }
                    state stamp(&mut self, slot: &write u8) {}
                }
            "#,
        ),
        (
            "shared-from-mutable local",
            "cannot derive WriteOnly reborrow authority from an exact Read parent loan",
            r#"
                machine stamp(slot: &write u8) {}

                machine forward(source: &mut u8) {
                    let r: &u8 = source;
                    stamp(&write r);
                }
            "#,
        ),
        (
            "transient mutable borrow of a shared local",
            "cannot derive Mutable reborrow authority from an exact Read parent loan",
            r#"
                machine stamp(slot: &mut u8) {}

                machine forward(source: &u8) {
                    let r: &u8 = source;
                    stamp(&mut r);
                }
            "#,
        ),
        (
            "transient mutable borrow of a write-only local",
            "widens write-only",
            r#"
                machine stamp(slot: &mut u8) {}

                machine forward(source: &mut u8) {
                    let w: &write u8 = &write source;
                    stamp(&mut w);
                }
            "#,
        ),
        (
            "let-bound child of a shared local",
            "cannot derive WriteOnly reborrow authority from an exact Read parent loan",
            r#"
                machine forward(source: &u8) {
                    let r: &u8 = source;
                    let w: &write u8 = &write r;
                }
            "#,
        ),
        (
            "let-bound child of a shared parameter",
            "cannot derive WriteOnly reborrow authority from an exact Read parent loan",
            r#"
                machine forward(source: &u8) {
                    let w: &write u8 = &write source;
                }
            "#,
        ),
        (
            "let-bound mutable child of a shared parameter",
            "cannot derive Mutable reborrow authority from an exact Read parent loan",
            r#"
                machine forward(source: &u8) {
                    let w: &mut u8 = &mut source;
                }
            "#,
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains(expected),
            "{name}: `&write`/`&mut` on a binding without the parent authority must take the lattice's directed diagnostic: {rendered}"
        );
    }
}

#[test]
fn mut_value_sources_stay_readable_beside_write_only_formation() {
    // A `mut` value binding is a formation source, never a write-only root:
    // ordinary reads through it in a state that forms `&write` subloans keep
    // passing expression validation unchanged.
    for (name, source) in [
        (
            "mutable local",
            r#"
                machine replace(value: &write u8 [0..=10]) {
                    value = 7;
                }

                machine inspect() -> u8 {
                    let mut x: u8 [0..=10] = 0;
                    replace(&write x);
                    x
                }
            "#,
        ),
        (
            "mutable value parameter",
            r#"
                data Outer { value: u8 [0..=10]; }

                machine replace(value: &write u8 [0..=10]) {
                    value = 7;
                }

                machine inspect(mut outer: Outer) -> u8 {
                    replace(&write outer.value);
                    outer.value
                }
            "#,
        ),
    ] {
        lower_typed_trees(typed_program(source), &CheckingRequest::settled()).unwrap_or_else(|errors| {
            panic!(
                "{name}: reading through a `mut` value binding beside a `&write` formation must stay legal: {errors:?}"
            )
        });
    }
}

#[test]
fn sum_case_payload_cannot_form_a_write_only_subloan() {
    let rendered = rendered_rejection(
        r#"
            data Choice [copy] {
                case Empty;
                case Value(value: u16);
            }
            machine replace(value: &write u16) {}
            machine done() {}
            machine forward(choice: &write Choice) {
                transition choice {
                    Choice::Value { value } -> replace(&write value)
                    Choice::Empty -> done()
                }
            }
        "#,
    );
    assert!(
        rendered.contains("write-only parameter `choice`")
            && (rendered.contains("never observation")
                || rendered.contains("never grants observation")),
        "a case/payload-derived subloan unexpectedly bypassed tag and payload observation fences: {rendered}"
    );
}

#[test]
fn consuming_receiver_exact_atom_write_only_subloans_are_forwardable() {
    // A consuming `self` owns its attached storage outright — the receiver
    // contract already treats that storage as mutable without a `mut`
    // marker — so the owned binding lends `&write` on the same exact-atom
    // terms a `&mut` place takes. The whole receiver, an explicit
    // `self.field` path, and a bare attached-field name all resolve through
    // the same non-observing walk.
    for (name, source) in [
        (
            "whole consuming receiver",
            r#"
                data Boxed { value: u8 [0..=10]; }

                machine consume(value: &write Boxed) {}

                machine Boxed::forward(self) {
                    consume(&write self);
                }
            "#,
        ),
        (
            "attached field through the receiver",
            r#"
                data Boxed { value: u8 [0..=10]; }

                machine replace(value: &write u8 [0..=10]) {
                    value = 7;
                }

                machine Boxed::forward(self) {
                    replace(&write self.value);
                }
            "#,
        ),
        (
            "bare attached field",
            r#"
                data Boxed { value: u8; }

                machine replace(value: &write u8) {
                    value = 7;
                }

                machine Boxed::forward(self) {
                    replace(&write value);
                }
            "#,
        ),
        (
            "whole receiver after an in-place store",
            r#"
                data Boxed { value: u8 [0..=10]; }

                machine consume(value: &write Boxed) {}

                machine Boxed::forward(self) {
                    self.value = 3;
                    consume(&write self);
                }
            "#,
        ),
        (
            "write-only local formed from a receiver field",
            r#"
                data Boxed { value: u8 [0..=10]; }

                machine Boxed::forward(self) {
                    let held: &write u8 [0..=10] = &write self.value;
                    held = 7;
                }
            "#,
        ),
        (
            "write-only local formed from a bare receiver field",
            r#"
                data Boxed { value: u8 [0..=10]; }

                machine Boxed::forward(self) {
                    let held: &write u8 [0..=10] = &write value;
                    held = 7;
                }
            "#,
        ),
        (
            "attached field in a transition argument",
            r#"
                data Boxed { value: u8 [0..=10]; }

                machine Boxed::forward(self) {
                    transition true { true -> next(&write self.value) }
                    state next(self, pad: &write u8 [0..=10]) {}
                }
            "#,
        ),
    ] {
        lower_typed_trees(typed_program(source), &CheckingRequest::settled()).unwrap_or_else(|errors| {
            panic!(
                "{name}: an exact-atom `&write` subloan lent from a consuming `self` should lower: {errors:?}"
            )
        });
    }
}

#[test]
fn consuming_receiver_write_only_subloans_reject_non_exact_referees() {
    // The owned receiver faces the identical atom-exact gate a `&mut` place
    // takes: shedding the field's range, strengthening it onto a narrower
    // referee, or naming another data type are the same mismatch, reported by
    // the directed subloan diagnostic rather than the generic projection
    // fence — with no unrelated declared `&write` root required.
    for (name, place, source) in [
        (
            "attached field sheds its range",
            "lends `self.value` of type `u8[0..=10]` as `&write` to a parameter declared `&write u8`",
            r#"
                data Boxed { value: u8 [0..=10]; }

                machine replace(value: &write u8) {}

                machine Boxed::forward(self) {
                    replace(&write self.value);
                }
            "#,
        ),
        (
            "bare attached field sheds its range",
            "lends `value` of type `u8[0..=10]` as `&write` to a parameter declared `&write u8`",
            r#"
                data Boxed { value: u8 [0..=10]; }

                machine replace(value: &write u8) {}

                machine Boxed::forward(self) {
                    replace(&write value);
                }
            "#,
        ),
        (
            "attached field takes a stronger range",
            "lends `self.value` of type `u8` as `&write` to a parameter declared `&write u8[0..=5]`",
            r#"
                data Boxed { value: u8; }

                machine replace(value: &write u8 [0..=5]) {}

                machine Boxed::forward(self) {
                    replace(&write self.value);
                }
            "#,
        ),
        (
            "whole receiver names another data",
            "lends `self` of type `Self` as `&write` to a parameter declared `&write Other`",
            r#"
                data Boxed { value: u8 [0..=10]; }
                data Other { value: u8 [0..=10]; }

                machine consume(value: &write Other) {}

                machine Boxed::forward(self) {
                    consume(&write self);
                }
            "#,
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains(place)
                && rendered.contains("must preserve the callee-declared constraint atoms exactly"),
            "{name}: a consuming-`self` `&write` subloan with a non-exact referee must take the directed diagnostic: {rendered}"
        );
        assert!(
            !rendered.contains("unsupported projection"),
            "{name}: the directed atom diagnostic, not the generic projection fence, should report the mismatch: {rendered}"
        );
    }
}

#[test]
fn consuming_receiver_write_only_local_captures_keep_exact_atoms() {
    // A `&write` local formed from an owned place must carry the declared
    // referee atom-for-atom too: before this rung a bare-name target whose
    // atoms did not match fell through the expression walk with no
    // diagnostic and silently compiled.
    for (name, place, source) in [
        (
            "receiver field capture sheds its range",
            "captures `self.value` of type `u8[0..=10]` as `&write u8`",
            r#"
                data Boxed { value: u8 [0..=10]; }

                machine Boxed::forward(self) {
                    let held: &write u8 = &write self.value;
                    held = 7;
                }
            "#,
        ),
        (
            "bare receiver field capture sheds its range",
            "captures `value` of type `u8[0..=10]` as `&write u8`",
            r#"
                data Boxed { value: u8 [0..=10]; }

                machine Boxed::forward(self) {
                    let held: &write u8 = &write value;
                    held = 7;
                }
            "#,
        ),
        (
            "mutable local capture sheds its range",
            "captures `x` of type `u8[0..=10]` as `&write u8`",
            r#"
                machine forward() {
                    let mut x: u8 [0..=10] = 0;
                    let held: &write u8 = &write x;
                    held = 7;
                }
            "#,
        ),
        (
            "write-only local capture sheds its range",
            "captures `limited` of type `u8[0..=10]` as `&write u8`",
            r#"
                machine forward(limited: &write u8 [0..=10]) {
                    let held: &write u8 = &write limited;
                    held = 7;
                }
            "#,
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains(place)
                && rendered.contains("must preserve the declared constraint atoms exactly"),
            "{name}: a `&write` local capture with a non-exact referee must take the directed diagnostic: {rendered}"
        );
    }
}

#[test]
fn consuming_receiver_write_only_subloans_keep_the_authority_fences() {
    // Owning the storage is the only authority a consuming `self` adds:
    // `&self`, `const self`, an erased attached field, and reference-typed
    // bindings keep their existing rejections — `&write` on a `&u8`
    // parameter answers to the reborrow lattice, not this gate.
    for (name, expected, source) in [
        (
            "shared receiver field path",
            "forms `&write` from an unsupported projection",
            r#"
                data Boxed { value: u8 [0..=10]; }

                machine replace(value: &write u8 [0..=10]) {}

                machine Boxed::forward(&self) {
                    replace(&write self.value);
                }
            "#,
        ),
        (
            "shared receiver whole place",
            "is not writable in this state",
            r#"
                data Boxed { value: u8 [0..=10]; }

                machine consume(value: &write Boxed) {}

                machine Boxed::forward(&self) {
                    consume(&write self);
                }
            "#,
        ),
        (
            "const consuming receiver",
            "forms `&write` from an unsupported projection",
            r#"
                data Boxed { value: u8 [0..=10]; }

                machine replace(value: &write u8 [0..=10]) {}

                machine Boxed::forward(const self) {
                    replace(&write self.value);
                }
            "#,
        ),
        (
            "erased attached field",
            "erased field `scratch` has no runtime value",
            r#"
                data Boxed { value: u8 [0..=10]; scratch [erased]: u8; }

                machine replace(value: &write u8) {}

                machine Boxed::forward(self) {
                    replace(&write self.scratch);
                }
            "#,
        ),
        (
            "shared reference parameter",
            "is not writable in this state",
            r#"
                data Boxed { value: u8 [0..=10]; }

                machine replace(value: &write u8) {}

                machine Boxed::forward(self, source: &u8) {
                    replace(&write source);
                }
            "#,
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains(expected),
            "{name}: `&write` formation without owned mutable authority must stay rejected: {rendered}"
        );
    }
}

#[test]
fn consuming_receiver_write_only_subloan_needs_a_write_only_parameter() {
    // Admission is only ever to a declared `&write` parameter: the same
    // exact place lent to a parameter that may read keeps its widening
    // rejection.
    let rendered = rendered_rejection(
        r#"
            data Boxed { value: u8; }

            machine read(value: u8) -> u8 {
                value
            }

            machine Boxed::forward(self) -> u8 {
                read(&write self.value)
            }
        "#,
    );
    assert!(
        rendered.contains("supplies `&write` to a parameter that may read"),
        "an owned place must not widen `&write` onto a readable parameter: {rendered}"
    );
}
