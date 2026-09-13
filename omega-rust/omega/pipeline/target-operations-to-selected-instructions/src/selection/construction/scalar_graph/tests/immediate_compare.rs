//! Direct scalar-graph selection of equality comparisons against unsigned U12 literals.
use super::*;
use legalized_operations::LegalizedScalarComparison as Comparison;

fn source_with_literal(
    target: target::NativeTarget,
    literal: u128,
    reused: bool,
) -> LegalizedScalarFunction {
    let mut source = super::control::graph(target, Comparison::Equal, false);
    let entry = source
        .blocks
        .iter_mut()
        .find(|block| block.id == source.entry_block)
        .unwrap();
    let mut comparison = entry.instructions.pop().unwrap();
    entry.instructions.truncate(2);
    let LegalizedScalarInstructionKind::Compare { left, .. } = &mut comparison.kind else {
        unreachable!();
    };
    *left = ValueId::new(2).unwrap();
    let constant_index = entry
        .instructions
        .iter()
        .position(|instruction| {
            instruction.result.map(|result| result.value) == Some(ValueId::new(1).unwrap())
        })
        .unwrap();
    let mut constant = entry.instructions.remove(constant_index);
    let comparison_index = entry.instructions.len();
    constant.result.as_mut().unwrap().definition_site = ValueDefinitionSite::Node {
        block: entry.id,
        node: comparison_index as u32,
    };
    comparison.result.as_mut().unwrap().definition_site = ValueDefinitionSite::Node {
        block: entry.id,
        node: comparison_index as u32 + 1,
    };
    entry.instructions.push(constant);
    entry.instructions.push(comparison);
    let literal_index = entry.instructions.len() - 2;
    entry.instructions[literal_index].kind =
        LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(literal));
    if reused {
        let block = source
            .blocks
            .iter_mut()
            .find(|block| block.id == BlockId::new(2).unwrap())
            .unwrap();
        let instruction = &mut block.instructions[0];
        instruction.kind = LegalizedScalarInstructionKind::BitwiseAnd {
            left: ValueId::new(1).unwrap(),
            right: ValueId::new(1).unwrap(),
        };
    }
    source.provenance.operations = source
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter().map(|row| row.operation))
        .collect();
    source
}

fn select(
    source: &LegalizedScalarFunction,
    target: target::NativeTarget,
) -> (
    SelectedFunction,
    SelectedSelectionConstraints,
    register_environment::ValidatedTargetRegisterEnvironment,
) {
    let environment = register_environment::baseline_target_register_environment(target).unwrap();
    let constraints = SelectedSelectionConstraints {
        keys: environment.selected_keys(),
        fixed_inputs: Vec::new(),
    };
    let selected = build(
        0,
        source,
        target,
        &constraints,
        environment.physical(),
        environment.constraints(),
    )
    .unwrap();
    (selected, constraints, environment)
}

fn validate(
    source: &LegalizedScalarFunction,
    selected: &SelectedFunction,
    target: target::NativeTarget,
    constraints: &SelectedSelectionConstraints,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) {
    crate::selection::validation::scalar_graph::validate(
        0,
        source,
        selected,
        target,
        constraints,
        environment.physical(),
        environment.constraints(),
    )
    .unwrap();
}

#[test]
fn u12_literal_compare_selects_immediate_and_validates() {
    let target = target::NativeTarget::linux_x64();
    let source = source_with_literal(target, 7, false);
    let (selected, constraints, environment) = select(&source, target);
    validate(&source, &selected, target, &constraints, &environment);

    let comparisons: Vec<_> = selected
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|instruction| {
            matches!(
                instruction.kind,
                SelectedInstructionKind::CompareI64
                    | SelectedInstructionKind::CompareI64Zero
                    | SelectedInstructionKind::CompareI64Immediate { .. }
            )
        })
        .collect();
    assert_eq!(comparisons.len(), 1);
    assert_eq!(
        comparisons[0].kind,
        SelectedInstructionKind::CompareI64Immediate {
            immediate: IntegerValue::Unsigned(7)
        }
    );
    assert_eq!(
        comparisons[0].constraint,
        constraints.keys.compare_i64_immediate
    );
    assert!(
        !selected
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(
                instruction.kind,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(7)
                }
            ))
    );
}

#[test]
fn out_of_range_or_reused_literal_keeps_register_compare() {
    let target = target::NativeTarget::linux_x64();
    for (literal, reused) in [(4096, false), (7, true)] {
        let source = source_with_literal(target, literal, reused);
        let (selected, constraints, environment) = select(&source, target);
        validate(&source, &selected, target, &constraints, &environment);
        assert!(
            selected
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| matches!(
                    instruction.kind,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(value)
                    } if value == literal
                ))
        );
        assert!(
            selected
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| instruction.kind == SelectedInstructionKind::CompareI64)
        );
        assert!(
            !selected
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| matches!(
                    instruction.kind,
                    SelectedInstructionKind::CompareI64Immediate { .. }
                ))
        );
    }
}

#[test]
fn immediate_compare_validation_rejects_immediate_and_kind_corruption() {
    let target = target::NativeTarget::linux_x64();
    let source = source_with_literal(target, 7, false);
    let (selected, constraints, environment) = select(&source, target);
    validate(&source, &selected, target, &constraints, &environment);

    let mut changed = selected.clone();
    let comparison = changed
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find(|instruction| {
            matches!(
                instruction.kind,
                SelectedInstructionKind::CompareI64Immediate { .. }
            )
        })
        .unwrap();
    comparison.kind = SelectedInstructionKind::CompareI64Immediate {
        immediate: IntegerValue::Unsigned(8),
    };
    assert!(
        crate::selection::validation::scalar_graph::validate(
            0,
            &source,
            &changed,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .is_err()
    );

    let mut changed = selected;
    let comparison = changed
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find(|instruction| {
            matches!(
                instruction.kind,
                SelectedInstructionKind::CompareI64Immediate { .. }
            )
        })
        .unwrap();
    comparison.kind = SelectedInstructionKind::CompareI64Zero;
    assert!(
        crate::selection::validation::scalar_graph::validate(
            0,
            &source,
            &changed,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .is_err()
    );
}
