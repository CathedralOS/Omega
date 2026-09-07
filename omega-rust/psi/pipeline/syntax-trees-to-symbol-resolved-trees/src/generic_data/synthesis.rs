//! Closed-instance discovery, fixed-point synthesis and the ordered rewrite.

use super::*;

pub(in crate::generic_data) fn desugar_generic_data_instances(
    syntax: &mut SyntaxTrees,
    warnings: &mut Vec<Diagnostic>,
) -> Result<(), Vec<Diagnostic>> {
    // Index generic data definitions by name (only those with type parameters;
    // a non-generic `Base<..>` is either plan-laid or an existing error path).
    // Generic bases that carry attached MACHINES (a generic container like
    // `Vec<T>` with `push`) are LEFT for the existing path: monomorphizing the
    // data without its generic machines (Phase 2) would break method
    // resolution (`self.items.push(..)` on a `Vec<i32>` field). Phase 1 =
    // method-less generic data only.
    let mut data_with_machines: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    // Attached machines per data name, as ROOT-ITEM indexes (the synthesis
    // loop clones them from a snapshot when it builds a container instance).
    let mut attached_machines: HashMap<String, Vec<usize>> = HashMap::new();
    for (item_index, item) in syntax.root_items().enumerate() {
        if let Item::Machine(machine) = item
            && let Some(attached) = &machine.attached_data
        {
            data_with_machines.insert(attached.as_str().to_string());
            attached_machines
                .entry(attached.as_str().to_string())
                .or_default()
                .push(item_index);
        }
    }

    let mut generic_data: HashMap<String, GenericData> = HashMap::new();
    for item in syntax.root_items() {
        let Item::Data(definition) = item else {
            continue;
        };
        if definition.type_parameters.is_empty() {
            continue;
        }
        // Quotient identity is semantic, not record shape. This temporary
        // record/container synthesizer cannot yet substitute the quotient's
        // carrier, relation, and exact Equivalence selection as one unit.
        // Leaving the generic quotient nominally visible lets the downstream
        // quotient fences reject representation literals; synthesizing an
        // empty record with `quotient: None` would make the class freely
        // constructible.
        if definition.quotient.is_some() {
            continue;
        }
        // These compiler-owned proof algebras never acquire runtime layout.
        // Keep their generic argument structurally visible so checked content
        // plans retain a normalized coordinate-space/unit identity instead of
        // collapsing it into a synthesized diagnostic spelling.
        if matches!(definition.name.as_str(), "IntervalSet" | "CountedQuantity") {
            continue;
        }
        let definition_parameters = syntax
            .tables
            .items
            .type_parameters(definition.type_parameters);
        // Machine-symbol parameters require method/template identity work this
        // record-only pass does not perform. Type and const parameters are both
        // supported; const arguments are substituted into fixed-array lengths.
        if definition_parameters
            .iter()
            .any(|parameter| matches!(parameter.kind, TypeParameterKind::Machine { .. }))
        {
            continue;
        }
        if data_with_machines.contains(definition.name.as_str()) {
            // A CONTAINER (generic data with attached machines) monomorphizes
            // ONLY when every method's own type parameters are covered by the
            // data's parameter names (T-on-method matching T-on-data --
            // decision: per-instance mono, instances always spelled). The
            // instance clones each method with T substituted (Phase 2 slice
            // 1), so `self.b.stored()` on a `Box<i32>` field resolves against
            // a CONCRETE machine and the T-typed value call materializes
            // (was the runtime silent-0). An uncovered method leaves the
            // whole container for the type-check-only path, as before.
            let data_parameters: Vec<(String, bool)> = syntax
                .tables
                .items
                .type_parameters(definition.type_parameters)
                .iter()
                .map(|parameter| {
                    (
                        parameter.name.as_str().to_string(),
                        matches!(parameter.kind, TypeParameterKind::Const { .. }),
                    )
                })
                .collect();
            let all_methods_covered =
                attached_machines[definition.name.as_str()]
                    .iter()
                    .all(|&item_index| {
                        let Some(Item::Machine(machine)) = syntax.root_items().nth(item_index)
                        else {
                            return false;
                        };
                        // DECLARATION-ONLY methods (the stdlib `Vec<T>` surface --
                        // empty state bodies, type-check-only) must NOT clone: a
                        // concrete clone of an empty body trips the
                        // returns-but-empty check that generic templates are
                        // exempt from. Such containers stay type-check-only.
                        let has_bodies = syntax
                            .tables
                            .items
                            .state_handles(machine.states)
                            .iter()
                            .any(|state| !syntax.tables.items.state(*state).statements.is_empty());
                        has_bodies
                            && syntax
                                .tables
                                .items
                                .type_parameters(machine.type_parameters)
                                .iter()
                                .all(|parameter| {
                                    let method_is_const =
                                        matches!(parameter.kind, TypeParameterKind::Const { .. });
                                    data_parameters.iter().any(|(name, data_is_const)| {
                                        name == parameter.name.as_str()
                                            && *data_is_const == method_is_const
                                    })
                                })
                    });
            if !all_methods_covered {
                continue;
            }
        }
        let parameter_names = definition_parameters
            .iter()
            .map(|parameter| parameter.name.as_str().to_string())
            .collect::<Vec<_>>();
        let const_parameter_types = definition_parameters
            .iter()
            .map(|parameter| match parameter.kind {
                TypeParameterKind::Const { type_reference } => Some(type_reference),
                _ => None,
            })
            .collect();
        generic_data.insert(
            definition.name.as_str().to_string(),
            GenericData {
                name: definition.name.as_str().to_owned(),
                origin_name: definition.name.clone(),
                is_public: definition.is_public,
                lifetime_parameters: definition.lifetime_parameters.clone(),
                parameter_names,
                const_parameter_types,
                where_facts: definition.where_facts,
                members: definition.members,
                properties: definition.properties,
                supply_mode: definition.supply_mode,
            },
        );
    }
    // Const-v0 declarations disappear during symbol resolution, but generic
    // data instances are selected before that stage. Retain every declaration
    // for structured-value canonicalization and the integer subset separately
    // for the existing expression/fact evaluator.
    let const_definitions: HashMap<String, ConstDefinition> = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Const(definition) = item else {
                return None;
            };
            Some((qualified_const_name(definition), definition.clone()))
        })
        .collect();
    let const_values: HashMap<String, i128> = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Const(definition) = item else {
                return None;
            };
            let ExpressionNode::Integer(value) = syntax.expressions.expression(definition.value)
            else {
                return None;
            };
            let value = integer_literal_value(value)?;
            let qualified_name = if definition.scope.as_str().is_empty() {
                definition.name.as_str().to_string()
            } else {
                format!(
                    "{}::{}",
                    definition.scope.as_str(),
                    definition.name.as_str()
                )
            };
            Some((qualified_name, value))
        })
        .collect();

    canonicalize_closed_domain_indices(syntax, &const_definitions, &const_values, warnings)
        .map_err(|diagnostic| vec![diagnostic])?;

    if generic_data.is_empty() {
        return Ok(());
    }

    // FIXPOINT. Each round scans every type-reference position for a
    // `Base<Args..>` spelling, synthesizes one concrete record per new distinct
    // spelling, and rewrites the spellings to the instances' plain names. A
    // NESTED generic (`Pair<T> { a: Box<T> }` used as `Pair<i32>`) synthesizes a
    // `Pair<i32>` record whose `a` field is a fresh `Box<i32>` spelling -- picked
    // up and monomorphized by the NEXT round. Terminates: each round rewrites
    // >=1 Generic node to Named (permanent) or stops, and the distinct concrete
    // spellings are finite.
    let mut synthesized: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut synthesized_origins: HashMap<String, String> = HashMap::new();
    let mut synthesized_sum_instances: HashMap<String, HashSet<String>> = HashMap::new();
    loop {
        let positions = collect_type_reference_positions(syntax);
        let mut rewrites: Vec<PendingRewrite> = Vec::new();
        let mut instantiations: Vec<Instantiation> = Vec::new();
        for position in positions {
            consider_generic_spelling(
                syntax,
                &generic_data,
                &const_definitions,
                &const_values,
                position,
                &mut rewrites,
                &mut instantiations,
                warnings,
            )
            .map_err(|diagnostic| vec![diagnostic])?;
        }
        if rewrites.is_empty() {
            break; // no more monomorphizable generic spellings
        }
        for instance in &instantiations {
            let base_info = &generic_data[&instance.base_name];
            if !matches!(
                generic_data_shape(syntax, base_info),
                Some(GenericDataShape::PureSum | GenericDataShape::MixedSum)
            ) {
                continue;
            }
            synthesized_sum_instances
                .entry(instance.base_name.clone())
                .or_default()
                .insert(instance.synthetic_name.clone());
        }
        // Synthesize each not-yet-built instance: the base's members cloned with
        // the type parameters substituted for the arguments.
        for instance in &instantiations {
            synthesized_origins.insert(instance.synthetic_name.clone(), instance.base_name.clone());
            if !synthesized.insert(instance.synthetic_name.clone()) {
                continue;
            }
            let base_info = &generic_data[&instance.base_name];
            let substitution: HashMap<String, TypeReferenceHandle> = base_info
                .parameter_names
                .iter()
                .cloned()
                .zip(instance.argument_handles.iter().copied())
                .collect();
            let const_parameter_values: HashMap<String, i128> = base_info
                .parameter_names
                .iter()
                .zip(&base_info.const_parameter_types)
                .filter_map(|(name, parameter_type)| {
                    parameter_type.as_ref()?;
                    let argument = substitution.get(name)?;
                    let TypeReferenceNode::Named(value) =
                        syntax.tables.type_references.type_reference(*argument)
                    else {
                        return None;
                    };
                    Some((name.clone(), value.as_str().parse().ok()?))
                })
                .collect();
            let const_parameter_type_names: HashMap<String, String> = base_info
                .parameter_names
                .iter()
                .zip(&base_info.const_parameter_types)
                .filter_map(|(name, parameter_type)| {
                    let parameter_type = parameter_type.as_ref()?;
                    let TypeReferenceNode::Named(type_name) = syntax
                        .tables
                        .type_references
                        .type_reference(*parameter_type)
                    else {
                        return None;
                    };
                    Some((name.clone(), type_name.as_str().to_string()))
                })
                .collect();
            let const_literals: HashMap<String, IntegerLiteral> = const_parameter_values
                .iter()
                .map(|(name, value)| {
                    let literal = IntegerLiteral::from_parts(
                        *value < 0,
                        IntegerRadix::Decimal,
                        value.unsigned_abs().to_string().as_str(),
                    )
                    .expect("a concrete const argument is a valid decimal integer literal");
                    (name.clone(), literal)
                })
                .collect();

            // A fact whose operands are all const-bound is an instantiation
            // obligation, not a standing runtime invariant. Prove it now and
            // omit it from the concrete record. Mixed facts retain their field
            // operands and receive the same const substitution as members.
            let snapshot = syntax.clone();
            let fact_expression_watermark = (syntax.expressions.expression_count() as u32)
                .checked_add(1)
                .expect("expression arena index overflow");
            let mut first_fact = Handle::invalid();
            let mut fact_count = 0u32;
            for (offset, fact) in snapshot
                .tables
                .items
                .proof_facts(base_info.where_facts)
                .iter()
                .enumerate()
            {
                let const_result = match fact {
                    ProofFact::Expression(expression) => evaluate_const_fact_expression(
                        &snapshot,
                        *expression,
                        &const_values,
                        &const_parameter_values,
                        None,
                    )
                    .map(|value| match value {
                        Some(ConstFactValue::Boolean(value)) => Some(value),
                        _ => None,
                    }),
                    ProofFact::Membership(membership) => evaluate_const_membership_fact(
                        &snapshot,
                        membership,
                        &const_values,
                        &const_parameter_values,
                        &const_parameter_type_names,
                    ),
                }
                .map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "const fact for generic instance `{}` is invalid: {reason}",
                        instance.synthetic_name
                    ))]
                })?;
                if let Some(value) = const_result {
                    if value {
                        continue;
                    }
                    return Err(vec![Diagnostic::error(format!(
                        "const fact for generic instance `{}` is false",
                        instance.synthetic_name
                    ))]);
                }
                let source = Handle::from_parts(
                    base_info
                        .where_facts
                        .start()
                        .arena_index()
                        .checked_add(u32::try_from(offset).expect("proof fact offset overflow"))
                        .expect("proof fact source handle overflow"),
                    base_info.where_facts.start().generation(),
                );
                let handle = syntax.copy_proof_fact_from(&snapshot, source);
                if fact_count == 0 {
                    first_fact = handle;
                }
                fact_count += 1;
            }
            replace_const_expression_names_from(syntax, fact_expression_watermark, &const_literals);
            let where_facts = HandleSpan::from_parts(first_fact, fact_count);

            // Retain the authored generic application as structural evidence.
            // The synthesized display name is diagnostic-only; downstream
            // identity and substitution resolve this base and argument tuple.
            let origin_arguments = syntax
                .tables
                .type_references
                .insert_type_reference_handles(instance.argument_handles.iter().copied());
            let generic_instance = syntax.tables.type_references.insert(
                syntax_trees::types::TypeReferenceNode::Generic {
                    base_name: Identifier::generated(instance.base_name.as_str()),
                    lifetime_arguments: Vec::new(),
                    arguments: origin_arguments,
                },
            );

            let members: Vec<DataMember> =
                syntax.tables.items.data_members(base_info.members).to_vec();
            let properties = base_info.properties;
            let mut first: Handle<DataMember> = Handle::invalid();
            let mut count = 0u32;
            for member in members {
                let substituted =
                    substitute_member(syntax, member, &substitution, &const_values, warnings);
                let handle = syntax.tables.items.append_data_member(substituted);
                if count == 0 {
                    first = handle;
                }
                count += 1;
            }
            syntax.push_root_item(Item::Data(DataDefinition {
                // The closed instance is compiler-generated, but its mandatory
                // derivation origin is the exact authored generic declaration.
                // Retain that span under the synthetic semantic spelling so
                // package ownership never falls back to an unresolved name.
                name: Identifier::new(
                    instance.synthetic_name.as_str(),
                    base_info.origin_name.source_span(),
                ),
                is_public: base_info.is_public,
                supply_mode: base_info.supply_mode,
                lifetime_parameters: base_info.lifetime_parameters.clone(),
                type_parameters: HandleSpan::default(),
                generic_instance: Some(generic_instance),
                properties,
                where_facts,
                members: HandleSpan::from_parts(first, count),
                quotient: None,
            }));

            // CONTAINER instance: clone each attached machine with the type
            // parameters substituted (Phase 2 slice 1). The clone copies from
            // a SNAPSHOT of the tree (same-tree deep copies need a & source
            // while appending into &mut tables), then a WATERMARK pass
            // rewrites `Named(T)` nodes created by the copy -- only the
            // clone's own subtree is younger than the watermark.
            let Some(machine_items) = attached_machines.get(&instance.base_name) else {
                continue;
            };
            let snapshot = syntax.clone();
            for &item_index in machine_items {
                let Some(Item::Machine(machine)) = snapshot.root_items().nth(item_index) else {
                    continue;
                };
                let type_watermark = syntax.tables.type_references.node_count();
                let expression_watermark = (syntax.expressions.expression_count() as u32)
                    .checked_add(1)
                    .expect("expression arena index overflow");
                let Item::Machine(mut clone) =
                    syntax.copy_item_from(&snapshot, &Item::Machine(machine.clone()))
                else {
                    continue;
                };
                // The clone is CONCRETE: attached to the synthetic record,
                // its type parameters cleared, its `Named(T)` type nodes
                // substituted with the instance arguments. The machine NAME
                // is the FULL parsed path ("Box::stored"), so the attached
                // segment is rewritten there too ("Box<i32>::stored") --
                // machine identity keys on the composed name.
                let method_tail = machine
                    .name
                    .as_str()
                    .rsplit("::")
                    .next()
                    .unwrap_or(machine.name.as_str())
                    .to_string();
                clone.name = Identifier::new(
                    format!("{}::{}", instance.synthetic_name, method_tail),
                    machine.name.source_span(),
                );
                clone.attached_data = Some(Identifier::generated(instance.synthetic_name.as_str()));
                clone.generic_data_template = machine.name.clone();
                clone.type_parameters = HandleSpan::default();
                for (handle, name) in syntax
                    .tables
                    .type_references
                    .named_nodes_from(type_watermark)
                {
                    if let Some(argument) = substitution.get(&name) {
                        let replacement = syntax
                            .tables
                            .type_references
                            .type_reference(*argument)
                            .clone();
                        syntax
                            .tables
                            .type_references
                            .replace_type_reference(handle, replacement);
                    }
                }
                for (handle, element_type, name) in syntax
                    .tables
                    .type_references
                    .const_parameter_array_nodes_from(type_watermark)
                {
                    let Some(length) = substitution.get(&name).and_then(|argument| {
                        match syntax.tables.type_references.type_reference(*argument) {
                            TypeReferenceNode::Named(value) => value.as_str().parse::<usize>().ok(),
                            _ => None,
                        }
                    }) else {
                        continue;
                    };
                    syntax.tables.type_references.replace_type_reference(
                        handle,
                        TypeReferenceNode::FixedArray {
                            element_type,
                            length: FixedArrayLength::Literal(length),
                        },
                    );
                }
                replace_const_expression_names_from(syntax, expression_watermark, &const_literals);
                syntax.push_root_item(Item::Machine(clone));
            }
        }

        // Rewrite this round's spellings to the synthesized instances' plain names.
        for rewrite in rewrites {
            // Keep occurrence custody separately from the instance's shared
            // derivation. Public/private exposure belongs to this use site.
            let original = syntax
                .tables
                .type_references
                .type_reference(rewrite.type_reference)
                .clone();
            let TypeReferenceNode::Generic { base_name, .. } = &original else {
                continue;
            };
            let instance_name = Identifier::new(rewrite.synthetic_name, base_name.source_span());
            let application = syntax.tables.type_references.insert(original);
            syntax
                .tables
                .type_references
                .retain_generic_application_origin(rewrite.type_reference, application);
            let rewritten = if rewrite.lifetime_arguments.is_empty() {
                TypeReferenceNode::Named(instance_name)
            } else {
                TypeReferenceNode::Generic {
                    base_name: instance_name,
                    lifetime_arguments: rewrite.lifetime_arguments,
                    arguments: HandleSpan::empty(),
                }
            };
            syntax
                .tables
                .type_references
                .replace_type_reference(rewrite.type_reference, rewritten);
        }
    }

    relabel_closed_data_uses_in_annotated_locals(syntax, &synthesized_origins);
    relabel_closed_data_uses_in_exact_assignments(syntax, &synthesized_origins);
    relabel_closed_data_uses_in_exact_calls_and_returns(syntax, &synthesized_origins);
    relabel_closed_sum_memberships_from_local_types(syntax, &synthesized_origins);
    let unique_sum_instances = synthesized_sum_instances
        .into_iter()
        .filter_map(|(base, instances)| {
            (instances.len() == 1).then(|| (base, instances.into_iter().next().unwrap()))
        })
        .collect();
    relabel_unique_closed_sum_paths(syntax, &unique_sum_instances);

    normalize_generic_template_const_expressions(syntax, &const_values, warnings)
        .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}
