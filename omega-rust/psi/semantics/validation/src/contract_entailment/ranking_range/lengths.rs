//! Exact slice metadata coordinates, kept separate from scalar parameter values.

use super::*;
use symbols::SymbolHandle;
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceHandle;

pub(super) fn is_slice(program: &TypedTrees, mut reference: TypeReferenceHandle) -> bool {
    let mut visited = Vec::new();
    while reference.is_valid() && !visited.contains(&reference) {
        visited.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Slice { .. } => return true,
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            _ => return false,
        }
    }
    false
}

pub(super) fn parameter<'program>(
    program: &'program TypedTrees,
    state: &State,
    expression: ExpressionHandle,
) -> Option<&'program StateParameter> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    if !path.symbol.is_valid()
        || path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    program.state_parameters(state).iter().find(|parameter| {
        parameter.symbol == path.symbol
            && !parameter.is_self
            && !parameter.is_mutable
            && !parameter.is_const
            && is_slice(program, parameter.type_reference)
    })
}

pub(super) fn bindings(
    program: &TypedTrees,
    state: &State,
    entry_parameters: Option<&[SymbolHandle]>,
) -> Vec<(SymbolHandle, String)> {
    let mut bindings = Vec::new();
    for (position, parameter) in program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
    {
        if !parameter.symbol.is_valid()
            || parameter.is_mutable
            || parameter.is_const
            || !is_slice(program, parameter.type_reference)
        {
            continue;
        }
        let identity = format!("\0ranking:length:{:?}", parameter.symbol);
        bindings.push((parameter.symbol, identity.clone()));
        if let Some(entries) = entry_parameters {
            let entry = entries[position];
            if entry != parameter.symbol
                && entries
                    .iter()
                    .filter(|candidate| **candidate == entry)
                    .count()
                    == 1
            {
                bindings.push((entry, identity));
            }
        }
    }
    bindings
}

pub(super) fn install(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    root: &State,
    bindings: &[(SymbolHandle, String)],
    engine: &mut Engine<'_>,
    expressions: &[ExpressionHandle],
) -> Option<()> {
    if bindings.is_empty() {
        return Some(());
    }
    for expression in expressions {
        install_expression(
            program,
            machine,
            state,
            root,
            bindings,
            engine,
            *expression,
            0,
        )?;
    }
    Some(())
}

fn install_expression(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    root: &State,
    bindings: &[(SymbolHandle, String)],
    engine: &mut Engine<'_>,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<()> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    // These handles are metadata projections only. A slice Name stays outside
    // the scalar language, including in expressions that algebraically cancel.
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(_)
    ) {
        for scope in [state, root] {
            let Some(receiver) = crate::places::collection_length_receiver(
                program,
                machine,
                Some(scope),
                expression,
            ) else {
                continue;
            };
            let Some(parameter) = parameter(program, scope, receiver) else {
                continue;
            };
            let Some((_, identity)) = bindings
                .iter()
                .find(|(symbol, _)| *symbol == parameter.symbol)
            else {
                continue;
            };
            if !engine.bind_strict_projection(expression, Polynomial::atom(identity.clone())) {
                return None;
            }
        }
    }
    let mut visit = |child| {
        install_expression(
            program,
            machine,
            state,
            root,
            bindings,
            engine,
            child,
            depth + 1,
        )
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            visit(binary.left)?;
            visit(binary.right)?;
        }
        ExpressionNode::Unary(unary) => visit(unary.operand)?,
        ExpressionNode::Atomic(atomic) => visit(atomic.value)?,
        ExpressionNode::Member(member) => visit(member.receiver)?,
        ExpressionNode::Indexed(indexed) => {
            visit(indexed.collection)?;
            visit(indexed.index)?;
        }
        ExpressionNode::Range(range) => {
            for endpoint in [range.start, range.end] {
                if endpoint.is_valid() {
                    visit(endpoint)?;
                }
            }
        }
        _ => {}
    }
    Some(())
}

pub(super) fn actual(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    bindings: &[(SymbolHandle, String)],
    engine: &mut Engine<'_>,
) -> Option<Polynomial> {
    let coordinate = |expression| {
        let parameter = parameter(program, state, expression)?;
        let (_, identity) = bindings
            .iter()
            .find(|(symbol, _)| *symbol == parameter.symbol)?;
        Some(Polynomial::atom(identity.clone()))
    };
    if let Some(length) = coordinate(expression) {
        return Some(length);
    }
    if !crate::places::has_builtin_subslice_meaning(program, machine, Some(state), expression) {
        return None;
    }
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return None;
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        return None;
    };
    let source = coordinate(indexed.collection)?;
    let start = if range.start.is_valid() {
        engine.normalize(range.start)?
    } else {
        Polynomial::default()
    };
    let end = if range.end.is_valid() {
        engine.normalize(range.end)?
    } else {
        source.clone()
    };
    let prove = |difference: Polynomial| {
        engine.requires_unsatisfiable
            || engine.prove_at_least(&engine.substituted(&difference), &BigInt::zero())
    };
    (prove(start.clone()) && prove(end.sub(&start)) && prove(source.sub(&end)))
        .then(|| end.sub(&start))
}
