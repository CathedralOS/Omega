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
            crate::const_generic_expressions::initializer_contains_call(
                typed,
                declaration.authored_initializer,
            )
            .map_err(|reason| super::failure(reference, reason))?
        } else {
            false
        };
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
        owners.push(append_probe(&mut probe, ordinal, leaf));
    }
    let checked = super::invocations::CheckedInitializers::prepare(&probe, authority, &owners)?;
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
            .calls_for_symbol(owners[index])
            .and_then(|calls| {
                let dependencies = calls.validate_custody(
                    declaration.authored_initializer,
                    declaration.materialized_initializer,
                )?;
                // Reconstructed closure dependencies, not the fold's claimed
                // roster, identify values to rejoin. Check both declaration
                // roots and direct substituted uses: changing a declaration
                // cannot leave a same-source helper copy at its former value.
                for dependency in dependencies {
                    let mut declarations = program.const_declarations().iter()
                        .filter(|declaration| declaration.symbol == dependency.declaration);
                    let declaration = declarations.next().ok_or("constant dependency lost its exact declaration")?;
                    if declarations.next().is_some() {
                        return Err("constant dependency declaration is ambiguous".into());
                    }
                    let identity = CanonicalConstIdentity {
                        type_name: String::new(),
                        encoding: declaration.canonical_value_encoding.clone()
                            .ok_or("constant dependency lost its canonical value")?,
                    };
                    let expected = identity.decode_encoding()
                        .ok_or("constant dependency has an invalid canonical value")?;
                    let mut dependency_leaves = Vec::new();
                    pair(program, Leaf {
                        owner: declaration.symbol,
                        original: dependency.expression,
                        materialized: dependency.expression,
                        destination: declaration.declared_type,
                        expected,
                    }, &mut Vec::new(), &mut dependency_leaves)?;
                    for leaf in dependency_leaves {
                        let destination = crate::const_generic_expressions::exact_probe_destination(program, leaf.destination)
                            .ok_or("constant dependency lost its exact scalar destination")?;
                        let value = calls.evaluate(leaf.materialized, destination)?.0;
                        if value.decode_encoding().as_ref() != Some(&leaf.expected) {
                            return Err("constant dependency or its substituted use drifted from its canonical value".into());
                        }
                    }
                }
                Ok(())
            })
            .map_err(|reason| super::failure(declaration.initializer_source_span, reason))?;
    }
    for (leaf, owner) in leaves.iter().zip(owners) {
        let result = (|| {
            let calls = checked.calls_for_symbol(owner)?;
            let destination = crate::const_generic_expressions::exact_probe_destination(
                program,
                leaf.destination,
            )
            .ok_or("retained initializer leaf lost its exact scalar destination")?;
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
            Ok(())
        })();
        result.map_err(|reason| {
            super::failure(program.expression_table.source_span(leaf.original), reason)
        })?;
    }
    Ok(())
}

fn append_probe(program: &mut TypedTrees, ordinal: usize, leaf: &Leaf) -> symbols::SymbolHandle {
    let name = format!("@const-replay-{ordinal}");
    let symbol =
        program
            .symbols
            .insert_generated_root_from(leaf.owner, symbols::SymbolKind::Machine, &name);
    let children = program
        .symbols
        .insert_generated_children(symbol, [(symbols::SymbolKind::State, name.as_str())]);
    let target = program.statement_table.insert_transition_target(
        typed_trees::statement::TransitionTargetNode::Value(leaf.original),
    );
    let mut state = typed_trees::state::State {
        symbol: children.start(),
        name: typed_trees::name::Identifier::generated(name.clone()),
        return_type: leaf.destination,
        ..Default::default()
    };
    let source_span = program.expression_table.source_span(leaf.original);
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        typed_trees::statement::StatementNode::Transition(
            typed_trees::statement::TableTransition {
                target,
                source_span,
                ..Default::default()
            },
        ),
    );
    let mut machine = typed_trees::machine::Machine {
        symbol,
        name: typed_trees::name::Identifier::generated(name),
        ..Default::default()
    };
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    symbol
}

fn pair(
    program: &TypedTrees,
    leaf: Leaf,
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
        crate::const_generic_expressions::exact_probe_destination(program, leaf.destination)
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
                        expected: expected.clone(),
                    },
                    active,
                    leaves,
                )?;
            }
        }
        _ => {
            return Err("retained initializer has no checked aggregate leaf correspondence".into());
        }
    }
    active.pop();
    Ok(())
}
