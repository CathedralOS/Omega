use super::desugar_generic_data_instances;
use omega_layout::{DataShape, build_layout_plan};
use omega_target::NativeTarget;
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use psi_source::SourceMap;
use psi_source_files_to_tokens::Lexer;
use psi_syntax_trees::expression::ExpressionNode;
use psi_syntax_trees::item::{DataMember, Item};
use psi_syntax_trees::statement::StatementNode;
use psi_syntax_trees::types::TypeReferenceNode;
use psi_tokens_to_syntax_trees::{parse_syntax_trees, parse_syntax_trees_with_id};
use std::{path::PathBuf, sync::Arc};

fn checked(source: &str) -> Result<CheckedTrees, Vec<Diagnostic>> {
    let mut sources = SourceMap::default();
    let source_id = sources
        .add(PathBuf::from("generic-instances.omg"), source.to_owned())
        .source_id;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees_with_id(source_id, &tokens).expect("parse");
    desugar_generic_data_instances(&mut syntax)?;
    let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
        &syntax,
        Arc::new(sources),
    )?;
    let typed = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|diagnostic| vec![diagnostic])?;
    psi_typed_trees_to_checked_trees::lower_typed_trees(typed)
}

fn rejected(source: &str, expected: &str) {
    let diagnostics = checked(source).expect_err("program should be rejected");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected)),
        "expected diagnostic containing {expected:?}, got: {:?}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
    );
}

fn fixed_range_loans(
    checked: &CheckedTrees,
) -> Vec<(psi_checked_trees::BorrowAccessKind, usize, usize)> {
    checked
        .facts
        .borrow
        .loans
        .iter()
        .filter_map(|(_, loan)| {
            let [psi_facts::PlaceSegment::FixedRange { start, end }] =
                checked.facts.borrow.loan_segments(loan)
            else {
                return None;
            };
            Some((loan.kind.clone(), *start, *end))
        })
        .collect()
}

#[test]
fn closed_data_instance_rejects_unsatisfied_property_bound() {
    rejected(
        r#"
            data Linear { value: u8; }
            data Cell<T [copy]> { value: T; }
            data Generated { value: Cell<Linear>; }
        "#,
        "does not satisfy `[copy]`",
    );
}

#[test]
fn direct_phantom_lifetime_generic_recasts_retain_exact_ranges_for_both_polarities() {
    let source = r#"
        data Phantom<'region, T> {
            tag: u8;
            value: T;
        }

        data Cell { bytes: [u8; 32]; }

        machine observe<'region>(value: &'region Phantom<'region, u32>) {}

        machine Cell::exercise<'region>(&mut self) {
            let shared: &Phantom<'region, u32> =
                &self.bytes[2] as &Phantom<'region, u32>;
            observe(shared);
            let mutable: &mut Phantom<'region, u32> =
                &mut self.bytes[12] as &mut Phantom<'region, u32>;
            mutable.value = 1;
        }
    "#;

    let checked = checked(source).expect("direct phantom-lifetime recasts should check");
    let instance = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Phantom<u32>")
        .expect("exact synthesized Type instance");
    assert_eq!(instance.lifetime_parameters.len(), 1);
    assert!(instance.generic_instance.is_some());
    assert!(checked.data_type_parameters(instance).is_empty());

    let loans = fixed_range_loans(&checked);
    assert_eq!(loans.len(), 2, "one exact loan per direct lifetime shell");
    assert!(loans.contains(&(psi_checked_trees::BorrowAccessKind::Read, 2, 10)));
    assert!(loans.contains(&(psi_checked_trees::BorrowAccessKind::Mutable, 12, 20)));
}

#[test]
fn direct_phantom_lifetime_shell_retains_checked_lifetime_identity() {
    let checked = checked(
        r#"
            data Phantom<'region, T> { tag: u8; value: T; }
            data Cell { bytes: [u8; 32]; }
            machine observe_left<'left>(value: &'left Phantom<'left, u32>) {}
            machine observe_right<'right>(value: &'right Phantom<'right, u32>) {}
            machine Cell::left<'left>(&mut self) {
                let view: &Phantom<'left, u32> =
                    &self.bytes[2] as &Phantom<'left, u32>;
                observe_left(view);
            }
            machine Cell::right<'right>(&mut self) {
                let view: &Phantom<'right, u32> =
                    &self.bytes[12] as &Phantom<'right, u32>;
                observe_right(view);
            }
        "#,
    )
    .expect("distinct erased lifetime spellings retain one shared physical instance");
    let instance_symbol = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Phantom<u32>")
        .expect("synthesized Phantom<u32>")
        .symbol;
    let mut shells = checked
        .expression_table
        .iter_expressions()
        .filter_map(|(_, expression)| {
            let psi_checked_trees::expression::ExpressionNode::Cast(cast) = expression else {
                return None;
            };
            let psi_checked_trees::types::TypeReferenceNode::Generic {
                base_symbol,
                lifetime_arguments,
                arguments,
                ..
            } = checked
                .type_reference_table
                .type_reference(cast.target_type)
            else {
                return None;
            };
            assert_eq!(*base_symbol, instance_symbol);
            assert!(
                checked
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .is_empty()
            );
            Some((
                cast.target_type,
                lifetime_arguments
                    .iter()
                    .map(|lifetime| lifetime.as_str().to_string())
                    .collect::<Vec<_>>(),
            ))
        })
        .collect::<Vec<_>>();
    shells.sort_by(|left, right| left.1.cmp(&right.1));
    assert_eq!(shells.len(), 2);
    assert_eq!(shells[0].1, ["left"]);
    assert_eq!(shells[1].1, ["right"]);
    assert_ne!(
        shells[0].0, shells[1].0,
        "checked trees retain distinct raw lifetime applications"
    );
    assert_eq!(
        checked.normalized_type_identity(shells[0].0),
        checked.normalized_type_identity(shells[1].0),
        "erased lifetime applications share physical normalized identity"
    );
    assert_eq!(fixed_range_loans(&checked).len(), 2);
}

#[test]
fn direct_phantom_lifetime_generic_recast_protects_edges_but_not_siblings() {
    for index in [3, 5, 10] {
        rejected(
            &format!(
                r#"
                    data Phantom<'region, T> {{ tag: u8; value: T; }}
                    data Cell {{ bytes: [u8; 16]; }}
                    machine observe<'region>(value: &'region Phantom<'region, u32>) {{}}
                    machine exercise<'region>(cell: &'region mut Cell) {{
                        let view: &Phantom<'region, u32> =
                            &cell.bytes[3] as &Phantom<'region, u32>;
                        cell.bytes[{index}] = 1;
                        observe(view);
                    }}
                "#
            ),
            &format!("mutates `cell.bytes[{index}]` while local borrow `view` is still active"),
        );
    }

    rejected(
        r#"
            data Phantom<'region, T> { tag: u8; value: T; }
            data Cell { bytes: [u8; 16]; }
            machine Cell::exercise<'region>(&mut self) {
                let view: &mut Phantom<'region, u32> =
                    &mut self.bytes[3] as &mut Phantom<'region, u32>;
                self.bytes[5] = 1;
                view.value = 2;
            }
        "#,
        "mutates `self.bytes[5]` while local borrow `view` is still active",
    );

    checked(
        r#"
            data Phantom<'region, T> { tag: u8; value: T; }
            data Cell { bytes: [u8; 16]; }
            machine exercise<'region>(cell: &'region mut Cell) {
                let view: &mut Phantom<'region, u32> =
                    &mut cell.bytes[3] as &mut Phantom<'region, u32>;
                cell.bytes[2] = 1;
                cell.bytes[11] = 1;
                view.value = 2;
            }
        "#,
    )
    .expect("the immediate siblings of [3, 11) remain disjoint");
}

#[test]
fn one_nested_phantom_lifetime_record_recast_retains_exact_ranges() {
    let checked = checked(
        r#"
            data Phantom<'region, T> { tag: u8; value: T; }
            data Holder<'region, T> { nested: Phantom<'region, T>; }
            data Cell { bytes: [u8; 32]; }
            machine observe<'region>(value: &'region Holder<'region, u32>) {}
            machine Cell::exercise<'region>(&mut self) {
                let shared: &Holder<'region, u32> =
                    &self.bytes[2] as &Holder<'region, u32>;
                observe(shared);
                let mutable: &mut Holder<'region, u32> =
                    &mut self.bytes[12] as &mut Holder<'region, u32>;
                mutable.nested.value = 1;
            }
        "#,
    )
    .expect("one nested phantom-lifetime record shell should check");

    let holder = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Holder<u32>")
        .expect("exact synthesized Holder<u32> instance");
    assert_eq!(holder.lifetime_parameters.len(), 1);
    assert!(holder.generic_instance.is_some());
    assert!(checked.data_type_parameters(holder).is_empty());
    let [psi_checked_trees::data::DataMember::Field(nested)] = checked.data_members(holder) else {
        panic!("Holder<u32> keeps one exact nested field")
    };
    let psi_checked_trees::types::TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = checked
        .type_reference_table
        .type_reference(nested.type_reference)
    else {
        panic!("the nested lifetime shell remains explicit in checked trees")
    };
    assert_eq!(lifetime_arguments.len(), 1);
    assert!(
        checked
            .type_reference_table
            .type_reference_handles(*arguments)
            .is_empty()
    );
    assert!(
        checked
            .data_definitions()
            .iter()
            .any(|data| data.symbol == *base_symbol && data.name.as_str() == "Phantom<u32>")
    );

    let loans = fixed_range_loans(&checked);
    assert_eq!(loans.len(), 2, "one exact loan per nested lifetime shell");
    assert!(loans.contains(&(psi_checked_trees::BorrowAccessKind::Read, 2, 10)));
    assert!(loans.contains(&(psi_checked_trees::BorrowAccessKind::Mutable, 12, 20)));
}

#[test]
fn broader_lifetime_generic_recast_targets_remain_fenced() {
    for source in [
        r#"
            data Borrowed<'region, T> { source: &'region u32; value: T; }
            data Cell { bytes: [u8; 32]; }
            machine exercise<'region>(cell: &'region mut Cell) {
                let view: &Borrowed<'region, u32> =
                    &cell.bytes[2] as &Borrowed<'region, u32>;
            }
        "#,
        r#"
            data Phantom<'region, T> { tag: u8; value: T; }
            data Cell { bytes: [u8; 32]; }
            machine exercise<'region>(cell: &'region mut Cell) {
                let view: &[Phantom<'region, u32>; 1] =
                    &cell.bytes[2] as &[Phantom<'region, u32>; 1];
            }
        "#,
        r#"
            data Phantom<'region, T> { tag: u8; value: T; }
            data Wrapper<T> { value: T; }
            data Holder<'region, T> {
                values: [Wrapper<Phantom<'region, T>>; 1];
            }
            data Cell { bytes: [u8; 32]; }
            machine exercise<'region>(cell: &'region mut Cell) {
                let view: &Holder<'region, u32> =
                    &cell.bytes[2] as &Holder<'region, u32>;
            }
        "#,
        r#"
            data Empty<'region, T> { bytes: [u8; 0]; }
            data Cell { bytes: [u8; 32]; }
            machine exercise<'region>(cell: &'region mut Cell) {
                let view: &Empty<'region, u32> =
                    &cell.bytes[2] as &Empty<'region, u32>;
            }
        "#,
        r#"
            data Evidence { case Only; }
            data Erased<'region, T> { value: T; proof [erased]: Evidence; }
            data Cell { bytes: [u8; 32]; }
            machine exercise<'region>(cell: &'region mut Cell) {
                let view: &Erased<'region, u32> =
                    &cell.bytes[2] as &Erased<'region, u32>;
            }
        "#,
        r#"
            data Leaf<'region, T> { tag: u8; value: T; }
            data Middle<'region, T> { leaf: Leaf<'region, T>; }
            data Outer<'region, T> { middle: Middle<'region, T>; }
            data Cell { bytes: [u8; 32]; }
            machine exercise<'region>(cell: &'region mut Cell) {
                let view: &Outer<'region, u32> =
                    &cell.bytes[2] as &Outer<'region, u32>;
            }
        "#,
        r#"
            data Leaf<'region, T> { tag: u8; value: T; }
            data Middle<'region, T> { leaf: Leaf<'region, T>; }
            data Diamond<'region, T> {
                direct: Leaf<'region, T>;
                nested: Middle<'region, T>;
            }
            data Cell { bytes: [u8; 32]; }
            machine exercise<'region>(cell: &'region mut Cell) {
                let view: &Diamond<'region, u32> =
                    &cell.bytes[2] as &Diamond<'region, u32>;
            }
        "#,
    ] {
        let diagnostics = checked(source).expect_err(
            "nonphantom, array, and deeper lifetime-generic targets must remain fenced",
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("recast")),
            "expected the ordinary recast diagnostic, got {:?}",
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn closed_generic_pair_recasts_retain_padded_ranges_for_both_polarities() {
    let source = r#"
        data Pair<T> {
            tag: u8;
            value: T;
        }

        data Cell {
            bytes: [u8; 32];
        }

        machine observe(value: &Pair<u32>) {
        }

        machine Cell::exercise(&mut self) {
            let shared: &Pair<u32> = &self.bytes[2] as &Pair<u32>;
            observe(shared);
            let mutable: &mut Pair<u32> =
                &mut self.bytes[12] as &mut Pair<u32>;
            mutable.value = 1;
        }
    "#;

    let checked = checked(source).expect("closed Pair<u32> recasts should check");
    let loans = fixed_range_loans(&checked);
    assert_eq!(loans.len(), 2, "one exact loan per generic recast");
    assert!(loans.contains(&(psi_checked_trees::BorrowAccessKind::Read, 2, 10)));
    assert!(loans.contains(&(psi_checked_trees::BorrowAccessKind::Mutable, 12, 20)));
}

#[test]
fn closed_generic_pair_recast_rejects_padding_overlap_but_keeps_siblings_writable() {
    rejected(
        r#"
            data Pair<T> { tag: u8; value: T; }
            data Cell { bytes: [u8; 16]; }
            machine observe(value: &Pair<u32>) {}
            machine Cell::exercise(&mut self) {
                let view: &Pair<u32> = &self.bytes[3] as &Pair<u32>;
                self.bytes[4] = 1;
                observe(view);
            }
        "#,
        "mutates `self.bytes[4]` while local borrow `view` is still active",
    );

    rejected(
        r#"
            data Pair<T> { tag: u8; value: T; }
            data Cell { bytes: [u8; 16]; }
            machine Cell::exercise(&mut self) {
                let view: &mut Pair<u32> =
                    &mut self.bytes[3] as &mut Pair<u32>;
                self.bytes[4] = 1;
                view.value = 2;
            }
        "#,
        "mutates `self.bytes[4]` while local borrow `view` is still active",
    );

    checked(
        r#"
            data Pair<T> { tag: u8; value: T; }
            data Cell { bytes: [u8; 16]; }
            machine Cell::exercise(&mut self) {
                let view: &mut Pair<u32> =
                    &mut self.bytes[3] as &mut Pair<u32>;
                self.bytes[2] = 1;
                self.bytes[11] = 1;
                view.value = 2;
            }
        "#,
    )
    .expect("the bytes immediately before and after [3, 11) are disjoint");
}

#[test]
fn closed_generic_specializations_retain_distinct_symbols_and_nested_holder_ranges() {
    let source = r#"
        data Pair<T> {
            tag: u8;
            value: T;
        }

        data Holder {
            prefix: u8;
            small: [Pair<u16>; 2];
            large: Pair<u32>;
            tail: u16;
        }

        data Cell {
            bytes: [u8; 64];
        }

        machine observe_holder(value: &Holder) {
        }

        machine observe_small(value: &[Pair<u16>; 2]) {
        }

        machine Cell::exercise(&mut self) {
            let holder: &Holder = &self.bytes[2] as &Holder;
            observe_holder(holder);
            let small: &[Pair<u16>; 2] =
                &self.bytes[32] as &[Pair<u16>; 2];
            observe_small(small);
            let large: &mut Pair<u32> =
                &mut self.bytes[44] as &mut Pair<u32>;
            large.value = 1;
        }
    "#;

    let checked = checked(source).expect("closed generic composition should check");
    let pair_u16 = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Pair<u16>")
        .expect("Pair<u16> instance");
    let pair_u32 = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Pair<u32>")
        .expect("Pair<u32> instance");
    assert_ne!(pair_u16.symbol, pair_u32.symbol);
    assert!(pair_u16.generic_instance.is_some());
    assert!(pair_u32.generic_instance.is_some());
    assert!(checked.data_type_parameters(pair_u16).is_empty());
    assert!(checked.data_type_parameters(pair_u32).is_empty());

    let loans = fixed_range_loans(&checked);
    assert!(loans.contains(&(psi_checked_trees::BorrowAccessKind::Read, 2, 26)));
    assert!(loans.contains(&(psi_checked_trees::BorrowAccessKind::Read, 32, 40)));
    assert!(loans.contains(&(psi_checked_trees::BorrowAccessKind::Mutable, 44, 52)));
}

#[test]
fn closed_literal_array_type_argument_retains_its_concrete_range() {
    let source = r#"
        data Pair<T> { tag: u8; value: T; }
        data Cell { bytes: [u8; 16]; }
        machine Cell::exercise(&mut self) {
            let view: &mut Pair<[u16; 2]> =
                &mut self.bytes[3] as &mut Pair<[u16; 2]>;
            view.value[1] = 2;
        }
    "#;

    let checked = checked(source).expect("closed array type argument should check");
    assert!(fixed_range_loans(&checked).contains(&(
        psi_checked_trees::BorrowAccessKind::Mutable,
        3,
        9
    )));
}

#[test]
fn closed_type_and_const_block_recasts_retain_exact_padded_ranges() {
    let source = r#"
        data Block<T, const N: u64> {
            tag: u8;
            values: [T; N];
        }
        data Cell { bytes: [u8; 32]; }
        machine observe(value: &Block<u16, 2>) {}
        machine Cell::exercise(&mut self) {
            let shared: &Block<u16, 2> =
                &self.bytes[2] as &Block<u16, 2>;
            observe(shared);
            let mutable: &mut Block<u16, 2> =
                &mut self.bytes[12] as &mut Block<u16, 2>;
            mutable.values[1] = 7;
        }
    "#;

    let checked = checked(source).expect("closed Type + integer Const recasts should check");
    let loans = fixed_range_loans(&checked);
    assert_eq!(loans.len(), 2, "one exact loan per const-generic recast");
    assert!(loans.contains(&(psi_checked_trees::BorrowAccessKind::Read, 2, 8)));
    assert!(loans.contains(&(psi_checked_trees::BorrowAccessKind::Mutable, 12, 18)));
}

#[test]
fn closed_type_and_const_block_protects_padding_but_not_siblings() {
    rejected(
        r#"
            data Block<T, const N: u64> { tag: u8; values: [T; N]; }
            data Cell { bytes: [u8; 16]; }
            machine Cell::exercise(&mut self) {
                let view: &mut Block<u16, 2> =
                    &mut self.bytes[3] as &mut Block<u16, 2>;
                self.bytes[4] = 1;
                view.values[0] = 2;
            }
        "#,
        "mutates `self.bytes[4]` while local borrow `view` is still active",
    );

    checked(
        r#"
            data Block<T, const N: u64> { tag: u8; values: [T; N]; }
            data Cell { bytes: [u8; 16]; }
            machine Cell::exercise(&mut self) {
                let view: &mut Block<u16, 2> =
                    &mut self.bytes[3] as &mut Block<u16, 2>;
                self.bytes[2] = 1;
                self.bytes[9] = 1;
                view.values[0] = 2;
            }
        "#,
    )
    .expect("the bytes immediately beside padded Block<u16, 2> stay disjoint");
}

#[test]
fn closed_const_specializations_retain_distinct_symbols_and_composed_ranges() {
    let source = r#"
        data Block<T, const N: u64> {
            tag: u8;
            values: [T; N];
        }
        data Holder {
            prefix: u8;
            small: [Block<u16, 2>; 2];
            large: Block<u16, 3>;
            tail: u8;
        }
        data Cell { bytes: [u8; 80]; }
        machine observe_holder(value: &Holder) {}
        machine observe_small(value: &[Block<u16, 2>; 2]) {}
        machine Cell::exercise(&mut self) {
            let holder: &Holder = &self.bytes[2] as &Holder;
            observe_holder(holder);
            let small: &[Block<u16, 2>; 2] =
                &self.bytes[32] as &[Block<u16, 2>; 2];
            observe_small(small);
            let large: &mut Block<u16, 3> =
                &mut self.bytes[48] as &mut Block<u16, 3>;
            large.tag = 7;
        }
    "#;

    let checked = checked(source).expect("const-generic record composition should check");
    let block_two = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Block<u16, 2>")
        .expect("Block<u16, 2> instance");
    let block_three = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Block<u16, 3>")
        .expect("Block<u16, 3> instance");
    assert_ne!(block_two.symbol, block_three.symbol);

    let loans = fixed_range_loans(&checked);
    assert!(loans.contains(&(psi_checked_trees::BorrowAccessKind::Read, 2, 26)));
    assert!(loans.contains(&(psi_checked_trees::BorrowAccessKind::Read, 32, 44)));
    assert!(loans.contains(&(psi_checked_trees::BorrowAccessKind::Mutable, 48, 56)));
}

#[test]
fn closed_zero_const_array_field_retains_nonzero_containing_record_range() {
    let source = r#"
        data Block<T, const N: u64> { tag: u8; values: [T; N]; }
        data Cell { bytes: [u8; 16]; }
        machine Cell::exercise(&mut self) {
            let view: &mut Block<u16, 0> =
                &mut self.bytes[3] as &mut Block<u16, 0>;
            view.tag = 1;
        }
    "#;

    let checked = checked(source).expect("a nonzero record may contain a zero const array");
    assert!(fixed_range_loans(&checked).contains(&(
        psi_checked_trees::BorrowAccessKind::Mutable,
        3,
        5
    )));
}

#[test]
fn named_and_expression_const_arguments_share_the_canonical_instance() {
    let source = r#"
        const TWO: u64 = 2;
        data Block<T, const N: u64> { tag: u8; values: [T; N]; }
        data Holder {
            literal: Block<u16, 2>;
            expression: Block<u16, 1 + 1>;
            named: Block<u16, TWO>;
        }
    "#;

    let checked = checked(source).expect("equivalent closed const arguments should normalize");
    assert_eq!(
        checked
            .data_definitions()
            .iter()
            .filter(|data| data.name.as_str() == "Block<u16, 2>")
            .count(),
        1
    );
}

#[test]
fn direct_boolean_const_argument_uses_one_canonical_instance_value() {
    let checked = checked(
        r#"
            data Flag<const ENABLED: bool> { marker: u8; }
            data Holder { enabled: Flag<true>; disabled: Flag<false>; }
        "#,
    )
    .expect("direct Boolean const arguments should normalize canonically");

    let values = checked
        .data_definitions()
        .iter()
        .filter_map(|definition| {
            let origin = definition.generic_instance?;
            let psi_checked_trees::types::TypeReferenceNode::Generic { arguments, .. } =
                checked.type_reference_table.type_reference(origin)
            else {
                return None;
            };
            let [argument] = checked
                .type_reference_table
                .type_reference_handles(*arguments)
            else {
                return None;
            };
            let psi_checked_trees::types::TypeReferenceNode::Named { symbol, name } =
                checked.type_reference_table.type_reference(*argument)
            else {
                return None;
            };
            (!symbol.is_valid()).then(|| {
                psi_language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
                    .expect("Boolean instance origin uses a canonical const atom")
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        values,
        [
            psi_language_semantics::const_value::CanonicalConstValue::boolean(true),
            psi_language_semantics::const_value::CanonicalConstValue::boolean(false),
        ]
    );
}

#[test]
fn structured_const_instance_recasts_retain_exact_ranges_for_both_polarities() {
    let source = r#"
        data UnitIndex { scale: u64; }
        data UnitIndices {}
        const UnitIndices::INDEX: UnitIndex = UnitIndex { scale: 2 };
        data Indexed<const U: UnitIndex> { marker: u8; }
        data Cell { bytes: [u8; 16]; }
        machine observe(value: &Indexed<UnitIndices::INDEX>) {}
        machine Cell::exercise(&mut self) {
            let shared: &Indexed<UnitIndices::INDEX> =
                &self.bytes[2] as &Indexed<UnitIndices::INDEX>;
            observe(shared);
            let mutable: &mut Indexed<UnitIndices::INDEX> =
                &mut self.bytes[8] as &mut Indexed<UnitIndices::INDEX>;
            mutable.marker = 7;
        }
    "#;

    let checked = checked(source).expect("structured const recasts should check");
    let loans = fixed_range_loans(&checked);
    assert_eq!(loans.len(), 2, "one exact loan per structured const recast");
    assert!(loans.contains(&(psi_checked_trees::BorrowAccessKind::Read, 2, 3)));
    assert!(loans.contains(&(psi_checked_trees::BorrowAccessKind::Mutable, 8, 9)));
}

#[test]
fn structured_const_instance_recast_rejects_overlap_but_keeps_siblings_writable() {
    rejected(
        r#"
            data UnitIndex { scale: u64; }
            data UnitIndices {}
            const UnitIndices::INDEX: UnitIndex = UnitIndex { scale: 2 };
            data Indexed<const U: UnitIndex> { marker: u8; }
            data Cell { bytes: [u8; 8]; }
            machine observe(value: &Indexed<UnitIndices::INDEX>) {}
            machine Cell::exercise(&mut self) {
                let view: &Indexed<UnitIndices::INDEX> =
                    &self.bytes[3] as &Indexed<UnitIndices::INDEX>;
                self.bytes[3] = 1;
                observe(view);
            }
        "#,
        "mutates `self.bytes[3]` while local borrow `view` is still active",
    );

    rejected(
        r#"
            data UnitIndex { scale: u64; }
            data UnitIndices {}
            const UnitIndices::INDEX: UnitIndex = UnitIndex { scale: 2 };
            data Indexed<const U: UnitIndex> { marker: u8; }
            data Cell { bytes: [u8; 8]; }
            machine Cell::exercise(&mut self) {
                let view: &mut Indexed<UnitIndices::INDEX> =
                    &mut self.bytes[3] as &mut Indexed<UnitIndices::INDEX>;
                self.bytes[3] = 1;
                view.marker = 2;
            }
        "#,
        "mutates `self.bytes[3]` while local borrow `view` is still active",
    );

    checked(
        r#"
            data UnitIndex { scale: u64; }
            data UnitIndices {}
            const UnitIndices::INDEX: UnitIndex = UnitIndex { scale: 2 };
            data Indexed<const U: UnitIndex> { marker: u8; }
            data Cell { bytes: [u8; 8]; }
            machine Cell::exercise(&mut self) {
                let view: &mut Indexed<UnitIndices::INDEX> =
                    &mut self.bytes[3] as &mut Indexed<UnitIndices::INDEX>;
                self.bytes[2] = 1;
                self.bytes[4] = 1;
                view.marker = 2;
            }
        "#,
    )
    .expect("the bytes immediately beside [3, 4) remain disjoint");
}

#[test]
fn structured_const_pure_sum_instances_retain_payloadless_and_payload_ranges() {
    let source = r#"
        data CountPayload { value: u16; }
        data Mode {
            case Idle;
            case Count(payload: CountPayload);
        }
        data Modes {}
        const Modes::IDLE: Mode = Mode::Idle;
        const Modes::COUNT: Mode =
            Mode::Count { payload: CountPayload { value: 7 } };
        data Indexed<const M: Mode> { marker: u8; }
        data Cell { bytes: [u8; 16]; }
        machine observe(value: &Indexed<Modes::IDLE>) {}
        machine Cell::exercise(&mut self) {
            let shared: &Indexed<Modes::IDLE> =
                &self.bytes[2] as &Indexed<Modes::IDLE>;
            observe(shared);
            let mutable: &mut Indexed<Modes::COUNT> =
                &mut self.bytes[8] as &mut Indexed<Modes::COUNT>;
            mutable.marker = 7;
        }
    "#;

    let checked = checked(source).expect("pure-sum structured const recasts should check");
    let loans = fixed_range_loans(&checked);
    assert_eq!(
        loans.len(),
        2,
        "one exact loan per payloadless or payload-bearing const recast"
    );
    assert!(loans.contains(&(psi_checked_trees::BorrowAccessKind::Read, 2, 3)));
    assert!(loans.contains(&(psi_checked_trees::BorrowAccessKind::Mutable, 8, 9)));
}

#[test]
fn structured_const_pure_sum_instance_rejects_overlap_but_keeps_siblings_writable() {
    rejected(
        r#"
            data CountPayload { value: u16; }
            data Mode { case Idle; case Count(payload: CountPayload); }
            data Modes {}
            const Modes::COUNT: Mode =
                Mode::Count { payload: CountPayload { value: 7 } };
            data Indexed<const M: Mode> { marker: u8; }
            data Cell { bytes: [u8; 8]; }
            machine observe(value: &Indexed<Modes::COUNT>) {}
            machine Cell::exercise(&mut self) {
                let view: &Indexed<Modes::COUNT> =
                    &self.bytes[3] as &Indexed<Modes::COUNT>;
                self.bytes[3] = 1;
                observe(view);
            }
        "#,
        "mutates `self.bytes[3]` while local borrow `view` is still active",
    );

    rejected(
        r#"
            data CountPayload { value: u16; }
            data Mode { case Idle; case Count(payload: CountPayload); }
            data Modes {}
            const Modes::COUNT: Mode =
                Mode::Count { payload: CountPayload { value: 7 } };
            data Indexed<const M: Mode> { marker: u8; }
            data Cell { bytes: [u8; 8]; }
            machine Cell::exercise(&mut self) {
                let view: &mut Indexed<Modes::COUNT> =
                    &mut self.bytes[3] as &mut Indexed<Modes::COUNT>;
                self.bytes[3] = 1;
                view.marker = 2;
            }
        "#,
        "mutates `self.bytes[3]` while local borrow `view` is still active",
    );

    checked(
        r#"
            data CountPayload { value: u16; }
            data Mode { case Idle; case Count(payload: CountPayload); }
            data Modes {}
            const Modes::COUNT: Mode =
                Mode::Count { payload: CountPayload { value: 7 } };
            data Indexed<const M: Mode> { marker: u8; }
            data Cell { bytes: [u8; 8]; }
            machine Cell::exercise(&mut self) {
                let view: &mut Indexed<Modes::COUNT> =
                    &mut self.bytes[3] as &mut Indexed<Modes::COUNT>;
                self.bytes[2] = 1;
                self.bytes[4] = 1;
                view.marker = 2;
            }
        "#,
    )
    .expect("the bytes immediately beside the pure-sum const view remain disjoint");
}

#[test]
fn unsupported_closed_generic_stored_arguments_and_open_forms_publish_no_loan() {
    rejected(
        r#"
            data Pair<T> { tag: u8; value: T; }
            data Cell { bytes: [u8; 16]; }
            machine Cell::exercise(&mut self) {
                let view: &Pair<bool> = &self.bytes[0] as &Pair<bool>;
            }
        "#,
        "must be recursively fact-free",
    );

    checked(
        r#"
            data Pair<T> { tag: u8; value: T; }
            data Cell { bytes: [u8; 16]; }
            machine Cell::exercise(&mut self) {
                let view: &mut Pair<AtomicU32> =
                    &mut self.bytes[0] as &mut Pair<AtomicU32>;
                self.bytes[0] = 1;
                view.tag = 2;
            }
        "#,
    )
    .expect("an atomic stored argument remains ordinary-valid but publishes no loan");

    rejected(
        r#"
            data Block<T, const N: u8> { tag: u8; values: [T; N]; }
            data Cell { bytes: [u8; 16]; }
            machine Cell::exercise(&mut self) {
                let view: &mut Block<u16, 256> =
                    &mut self.bytes[0] as &mut Block<u16, 256>;
            }
        "#,
        "does not fit `u8`",
    );

    assert!(
        checked(
            r#"
                data Pair<T> { tag: u8; value: T; }
                data Cell { bytes: [u8; 16]; }
                machine inspect<T>(cell: &mut Cell) {
                    let view: &mut Pair<T> =
                        &mut cell.bytes[0] as &mut Pair<T>;
                    view.tag = 1;
                }
            "#,
        )
        .is_err(),
        "an open generic application must remain outside precise recast admission"
    );

    assert!(
        checked(
            r#"
                data Block<T, const N: u64> { tag: u8; values: [T; N]; }
                data Cell { bytes: [u8; 16]; }
                machine inspect<const N: u64>(cell: &mut Cell) {
                    let view: &mut Block<u16, N> =
                        &mut cell.bytes[0] as &mut Block<u16, N>;
                    view.tag = 1;
                }
            "#,
        )
        .is_err(),
        "an open const application must remain outside precise recast admission"
    );
}

fn local_initializer<'syntax>(
    syntax: &'syntax psi_syntax_trees::SyntaxTrees,
    local_name: &str,
) -> &'syntax ExpressionNode {
    let expression = syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .flat_map(|machine| syntax.items.state_handles(machine.states))
        .flat_map(|state| {
            syntax
                .items
                .statements(syntax.items.state(*state).statements)
        })
        .find_map(|statement| match syntax.statements.statement(*statement) {
            StatementNode::LocalData(local) if local.name.as_str() == local_name => {
                Some(local.initial_value)
            }
            _ => None,
        })
        .expect("named local initializer");
    syntax.expressions.expression(expression)
}

#[test]
fn closed_generic_record_literal_uses_annotated_local_instance() {
    let source = r#"
        data Box<T> { value: T; }
        machine run() -> i32 {
            let boxed: Box<i32> = Box { value: 7 };
            boxed.value
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");

    desugar_generic_data_instances(&mut syntax).expect("monomorphize");

    let ExpressionNode::StructLiteral(literal) = local_initializer(&syntax, "boxed") else {
        panic!("boxed initializer should remain a record literal");
    };
    assert_eq!(literal.type_name.as_str(), "Box<i32>");
}

#[test]
fn closed_generic_data_retains_its_authored_declaration_origin() {
    let source = r#"
        pub data Box<T> { value: T; }
        data Holder { boxed: Box<i32>; }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");
    let base_span = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == "Box" => {
                Some(definition.name.source_span())
            }
            _ => None,
        })
        .expect("generic base declaration");

    desugar_generic_data_instances(&mut syntax).expect("monomorphize");

    let instance = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == "Box<i32>" => Some(definition),
            _ => None,
        })
        .expect("closed generic instance");
    assert!(instance.name.is_source_backed());
    assert_eq!(instance.name.source_span(), base_span);
}

#[test]
fn nested_closed_generic_record_literal_uses_concrete_field_instance() {
    let source = r#"
        data Box<T> { value: T; }
        data Holder<T> { boxed: Box<T>; }
        machine run() -> i32 {
            let holder: Holder<i32> = Holder { boxed: Box { value: 7 } };
            holder.boxed.value
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");

    desugar_generic_data_instances(&mut syntax).expect("monomorphize");

    let ExpressionNode::StructLiteral(holder) = local_initializer(&syntax, "holder") else {
        panic!("holder initializer should remain a record literal");
    };
    assert_eq!(holder.type_name.as_str(), "Holder<i32>");
    let boxed = syntax
        .expressions
        .struct_fields(holder.fields)
        .iter()
        .find(|field| field.name.as_str() == "boxed")
        .expect("boxed field");
    let ExpressionNode::StructLiteral(boxed) = syntax.expressions.expression(boxed.value) else {
        panic!("boxed field should remain a record literal");
    };
    assert_eq!(boxed.type_name.as_str(), "Box<i32>");
}

#[test]
fn closed_generic_erased_record_elaborates_and_lays_out_material_fields_only() {
    let checked = checked(
        r#"
        data Evidence { case Only; case WithPayload(value: i32); }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine run() -> i32 {
            let boxed: Box<i32> = Box { value: 7 };
            boxed.value
        }
        "#,
    )
    .expect("closed generic erased record should check");

    let literal = checked
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            psi_checked_trees::expression::ExpressionNode::StructLiteral(literal)
                if literal.type_name.as_str() == "Box<i32>" =>
            {
                Some(literal)
            }
            _ => None,
        })
        .expect("closed Box literal");
    let fields = checked.expression_table.struct_fields(literal.fields);
    assert_eq!(
        fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["value", "proof"]
    );
    assert!(matches!(
        checked.expression_table.expression(fields[1].value),
        psi_checked_trees::expression::ExpressionNode::Name(_)
    ));
    let evidence = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Evidence")
        .expect("Evidence definition");
    let only_symbol = checked
        .data_members(evidence)
        .iter()
        .find_map(|member| match member {
            psi_checked_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == "Only" =>
            {
                Some(variant.symbol)
            }
            _ => None,
        })
        .expect("Only variant");

    let layout = build_layout_plan(&checked, NativeTarget::host(), &[]).expect("layout");
    let boxed = layout
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "Box<i32>")
        .expect("closed Box layout");
    let DataShape::Record { fields } = boxed.shape else {
        panic!("closed Box should have record layout");
    };
    assert_eq!(layout.fields.span_or_empty(fields).len(), 1);
    assert_eq!(boxed.layout.size, 4);

    assert!(
        checked
            .expression_table
            .iter_expressions()
            .any(|(_, expression)| matches!(
                expression,
                psi_checked_trees::expression::ExpressionNode::Name(path)
                    if path.symbol == only_symbol
            )),
        "the erased witness remains available to proof checking even though layout erases it",
    );
}

#[test]
fn closed_generic_record_literal_checks_substituted_field_type() {
    rejected(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine run() -> i32 {
            let boxed: Box<i32> = Box { value: true };
            0
        }
        "#,
        "stores a boolean into a `i32` field",
    );
}

#[test]
fn closed_generic_record_literal_checks_concrete_field_names() {
    rejected(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine run() -> i32 {
            let boxed: Box<i32> = Box { wrong: 7 };
            0
        }
        "#,
        "data `Box<i32>` has no field `wrong`",
    );
}

#[test]
fn closed_generic_erased_record_rejects_ambiguous_omitted_evidence() {
    rejected(
        r#"
        data Evidence { case First; case Second; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine run() -> i32 {
            let boxed: Box<i32> = Box { value: 7 };
            0
        }
        "#,
        "no unique accessible nullary constructor",
    );
}

#[test]
fn closed_generic_erased_record_accepts_explicit_ambiguous_evidence() {
    checked(
        r#"
        data Evidence { case First; case Second; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine run() -> i32 {
            let boxed: Box<i32> = Box {
                value: 7,
                proof: Evidence::Second,
            };
            boxed.value
        }
        "#,
    )
    .expect("explicit evidence should remain legal");
}

#[test]
fn distinct_closed_generic_erased_record_instances_validate_independently() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine run() -> i32 {
            let integer: Box<i32> = Box { value: 7 };
            let boolean: Box<bool> = Box { value: true };
            integer.value
        }
        "#,
    )
    .expect("each closed instance should use its own substituted field type");
}

#[test]
fn closed_generic_record_literal_uses_exact_assignment_destination() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        data Holder { integer: Box<i32>; boolean: Box<bool>; }
        machine Holder::replace(&mut self) {
            self.integer = Box { value: 7 };
            self.boolean = Box { value: true };
        }
        "#,
    )
    .expect("an exact field assignment should select each closed record identity");
}

#[test]
fn closed_generic_erased_record_still_rejects_generic_evidence_omission() {
    rejected(
        r#"
        data Evidence<U> { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence<i32>; }
        machine run() -> i32 {
            let boxed: Box<i32> = Box { value: 7 };
            0
        }
        "#,
        "no unique accessible nullary constructor",
    );
}

#[test]
fn closed_generic_data_instance_preserves_public_visibility() {
    let source = r#"
        pub data Box<T> { value: T; }
        machine run(value: Box<i32>) {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");

    desugar_generic_data_instances(&mut syntax).expect("monomorphize");

    let instance = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == "Box<i32>" => Some(definition),
            _ => None,
        })
        .expect("closed Box definition");
    assert!(instance.is_public);
}

#[test]
fn closed_generic_composite_shells_substitute_nested_type_parameters() {
    let source = r#"
        data Shell<T> {
            shared: &T;
            values: [T];
            nested: &[T; 2];
        }
        data Generated { value: Shell<u32>; }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");
    desugar_generic_data_instances(&mut syntax).expect("monomorphize composite shells");

    let shell = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == "Shell<u32>" => Some(definition),
            _ => None,
        })
        .expect("closed Shell instance");
    let [
        DataMember::Field(shared),
        DataMember::Field(values),
        DataMember::Field(nested),
    ] = syntax.items.data_members(shell.members)
    else {
        panic!("Shell<u32> retains its three fields")
    };
    let TypeReferenceNode::Reference { referee, .. } =
        syntax.type_references.type_reference(shared.type_reference)
    else {
        panic!("shared remains a reference")
    };
    assert!(matches!(
        syntax.type_references.type_reference(*referee),
        TypeReferenceNode::Named(name) if name.as_str() == "u32"
    ));
    let TypeReferenceNode::Slice { element_type } =
        syntax.type_references.type_reference(values.type_reference)
    else {
        panic!("values remains a slice")
    };
    assert!(matches!(
        syntax.type_references.type_reference(*element_type),
        TypeReferenceNode::Named(name) if name.as_str() == "u32"
    ));
    let TypeReferenceNode::Reference { referee, .. } =
        syntax.type_references.type_reference(nested.type_reference)
    else {
        panic!("nested remains a reference")
    };
    let TypeReferenceNode::FixedArray { element_type, .. } =
        syntax.type_references.type_reference(*referee)
    else {
        panic!("nested reference retains its fixed-array referee")
    };
    assert!(matches!(
        syntax.type_references.type_reference(*element_type),
        TypeReferenceNode::Named(name) if name.as_str() == "u32"
    ));
}

#[test]
fn closed_generic_reference_instance_retains_erased_lifetime_application() {
    let source = r#"
        data Borrowed<'scope, T> { value: &'scope T; }
        data Generated<'scope> { value: Borrowed<'scope, u32>; }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");
    desugar_generic_data_instances(&mut syntax).expect("monomorphize lifetime-bearing reference");

    let borrowed = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == "Borrowed<u32>" => {
                Some(definition)
            }
            _ => None,
        })
        .expect("closed Borrowed instance");
    assert_eq!(
        borrowed
            .lifetime_parameters
            .iter()
            .map(|parameter| parameter.as_str())
            .collect::<Vec<_>>(),
        ["scope"]
    );
    let [DataMember::Field(value)] = syntax.items.data_members(borrowed.members) else {
        panic!("Borrowed<u32> retains its reference field")
    };
    let TypeReferenceNode::Reference {
        referee, lifetime, ..
    } = syntax.type_references.type_reference(value.type_reference)
    else {
        panic!("value remains a reference")
    };
    assert_eq!(lifetime.as_ref().map(|name| name.as_str()), Some("scope"));
    assert!(matches!(
        syntax.type_references.type_reference(*referee),
        TypeReferenceNode::Named(name) if name.as_str() == "u32"
    ));

    let generated = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == "Generated" => Some(definition),
            _ => None,
        })
        .expect("Generated wrapper");
    let [DataMember::Field(value)] = syntax.items.data_members(generated.members) else {
        panic!("Generated retains its one field")
    };
    let TypeReferenceNode::Generic {
        base_name,
        lifetime_arguments,
        arguments,
    } = syntax.type_references.type_reference(value.type_reference)
    else {
        panic!("wrapper retains an erased-lifetime application")
    };
    assert_eq!(base_name.as_str(), "Borrowed<u32>");
    assert_eq!(
        lifetime_arguments
            .iter()
            .map(|argument| argument.as_str())
            .collect::<Vec<_>>(),
        ["scope"]
    );
    assert!(arguments.is_empty());
}

#[test]
fn closed_generic_sum_preserves_payload_relevance_and_identities() {
    let source = r#"
        data Evidence { case Only; }
        data Maybe<T> {
            case #1 None;
            case #2 Some(#1 value: T, #2 proof [erased]: Evidence, retired #3);
            retired #4;
        }
        machine run() -> i32 {
            let maybe: Maybe<i32> = Maybe::Some { value: 7 };
            0
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");

    desugar_generic_data_instances(&mut syntax).expect("monomorphize pure sum");

    let definition = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == "Maybe<i32>" => Some(definition),
            _ => None,
        })
        .expect("closed Maybe definition");
    assert!(definition.type_parameters.is_empty());
    let members = syntax.items.data_members(definition.members);
    assert!(matches!(members[0], DataMember::Variant(ref variant) if variant.identity == Some(1)));
    let DataMember::Variant(some) = &members[1] else {
        panic!("Some variant");
    };
    assert_eq!(some.identity, Some(2));
    assert_eq!(some.retired_payload_identities, [3]);
    let payload = syntax.items.data_payload_fields(some.payload);
    assert_eq!(payload.len(), 2);
    assert_eq!(payload[0].identity, Some(1));
    assert_eq!(payload[1].identity, Some(2));
    assert!(payload[1].relevance.is_erased());
    assert!(matches!(
        syntax.type_references.type_reference(payload[0].type_reference),
        TypeReferenceNode::Named(name) if name.as_str() == "i32"
    ));
    assert!(matches!(members[2], DataMember::Retired(4)));

    let ExpressionNode::StructLiteral(literal) = local_initializer(&syntax, "maybe") else {
        panic!("Maybe::Some literal");
    };
    assert_eq!(literal.type_name.as_str(), "Maybe<i32>");
    assert_eq!(
        literal.case_name.as_ref().map(|name| name.as_str()),
        Some("Some")
    );
}

#[test]
fn closed_generic_erased_sum_elaborates_and_lays_out_material_payload_only() {
    let checked = checked(
        r#"
        data Evidence { case Only; case WithPayload(value: i32); }
        data Maybe<T> {
            case None;
            case Some(value: T, proof [erased]: Evidence);
            case ProvenOnly(proof [erased]: Evidence);
        }
        machine run() -> i32 {
            let maybe: Maybe<i32> = Maybe::Some { value: 7 };
            transition maybe {
                Maybe::Some { value, proof as _ } -> value
                Maybe::None -> 0
                Maybe::ProvenOnly { proof as _ } -> 1
            }
        }
        "#,
    )
    .expect("closed generic erased sum should check");

    let definition = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Maybe<i32>")
        .expect("closed Maybe definition");
    assert!(definition.type_parameters.is_empty());
    let some = checked
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            psi_checked_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == "Some" =>
            {
                Some(variant)
            }
            _ => None,
        })
        .expect("Some variant");
    assert_eq!(checked.data_payload_fields(some).len(), 2);

    let layout = build_layout_plan(&checked, NativeTarget::host(), &[]).expect("layout");
    let maybe = layout
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "Maybe<i32>")
        .expect("closed Maybe layout");
    let DataShape::Enum { variants, .. } = maybe.shape else {
        panic!("closed Maybe should have sum layout");
    };
    let variants = layout.variants.span_or_empty(variants);
    assert_eq!(variants.len(), 3);
    assert_eq!(layout.fields.span_or_empty(variants[1].fields).len(), 1);
    assert!(layout.fields.span_or_empty(variants[2].fields).is_empty());
}

#[test]
fn closed_generic_sum_payload_reaches_nested_record_fixpoint() {
    let checked = checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; }
        data Maybe<T> {
            case None;
            case Some(boxed: Box<T>, proof [erased]: Evidence);
        }
        machine run() -> i32 {
            let maybe: Maybe<i32> = Maybe::Some {
                boxed: Box { value: 7 },
            };
            transition maybe {
                Maybe::Some { boxed, proof as _ } -> boxed.value
                Maybe::None -> 0
            }
        }
        "#,
    )
    .expect("a nested closed payload should reach the synthesis fixpoint");

    for expected in ["Maybe<i32>", "Box<i32>"] {
        assert!(
            checked
                .data_definitions()
                .iter()
                .any(|definition| definition.name.as_str() == expected
                    && definition.type_parameters.is_empty()),
            "expected closed definition {expected}"
        );
    }
    let maybe = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Maybe<i32>")
        .expect("closed Maybe definition");
    let some = checked
        .data_members(maybe)
        .iter()
        .find_map(|member| match member {
            psi_checked_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == "Some" =>
            {
                Some(variant)
            }
            _ => None,
        })
        .expect("Some variant");
    let boxed = &checked.data_payload_fields(some)[0];
    assert!(matches!(
        checked
            .type_reference_table
            .type_reference(boxed.type_reference),
        psi_checked_trees::types::TypeReferenceNode::Named { name, .. }
            if name.as_str() == "Box<i32>"
    ));
}

#[test]
fn closed_generic_sum_requires_explicit_generic_evidence() {
    rejected(
        r#"
        data Evidence<U> { case Only; }
        data Maybe<T> { case None; case Some(value: T, proof [erased]: Evidence<i32>); }
        machine run() -> i32 {
            let maybe: Maybe<i32> = Maybe::Some { value: 7 };
            0
        }
        "#,
        "no unique accessible nullary constructor",
    );
}

#[test]
fn closed_generic_sum_accepts_explicit_generic_evidence() {
    checked(
        r#"
        data Evidence<U> { case Only; }
        data Maybe<T> { case None; case Some(value: T, proof [erased]: Evidence<i32>); }
        machine run() -> i32 {
            let maybe: Maybe<i32> = Maybe::Some {
                value: 7,
                proof: Evidence::Only,
            };
            transition maybe {
                Maybe::Some { value, proof as _ } -> value
                Maybe::None -> 0
            }
        }
        "#,
    )
    .expect("an explicit closed generic evidence term should remain valid");
}

#[test]
fn closed_generic_sum_erased_payload_cannot_drive_runtime_data() {
    rejected(
        r#"
        data Maybe<T> { case None; case Some(value: T, proof [erased]: i32); }
        machine run() -> i32 {
            let maybe: Maybe<i32> = Maybe::Some { value: 7, proof: 9 };
            transition maybe {
                Maybe::Some { value as _, proof } -> proof
                Maybe::None -> 0
            }
        }
        "#,
        "has no runtime value, address, read, write, or cleanup",
    );
}

#[test]
fn closed_generic_sum_retains_erased_linear_payload_obligation() {
    rejected(
        r#"
        data Receipt [linear] { case Issued; }
        data Maybe<T> { case None; case Some(value: T, proof [erased]: Receipt); }
        machine run() -> i32 {
            let maybe: Maybe<i32> = Maybe::Some { value: 7, proof: Receipt::Issued };
            0
        }
        "#,
        "linear value `maybe",
    );
}

#[test]
fn mixed_generic_sum_preserves_common_and_payload_relevance() {
    let checked = checked(
        r#"
        data Evidence { case Only; }
        data Mixed<T> {
            common: T;
            proof [erased]: Evidence;
            case None;
            case Some(value: T, case_proof [erased]: Evidence);
        }
        machine run() -> i32 {
            let mixed: Mixed<i32> = Mixed::Some { common: 1, value: 7 };
            transition mixed {
                Mixed::Some { value, case_proof as _ } -> value
                Mixed::None -> mixed.common
            }
        }
        "#,
    )
    .expect("closed mixed generic data should check");

    let definition = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Mixed<i32>")
        .expect("closed mixed definition");
    assert!(checked.data_members(definition).iter().any(|member| {
        matches!(member, psi_checked_trees::data::DataMember::Field(field)
            if field.name.as_str() == "proof" && field.relevance.is_erased())
    }));
    let layout = build_layout_plan(&checked, NativeTarget::host(), &[]).expect("mixed layout");
    let mixed = layout
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "Mixed<i32>")
        .expect("closed mixed layout");
    let DataShape::Enum {
        common_fields,
        variants,
    } = mixed.shape
    else {
        panic!("closed mixed data should have sum layout");
    };
    assert_eq!(layout.fields.span_or_empty(common_fields).len(), 1);
    let variants = layout.variants.span_or_empty(variants);
    assert!(layout.fields.span_or_empty(variants[0].fields).is_empty());
    assert_eq!(layout.fields.span_or_empty(variants[1].fields).len(), 1);
}

#[test]
fn closed_generic_sum_does_not_rewrite_non_case_names() {
    let source = r#"
        data Maybe<T> { case None; case Some(value: T); }
        machine run() -> i32 {
            let maybe: Maybe<i32> = Maybe::Some { value: 7 };
            Maybe::DEFAULT
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");
    desugar_generic_data_instances(&mut syntax).expect("monomorphize pure sum");

    assert!(
        syntax
            .expressions
            .iter_expressions()
            .any(|(_, expression)| {
                let ExpressionNode::Name(path) = expression else {
                    return false;
                };
                matches!(
                    syntax.expressions.identifier_path_members(*path),
                    [base, member]
                        if base.as_str() == "Maybe" && member.as_str() == "DEFAULT"
                )
            })
    );
}

#[test]
fn closed_generic_sum_rewrites_concrete_call_return_and_assignment_uses() {
    checked(
        r#"
        data Maybe<T> { case None; case Some(value: T); }
        data Holder { value: Maybe<i32>; }
        machine make() -> Maybe<i32> { Maybe::Some { value: 7 } }
        machine inspect(value: Maybe<i32>) -> i32 {
            transition value {
                Maybe::Some { value } -> value
                Maybe::None -> 0
            }
        }
        machine run() -> i32 {
            let holder: Holder = Holder { value: Maybe::None };
            holder.value = make();
            inspect(holder.value)
        }
        "#,
    )
    .expect("the unique closed sum identity should cover concrete executable contexts");
}

#[test]
fn closed_generic_sum_does_not_capture_generic_machine_template_paths() {
    let source = r#"
        data Maybe<T> { case None; case Some(value: T); }
        data Holder { value: Maybe<i32>; }
        machine empty<T>() -> Maybe<T> { Maybe::None }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");
    desugar_generic_data_instances(&mut syntax).expect("monomorphize pure sum");

    assert!(
        syntax
            .expressions
            .iter_expressions()
            .any(|(_, expression)| {
                let ExpressionNode::Name(path) = expression else {
                    return false;
                };
                matches!(
                    syntax.expressions.identifier_path_members(*path),
                    [base, case] if base.as_str() == "Maybe" && case.as_str() == "None"
                )
            })
    );
}

#[test]
fn distinct_closed_instances_of_one_generic_sum_select_exact_paths() {
    checked(
        r#"
        data Maybe<T> { case None; case Some(value: T); }
        machine inspect_integer(value: Maybe<i32>) -> i32 {
            transition value {
                Maybe::Some { value } -> value
                Maybe::None -> 0
            }
        }
        machine inspect_boolean(value: Maybe<bool>) -> bool {
            transition value {
                Maybe::Some { value } -> value
                Maybe::None -> false
            }
        }
        machine run() -> i32 {
            let integer: Maybe<i32> = Maybe::Some { value: 7 };
            let boolean: Maybe<bool> = Maybe::Some { value: true };
            let checked: bool = inspect_boolean(boolean);
            inspect_integer(integer)
        }
        "#,
    )
    .expect("each generic sum occurrence should select its exact closed identity");
}

#[test]
fn distinct_closed_generic_sums_use_unique_exact_call_parameter() {
    checked(
        r#"
        data Evidence { case Only; }
        data Maybe<T> { case None; case Some(value: T, proof [erased]: Evidence); }
        data Holder { boolean: Maybe<bool>; }
        machine take(value: Maybe<i32>) -> i32 { 0 }
        machine run() -> i32 { take(Maybe::Some { value: 7 }) }
        "#,
    )
    .expect("the unique call parameter should select Maybe<i32>");
}

#[test]
fn result_domain_overloads_share_exact_erased_generic_argument_contexts() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        data Maybe<T> { case None; case Some(value: T, proof [erased]: Evidence); }
        data Holder { boolean_box: Box<bool>; boolean_maybe: Maybe<bool>; }

        machine take(boxed: Box<i32>, maybe: Maybe<i32>) -> i32 { 1 }
        machine take(boxed: Box<i32>, maybe: Maybe<i32>) -> i32 in Saturating {
            2 as i32 in Saturating
        }

        machine run() -> i32 {
            let selected: i32 in Saturating = take(
                Box { value: 7 },
                Maybe::Some { value: 9 }
            );
            selected as i32
        }
        "#,
    )
    .expect("result-domain overloads should share their exact record/sum parameter context");
}

#[test]
fn overloaded_generic_sum_call_literal_remains_fail_closed() {
    rejected(
        r#"
        data Evidence { case Only; }
        data Maybe<T> { case None; case Some(value: T, proof [erased]: Evidence); }
        machine take(value: Maybe<i32>) -> i32 { 1 }
        machine take(value: Maybe<bool>) -> i32 { 2 }
        machine run() -> i32 { take(Maybe::Some { value: 7 }) }
        "#,
        "construction of erased generic data `Maybe` is unsupported in this context",
    );
}

#[test]
fn closed_generic_sum_preserves_generic_zero_home_lemma() {
    checked(
        r#"
        data Optional<T> { case None; case Some(value: T); }
        machine zero_is_none<T>()
        ensures
            zero_value<Optional<T>>() == Optional::None
        {
        }
        data Holder { value: Optional<u64>; }
        "#,
    )
    .expect("closing one runtime instance must not specialize the generic zero-home lemma");
}

#[test]
fn recursive_generic_sum_stays_in_structural_proof_form() {
    checked(
        r#"
        data Seq<T> {
            case Empty;
            case Cons(head: T, tail: Seq<T>);
        }
        machine append(s: Seq<u64>, t: Seq<u64>) -> Seq<u64>
        terminates by s;
        {
            transition s {
                Seq::Empty -> (t)
                Seq::Cons { head, tail } -> Seq::Cons {
                    head: head,
                    tail: append(tail, t)
                }
            }
        }
        machine append_empty_right(s: Seq<u64>) -> Seq<u64>
        terminates by s;
        ensures
            append(s, Seq::Empty) == s
        {
            transition s {
                Seq::Empty -> Seq::Empty
                Seq::Cons { head, tail } -> Seq::Cons {
                    head: head,
                    tail: append_empty_right(tail)
                }
            }
        }
        "#,
    )
    .expect("recursive generic proof data must retain its structural entailment form");
}

#[test]
fn bare_erased_generic_literal_uses_exact_return_context() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine make() -> Box<i32> {
            Box { value: 7 }
        }
        "#,
    )
    .expect("an exact return type should select the closed record identity");
}

#[test]
fn bare_erased_generic_literal_uses_exact_call_context() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine consume(boxed: Box<i32>) -> i32 { boxed.value }
        machine run() -> i32 {
            consume(Box { value: 7 })
        }
        "#,
    )
    .expect("a unique exact parameter should select the closed record identity");
}

#[test]
fn bare_erased_generic_literals_use_direct_self_call_context() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        data Maybe<T> { case None; case Some(value: T, proof [erased]: Evidence); }
        data Main { boolean_box: Box<bool>; boolean_maybe: Maybe<bool>; }

        machine Main::consume(
            &self,
            boxed: Box<i32>,
            maybe: Maybe<i32>
        ) -> i32 {
            transition maybe {
                Maybe::Some { value as _, proof as _ } -> (boxed.value)
                Maybe::None -> 0
            }
        }

        machine Main::run(&self) -> i32 {
            self.consume(Box { value: 7 }, Maybe::Some { value: 9 })
        }
        "#,
    )
    .expect("the enclosing attached data should select a direct self-call context");
}

#[test]
fn bare_erased_generic_literal_uses_direct_self_statement_call_context() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        data Main { stored: i32; boolean_box: Box<bool>; }

        machine Main::store(&mut self, boxed: Box<i32>) {
            self.stored = boxed.value;
        }

        machine Main::run(&mut self) -> i32 {
            self.store(Box { value: 7 });
            self.stored
        }
        "#,
    )
    .expect("the enclosing attached data should select a direct self statement-call context");
}

#[test]
fn bare_erased_generic_literals_use_exact_local_receiver_call_context() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        data Consumer { marker: i32; }
        machine Consumer::take(&self, boxed: Box<i32>) -> i32 { boxed.value }
        machine run(consumer: Consumer) -> i32 {
            consumer.take(Box { value: 7 })
        }
        "#,
    )
    .expect("an explicitly typed local receiver should select its exact attached owner");
}

#[test]
fn bare_erased_generic_literals_use_exact_self_field_receiver_call_context() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        data Consumer { marker: i32; }
        machine Consumer::take(&self, boxed: Box<i32>) -> i32 { boxed.value }
        data Main { consumer: Consumer; boolean_box: Box<bool>; }
        machine Main::run(&self) -> i32 {
            self.consumer.take(Box { value: 7 })
        }
        "#,
    )
    .expect("a direct self-field receiver should select its exact attached owner");
}

#[test]
fn bare_erased_generic_literals_use_exact_local_receiver_statement_call_context() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        data Consumer { stored: i32; }
        machine Consumer::store(&mut self, boxed: Box<i32>) {
            self.stored = boxed.value;
        }
        machine run(mut consumer: Consumer) -> i32 {
            consumer.store(Box { value: 7 });
            consumer.stored
        }
        "#,
    )
    .expect("an explicitly typed local receiver should contextualize a statement call");
}

#[test]
fn exact_local_receiver_overload_disagreement_remains_fail_closed() {
    rejected(
        r#"
        data Evidence { case Only; }
        data Maybe<T> { case None; case Some(value: T, proof [erased]: Evidence); }
        data Consumer { marker: i32; }
        machine Consumer::take(&self, value: Maybe<i32>) -> i32 { 1 }
        machine Consumer::take(&self, value: Maybe<bool>) -> i32 { 2 }
        machine run(consumer: Consumer) -> i32 {
            consumer.take(Maybe::Some { value: 7 })
        }
        "#,
        "construction of erased generic data `Maybe` is unsupported in this context",
    );
}

#[test]
fn computed_attached_receiver_context_remains_fail_closed() {
    rejected(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        data Consumer { marker: i32; }
        machine Consumer::take(&self, boxed: Box<i32>) -> i32 { boxed.value }
        machine choose(left: Consumer, right: Consumer, first: bool) -> Consumer {
            transition first { true -> (left) false -> (right) }
        }
        machine run(left: Consumer, right: Consumer) -> i32 {
            choose(left, right, true).take(Box { value: 7 })
        }
        "#,
        "construction of erased generic data `Box` is unsupported in this context",
    );
}

#[test]
fn closed_generic_erased_records_use_concrete_attached_machine_instances() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine Box::stored<T>(&self) -> T { self.value }

        data Main { integer: Box<i32>; boolean: Box<bool>; }
        machine Main::run(&self) -> i32 {
            let flag: bool = self.boolean.stored();
            self.integer.stored()
        }
        "#,
    )
    .expect("closed generic records should clone checked attached methods over erased layout");
}

#[test]
fn parameter_distinct_direct_self_overloads_remain_fail_closed() {
    rejected(
        r#"
        data Evidence { case Only; }
        data Maybe<T> { case None; case Some(value: T, proof [erased]: Evidence); }
        data Main { boolean: Maybe<bool>; }

        machine Main::take(&self, value: Maybe<i32>) -> i32 { 1 }
        machine Main::take(&self, value: Maybe<bool>) -> i32 { 2 }
        machine Main::run(&self) -> i32 {
            self.take(Maybe::Some { value: 7 })
        }
        "#,
        "construction of erased generic data `Maybe` is unsupported in this context",
    );
}

#[test]
fn unused_erased_generic_schema_is_accepted() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine run() -> i32 { 0 }
        "#,
    )
    .expect("an unused generic schema has no runtime erased representation");
}

#[test]
fn structured_const_field_order_has_one_canonical_instance_identity() {
    let source = r#"
        data UnitIndex { scale: u64; exponent: i32; }
        data UnitIndices {}
        const UnitIndices::A: UnitIndex = UnitIndex { scale: 1, exponent: -2 };
        const UnitIndices::B: UnitIndex = UnitIndex { exponent: -2, scale: 1 };

        data Indexed<const U: UnitIndex> { marker: u8; }
        data Holder {
            left: Indexed<UnitIndices::A>;
            right: Indexed<UnitIndices::B>;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");

    desugar_generic_data_instances(&mut syntax).expect("monomorphize");

    let holder = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(data) if data.name.as_str() == "Holder" => Some(data),
            _ => None,
        })
        .expect("Holder");
    let fields = syntax.items.data_members(holder.members);
    let [DataMember::Field(left), DataMember::Field(right)] = fields else {
        panic!("Holder fields");
    };
    let TypeReferenceNode::Named(left) = syntax.type_references.type_reference(left.type_reference)
    else {
        panic!("left canonical instance");
    };
    let TypeReferenceNode::Named(right) =
        syntax.type_references.type_reference(right.type_reference)
    else {
        panic!("right canonical instance");
    };
    assert_eq!(left, right);
    assert_eq!(
        syntax
            .root_items()
            .filter(|item| matches!(
                item,
                Item::Data(data) if data.name.as_str() == left.as_str()
            ))
            .count(),
        1
    );
}

#[test]
fn runtime_monomorphization_preserves_erased_lifetime_application() {
    let source = r#"
        data View<'buf, T> {
            body: &'buf i32;
            value: T;
        }

        data Holder<'call> {
            view: View<'call, i32>;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");

    desugar_generic_data_instances(&mut syntax).expect("monomorphize");

    let holder = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(data) if data.name.as_str() == "Holder" => Some(data),
            _ => None,
        })
        .expect("Holder");
    let DataMember::Field(view) = &syntax.items.data_members(holder.members)[0] else {
        panic!("Holder.view");
    };
    let TypeReferenceNode::Generic {
        base_name,
        lifetime_arguments,
        arguments,
    } = syntax.type_references.type_reference(view.type_reference)
    else {
        panic!("lifetime application should survive as an erased generic shell");
    };
    assert!(base_name.as_str().starts_with("View<"));
    assert_eq!(lifetime_arguments[0].as_str(), "call");
    assert!(arguments.is_empty());

    let instance = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(data) if data.name.as_str() == base_name.as_str() => Some(data),
            _ => None,
        })
        .expect("synthesized View instance");
    assert_eq!(instance.lifetime_parameters[0].as_str(), "buf");
    assert!(instance.type_parameters.is_empty());
}

#[test]
fn lifetime_bearing_local_instances_become_positional_type_arguments() {
    let source = r#"
        data Borrowed<'borrow, T> {
            value: &'borrow T;
        }

        data BorrowBox<'boxed, T> {
            value: T;
        }

        data Outer<'outer, T> {
            value: T;
        }

        data Generated<'call> {
            value: Outer<'call, BorrowBox<'call, Borrowed<'call, u32>>>;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");

    desugar_generic_data_instances(&mut syntax)
        .expect("normalize the positional lifetime-bearing Type argument graph");

    let find = |name: &str| {
        syntax
            .root_items()
            .find_map(|item| match item {
                Item::Data(definition) if definition.name.as_str() == name => Some(definition),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    let borrowed = find("Borrowed<u32>");
    let boxed = find("BorrowBox<Borrowed<u32>>");
    let outer = find("Outer<BorrowBox<Borrowed<u32>>>");
    assert_eq!(
        boxed
            .lifetime_parameters
            .iter()
            .map(|parameter| parameter.as_str())
            .collect::<Vec<_>>(),
        ["boxed"]
    );
    assert_eq!(
        outer
            .lifetime_parameters
            .iter()
            .map(|parameter| parameter.as_str())
            .collect::<Vec<_>>(),
        ["outer"]
    );

    let assert_field_application = |owner: &psi_syntax_trees::item::DataDefinition,
                                    expected_base: &str,
                                    expected_lifetime: &str| {
        let [DataMember::Field(value)] = syntax.items.data_members(owner.members) else {
            panic!("{} retains one field", owner.name.as_str())
        };
        let TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } = syntax.type_references.type_reference(value.type_reference)
        else {
            panic!(
                "{} field retains an erased application",
                owner.name.as_str()
            )
        };
        assert_eq!(base_name.as_str(), expected_base);
        assert_eq!(
            lifetime_arguments
                .iter()
                .map(|argument| argument.as_str())
                .collect::<Vec<_>>(),
            [expected_lifetime]
        );
        assert!(arguments.is_empty());
    };
    assert_field_application(boxed, borrowed.name.as_str(), "boxed");
    assert_field_application(outer, boxed.name.as_str(), "outer");

    let generated = find("Generated");
    assert_field_application(generated, outer.name.as_str(), "call");

    let TypeReferenceNode::Generic {
        arguments: boxed_origin_arguments,
        ..
    } = syntax
        .type_references
        .type_reference(boxed.generic_instance.expect("BorrowBox origin"))
    else {
        panic!("BorrowBox retains its generic origin")
    };
    let [boxed_argument] = syntax
        .type_references
        .type_reference_handles(*boxed_origin_arguments)
    else {
        panic!("BorrowBox origin retains one Type argument")
    };
    let TypeReferenceNode::Generic {
        base_name,
        lifetime_arguments,
        arguments,
    } = syntax.type_references.type_reference(*boxed_argument)
    else {
        panic!("BorrowBox origin retains its lifetime-bearing Type argument")
    };
    assert_eq!(base_name.as_str(), borrowed.name.as_str());
    assert_eq!(lifetime_arguments[0].as_str(), "boxed");
    assert!(arguments.is_empty());
}

#[test]
fn arithmetic_domain_type_arguments_have_distinct_canonical_instances() {
    let source = r#"
        data Cell<T> {
            value: T;
        }

        data Generated {
            wrapping: Cell<u32 in Wrapping>;
            saturating: Cell<u32 in Saturating>;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");

    desugar_generic_data_instances(&mut syntax)
        .expect("normalize exact arithmetic-domain Type arguments");

    let find = |name: &str| {
        syntax
            .root_items()
            .find_map(|item| match item {
                Item::Data(definition) if definition.name.as_str() == name => Some(definition),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    let wrapping = find("Cell<u32 in Wrapping>");
    let saturating = find("Cell<u32 in Saturating>");
    assert_ne!(wrapping.name, saturating.name);

    let assert_exact_domain =
        |type_reference, expected: psi_numerics::arithmetic::ArithmeticDomain| {
            let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = syntax.type_references.type_reference(type_reference)
            else {
                panic!("instance retains the constrained Type argument")
            };
            assert!(matches!(
                syntax.type_references.type_reference(*base_type),
                TypeReferenceNode::Named(name) if name.as_str() == "u32"
            ));
            assert_eq!(
                syntax.type_references.constraints(*constraints),
                [psi_syntax_trees::types::TypeConstraintNode::ArithmeticDomain(expected)]
            );
        };
    for (instance, domain) in [
        (
            wrapping,
            psi_numerics::arithmetic::ArithmeticDomain::Wrapping,
        ),
        (
            saturating,
            psi_numerics::arithmetic::ArithmeticDomain::Saturating,
        ),
    ] {
        let [DataMember::Field(value)] = syntax.items.data_members(instance.members) else {
            panic!("{} retains one field", instance.name.as_str())
        };
        assert_exact_domain(value.type_reference, domain);

        let TypeReferenceNode::Generic { arguments, .. } = syntax
            .type_references
            .type_reference(instance.generic_instance.expect("retained instance origin"))
        else {
            panic!("instance origin remains structural")
        };
        let [argument] = syntax.type_references.type_reference_handles(*arguments) else {
            panic!("instance origin retains one Type argument")
        };
        assert_exact_domain(*argument, domain);
    }
}

#[test]
fn unindexed_declared_domain_type_arguments_retain_the_exact_constraint() {
    let source = r#"
        data Token {}
        domain Token::Issued;

        data Cell<T> {
            value: T;
        }

        data Generated {
            issued: Cell<Token in Issued>;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");

    desugar_generic_data_instances(&mut syntax)
        .expect("normalize exact unindexed declared-domain Type argument");

    let instance = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == "Cell<Token in Issued>" => {
                Some(definition)
            }
            _ => None,
        })
        .expect("declared-domain Cell instance");
    let assert_exact_domain = |type_reference| {
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = syntax.type_references.type_reference(type_reference)
        else {
            panic!("instance retains the constrained Type argument")
        };
        assert!(matches!(
            syntax.type_references.type_reference(*base_type),
            TypeReferenceNode::Named(name) if name.as_str() == "Token"
        ));
        let [psi_syntax_trees::types::TypeConstraintNode::Domain(domain)] =
            syntax.type_references.constraints(*constraints)
        else {
            panic!("instance retains one declared-domain constraint")
        };
        assert_eq!(domain.name.as_str(), "Issued");
        assert!(domain.arguments.is_empty());
    };

    let [DataMember::Field(value)] = syntax.items.data_members(instance.members) else {
        panic!("declared-domain Cell instance retains one field")
    };
    assert_exact_domain(value.type_reference);

    let TypeReferenceNode::Generic { arguments, .. } = syntax
        .type_references
        .type_reference(instance.generic_instance.expect("retained instance origin"))
    else {
        panic!("instance origin remains structural")
    };
    let [argument] = syntax.type_references.type_reference_handles(*arguments) else {
        panic!("instance origin retains one Type argument")
    };
    assert_exact_domain(*argument);
}

#[test]
fn concrete_conformance_arguments_follow_generic_result_rewrites() {
    let source = r#"
        data Unit {}
        data Algebra<T> { value: T; }

        trait Projection<A> {
            machine project(subject: &Self) -> A;
        }

        data Subject {}

        machine Subject::project(subject: &Subject) -> Algebra<Unit>
        satisfies Projection<Algebra<Unit>>::project
        {
            Algebra { value: Unit {} }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");

    desugar_generic_data_instances(&mut syntax).expect("monomorphize");

    let machine = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Machine(machine) if machine.name.as_str().ends_with("::project") => Some(machine),
            _ => None,
        })
        .expect("project machine");
    let state = syntax.items.state(
        *syntax
            .items
            .state_handles(machine.states)
            .first()
            .expect("entry"),
    );
    let conformance = syntax
        .items
        .satisfies_clauses(machine.satisfies)
        .first()
        .expect("Projection conformance");
    let conformance_argument = *syntax
        .type_references
        .type_reference_handles(conformance.arguments)
        .first()
        .expect("concrete algebra argument");

    let TypeReferenceNode::Named(result) = syntax.type_references.type_reference(state.return_type)
    else {
        panic!("concrete generic result should become one synthesized named instance");
    };
    let TypeReferenceNode::Named(argument) =
        syntax.type_references.type_reference(conformance_argument)
    else {
        panic!("conformance argument should follow the result rewrite");
    };
    assert_eq!(result, argument);
}
