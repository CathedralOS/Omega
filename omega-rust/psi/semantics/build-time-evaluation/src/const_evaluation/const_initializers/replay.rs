//! Re-evaluate retained declaration roots against the receiving typed program.
//!
//! The materialized value and its receipt cannot validate one another. Pair
//! authored and materialized leaves by exact declared storage shape, then check
//! both against the declaration's canonical value. Disposable probe machines
//! give detached expressions ordinary checking context; they remain in a private
//! clone and never add a runtime declaration or a portable representation.

use std::sync::Arc;

use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstIdentity, DecodedCanonicalConstValue};
use typed_trees::{
    TypedTrees,
    data::DataMember,
    expression::{ExpressionHandle, ExpressionNode},
    types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode},
};

struct Leaf {
    owner: symbols::SymbolHandle,
    original: ExpressionHandle,
    materialized: ExpressionHandle,
    destination: TypeReferenceHandle,
    /// Structured leaves replay as whole checked-interpreter probes on both
    /// roots and compare independently encoded results; scalar leaves use the
    /// exact probe evaluator.
    structured: bool,
    expected: DecodedCanonicalConstValue,
}

pub(crate) fn validate(
    typed: &TypedTrees,
    authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<Diagnostic>> {
    let mut leaves = Vec::new();
    let mut replay_declarations = Vec::new();
    for declaration in typed.const_declarations() {
        let reference = declaration.initializer_source_span;
        // Every declaration retains its value root, including literal-only
        // declarations. Missing storage is invalid before deciding whether its
        // attached call roster requires replay; erasure cannot remove that duty.
        if !typed
            .expression_table
            .expression_is_valid(declaration.materialized_initializer)
            || typed
                .expression_table
                .source_span(declaration.materialized_initializer)
                != reference
        {
            return Err(super::failure(
                reference,
                "constant declaration lost its exact materialized initializer root",
            ));
        }
        let mut retained_call = false;
        for occurrence in typed
            .expression_table
            .authored_selection_occurrences(declaration.materialized_initializer)
        {
            let selection = typed
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or_else(|| {
                    super::failure(
                        reference,
                        "constant initializer lost its attached selection occurrence",
                    )
                })?;
            retained_call |= selection.kind() == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call;
        }
        let authored_call = if declaration.authored_initializer.is_valid() {
            crate::const_evaluation::const_generic_expressions::initializer_contains_call(
                typed,
                declaration.authored_initializer,
            )
            .map_err(|reason| super::failure(reference, reason))?
        } else {
            false
        };
        let destination = float_destination(typed, declaration.declared_type);
        let encoded_float = declaration
            .canonical_value_encoding
            .as_deref()
            .is_some_and(|encoding| encoding.starts_with("float:"));
        if destination.is_some() || encoded_float {
            let destination = destination.ok_or_else(|| {
                super::failure(
                    reference,
                    "floating constant result encoding lost its exact declared format",
                )
            })?;
            validate_anonymous_float(
                typed,
                declaration,
                destination,
                authority.clone(),
                retained_call || authored_call,
            )
            .map_err(|reason| super::failure(reference, reason))?;
            continue;
        }
        if !retained_call && !authored_call {
            continue;
        }
        if !typed
            .expression_table
            .expression_is_valid(declaration.authored_initializer)
            || declaration.authored_initializer == declaration.materialized_initializer
            || typed
                .expression_table
                .source_span(declaration.authored_initializer)
                != reference
            || typed
                .expression_table
                .source_span(declaration.materialized_initializer)
                != reference
        {
            return Err(super::failure(
                reference,
                "retained call declaration lost its distinct source-owned initializer roots",
            ));
        }
        replay_declarations.push(declaration);
        let identity = CanonicalConstIdentity {
            type_name: String::new(),
            encoding: declaration
                .canonical_value_encoding
                .clone()
                .ok_or_else(|| {
                    super::failure(
                        declaration.initializer_source_span,
                        "retained invocation lost its canonical result",
                    )
                })?,
        };
        let expected = identity.decode_encoding().ok_or_else(|| {
            super::failure(
                declaration.initializer_source_span,
                "retained invocation has invalid canonical result",
            )
        })?;
        pair(
            typed,
            Leaf {
                owner: declaration.symbol,
                original: declaration.authored_initializer,
                materialized: declaration.materialized_initializer,
                destination: declaration.declared_type,
                structured: false,
                expected,
            },
            &mut Vec::new(),
            &mut leaves,
        )
        .map_err(|reason| super::failure(declaration.initializer_source_span, reason))?;
    }
    if leaves.is_empty() {
        return Ok(());
    }
    let mut probe = typed.clone();
    let mut owners = Vec::new();
    for (ordinal, leaf) in leaves.iter().enumerate() {
        let original = super::invocations::append_probe(
            &mut probe,
            leaf.owner,
            format!("@const-replay-{ordinal}"),
            leaf.original,
            leaf.destination,
        );
        // Structured leaves interpret each root as a whole value: the scalar
        // probe only reaches leaves whose declared carrier is scalar.
        let materialized = leaf.structured.then(|| {
            super::invocations::append_probe(
                &mut probe,
                leaf.owner,
                format!("@const-replay-materialized-{ordinal}"),
                leaf.materialized,
                leaf.destination,
            )
        });
        owners.push((original, materialized));
    }
    let probe_symbols = owners
        .iter()
        .flat_map(|(original, materialized)| {
            materialized.map_or_else(
                || vec![*original],
                |materialized| vec![*original, materialized],
            )
        })
        .collect::<Vec<_>>();
    // The actual source recipe must precede specialization. Prepared target
    // custody alone cannot distinguish another valid instance of one template.
    for declaration in &replay_declarations {
        let ordinal = leaves
            .iter()
            .position(|leaf| leaf.owner == declaration.symbol)
            .ok_or_else(|| {
                super::failure(
                    declaration.initializer_source_span,
                    "constant lost its authored replay owner",
                )
            })?;
        validate_authored_custody(&probe, owners[ordinal].0, declaration)
            .map_err(|reason| super::failure(declaration.initializer_source_span, reason))?;
    }
    let checked =
        super::invocations::CheckedInitializers::prepare(&probe, authority, &probe_symbols)?;
    let program = checked.typed();
    for declaration in replay_declarations {
        let index = leaves
            .iter()
            .position(|leaf| leaf.owner == declaration.symbol)
            .ok_or_else(|| {
                super::failure(
                    declaration.initializer_source_span,
                    "retained call declaration has no replayable leaf",
                )
            })?;
        checked
            .calls_for_symbol(owners[index].0)
            .and_then(|calls| {
                let dependencies = calls.validate_custody(
                    declaration.authored_initializer,
                    declaration.materialized_initializer,
                )?;
                validate_dependency_values(program, &dependencies, |expression, destination| {
                    calls
                        .evaluate_scalar(expression, destination)
                        .map(|(value, _)| value)
                })?;
                Ok(())
            })
            .map_err(|reason| super::failure(declaration.initializer_source_span, reason))?;
    }
    for (leaf, (original_owner, materialized_owner)) in leaves.iter().zip(owners) {
        let result = (|| {
            let calls = checked.calls_for_symbol(original_owner)?;
            if let Some(destination) =
                crate::const_evaluation::const_generic_expressions::exact_probe_destination(
                    program,
                    leaf.destination,
                )
            {
                let original = calls.evaluate(leaf.original, destination)?.0;
                let materialized = calls.evaluate(leaf.materialized, destination)?.0;
                if original.decode_encoding().as_ref() != Some(&leaf.expected)
                    || materialized.decode_encoding().as_ref() != Some(&leaf.expected)
                {
                    return Err(
                        "retained constant invocation, materialized value, or canonical result drifted"
                            .to_owned(),
                    );
                }
                return Ok(());
            }
            // Structured leaf: both roots interpret as whole values against
            // the declared carrier and re-encode independently.
            let (original, _) = calls.evaluate_value()?;
            let materialized_calls = checked.calls_for_symbol(
                materialized_owner
                    .ok_or("structured initializer leaf lost its materialized probe")?,
            )?;
            let (materialized, _) = materialized_calls.evaluate_value()?;
            let original =
                super::materialize::canonical_value(program, leaf.destination, &original)?;
            let materialized =
                super::materialize::canonical_value(program, leaf.destination, &materialized)?;
            if original != leaf.expected || materialized != leaf.expected {
                return Err(
                    "retained constant invocation, materialized value, or canonical result drifted"
                        .to_owned(),
                );
            }
            Ok(())
        })();
        result.map_err(|reason| {
            super::failure(program.expression_table.source_span(leaf.original), reason)
        })?;
    }
    Ok(())
}

fn validate_authored_custody(
    program: &TypedTrees,
    owner: symbols::SymbolHandle,
    declaration: &typed_trees::constant::ConstDeclaration,
) -> Result<(), String> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == owner)
        .ok_or("constant lost its authored replay machine")?;
    let [state] = program.machine_states(machine) else {
        return Err("constant lost its authored replay state".into());
    };
    crate::const_evaluation::const_generic_expressions::validate_authored_initializer_call_custody(
        program,
        machine,
        state,
        declaration.authored_initializer,
        declaration.materialized_initializer,
    )
    .map(|_| ())
}

fn float_destination(
    program: &TypedTrees,
    destination: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(destination)
    else {
        return None;
    };
    match program.symbols.builtin_type_atom(*symbol)? {
        symbols::BuiltinTypeAtom::F32 => Some(PrimitiveType::F32),
        symbols::BuiltinTypeAtom::F64 => Some(PrimitiveType::F64),
        _ => None,
    }
}

/// Float declaration values are materializable bits, not generic-index atoms.
/// Recompute the authored value before comparing either retained result claim.
fn validate_anonymous_float(
    typed: &TypedTrees,
    declaration: &typed_trees::constant::ConstDeclaration,
    destination: PrimitiveType,
    authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
    has_calls: bool,
) -> Result<(), String> {
    use crate::const_evaluation::const_generic_expressions::value::{ScalarValue, evaluate_scalar};
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic as Intrinsic,
        AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionTarget as Target,
    };
    use numerics::literals::FloatFormat;

    let original = declaration.authored_initializer;
    let materialized = declaration.materialized_initializer;
    if !typed.expression_table.expression_is_valid(original)
        || typed.expression_table.source_span(original) != declaration.initializer_source_span
    {
        return Err("floating constant lost its distinct source-owned initializer roots".into());
    }
    if matches!(
        typed.expression_table.expression(original),
        ExpressionNode::Float(_) | ExpressionNode::Integer(_)
    ) {
        if typed
            .expression_table
            .authored_selection_occurrences(original)
            .next()
            .is_some()
            || typed
                .expression_table
                .authored_selection_occurrences(materialized)
                .next()
                .is_some()
        {
            return Err(
                "floating constant literal root retains computation or declaration custody".into(),
            );
        }
        let format = match destination {
            PrimitiveType::F32 => FloatFormat::F32,
            PrimitiveType::F64 => FloatFormat::F64,
            _ => return Err("floating constant lost its exact declared format".into()),
        };
        let bits = float_literal_bits(typed, original, format)?;
        if float_literal_bits(typed, materialized, format)? != bits
            || declaration.canonical_value_encoding.as_deref()
                != Some(ScalarValue::Float { format, bits }.encoding().as_str())
        {
            return Err("floating constant literal bits or result encoding drifted".into());
        }
        return Ok(());
    }
    if original == materialized {
        return Err("floating constant computation lost its distinct materialized root".into());
    }
    let mut probe = typed.clone();
    let owner = super::invocations::append_probe(
        &mut probe,
        declaration.symbol,
        "@const-float-replay".to_owned(),
        original,
        declaration.declared_type,
    );
    if has_calls {
        validate_authored_custody(&probe, owner, declaration)?;
    }
    let checked = has_calls
        .then(|| {
            super::invocations::CheckedInitializers::prepare(&probe, authority.clone(), &[owner])
        })
        .transpose()
        .map_err(|diagnostics| {
            format!("floating initializer call preparation failed: {diagnostics:?}")
        })?;
    let probe = checked
        .as_ref()
        .map_or(&probe, super::invocations::CheckedInitializers::typed);
    let calls = checked
        .as_ref()
        .map(|checked| checked.calls_for_symbol(owner))
        .transpose()?;
    let scalar_calls = calls.as_ref().map(|calls| {
        calls as &dyn crate::const_evaluation::const_generic_expressions::value::ConstantCalls
    });
    let machine = probe
        .machines()
        .iter()
        .find(|machine| machine.symbol == owner)
        .ok_or("floating constant lost its receiving probe")?;
    let [state] = probe.machine_states(machine) else {
        return Err("floating constant lost its receiving expression state".into());
    };
    crate::machine_execution::admission::require_const_expression_selection(
        probe,
        machine,
        declaration.initializer_source_span,
        authority.as_deref(),
    )?;
    let dependencies = match &calls {
        Some(calls) => calls.validate_custody(original, materialized)?,
        None => crate::const_evaluation::const_generic_expressions::validate_retained_initializer_call_custody(
            probe, machine, state, original, materialized,
        )?,
    };
    let dependency_roots =
        validate_dependency_values(probe, &dependencies, |expression, destination| {
            evaluate_scalar(probe, machine, state, expression, destination, scalar_calls)
                .map(|(value, _)| value)
        })?;
    let (evaluated, _) =
        evaluate_scalar(probe, machine, state, original, destination, scalar_calls)?;
    let ScalarValue::Float { format, bits } = &evaluated else {
        return Err("floating constant replay did not produce selected format bits".into());
    };

    let mut operators = Vec::new();
    let mut visited = Vec::new();
    let mut pending = vec![original];
    pending.extend(dependency_roots);
    while let Some(expression) = pending.pop() {
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        if let ExpressionNode::Binary(binary) = probe.expression_table.expression(expression) {
            if !validation::has_builtin_binary_expression_meaning(
                probe,
                machine,
                Some(state),
                expression,
            ) {
                return Err("floating constant replay lost selected builtin meaning".into());
            }
            let reference = probe.expression_table.source_span(expression);
            if !operators.contains(&reference) {
                operators.push(reference);
            }
            pending.push(binary.right);
            pending.push(binary.left);
        } else if let ExpressionNode::Match(dispatch) =
            probe.expression_table.expression(expression)
        {
            pending.push(dispatch.subject);
            for arm in probe.expression_table.match_arms(dispatch.arms) {
                pending.push(arm.value);
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    pending.push(pattern);
                }
            }
        } else if let ExpressionNode::Call(call) = probe.expression_table.expression(expression) {
            if call.receiver.is_valid() {
                pending.push(call.receiver);
            }
            pending.extend(
                probe
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied(),
            );
        }
    }
    let mut retained_operators = Vec::new();
    for occurrence in probe
        .expression_table
        .authored_selection_occurrences(materialized)
    {
        let selection = probe
            .authored_declaration_selections()
            .get(occurrence)
            .ok_or("floating constant lost its retained operator selection")?;
        if selection.kind() == Kind::Call {
            // The shared custody check already reconstructed each exact call
            // target from the authored invocation closure.
            continue;
        }
        if selection.kind() == Kind::StaticPathSegment
            && let Target::Resolved(selected) = selection.target()
            && dependencies
                .iter()
                .any(|dependency| dependency.declaration == selected.selected_symbol())
        {
            continue;
        }
        if selection.kind() != Kind::Operator
            || selection.target() != Target::Intrinsic(Intrinsic::BuiltinOperator)
        {
            return Err("anonymous floating constant retained a non-builtin selection".into());
        }
        if !retained_operators.contains(&selection.source_span()) {
            retained_operators.push(selection.source_span());
        }
    }
    if operators.len() != retained_operators.len()
        || operators
            .iter()
            .any(|reference| !retained_operators.contains(reference))
    {
        return Err("floating constant authored and retained operator custody drifted".into());
    }
    let ExpressionNode::Float(literal) = probe.expression_table.expression(materialized) else {
        return Err("floating constant materialization is not a floating literal".into());
    };
    if literal.landing().is_some_and(|landing| landing != *format) {
        return Err("floating constant materialized format drifted".into());
    }
    let materialized_bits = match format {
        FloatFormat::F32 => u64::from(literal.value_f32().to_bits()),
        FloatFormat::F64 => literal.value_f64().to_bits(),
    };
    if materialized_bits != *bits
        || declaration.canonical_value_encoding.as_deref() != Some(evaluated.encoding().as_str())
    {
        return Err(
            "floating constant computation, materialized bits, or result encoding drifted".into(),
        );
    }
    Ok(())
}

fn float_literal_bits(
    program: &TypedTrees,
    expression: ExpressionHandle,
    format: numerics::literals::FloatFormat,
) -> Result<u64, String> {
    use numerics::literals::{FloatFormat, FloatLiteral};
    let literal = match program.expression_table.expression(expression) {
        ExpressionNode::Float(literal)
            if literal.landing().is_none_or(|landing| landing == format) =>
        {
            literal.clone()
        }
        ExpressionNode::Integer(literal) if literal.landing().is_none() => {
            let value = literal
                .value_bignum()
                .ok_or("floating constant has invalid anonymous integer value")?;
            FloatLiteral::parse(&value.to_string())
                .ok_or("floating constant integer value cannot select format bits")?
        }
        _ => {
            return Err(
                "floating constant literal root lost its carrier or retained computation".into(),
            );
        }
    };
    Ok(match format {
        FloatFormat::F32 => {
            let value = literal.value_f32();
            if value.is_nan() {
                return Err(
                    "floating declaration identity requires explicit NaN representation bits"
                        .into(),
                );
            }
            u64::from(value.to_bits())
        }
        FloatFormat::F64 => {
            let value = literal.value_f64();
            if value.is_nan() {
                return Err(
                    "floating declaration identity requires explicit NaN representation bits"
                        .into(),
                );
            }
            value.to_bits()
        }
    })
}

/// Reconstructed closure dependencies identify both declaration values and
/// direct substituted uses. Both must still equal the receiving declaration's
/// canonical value; a stale helper copy cannot validate a changed declaration.
fn validate_dependency_values(
    program: &TypedTrees,
    dependencies: &[crate::const_evaluation::const_generic_expressions::DependencyValue],
    mut evaluate: impl FnMut(
        ExpressionHandle,
        PrimitiveType,
    ) -> Result<
        crate::const_evaluation::const_generic_expressions::value::ScalarValue,
        String,
    >,
) -> Result<Vec<ExpressionHandle>, String> {
    let mut authored_roots = Vec::new();
    for dependency in dependencies {
        let mut declarations = program
            .const_declarations()
            .iter()
            .filter(|declaration| declaration.symbol == dependency.declaration);
        let declaration = declarations
            .next()
            .ok_or("constant dependency lost its exact declaration")?;
        if declarations.next().is_some() {
            return Err("constant dependency declaration is ambiguous".into());
        }
        // Floating declarations have determined runtime bits, not canonical
        // generic-index atoms. Replay the original and its substituted use at
        // the declared format before comparing their materialization encoding.
        if let Some(destination @ (PrimitiveType::F32 | PrimitiveType::F64)) =
            crate::const_evaluation::const_generic_expressions::scalar_probe_destination(
                program,
                declaration.declared_type,
            )
        {
            if !program
                .expression_table
                .expression_is_valid(declaration.authored_initializer)
                || program
                    .expression_table
                    .source_span(declaration.authored_initializer)
                    != declaration.initializer_source_span
            {
                return Err("constant dependency lost its exact authored initializer".into());
            }
            let expected = declaration
                .canonical_value_encoding
                .as_deref()
                .ok_or("floating constant dependency lost its materialized bits")?;
            for expression in [declaration.authored_initializer, dependency.expression] {
                let value = evaluate(expression, destination)?;
                if !matches!(
                    value,
                    crate::const_evaluation::const_generic_expressions::value::ScalarValue::Float { .. }
                ) || value.encoding() != expected
                {
                    return Err(
                        "floating constant dependency or its substituted use drifted".into(),
                    );
                }
            }
            if !authored_roots.contains(&declaration.authored_initializer) {
                authored_roots.push(declaration.authored_initializer);
            }
            continue;
        }
        let expected = CanonicalConstIdentity {
            type_name: String::new(),
            encoding: declaration
                .canonical_value_encoding
                .clone()
                .ok_or("constant dependency lost its canonical value")?,
        }
        .decode_encoding()
        .ok_or("constant dependency has an invalid canonical value")?;
        if let Some(destination) =
            crate::const_evaluation::const_generic_expressions::exact_probe_destination(
                program,
                declaration.declared_type,
            )
        {
            if !program
                .expression_table
                .expression_is_valid(declaration.authored_initializer)
                || program
                    .expression_table
                    .source_span(declaration.authored_initializer)
                    != declaration.initializer_source_span
            {
                return Err("constant dependency lost its exact authored initializer".into());
            }
            if !authored_roots.contains(&declaration.authored_initializer) {
                let authored =
                    evaluate(declaration.authored_initializer, destination)?.into_index()?;
                if authored.decode_encoding().as_ref() != Some(&expected) {
                    return Err(
                        "constant dependency authored computation drifted from its canonical value"
                            .into(),
                    );
                }
            }
        }
        let mut leaves = Vec::new();
        pair(
            program,
            Leaf {
                owner: declaration.symbol,
                original: dependency.expression,
                materialized: dependency.expression,
                destination: declaration.declared_type,
                structured: false,
                expected,
            },
            &mut Vec::new(),
            &mut leaves,
        )?;
        for leaf in leaves {
            let actual = if leaf.structured {
                dependency_leaf_value(program, leaf.materialized)?
            } else {
                let destination =
                    crate::const_evaluation::const_generic_expressions::exact_probe_destination(
                        program,
                        leaf.destination,
                    )
                    .ok_or("constant dependency lost its exact scalar destination")?;
                evaluate(leaf.materialized, destination)?
                    .into_index()?
                    .decode_encoding()
                    .ok_or("constant dependency has an invalid scalar value")?
            };
            if actual != leaf.expected {
                return Err(
                    "constant dependency or its substituted use drifted from its canonical value"
                        .into(),
                );
            }
        }
        if declaration.authored_initializer.is_valid()
            && !authored_roots.contains(&declaration.authored_initializer)
        {
            authored_roots.push(declaration.authored_initializer);
        }
    }
    Ok(authored_roots)
}

/// Reconstruct the canonical value one aggregate dependency leaf crossed as:
/// an exact constant use decodes its selected declaration's own canonical
/// result, while a payloadless case rebuilds its variant identity from the
/// receiving forest's symbol table.
fn dependency_leaf_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Result<DecodedCanonicalConstValue, String> {
    let ExpressionNode::Name(name) = program.expression_table.expression(expression) else {
        return Err("aggregate dependency leaf is not an exact constant or case use".into());
    };
    match program.symbols.get(name.symbol).kind {
        symbols::SymbolKind::Const => {
            let mut declarations = program
                .const_declarations()
                .iter()
                .filter(|declaration| declaration.symbol == name.symbol);
            let declaration = declarations
                .next()
                .ok_or("constant dependency lost its exact declaration")?;
            if declarations.next().is_some() {
                return Err("constant dependency declaration is ambiguous".into());
            }
            CanonicalConstIdentity {
                type_name: String::new(),
                encoding: declaration
                    .canonical_value_encoding
                    .clone()
                    .ok_or("constant dependency lost its canonical value")?,
            }
            .decode_encoding()
            .ok_or_else(|| "constant dependency has an invalid canonical value".to_owned())
        }
        symbols::SymbolKind::Variant => {
            let owner = program.symbols.get(name.symbol).parent;
            let definition = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == owner)
                .ok_or("case dependency lost its exact owner declaration")?;
            Ok(DecodedCanonicalConstValue::Variant {
                type_name: definition.name.as_str().to_owned(),
                case_name: program.symbols.name(name.symbol).to_owned(),
                fields: Vec::new(),
            })
        }
        _ => Err("aggregate dependency leaf is not an exact constant or case use".into()),
    }
}

fn pair(
    program: &TypedTrees,
    mut leaf: Leaf,
    active: &mut Vec<ExpressionHandle>,
    leaves: &mut Vec<Leaf>,
) -> Result<(), String> {
    let table = &program.expression_table;
    if !table.expression_is_valid(leaf.original)
        || !table.expression_is_valid(leaf.materialized)
        || active.contains(&leaf.original)
        || table.source_span(leaf.original) != table.source_span(leaf.materialized)
    {
        return Err("retained initializer lost its exact authored/materialized roots".into());
    }
    if let Some(destination) =
        crate::const_evaluation::const_generic_expressions::exact_probe_destination(
            program,
            leaf.destination,
        )
    {
        match (&leaf.expected, destination) {
            (DecodedCanonicalConstValue::Boolean(_), PrimitiveType::Bool) => {}
            (DecodedCanonicalConstValue::Integer { type_name, .. }, primitive)
                if type_name == primitive.name() => {}
            _ => {
                return Err(
                    "retained initializer canonical leaf has a different declared carrier".into(),
                );
            }
        }
        leaves.push(leaf);
        return Ok(());
    }
    active.push(leaf.original);
    match (
        table.expression(leaf.original),
        table.expression(leaf.materialized),
        program
            .type_reference_table
            .type_reference(leaf.destination),
        &leaf.expected,
    ) {
        (
            ExpressionNode::ArrayLiteral(original),
            ExpressionNode::ArrayLiteral(materialized),
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            },
            DecodedCanonicalConstValue::Array { values, type_name },
        ) => {
            if type_name != &program.display_type_reference(leaf.destination)
                || original.len() != *length
                || materialized.len() != *length
            {
                return Err("retained initializer array carrier or span drifted".into());
            }
            let originals = table.expression_handles(*original);
            let materialized = table.expression_handles(*materialized);
            if originals.len() != *length
                || materialized.len() != *length
                || values.len() != *length
            {
                return Err("retained initializer array shape drifted".into());
            }
            for ((original, materialized), expected) in
                originals.iter().zip(materialized).zip(values)
            {
                pair(
                    program,
                    Leaf {
                        owner: leaf.owner,
                        original: *original,
                        materialized: *materialized,
                        destination: *element_type,
                        structured: false,
                        expected: expected.clone(),
                    },
                    active,
                    leaves,
                )?;
            }
        }
        (
            ExpressionNode::StructLiteral(original),
            ExpressionNode::StructLiteral(materialized),
            _,
            expected,
        ) => {
            let symbol = program.type_reference_table.type_symbol(leaf.destination);
            if !symbol.is_valid()
                || original.type_symbol != symbol
                || materialized.type_symbol != symbol
                || original.case_symbol != materialized.case_symbol
            {
                return Err("retained initializer constructor identity drifted".into());
            }
            let definition = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == symbol)
                .ok_or("retained initializer constructor has no exact declaration")?;
            let type_name = match expected {
                DecodedCanonicalConstValue::Record { type_name, .. }
                | DecodedCanonicalConstValue::Variant { type_name, .. } => type_name,
                _ => return Err("retained canonical value is not a nominal constructor".into()),
            };
            if type_name != definition.name.as_str()
                || program.data_members(definition).len() != definition.members.len()
            {
                return Err("retained initializer nominal carrier or member span drifted".into());
            }
            let (fields, expected) = match (original.case_symbol, expected) {
                (None, DecodedCanonicalConstValue::Record { fields, .. }) => (
                    program
                        .data_members(definition)
                        .iter()
                        .filter_map(|member| {
                            if let DataMember::Field(field) = member {
                                Some(field)
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>(),
                    fields,
                ),
                (
                    Some(case),
                    DecodedCanonicalConstValue::Variant {
                        case_name, fields, ..
                    },
                ) => {
                    let variant = program
                        .data_members(definition)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Variant(variant)
                                if variant.symbol == case && variant.name.as_str() == case_name =>
                            {
                                Some(variant)
                            }
                            _ => None,
                        })
                        .ok_or("retained initializer case identity drifted")?;
                    if program.data_payload_fields(variant).len() != variant.payload.len() {
                        return Err("retained initializer case payload span drifted".into());
                    }
                    (
                        program.data_payload_fields(variant).iter().collect(),
                        fields,
                    )
                }
                _ => return Err("retained initializer canonical constructor shape drifted".into()),
            };
            let originals = table.struct_fields(original.fields);
            if originals.len() != original.fields.len()
                || table.struct_fields(materialized.fields).len() != materialized.fields.len()
            {
                return Err("retained initializer constructor field span drifted".into());
            }
            let materialized = table.struct_fields(materialized.fields);
            if originals.len() != fields.len()
                || materialized.len() != fields.len()
                || expected.len() != fields.len()
            {
                return Err("retained initializer field roster drifted".into());
            }
            for (field, (name, expected)) in fields.iter().zip(expected) {
                if field.name.as_str() != name {
                    return Err("retained canonical field order drifted".into());
                }
                let matching_originals = originals
                    .iter()
                    .filter(|candidate| candidate.field_symbol == field.symbol)
                    .collect::<Vec<_>>();
                let [original] = matching_originals.as_slice() else {
                    return Err("retained initializer lost an exact original field".into());
                };
                let matching_materialized = materialized
                    .iter()
                    .filter(|candidate| candidate.field_symbol == field.symbol)
                    .collect::<Vec<_>>();
                let [materialized] = matching_materialized.as_slice() else {
                    return Err("retained initializer lost an exact materialized field".into());
                };
                if original.name != field.name || materialized.name != field.name {
                    return Err(
                        "retained initializer field metadata differs from its declaration".into(),
                    );
                }
                pair(
                    program,
                    Leaf {
                        owner: leaf.owner,
                        original: original.value,
                        materialized: materialized.value,
                        destination: field.type_reference,
                        structured: false,
                        expected: expected.clone(),
                    },
                    active,
                    leaves,
                )?;
            }
        }
        _ => {
            // Evaluation produced this aggregate leaf rather than literal
            // correspondence: both roots replay as whole structured probes
            // whose independently encoded results must equal the canonical
            // value.
            if matches!(
                leaf.expected,
                DecodedCanonicalConstValue::Array { .. }
                    | DecodedCanonicalConstValue::Record { .. }
                    | DecodedCanonicalConstValue::Variant { .. }
            ) {
                leaf.structured = true;
                leaves.push(leaf);
                active.pop();
                return Ok(());
            }
            return Err("retained initializer has no checked aggregate leaf correspondence".into());
        }
    }
    active.pop();
    Ok(())
}

#[cfg(test)]
#[path = "tests/noncall_float_replay.rs"]
mod noncall_float_replay;
