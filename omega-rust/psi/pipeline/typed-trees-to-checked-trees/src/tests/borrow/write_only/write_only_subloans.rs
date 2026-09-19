use super::{rendered_rejection, typed};
use crate::lower_typed_trees;

#[test]
fn direct_unconstrained_primitive_record_field_is_writable() {
    lower_typed_trees(typed(
        r#"
            data Pair {
                left: u8;
                right: u16;
            }

            machine fill(pair: &write Pair) {
                pair.left = 1;
                pair.right = 2;
            }
        "#,
    ))
    .expect("one-level primitive record-field writes should lower");
}

#[test]
fn nested_unconstrained_primitive_record_field_is_writable() {
    lower_typed_trees(typed(
        r#"
            data Inner { value: u8; }
            data Outer { inner: Inner; }

            machine fill(outer: &write Outer) {
                outer.inner.value = 1;
            }
        "#,
    ))
    .expect("nested invariant-free record-field writes should lower");
}

#[test]
fn exact_common_field_write_only_subloan_is_forwardable() {
    lower_typed_trees(typed(
        r#"
            data Inner { value: u16; sibling: u16; }
            data Outer { inner: Inner; other: Inner; }

            machine replace(value: &write u16) {
                value = 7;
            }

            machine forward(outer: &write Outer) {
                replace(&write outer.inner.value);
            }
        "#,
    ))
    .expect("an exact common-field path may form a narrower write-only subloan");
}

#[test]
fn exact_literal_indexed_write_only_subloan_is_forwardable() {
    lower_typed_trees(typed(
        r#"
            data Inner { values: [u16; 2]; sibling: u16; }
            data Outer { inner: Inner; other: Inner; }

            machine replace(value: &write u16) {
                value = 7;
            }

            machine forward(outer: &write Outer) {
                replace(&write outer.inner.values[1]);
            }
        "#,
    ))
    .expect("one exact literal fixed-array index may finish a common-field subloan");
}

#[test]
fn exact_direct_root_literal_indexed_write_only_subloan_is_forwardable() {
    lower_typed_trees(typed(
        r#"
            machine replace(value: &write u16) {
                value = 7;
            }

            machine forward(values: &write [u16; 2]) {
                replace(&write values[1]);
            }
        "#,
    ))
    .expect("one exact literal index may narrow a direct write-only fixed-array root");
}

#[test]
fn finite_literal_index_suffix_may_narrow_a_direct_write_only_root() {
    lower_typed_trees(typed(
        r#"
            machine replace(value: &write u16) {
                value = 7;
            }

            machine forward(values: &write [[[[[[u16; 7]; 6]; 5]; 4]; 3]; 2]) {
                replace(&write values[1][2][3][4][5][6]);
            }
        "#,
    ))
    .expect("a finite literal-index suffix may narrow a nested direct write-only array root");
}

#[test]
fn finite_literal_index_suffix_may_finish_a_common_field_subloan() {
    lower_typed_trees(typed(
        r#"
            data Outer { values: [[[[[[u16; 7]; 6]; 5]; 4]; 3]; 2]; sibling: u16; }

            machine replace(value: &write u16) {
                value = 7;
            }

            machine forward(outer: &write Outer) {
                replace(&write outer.values[1][2][3][4][5][6]);
            }
        "#,
    ))
    .expect("a finite literal-index suffix may finish a common-field write-only subloan");
}

#[test]
fn literal_indexed_write_only_subloan_narrows_a_local_root() {
    lower_typed_trees(typed(
        r#"
            machine replace(value: &write u16) {
                value = 7;
            }

            machine forward(values: &write [u16; 2]) {
                let alias: &write [u16; 2] = &write values;
                replace(&write alias[1]);
            }
        "#,
    ))
    .expect("the shared projection walk narrows a write-only local root by literal index at the call boundary, exactly as local formation admits the same place");
}

#[test]
fn literal_indexed_write_only_subloan_interleaves_member_and_index_hops() {
    lower_typed_trees(typed(
        r#"
            data Inner [copy] { values: [u16; 2]; }
            data Outer { inners: [Inner; 2]; }

            machine replace(value: &write u16) {
                value = 7;
            }

            machine forward(outer: &write Outer) {
                replace(&write outer.inners[0].values[1]);
            }
        "#,
    ))
    .expect("member and literal-index hops compose under the shared projection walk, matching local formation of the same place");
}

#[test]
fn literal_indexed_write_only_subloan_requires_builtin_indexing() {
    let rendered = rendered_rejection(
        r#"
            boundary operator [] Collection::read(items: &[u16], index: u64) -> u16;
            machine replace(value: &write u16) {}
            machine forward(values: &write [u16; 2]) {
                replace(&write values[1]);
            }
        "#,
    );
    assert!(
        rendered.contains("forms `&write` from an unsupported projection"),
        "an authored index operator must not substitute for builtin coordinates: {rendered}"
    );
}

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
fn wider_indexed_write_only_subloans_remain_fenced() {
    for (name, source) in [
        (
            "runtime index",
            r#"
                data Outer { values: [u16; 2]; }
                machine replace(value: &write u16) {}
                machine forward(outer: &write Outer, index: u64) {
                    replace(&write outer.values[index]);
                }
            "#,
        ),
        (
            "out of bounds index",
            r#"
                data Outer { values: [u16; 2]; }
                machine replace(value: &write u16) {}
                machine forward(outer: &write Outer) {
                    replace(&write outer.values[2]);
                }
            "#,
        ),
        (
            "nested array element",
            r#"
                data Outer { values: [[u16; 2]; 2]; }
                machine replace(value: &write [u16; 2]) {}
                machine forward(outer: &write Outer) {
                    replace(&write outer.values[0]);
                }
            "#,
        ),
        (
            "record element",
            r#"
                data Leaf [copy] { value: u16; }
                data Outer { values: [Leaf; 2]; }
                machine replace(value: &write Leaf) {}
                machine forward(outer: &write Outer) {
                    replace(&write outer.values[0]);
                }
            "#,
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains("forms `&write` from an unsupported projection")
                && rendered
                    .contains("finite nonempty suffix of in-bounds literal fixed-array indexes"),
            "{name} unexpectedly crossed the literal-indexed subloan gate: {rendered}"
        );
    }
}

#[test]
fn non_field_and_non_closed_field_write_only_subloans_remain_fenced() {
    for (name, source) in [
        (
            "range",
            r#"
                data Outer { values: [u16; 2]; }
                machine replace(values: &write [u16; 1]) {}
                machine forward(outer: &write Outer) {
                    replace(&write outer.values[0..1]);
                }
            "#,
        ),
        (
            "generic leaf",
            r#"
                data Leaf<T [copy]> [copy] { value: T; }
                data Outer { leaf: Leaf<u16>; }
                machine replace(value: &write Leaf<u16>) {}
                machine forward(outer: &write Outer) {
                    replace(&write outer.leaf);
                }
            "#,
        ),
        (
            "invariant-bearing leaf",
            r#"
                data Leaf [copy]
                where value <= limit,
                { value: u16; limit: u16; }
                data Outer { leaf: Leaf; }
                machine replace(value: &write Leaf) {}
                machine forward(outer: &write Outer) {
                    replace(&write outer.leaf);
                }
            "#,
        ),
        (
            "affine leaf",
            r#"
                data Leaf { value: u16; }
                data Outer { leaf: Leaf; }
                machine replace(value: &write Leaf) {}
                machine forward(outer: &write Outer) {
                    replace(&write outer.leaf);
                }
            "#,
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains("forms `&write` from an unsupported projection")
                && rendered.contains("content-independent common-field path"),
            "{name} unexpectedly crossed the common-field subloan gate: {rendered}"
        );
    }
}

#[test]
fn closed_ranged_record_field_subloan_rejects_a_weakened_referee() {
    // Assignment stores may displace a closed-ranged leaf in place, and a
    // `&write` subloan now carries atoms — but only atom-for-atom. Lending
    // `outer.value` to a plain `&write u8` would shed the declared bound at
    // the borrow boundary, so the weakened referee stays rejected.
    let rendered = rendered_rejection(
        r#"
            data Outer { value: u8 [0..=10]; }

            machine replace(value: &write u8) {
                value = 0;
            }

            machine forward(outer: &write Outer) {
                replace(&write outer.value);
            }
        "#,
    );
    assert!(
        rendered.contains("lends `outer.value` of type `u8[0..=10]` as `&write`")
            && rendered.contains("declared `&write u8`")
            && rendered.contains("must preserve the callee-declared constraint atoms exactly"),
        "a ranged leaf must not attenuate into a wider `&write` referee: {rendered}"
    );
}

#[test]
fn closed_ranged_record_field_subloan_rejects_a_narrower_referee() {
    // A stronger referee is no more honest than a weaker one: the callee's
    // own store obligation would require values the caller's place never
    // declared.
    let rendered = rendered_rejection(
        r#"
            data Outer { value: u8 [0..=10]; }

            machine replace(value: &write u8 [0..=5]) {}

            machine forward(outer: &write Outer) {
                replace(&write outer.value);
            }
        "#,
    );
    assert!(
        rendered.contains("must preserve the callee-declared constraint atoms exactly"),
        "a ranged leaf must not lend to a narrower `&write` referee: {rendered}"
    );
}

#[test]
fn policy_qualified_record_field_subloan_rejects_a_weakened_referee() {
    // Assignment stores may displace a policy-only leaf in place — the policy
    // is a behaviour tag, not a membership bound. A `&write` subloan carries
    // atoms now, but a plain `&write u32` referee would shed the field's
    // `Wrapping` atom at the boundary, so the weakened referee stays rejected.
    let rendered = rendered_rejection(
        r#"
            data Outer { value: u32 in Wrapping; }

            machine replace(value: &write u32) {
                value = 0;
            }

            machine forward(outer: &write Outer) {
                replace(&write outer.value);
            }
        "#,
    );
    assert!(
        rendered.contains("lends `outer.value` of type `u32[in Wrapping]` as `&write`")
            && rendered.contains("must preserve the callee-declared constraint atoms exactly"),
        "a policy leaf must not attenuate into a policy-free `&write` referee: {rendered}"
    );
}

#[test]
fn policy_qualified_record_field_subloan_rejects_a_different_policy() {
    // `in <policy>` is a behaviour tag, and a different tag is a different
    // obligation — exact identity rejects the mismatch rather than weakening
    // or strengthening silently.
    let rendered = rendered_rejection(
        r#"
            data Outer { value: u32 in Wrapping; }

            machine replace(value: &write u32 in Saturating) {}

            machine forward(outer: &write Outer) {
                replace(&write outer.value);
            }
        "#,
    );
    assert!(
        rendered.contains("must preserve the callee-declared constraint atoms exactly"),
        "a Wrapping leaf must not lend to a Saturating `&write` referee: {rendered}"
    );
}

#[test]
fn qualified_scalar_write_only_referees_are_admitted() {
    // A qualified `&write` referee keeps its atoms on the place itself: every
    // store through the reference still owes the declared bound or domain, so
    // the referee admits exactly the constrained leaves a field store may
    // displace and nothing weaker or different.
    for (name, source) in [
        (
            "closed ranged scalar",
            r#"
                machine replace(value: &write u8 [0..=10]) {
                    value = 7;
                }

                machine forward(limited: &write u8 [0..=10]) {
                    replace(&write limited);
                }
            "#,
        ),
        (
            "arithmetic policy scalar",
            r#"
                machine replace(value: &write u32 in Wrapping) {
                    value = 7;
                }

                machine forward(counter: &write u32 in Wrapping) {
                    replace(&write counter);
                }
            "#,
        ),
        (
            "plain domain carrier",
            r#"
                domain [u8; 8]::Utf8
                requires
                    valid_utf8(self);

                machine rename(label: &write [u8; 8] in Utf8, next: [u8; 8] in Utf8) {
                    label = next;
                }

                machine forward(label: &write [u8; 8] in Utf8, next: [u8; 8] in Utf8) {
                    rename(&write label, next);
                }
            "#,
        ),
        (
            "domain-qualified record",
            r#"
                data Leaf [copy] { value: u16; }
                domain Leaf::Valid requires self.value <= 10;

                machine replace(value: &write Leaf in Valid, next: Leaf in Valid) {
                    value = next;
                }
            "#,
        ),
    ] {
        lower_typed_trees(typed(source)).unwrap_or_else(|errors| {
            panic!("{name}: qualified `&write` referee should lower: {errors:?}")
        });
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
        lower_typed_trees(typed(source)).unwrap_or_else(|errors| {
            panic!("{name}: exact-atom `&write` subloan should lower: {errors:?}")
        });
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
fn domain_qualified_carrier_keeps_element_subloans_fenced() {
    // A whole-leaf `&write` on a domain-qualified carrier is exact, but an
    // element subloan cannot re-establish whole-value membership: the
    // projection walk refuses to descend into the qualified array at all, so
    // the partial write stays outside the envelope.
    let rendered = rendered_rejection(
        r#"
            domain [u8; 8]::Utf8
            requires
                valid_utf8(self);

            data Tagged { label: [u8; 8] in Utf8; }
            data Outer { tagged: Tagged; }

            machine fill(byte: &write u8) {}

            machine forward(outer: &write Outer) {
                fill(&write outer.tagged.label[0]);
            }
        "#,
    );
    assert!(
        rendered.contains("forms `&write` from an unsupported projection"),
        "an element subloan of a domain-qualified carrier must stay fenced: {rendered}"
    );
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
fn projected_write_only_local_does_not_allow_bare_observation() {
    let rendered = rendered_rejection(
        r#"
            data Leaf [copy] { value: u16; }
            data Outer { leaf: Leaf; }
            machine replace(value: &write Leaf) {}
            machine forward(outer: &write Outer) {
                let child: &write Leaf = &write outer.leaf;
                replace(child);
            }
        "#,
    );
    assert!(
        rendered.contains("reads write-only parameter `child`")
            && !rendered.contains("forms `&write` from an unsupported projection"),
        "a projected local must retain its no-read obligation: {rendered}"
    );
}

#[test]
fn projected_write_only_locals_capture_primitive_and_record_paths() {
    for access in ["write", "mut"] {
        for body in [
            "let held: &write Record = &write outer.records[1]; held.value = 17;",
            "let held: &write u16 = &write outer.records[1].value; fill(&write held);",
            "let held: &write [Record; 2] = &write outer.records; let child: &write Record = &write held[1]; child.value = 17;",
            "let held: &write Record = &write outer.records[1]; let child: &write u16 = &write held.value; fill(&write child);",
        ] {
            let source = format!(
                "data Record [copy] {{ value: u16; }}
                 data Outer {{ records: [Record; 2]; }}
                 machine fill(value: &write u16) {{ value = 17; }}
                 machine forward(outer: &{access} Outer) {{ {body} }}"
            );
            lower_typed_trees(typed(&source))
                .unwrap_or_else(|errors| panic!("{source}: {errors:?}"));
        }
    }
}

#[test]
fn projected_write_only_local_rejects_dynamic_and_mutable_capture() {
    for body in [
        "let held: &write Record = &write records[index]; held.value = 17;",
        "let held: &write Record = &write records[2]; held.value = 17;",
        "let mut held: &write Record = &write records[1]; held.value = 17;",
    ] {
        let source = format!(
            "data Record [copy] {{ value: u16; }}
             machine forward(records: &write [Record; 2], index: u64) {{ {body} }}"
        );
        let rendered = rendered_rejection(&source);
        assert!(
            rendered.contains("forms `&write` from an unsupported projection"),
            "{source}: {rendered}"
        );
    }
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
        let result = lower_typed_trees(typed(&source));
        assert_eq!(result.is_ok(), accepted, "{source}");
    }
}

#[test]
fn projected_write_only_local_does_not_widen_access() {
    for body in [
        "let held: &write Record = &write records[1]; let observed: u16 = held.value;",
        "let held: &write Record = &write records[1]; read(&held);",
        "let held: &write Record = &write records[1]; mutate(&mut held);",
    ] {
        let source = format!(
            "data Record [copy] {{ value: u16; }}
             machine read(value: &Record) {{}}
             machine mutate(value: &mut Record) {{}}
             machine forward(records: &write [Record; 2]) {{ {body} }}"
        );
        let rendered = rendered_rejection(&source);
        assert!(rendered.contains("write-only"), "{source}: {rendered}");
        assert!(
            !rendered.contains("forms `&write` from an unsupported projection"),
            "formation should be admitted: {rendered}"
        );
    }
}

#[test]
fn mut_rooted_exact_atom_write_only_subloans_are_forwardable() {
    // An `&mut` place may lend `&write` when the lent place carries the
    // callee's declared referee atom-for-atom: the `&mut`→`&write`
    // attenuation keeps every constraint atom the place declared. The gate
    // runs whether or not an unrelated `&write` root exists in the state —
    // the same call that a declared `&write` parameter once pushed into the
    // generic projection fence is admitted on its own atoms.
    for (name, source) in [
        (
            "whole mutable root",
            r#"
                machine replace(value: &write u8 [0..=10]) {
                    value = 7;
                }

                machine forward(limited: &mut u8 [0..=10]) {
                    replace(&write limited);
                }
            "#,
        ),
        (
            "mutable record field",
            r#"
                data Outer { value: u8 [0..=10]; }

                machine replace(value: &write u8 [0..=10]) {
                    value = 7;
                }

                machine forward(outer: &mut Outer) {
                    replace(&write outer.value);
                }
            "#,
        ),
        (
            "mutable record field beside a write-only root",
            r#"
                data Outer { value: u8 [0..=10]; }

                machine replace(value: &write u8 [0..=10]) {
                    value = 7;
                }

                machine forward(outer: &mut Outer, pad: &write u16) {
                    pad = 3;
                    replace(&write outer.value);
                }
            "#,
        ),
        (
            "mutable carrier local",
            r#"
                data Outer { value: u8 [0..=10]; }

                machine replace(value: &write u8 [0..=10]) {
                    value = 7;
                }

                machine forward(outer: &mut Outer) {
                    let alias: &mut Outer = &mut outer;
                    replace(&write alias.value);
                }
            "#,
        ),
        (
            "attached field under a mutable receiver",
            r#"
                data Boxed { value: u8 [0..=10]; }

                machine replace(value: &write u8 [0..=10]) {
                    value = 7;
                }

                machine Boxed::forward(&mut self) {
                    replace(&write self.value);
                }
            "#,
        ),
    ] {
        lower_typed_trees(typed(source)).unwrap_or_else(|errors| {
            panic!(
                "{name}: an exact-atom `&write` subloan lent from a mutable place should lower: {errors:?}"
            )
        });
    }
}

#[test]
fn mut_rooted_write_only_subloans_reject_non_exact_referees() {
    // The same place lent through `&write` instead of `&mut` produced this
    // directed diagnostic; the `&mut`→`&write` attenuation must face the
    // identical atom-exact gate — with and without an unrelated declared
    // `&write` root — rather than silently dropping or strengthening atoms.
    for (name, place, source) in [
        (
            "whole mutable root sheds its range",
            "lends `limited` of type `u8[0..=10]` as `&write` to a parameter declared `&write u8`",
            r#"
                machine replace(value: &write u8) {}

                machine forward(limited: &mut u8 [0..=10]) {
                    replace(&write limited);
                }
            "#,
        ),
        (
            "mutable record field sheds its range",
            "lends `outer.value` of type `u8[0..=10]` as `&write` to a parameter declared `&write u8`",
            r#"
                data Outer { value: u8 [0..=10]; }

                machine replace(value: &write u8) {}

                machine forward(outer: &mut Outer) {
                    replace(&write outer.value);
                }
            "#,
        ),
        (
            "mutable record field beside a write-only root",
            "lends `outer.value` of type `u8[0..=10]` as `&write` to a parameter declared `&write u8`",
            r#"
                data Outer { value: u8 [0..=10]; }

                machine replace(value: &write u8) {}

                machine forward(outer: &mut Outer, pad: &write u16) {
                    pad = 3;
                    replace(&write outer.value);
                }
            "#,
        ),
        (
            "mutable record field takes a stronger range",
            "lends `outer.value` of type `u8[0..=10]` as `&write` to a parameter declared `&write u8[0..=5]`",
            r#"
                data Outer { value: u8 [0..=10]; }

                machine replace(value: &write u8 [0..=5]) {}

                machine forward(outer: &mut Outer) {
                    replace(&write outer.value);
                }
            "#,
        ),
        (
            "mutable carrier local sheds its range",
            "lends `alias.value` of type `u8[0..=10]` as `&write` to a parameter declared `&write u8`",
            r#"
                data Outer { value: u8 [0..=10]; }

                machine replace(value: &write u8) {}

                machine forward(outer: &mut Outer) {
                    let alias: &mut Outer = &mut outer;
                    replace(&write alias.value);
                }
            "#,
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains(place)
                && rendered.contains("must preserve the callee-declared constraint atoms exactly"),
            "{name}: an `&mut`-rooted `&write` subloan with a non-exact referee must take the directed diagnostic: {rendered}"
        );
        assert!(
            !rendered.contains("unsupported projection"),
            "{name}: the directed atom diagnostic, not the generic projection fence, should report the mismatch: {rendered}"
        );
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
fn mut_rooted_write_only_reads_stay_legal() {
    // Mutable formation sources are never write-only roots: ordinary reads
    // through an `&mut` place in a state that forms `&write` subloans must
    // keep passing expression validation unchanged.
    lower_typed_trees(typed(
        r#"
            data Outer { value: u8 [0..=10]; }

            machine replace(value: &write u8 [0..=10]) {
                value = 7;
            }

            machine inspect(outer: &mut Outer) -> u8 {
                replace(&write outer.value);
                outer.value
            }
        "#,
    ))
    .expect("reading through a mutable place beside a `&write` formation must stay legal");
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
        lower_typed_trees(typed(source)).unwrap_or_else(|errors| {
            panic!(
                "{name}: an exact-atom `&write` subloan lent from a `mut` value binding should lower: {errors:?}"
            )
        });
    }
}

#[test]
fn mut_value_binding_write_only_subloans_reject_non_exact_referees() {
    // The `mut` value binding faces the identical atom-exact gate an `&mut`
    // place takes: shedding the place's range onto a wider referee and
    // strengthening it onto a narrower one are the same mismatch, reported
    // by the directed subloan diagnostic rather than the generic projection
    // fence — with and without an unrelated declared `&write` root.
    for (name, place, source) in [
        (
            "mutable local sheds its range",
            "lends `x` of type `u8[0..=10]` as `&write` to a parameter declared `&write u8`",
            r#"
                machine replace(value: &write u8) {}

                machine forward() {
                    let mut x: u8 [0..=10] = 0;
                    replace(&write x);
                }
            "#,
        ),
        (
            "mutable local takes a stronger range",
            "lends `x` of type `u8[0..=10]` as `&write` to a parameter declared `&write u8[0..=5]`",
            r#"
                machine replace(value: &write u8 [0..=5]) {}

                machine forward() {
                    let mut x: u8 [0..=10] = 0;
                    replace(&write x);
                }
            "#,
        ),
        (
            "mutable value parameter sheds its range",
            "lends `x` of type `u8[0..=10]` as `&write` to a parameter declared `&write u8`",
            r#"
                machine replace(value: &write u8) {}

                machine forward(mut x: u8 [0..=10]) {
                    replace(&write x);
                }
            "#,
        ),
        (
            "mutable value parameter sheds it beside a write-only root",
            "lends `x` of type `u8[0..=10]` as `&write` to a parameter declared `&write u8`",
            r#"
                machine replace(value: &write u8) {}

                machine forward(mut x: u8 [0..=10], pad: &write u16) {
                    pad = 3;
                    replace(&write x);
                }
            "#,
        ),
        (
            "mutable local record field sheds its range",
            "lends `outer.value` of type `u8[0..=10]` as `&write` to a parameter declared `&write u8`",
            r#"
                data Outer { value: u8 [0..=10]; }

                machine replace(value: &write u8) {}

                machine forward() {
                    let mut outer: Outer = Outer { value: 1 };
                    replace(&write outer.value);
                }
            "#,
        ),
        (
            "mutable value parameter record field sheds its range",
            "lends `outer.value` of type `u8[0..=10]` as `&write` to a parameter declared `&write u8`",
            r#"
                data Outer { value: u8 [0..=10]; }

                machine replace(value: &write u8) {}

                machine forward(mut outer: Outer) {
                    replace(&write outer.value);
                }
            "#,
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains(place)
                && rendered.contains("must preserve the callee-declared constraint atoms exactly"),
            "{name}: a `mut`-value-rooted `&write` subloan with a non-exact referee must take the directed diagnostic: {rendered}"
        );
        assert!(
            !rendered.contains("unsupported projection"),
            "{name}: the directed atom diagnostic, not the generic projection fence, should report the mismatch: {rendered}"
        );
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
        lower_typed_trees(typed(source)).unwrap_or_else(|errors| {
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
        lower_typed_trees(typed(source)).unwrap_or_else(|errors| {
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

#[test]
fn consuming_receiver_stays_readable_beside_write_only_formation() {
    // A consuming `self` is a formation source, never a write-only root:
    // ordinary reads through it in a state that forms `&write` subloans keep
    // passing expression validation unchanged.
    lower_typed_trees(typed(
        r#"
            data Boxed { value: u8 [0..=10]; }

            machine replace(value: &write u8 [0..=10]) {
                value = 7;
            }

            machine Boxed::inspect(self) -> u8 {
                replace(&write self.value);
                self.value
            }
        "#,
    ))
    .expect("reading through a consuming `self` beside a `&write` formation must stay legal");
}
