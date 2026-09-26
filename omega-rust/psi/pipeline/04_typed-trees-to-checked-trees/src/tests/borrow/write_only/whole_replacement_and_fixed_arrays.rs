use super::rendered_rejection;
use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;

#[test]
fn closed_material_copy_sums_are_wholly_replaceable() {
    lower_typed_trees(
        typed_program(
            r#"
            data Choice [copy] {
                case Empty;
                case Value(value: u16);
            }
            data Holder { choice: Choice; sibling: u8; }

            machine replace(
                direct: &write Choice,
                holder: &write Holder,
                replacement: Choice
            ) {
                direct = replacement;
                direct = Choice::Empty;
                holder.choice = Choice::Value { value: 9 };
            }

            machine forward(direct: &write Choice, replacement: Choice) {
                replace_direct(&write direct, replacement);
            }

            machine replace_direct(direct: &write Choice, replacement: Choice) {
                direct = replacement;
            }
        "#,
        ),
        &CheckingRequest::settled(),
    )
    .expect("a closed material copy sum is one atomic whole-value store");
}

#[test]
fn ineligible_record_leaves_remain_outside_whole_replacement() {
    for (name, source, expected) in [
        (
            "affine",
            r#"
                data Leaf { value: u16; }
                data Holder { leaf: Leaf; }
                machine replace(holder: &write Holder, replacement: Leaf) {
                    holder.leaf = move replacement;
                }
            "#,
            "whole eligible unrestricted record",
        ),
        (
            "linear",
            r#"
                data Leaf [linear] { value: u16; }
                data Holder { leaf: Leaf; }
                machine replace(holder: &write Holder, replacement: Leaf) {
                    holder.leaf = move replacement;
                }
            "#,
            "whole eligible unrestricted record",
        ),
        (
            "generic",
            r#"
                data Leaf<T [copy]> [copy] { value: T; }
                data Holder { leaf: Leaf<u16>; }
                machine replace(holder: &write Holder, replacement: Leaf<u16>) {
                    holder.leaf = replacement;
                }
            "#,
            "whole eligible unrestricted record",
        ),
        (
            "invariant bearing",
            r#"
                data Leaf [copy]
                where
                    value <= limit,
                {
                    value: u16;
                    limit: u16;
                }
                data Holder { leaf: Leaf; }
                machine replace(holder: &write Holder, replacement: Leaf) {
                    holder.leaf = replacement;
                }
            "#,
            "invariant-dependent",
        ),
        (
            "unproven domain value",
            r#"
                data Leaf [copy] { value: u16; }
                domain Leaf::Valid
                requires
                    self.value <= 10;
                data Holder { leaf: Leaf in Valid; }
                machine replace(holder: &write Holder, replacement: Leaf) {
                    holder.leaf = replacement;
                }
            "#,
            "requires every write to be established in that domain",
        ),
        (
            "erased",
            r#"
                data Leaf [copy] { value: u16; }
                data Holder { leaf [erased]: Leaf; }
                machine replace(holder: &write Holder, replacement: Leaf) {
                    holder.leaf = replacement;
                }
            "#,
            "erased field `leaf` has no runtime value",
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains(expected),
            "{name} leaf unexpectedly crossed the bounded record gate: {rendered}"
        );
    }
}

#[test]
fn ineligible_sum_shapes_remain_outside_whole_replacement() {
    for (name, source, expected) in [
        (
            "affine",
            r#"
                data Choice { case Empty; case Value(value: u16); }
                machine replace(choice: &write Choice, replacement: Choice) {
                    choice = move replacement;
                }
            "#,
            "freely discardable supported root",
        ),
        (
            "linear",
            r#"
                data Choice [linear] { case Empty; case Value(value: u16); }
                machine replace(choice: &write Choice, replacement: Choice) {
                    choice = move replacement;
                }
            "#,
            "freely discardable supported root",
        ),
        (
            "generic",
            r#"
                data Choice<T [copy]> [copy] { case Empty; case Value(value: T); }
                data Holder { choice: Choice<u16>; }
                machine replace(holder: &write Holder, replacement: Choice<u16>) {
                    holder.choice = replacement;
                }
            "#,
            "unsupported write-only projection",
        ),
        (
            "invariant and zero gate",
            r#"
                data Choice [copy]
                where
                    1 == 0,
                {
                    case Empty;
                    case Value(value: u16);
                }
                machine replace(choice: &write Choice, replacement: Choice) {
                    choice = replacement;
                }
            "#,
            "freely discardable supported root",
        ),
        (
            "erased payload",
            r#"
                data Choice [copy] {
                    case Empty;
                    case Value(value: u16, proof [erased]: u16);
                }
                machine replace(choice: &write Choice, replacement: Choice) {
                    choice = replacement;
                }
            "#,
            "freely discardable supported root",
        ),
        (
            "proof-only quotient",
            r#"
                data Carrier [copy] { case Unit; }
                proposition same(left: Carrier, right: Carrier) = left == right;
                data Choice = Carrier % same;
                machine replace(choice: &write Choice, replacement: Choice) {
                    choice = replacement;
                }
            "#,
            "freely discardable supported root",
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains(expected),
            "{name} sum unexpectedly crossed the bounded whole-value gate: {rendered}"
        );
    }
}

#[test]
fn copy_sum_observation_matching_and_swap_attempts_remain_rejected() {
    for (name, body) in [
        ("observation/take", "let prior: Choice = choice;"),
        (
            "matching/payload projection",
            "transition choice { Choice::Value { value } -> value Choice::Empty -> 0 }",
        ),
        (
            "swap",
            "let prior: Choice = choice; choice = replacement; replacement = prior;",
        ),
    ] {
        let source = format!(
            r#"
                data Choice [copy] {{
                    case Empty;
                    case Value(value: u16);
                }}
                machine inspect(choice: &write Choice, replacement: Choice) -> u16 {{
                    {body}
                    transition {{ _ -> 0 }}
                }}
            "#
        );
        let rendered = rendered_rejection(&source);
        assert!(
            rendered.contains("write-only parameter `choice`")
                && (rendered.contains("never observation")
                    || rendered.contains("never grants observation")),
            "{name} unexpectedly observed a write-only sum: {rendered}"
        );
    }
}

#[test]
fn fixed_arrays_of_material_copy_records_support_the_closed_operation_set() {
    lower_typed_trees(
        typed_program(
            r#"
            data Leaf [copy] { value: u16; enabled: bool; }
            data Holder { leaves: [Leaf; 4]; sibling: u8; }

            machine fill(
                direct: &write [Leaf; 4],
                holder: &write Holder,
                whole: [Leaf; 4],
                first: Leaf,
                second: Leaf,
                index: u64 [0..=3]
            ) {
                let direct_length: u64 = direct.len;
                direct = whole;
                direct[0] = first;
                direct[index] = second;
                direct[1..3] = [first, second];

                let nested_length: u64 = holder.leaves.len;
                holder.leaves = whole;
                holder.leaves[0] = first;
                holder.leaves[index] = second;
                holder.leaves[1..=2] = [first, second];
            }
        "#,
        ),
        &CheckingRequest::settled(),
    )
    .expect("material plain copy records stay atomic across the closed fixed-array operations");
}

#[test]
fn recursively_literal_fixed_arrays_support_atomic_outer_operations() {
    lower_typed_trees(
        typed_program(
            r#"
            data Holder { grids: [[u16; 2]; 4]; sibling: u8; }

            machine fill(
                direct: &write [[u16; 2]; 4],
                deep: &write [[[u16; 2]; 2]; 2],
                holder: &write Holder,
                whole: [[u16; 2]; 4],
                row: [u16; 2],
                index: u64 [0..=3]
            ) {
                let direct_length: u64 = direct.len;
                direct = whole;
                direct[0] = row;
                direct[index] = row;
                direct[1..3] = [[1, 2], [3, 4]];
                let deep_length: u64 = deep.len;
                deep = [[[1, 2], [3, 4]], [[5, 6], [7, 8]]];

                let nested_length: u64 = holder.grids.len;
                holder.grids = whole;
                holder.grids[0] = row;
                holder.grids[index] = row;
                holder.grids[1..=2] = [[5, 6], [7, 8]];
            }
        "#,
        ),
        &CheckingRequest::settled(),
    )
    .expect("nested fixed arrays remain atomic across the outer closed operation set");
}

#[test]
fn immutable_local_copy_bounds_are_writable() {
    lower_typed_trees(
        typed_program(
            r#"
            machine fill(values: &write [u16; 4]) {
                let first: u64 = 1;
                let first_alias: u64 = first;
                let last: u64 = 2;
                let last_alias: u64 = last;
                values[first_alias..=last_alias] = [7, 8];
            }
        "#,
        ),
        &CheckingRequest::settled(),
    )
    .expect("finite immutable local-copy bounds should normalize without observing the referent");
}

#[test]
fn non_byte_fixed_array_range_shape_fences_remain_closed() {
    for (name, source, expected) in [
        (
            "symbolic bound",
            r#"
                machine fill(values: &write [u16; 4], start: u64 [0..=2]) {
                    values[start..3] = [1, 2, 3];
                }
            "#,
            "bounds are not statically known",
        ),
        (
            "open end",
            r#"
                machine fill(values: &write [u32; 4]) {
                    values[1..] = [1, 2, 3];
                }
            "#,
            "omitted end",
        ),
        (
            "nonliteral replacement",
            r#"
                machine fill(values: &write [u16; 4], replacement: [u16; 2]) {
                    values[1..3] = replacement;
                }
            "#,
            "from a non-literal value",
        ),
        (
            "wrong-width literal",
            r#"
                machine fill(values: &write [u16; 4]) {
                    values[1..3] = [7];
                }
            "#,
            "must supply exactly 2 element(s)",
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains(expected),
            "{name} unexpectedly crossed the closed range gate: {rendered}"
        );
    }
}

#[test]
fn ineligible_fixed_array_element_shapes_remain_rejected() {
    for (name, source) in [
        (
            "atomic",
            r#"machine fill(values: &write [AtomicU32; 2]) {}"#,
        ),
        (
            "nested atomic",
            r#"machine fill(values: &write [[AtomicU32; 2]; 2]) {}"#,
        ),
        (
            "constrained",
            r#"machine fill(values: &write [u16 [0..=10]; 2]) {}"#,
        ),
        ("generic", r#"machine fill<T>(values: &write [T; 2]) {}"#),
        (
            "noncopy",
            r#"
                data Receipt [linear] { code: u8; }
                machine fill(values: &write [Receipt; 2]) {}
            "#,
        ),
        (
            "affine sum",
            r#"
                data Choice {
                    case First(value: u16);
                    case Second;
                }
                machine fill(values: &write [Choice; 2]) {}
            "#,
        ),
        (
            "erased copy sum shape",
            r#"
                data Choice [copy] {
                    case Empty;
                    case Value(value: u16, proof [erased]: u16);
                }
                machine fill(values: &write [Choice; 2]) {}
            "#,
        ),
        (
            "qualified element",
            r#"
                domain [u8; 4]::Utf8
                requires
                    valid_utf8(self);

                machine fill(values: &write [[u8; 4] in Utf8; 2]) {}
            "#,
        ),
        (
            "affine record",
            r#"
                data Leaf { value: u16; }
                machine fill(values: &write [Leaf; 2]) {}
            "#,
        ),
        (
            "generic copy record",
            r#"
                data Leaf<T [copy]> [copy] { value: T; }
                machine fill(values: &write [Leaf<u16>; 2]) {}
            "#,
        ),
        (
            "invariant-bearing copy record",
            r#"
                data Leaf [copy]
                where
                    value <= limit,
                {
                    value: u16;
                    limit: u16;
                }
                machine fill(values: &write [Leaf; 2]) {}
            "#,
        ),
        (
            "qualified copy record",
            r#"
                data Leaf [copy] { value: u16; }
                domain Leaf::Valid
                requires
                    self.value <= 10;
                machine fill(values: &write [Leaf in Valid; 2]) {}
            "#,
        ),
        (
            "erased copy record shape",
            r#"
                data Leaf [copy] { value: u16; proof [erased]: u16; }
                machine fill(values: &write [Leaf; 2]) {}
            "#,
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains(
                "recursively literal fixed arrays whose ultimate elements are unrestricted primitive scalars or eligible material `[copy]` records or sums"
            ),
            "{name} array unexpectedly reached the checked write-only slice: {rendered}"
        );
    }
}

#[test]
fn non_byte_fixed_array_indexes_still_require_an_ordinary_bounds_proof() {
    for (name, source) in [
        (
            "literal out of bounds",
            r#"
                machine fill(values: &write [u16; 2]) {
                    values[2] = 7;
                }
            "#,
        ),
        (
            "unproved dynamic index",
            r#"
                machine fill(values: &write [u32; 2], index: u64) {
                    values[index] = 7;
                }
            "#,
        ),
    ] {
        let rendered = rendered_rejection(source);
        let expected = if name == "literal out of bounds" {
            "the literal index is outside the fixed array"
        } else {
            "cannot prove index `index` is within length 2"
        };
        assert!(
            rendered.contains(expected),
            "{name} unexpectedly bypassed ordinary index checking: {rendered}"
        );
    }
}
