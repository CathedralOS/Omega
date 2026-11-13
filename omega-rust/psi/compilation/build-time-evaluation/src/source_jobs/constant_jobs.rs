//! Evaluate retained constant computations through the accepted invocation
//! bridge, then materialize their snapshots in the original typed owner.
//! Dependency discovery reads identities only; it never interprets an
//! expression or chooses a result. Every job still passes typed preparation,
//! structural discharge, publication and hermetic invocation admission.

use std::collections::{HashMap, HashSet, VecDeque};

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::references::TypedReferences;
use typed_trees::TypedTrees;

use crate::{BuildTimeValue, InvocationTarget, SourceCompletionAccounting};

pub(crate) fn has_pending(program: &TypedTrees) -> bool {
    program.const_declarations().iter().any(|constant| {
        constant.evaluated_value.is_none()
            && program.tables.machines.is_valid(constant.evaluation_entry)
    })
}

/// Join the emitted value to its actual accepted invocation before another
/// source owner is frozen. Merely retaining an initializer machine cannot
/// justify a substituted literal: the requester, source, call and outcome
/// must still be the ones the interpreter used.
pub fn validate_constant_evaluations(
    program: &TypedTrees,
    accounting: &SourceCompletionAccounting,
) -> Result<(), String> {
    accounting.validate_charges()?;
    for constant in program.const_declarations() {
        if !program.tables.machines.is_valid(constant.evaluation_entry) {
            continue;
        }
        let entry = program.tables.machines.get(constant.evaluation_entry);
        let mut matching = accounting.receipts().iter().filter_map(|record| match record {
            crate::PureEvaluationRecord::Invocation(receipt)
                if receipt.requester() == constant.symbol => Some(receipt),
            _ => None,
        });
        let receipt = matching.next().ok_or("constant result has no accepted invocation receipt")?;
        if matching.next().is_some() || receipt.machine() != entry.symbol {
            return Err("constant result has ambiguous or swapped invocation custody".into());
        }
        let source = receipt.request_source();
        if program.identity.get() != source.identity.get() {
            return Err("constant receipt belongs to another source owner".into());
        }
        // Evaluation changes the constant records and materialized expression
        // values. All other source data must be the original request owner.
        // The expression dependency closure is compared separately below, so
        // later independent constant materializations need not be identical.
        let mut comparison = program.clone();
        comparison.expression_table = source.expression_table.clone();
        comparison.tables.const_declarations = source.tables.const_declarations.clone();
        if &comparison != source {
            return Err("constant receipt has stale declaration or execution metadata".into());
        }
        let original = source.const_declaration_by_symbol(constant.symbol)
            .ok_or("constant receipt lost its original requesting declaration")?;
        let mut retained = original.clone();
        retained.materialized_initializer = constant.materialized_initializer;
        retained.evaluated_value = constant.evaluated_value.clone();
        if &retained != constant {
            return Err("constant receipt belongs to another initializer request".into());
        }
        if !program.expression_table.expression_is_valid(constant.authored_initializer)
            || !program.expression_table.expression_is_valid(constant.materialized_initializer)
        {
            return Err("constant lost its authored or materialized expression".into());
        }
        let references = request_references(program, constant.authored_initializer);
        for expression in references.expressions {
            if !source.expression_table.expression_is_valid(expression)
                || source.expression_table.expression(expression)
                    != program.expression_table.expression(expression)
                || source.expression_table.source_span(expression)
                    != program.expression_table.source_span(expression)
            {
                return Err("constant receipt has a stale authored computation".into());
            }
        }
        let crate::PureTerminalEvaluationOutcome::Materialized { value, .. } = receipt.outcome() else {
            return Err("constant receipt did not return a materialized value".into());
        };
        let value = decoded(program, constant.declared_type, value)?;
        let expected = language_semantics::const_value::CanonicalConstIdentity::from_declaration_value(&value)
            .ok_or("constant receipt result has no canonical encoding")?;
        if constant.evaluated_value.as_ref().and_then(|value| value.encoding())
            != Some(expected.encoding.as_str())
        {
            return Err("constant value differs from its accepted invocation outcome".into());
        }
        let current = program.expression_table.expression(constant.materialized_initializer);
        match (&value, current) {
            (language_semantics::const_value::DecodedCanonicalConstValue::Boolean(value), ExpressionNode::Boolean(current))
                if value == current => {},
            (language_semantics::const_value::DecodedCanonicalConstValue::Integer { value, .. }, ExpressionNode::Integer(current))
                if current.value_i64().map(i128::from).or_else(|| current.value_u64().map(i128::from)) == Some(*value) => {},
            _ => return Err("constant materialization differs from its invocation outcome".into()),
        }
    }
    Ok(())
}

#[derive(Default)]
struct References {
    symbols: HashSet<SymbolHandle>,
    calls: Vec<SymbolHandle>,
    expressions: HashSet<ExpressionHandle>,
}
impl Extend<SymbolHandle> for References {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
        self.symbols.extend(symbols);
    }
}
impl TypedReferences for References {
    fn derived_declaration_references(&self) -> bool { false }
    fn callable_use(&mut self, symbol: SymbolHandle) {
        self.calls.push(symbol);
    }
    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        self.expressions.insert(expression)
    }
}

fn dependencies(program: &TypedTrees, entry: SymbolHandle) -> HashSet<SymbolHandle> {
    let mut references = References::default();
    let mut pending = vec![entry];
    let mut visited = HashSet::new();
    while let Some(callee) = pending.pop() {
        let Some(machine) = program.machine_by_symbol(callee)
            .or_else(|| program.machine_holding_state(callee)) else { continue };
        if !visited.insert(machine.symbol) {
            continue;
        }
        typed_trees::references::visit_machine(program, &mut references, machine);
        pending.append(&mut references.calls);
    }
    references.symbols
}

fn request_references(program: &TypedTrees, expression: ExpressionHandle) -> References {
    let mut references = References::default();
    typed_trees::references::visit_expression(program, &mut references, expression);
    let mut visited = HashSet::new();
    while let Some(callee) = references.calls.pop() {
        let Some(machine) = program.machine_by_symbol(callee)
            .or_else(|| program.machine_holding_state(callee)) else { continue };
        if visited.insert(machine.symbol) {
            typed_trees::references::visit_machine(program, &mut references, machine);
        }
    }
    references
}

/// A returned scalar keeps its declared carrier, including the u64 bit pattern
/// transported by BuildTimeValue::Int. Reference and authority-bearing values
/// are not constant snapshots; further carriers need their actual materializer.
fn decoded(
    program: &TypedTrees,
    carrier: typed_trees::types::TypeReferenceHandle,
    value: &BuildTimeValue,
) -> Result<language_semantics::const_value::DecodedCanonicalConstValue, String> {
    use language_semantics::const_value::DecodedCanonicalConstValue;
    let carrier = program.unwrapped_type_reference(carrier)
        .ok_or("constant result has no exact carrier")?;
    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(carrier) else {
            return Err("no product route yet: constant result needs its pure snapshot materializer".into());
        };
    let atom = program.symbols.builtin_type_atom(*symbol)
        .ok_or("no product route yet: constant result needs its pure snapshot materializer")?;
    let primitive = match atom {
        symbols::BuiltinTypeAtom::Bool => typed_trees::types::PrimitiveType::Bool,
        atom => typed_trees::types::PrimitiveType::from_builtin_atom(atom)
            .ok_or("constant result has no builtin carrier")?,
    };
    match (primitive, value) {
        (typed_trees::types::PrimitiveType::Bool, BuildTimeValue::Bool(value)) =>
            Ok(DecodedCanonicalConstValue::Boolean(*value)),
        (primitive, BuildTimeValue::Int(value)) if primitive.accepts_integer_literal() =>
            Ok(DecodedCanonicalConstValue::Integer {
                type_name: primitive.name().to_owned(),
                value: if primitive.is_signed_integer() { i128::from(*value) }
                    else { i128::from(*value as u64) },
            }),
        _ => Err("no product route yet: constant result needs its pure snapshot materializer".into()),
    }
}

fn materialize_uses(program: &mut TypedTrees) -> Result<(), String> {
    // Authored initializers remain source correspondence. Visit ordinary
    // declarations and the emitted initializer machines, not those originals.
    struct Uses(References);
    impl Extend<SymbolHandle> for Uses {
        fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
    }
    impl TypedReferences for Uses {
        fn derived_declaration_references(&self) -> bool { false }
        fn declaration(&mut self, symbol: SymbolHandle) -> bool {
            self.0.symbols.contains(&symbol)
        }
        fn expression(&mut self, expression: ExpressionHandle) -> bool {
            self.0.expressions.insert(expression)
        }
    }
    let mut uses = Uses(References {
        symbols: program.machines().iter().map(|machine| machine.symbol).collect(),
        ..Default::default()
    });
    typed_trees::references::visit_program(program, &mut uses);
    let named = uses.0.expressions.into_iter().filter_map(|expression| {
        let path = program.expression_table.name_path_node(expression)?;
        let symbol = path.resolved_symbol()?;
        let declaration = program.const_declaration_by_symbol(symbol)?;
        let value = typed_trees::canonical_const_declaration_value(program, symbol)?
            .decode_encoding()?;
        Some((expression, declaration.declared_type, value))
    }).collect::<Vec<_>>();
    for (expression, carrier, value) in named {
        let node = program.materialize_canonical_const_value(carrier, &value)
            .ok_or("constant value does not materialize in its declared carrier")?;
        *program.expression_table.expression_mut(expression) = node;
    }
    Ok(())
}

pub(crate) fn evaluate(
    program: &mut TypedTrees,
    authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<SourceCompletionAccounting, Vec<Diagnostic>> {
    let jobs = program.const_declarations().iter().filter(|constant| {
        constant.evaluated_value.is_none()
            && program.tables.machines.is_valid(constant.evaluation_entry)
    }).map(|constant| (constant.symbol, program.tables.machines.get(constant.evaluation_entry).symbol))
        .collect::<Vec<_>>();
    let mut accounting = SourceCompletionAccounting::default();
    if jobs.is_empty() {
        return Ok(accounting);
    }
    let positions = jobs.iter().enumerate().map(|(position, (symbol, _))| (*symbol, position))
        .collect::<HashMap<_, _>>();
    let mut successors = vec![Vec::new(); jobs.len()];
    let mut remaining = vec![0usize; jobs.len()];
    for (position, (_, entry)) in jobs.iter().enumerate() {
        for dependency in dependencies(program, *entry) {
            if let Some(&dependency) = positions.get(&dependency) {
                remaining[position] += 1;
                successors[dependency].push(position);
            }
        }
    }
    let mut ready = remaining.iter().enumerate().filter_map(|(position, count)|
        (*count == 0).then_some(position)).collect::<VecDeque<_>>();
    let mut completed = 0usize;
    while let Some(position) = ready.pop_front() {
        let (symbol, entry) = jobs[position];
        let result = (|| -> Result<(), String> {
            materialize_uses(program)?;
            let prepared = super::PreparedBuildMachineProgram::prepare(program, authority.clone())
                .map_err(|errors| diagnostics::join_messages(&errors))?;
            let value = prepared.entry(symbol, entry)?.invoke_recorded(
                InvocationTarget { name: None, layout_target: None, carrier_layout: None, address_model: None },
                &[], &mut accounting,
            )?.into_value();
            let handle = program.roots.const_declarations.handles()
                .find(|handle| program.tables.const_declarations.get(*handle).symbol == symbol)
                .ok_or("constant evaluation lost its requesting declaration")?;
            let carrier = program.tables.const_declarations.get(handle).declared_type;
            let value = decoded(program, carrier, &value)?;
            let encoding = language_semantics::const_value::CanonicalConstIdentity::from_declaration_value(&value)
                .ok_or("constant result has no canonical declaration encoding")?.encoding;
            let node = program.materialize_canonical_const_value(carrier, &value)
                .ok_or("constant result does not materialize in its declared carrier")?;
            let expression = program.expression_table.insert(node);
            let span = program.tables.const_declarations.get(handle).initializer_source_span;
            program.expression_table.set_source_span(expression, span);
            let declaration = program.tables.const_declarations.get_mut(handle);
            declaration.materialized_initializer = expression;
            declaration.evaluated_value = Some(encoding.into());
            Ok(())
        })();
        if let Err(reason) = result {
            let mut error = Diagnostic::error(format!("constant `{}` evaluation: {reason}", program.symbols.name(symbol)));
            if let Some(span) = program.symbols.symbol_source_span(symbol) { error = error.with_source_span(span); }
            return Err(vec![error]);
        }
        completed += 1;
        for &successor in &successors[position] {
            remaining[successor] -= 1;
            if remaining[successor] == 0 { ready.push_back(successor); }
        }
    }
    if completed != jobs.len() {
        return Err(vec![Diagnostic::error("constant evaluation dependency cycle")]);
    }
    materialize_uses(program).map_err(|reason| vec![Diagnostic::error(reason)])?;
    Ok(accounting)
}
