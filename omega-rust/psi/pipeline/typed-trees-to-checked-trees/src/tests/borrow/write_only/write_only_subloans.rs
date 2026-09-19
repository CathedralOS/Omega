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
            "qualified leaf",
            r#"
                data Leaf [copy] { value: u16; }
                domain Leaf::Valid requires self.value <= 10;
                data Outer { leaf: Leaf in Valid; }
                machine replace(value: &write Leaf in Valid) {}
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
            "constrained leaf",
            r#"
                data Outer { value: u16 [0..=10]; }
                machine replace(value: &write u16 [0..=10]) {}
                machine forward(outer: &write Outer) {
                    replace(&write outer.value);
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
fn closed_ranged_record_field_subloan_remains_fenced() {
    // Assignment stores may displace a closed-ranged leaf in place, but a
    // `&write` subloan attenuates to the callee's declared referee — `&write
    // u8` here — where the field's bound is no longer visible. Until ranged
    // write-only referees exist, that boundary keeps ranged leaves fenced.
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
        rendered.contains("forms `&write` from an unsupported projection"),
        "a ranged leaf must not attenuate into a wider `&write` referee: {rendered}"
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
