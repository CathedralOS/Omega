use bignum::BigInt;
/* Closed-instance discovery, fixed-point synthesis and the ordered rewrite. */
use crate::resolution::declarations::generic_data::ClosedArgumentIdentity;
use crate::resolution::declarations::generic_data::ConstFactValue;
use crate::resolution::declarations::generic_data::ConstScalarValue;
use crate::resolution::declarations::generic_data::GenericData;
use crate::resolution::declarations::generic_data::Instantiation;
use crate::resolution::declarations::generic_data::PendingRewrite;
use crate::resolution::declarations::generic_data::canonicalize_closed_domain_indices;
use crate::resolution::declarations::generic_data::capture_machine_runtime_template_names;
use crate::resolution::declarations::generic_data::uses::{
    closed_constructor_carrier, relabel_closed_data_uses_in_constants,
    relabel_closed_data_uses_in_exact_assignments,
    relabel_closed_data_uses_in_exact_calls_and_returns,
    relabel_closed_sum_memberships_from_local_types,
};
use crate::resolution::tables::syntax_items;
use crate::resolution::tables::syntax_type_references;

use super::substitution::substitute_copied_type_names;
use super::work_domain::{GenericDataWorkDomain, SynthesisWork, TypePositionPurpose};
use crate::resolution::declarations::generic_data::evaluate_const_fact_expression;
use crate::resolution::declarations::generic_data::evaluate_const_membership_fact;
use crate::resolution::declarations::generic_data::normalize_domain_head_positions;
use crate::resolution::declarations::generic_data::normalize_generic_template_const_expressions;
use crate::resolution::declarations::generic_data::prepare_closed_application;

use super::source_modules::module_path;
use super::substitution::fold_impossible_case_memberships;
use super::substitution::variant_fact_verdict;
use crate::resolution::declarations::declaration_selection::ConstantSelection;
use crate::resolution::declarations::generic_data::replace_const_expression_names_from;
use crate::resolution::declarations::generic_data::replace_machine_const_expression_names_from;
use crate::resolution::declarations::generic_data::substitute_member;
use crate::resolution::declarations::type_equations::normalized_boolean_argument;
use arena::Handle;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use numeric_literals::IntegerLiteral;
use numeric_literals::IntegerRadix;
use std::collections::HashMap;
use std::collections::HashSet;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::DataDefinition;
use syntax_trees::item::DataMember;
use syntax_trees::item::Item;
use syntax_trees::item::ProofFact;
use syntax_trees::item::TypeParameterKind;
use syntax_trees::types::FixedArrayLength;
use syntax_trees::types::TypeReferenceHandle;
use syntax_trees::types::TypeReferenceNode;

pub(crate) fn desugar_generic_data_instances_in(
    syntax: &mut SyntaxTrees,
    warnings: &mut Vec<Diagnostic>,
    selection: Option<&ConstantSelection>,
    domain: GenericDataWorkDomain<'_>,
) -> Result<(), Vec<Diagnostic>> {
    if domain.type_roots.is_empty() && domain.declarations.is_empty() {
        return Ok(());
    }
    let work = SynthesisWork::new(syntax, domain);
    let mut attached_machines: HashMap<syntax_trees::item::ItemHandle, Vec<usize>> = HashMap::new();
    let roots =
        crate::resolution::declarations::declaration_selection::RootDeclarations::of(syntax);
    for (item_index, item) in syntax.root_items().enumerate() {
        if let Some(machine) = item.as_machine()
            && let Some(attached) = &machine.attached_data
            && let Some(declaration) =
                super::discovery::selected_data_item_in(&roots, selection, attached)
        {
            attached_machines
                .entry(declaration)
                .or_default()
                .push(item_index);
        }
    }

    let mut generic_data: HashMap<syntax_trees::item::ItemHandle, GenericData> = HashMap::new();
    for &declaration in syntax.root_item_handles() {
        let Some(definition) = syntax.root_item(declaration).as_data() else {
            continue;
        };
        if definition.type_parameters.is_empty() {
            continue;
        }
        if definition.quotient.is_some() || !definition.conformance_bounds.is_empty() {
            continue;
        }
        if matches!(
            definition.name.as_str(syntax),
            "IntervalSet" | "CountedQuantity"
        ) && module_path(selection, definition.name.source_span().source_id).is_none()
        {
            continue;
        }
        let definition_parameters =
            syntax_items(syntax).type_parameters(definition.type_parameters);
        if definition_parameters.iter().any(|parameter| {
            matches!(
                parameter.kind,
                TypeParameterKind::Value { .. } | TypeParameterKind::Machine { .. }
            )
        }) {
            continue;
        }
        if attached_machines.contains_key(&declaration) {
            let data_parameters: Vec<(String, bool)> = syntax_items(syntax)
                .type_parameters(definition.type_parameters)
                .iter()
                .map(|parameter| {
                    (
                        parameter.name.as_str(syntax).to_string(),
                        parameter.kind.is_const_or_value(),
                    )
                })
                .collect();
            let all_methods_covered = attached_machines[&declaration].iter().all(|&item_index| {
                let Some(Item::Machine(machine)) = syntax.root_items().nth(item_index) else {
                    return false;
                };
                syntax_items(syntax)
                    .type_parameters(machine.type_parameters)
                    .iter()
                    .all(|parameter| {
                        let method_is_const = parameter.kind.is_const_or_value();
                        data_parameters.iter().any(|(name, data_is_const)| {
                            name == parameter.name.as_str(syntax)
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
            .map(|parameter| parameter.name.as_str(syntax).to_string())
            .collect::<Vec<_>>();
        let const_parameter_types = definition_parameters
            .iter()
            .map(|parameter| match parameter.kind {
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => Some(type_reference),
                _ => None,
            })
            .collect();
        generic_data.insert(
            declaration,
            GenericData {
                declaration,
                name: definition.name.as_str(syntax).to_owned(),
                origin_name: definition.name.clone(),
                is_public: definition.is_public,
                lifetime_parameters: definition.lifetime_parameters.clone(),
                parameter_names,
                const_parameter_types,
                cleanup_selection: definition.cleanup_selection.clone(),
                requires_facts: definition.requires_facts,
                type_equations: super::equations::classify_type_equations(
                    syntax,
                    definition_parameters,
                    syntax.tables.items.proof_facts(definition.requires_facts),
                ),
                members: definition.members,
                properties: definition.properties,
                supply_mode: definition.supply_mode,
            },
        );
    }
    let domain_head_positions = work.type_positions(syntax, TypePositionPurpose::Identity, true);
    normalize_domain_head_positions(syntax, selection, &domain_head_positions);

    canonicalize_closed_domain_indices(syntax, selection, warnings, &work)
        .map_err(|diagnostic| vec![diagnostic])?;

    if generic_data.is_empty() {
        return Ok(());
    }
    let mut synthesized = Vec::new();
    for &declaration in syntax.root_item_handles() {
        let Some(definition) = syntax.root_item(declaration).as_data() else {
            continue;
        };
        let Some(origin) = definition.generic_instance else {
            continue;
        };
        let Some(ClosedArgumentIdentity::Instance(template, argument_identity)) =
            super::closed_argument_identity(syntax, selection, origin, false)
        else {
            return Err(vec![
                Diagnostic::error(format!(
                    "existing generic instance `{}` lost its exact template and argument identity",
                    definition.name.as_str(syntax),
                ))
                .with_source_span(definition.name.source_span()),
            ]);
        };
        let TypeReferenceNode::Generic { arguments, .. } =
            syntax.type_references.type_reference(origin)
        else {
            return Err(vec![Diagnostic::error(
                "existing generic instance lost its original application structure",
            )]);
        };
        synthesized.push(Instantiation {
            synthetic_name: definition.name.as_str(syntax).to_owned(),
            declaration,
            template,
            argument_handles: syntax_type_references(syntax)
                .type_reference_handles(*arguments)
                .to_vec(),
            argument_identity,
        });
    }
    loop {
        {
            let domain_positions = work.type_positions(syntax, TypePositionPurpose::Identity, true);
            normalize_domain_head_positions(syntax, selection, &domain_positions);
            canonicalize_closed_domain_indices(syntax, selection, warnings, &work)
                .map_err(|diagnostic| vec![diagnostic])?;
        }
        let storage_positions: std::collections::HashSet<TypeReferenceHandle> = work
            .type_positions(syntax, TypePositionPurpose::Materialization, false)
            .into_iter()
            .collect();
        let mut rewrites: Vec<PendingRewrite> = Vec::new();
        let mut instantiations: Vec<Instantiation> = Vec::new();
        for position in work.type_positions(syntax, TypePositionPurpose::Identity, false) {
            let Some(application) =
                prepare_closed_application(syntax, &generic_data, selection, position, warnings)
                    .map_err(|diagnostic| vec![diagnostic])?
            else {
                continue;
            };
            let prior = synthesized
                .iter()
                .chain(instantiations.iter())
                .find(|instance| {
                    instance.template == application.template
                        && instance.argument_identity == application.argument_identity
                });
            if prior.is_none() && !storage_positions.contains(&position) {
                continue;
            }
            let synthetic_name = if let Some(prior) = prior {
                prior.synthetic_name.clone()
            } else {
                let display = format!(
                    "{}<{}>",
                    generic_data[&application.template].name,
                    application.argument_names.join(", ")
                );
                if synthesized
                    .iter()
                    .chain(instantiations.iter())
                    .any(|instance| instance.synthetic_name == display)
                {
                    format!("{display}__{}", synthesized.len() + instantiations.len())
                } else {
                    display
                }
            };
            let lookup_name = if let Some(selection) = selection {
                let Some(path) = selection.data_lookup_path(syntax, &application.base_name) else {
                    continue;
                };
                if let Some((prefix, _)) = path.rsplit_once("::") {
                    format!("{prefix}::{synthetic_name}")
                } else {
                    synthetic_name.clone()
                }
            } else {
                synthetic_name.clone()
            };
            rewrites.push(PendingRewrite {
                type_reference: position,
                synthetic_name: lookup_name,
                lifetime_arguments: application.lifetime_arguments,
            });
            if prior.is_none() {
                instantiations.push(Instantiation {
                    synthetic_name,
                    declaration: Handle::invalid(),
                    template: application.template,
                    argument_handles: application.argument_handles,
                    argument_identity: application.argument_identity,
                });
            }
        }
        if rewrites.is_empty() {
            break;
        }
        if !instantiations.is_empty() {
            let snapshot = syntax.clone();
            for instance in &instantiations {
                synthesize_instance(
                    syntax,
                    &snapshot,
                    warnings,
                    &mut synthesized,
                    &generic_data,
                    &attached_machines,
                    selection,
                    instance,
                )?;
            }
        }

        for rewrite in rewrites {
            let original = syntax_type_references(syntax)
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

    let declarations = work.declarations(syntax);
    relabel_closed_data_uses_in_constants(syntax, &synthesized, selection, &declarations);
    relabel_closed_data_uses_in_exact_assignments(syntax, &synthesized, selection, &declarations);
    relabel_closed_data_uses_in_exact_calls_and_returns(
        syntax,
        &synthesized,
        selection,
        &declarations,
    );
    relabel_closed_sum_memberships_from_local_types(syntax, &synthesized, selection, &declarations);

    normalize_generic_template_const_expressions(syntax, selection, warnings, &declarations)
        .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}

/*
    Build one closed instance of a generic data declaration, unless an
    earlier round already built the same template and argument tuple: the
    template's members, facts and attached machines with its type and
    `const` parameters substituted, as a new concrete record.
*/
fn synthesize_instance(
    syntax: &mut SyntaxTrees,
    snapshot: &SyntaxTrees,
    warnings: &mut Vec<Diagnostic>,
    synthesized: &mut Vec<Instantiation>,
    generic_data: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
    attached_machines: &HashMap<syntax_trees::item::ItemHandle, Vec<usize>>,
    selection: Option<&ConstantSelection>,
    instance: &Instantiation,
) -> Result<(), Vec<Diagnostic>> {
    if synthesized.iter().any(|prior| {
        prior.template == instance.template && prior.argument_identity == instance.argument_identity
    }) {
        return Ok(());
    }
    let base_info = &generic_data[&instance.template];
    let substitution: HashMap<String, TypeReferenceHandle> = base_info
        .parameter_names
        .iter()
        .cloned()
        .zip(instance.argument_handles.iter().copied())
        .collect();
    let const_parameter_values: HashMap<String, BigInt> = base_info
        .parameter_names
        .iter()
        .zip(&base_info.const_parameter_types)
        .filter_map(|(name, parameter_type)| {
            parameter_type.as_ref()?;
            let argument = substitution.get(name)?;
            let value = syntax_type_references(syntax)
                .type_reference(*argument)
                .as_named()?;
            Some((name.clone(), value.as_str(syntax).parse().ok()?))
        })
        .collect();
    let const_parameter_types = base_info
        .parameter_names
        .iter()
        .zip(&base_info.const_parameter_types)
        .filter_map(|(name, parameter_type)| {
            let carrier = crate::resolution::declarations::generic_data::const_integer_type(
                syntax,
                (*parameter_type)?,
                selection,
            )?;
            Some((name.clone(), carrier))
        })
        .collect::<HashMap<_, _>>();
    let mut const_expressions: HashMap<String, ExpressionNode> = const_parameter_values
        .iter()
        .map(|(name, value)| {
            use numeric_literals::IntegerLanding;
            let Some(landed_type) = const_parameter_types.get(name).copied() else {
                return Err(vec![Diagnostic::error(format!(
                    "const parameter `{name}` has no supported integer carrier for substitution"
                ))]);
            };
            let literal = IntegerLiteral::from_parts(
                value.is_negative(),
                IntegerRadix::Decimal,
                value.abs().to_string().as_str(),
            )
            .expect("a concrete const argument is a valid decimal integer literal")
            .with_landing(IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            });
            Ok((name.clone(), ExpressionNode::Integer(literal)))
        })
        .collect::<Result<_, Vec<Diagnostic>>>()?;
    let mut const_parameter_scalars = const_parameter_values
        .iter()
        .map(|(name, value)| {
            let integer_type = const_parameter_types.get(name).copied().ok_or_else(|| {
                vec![Diagnostic::error(
                    "const binding lost its selected integer carrier",
                )]
            })?;
            Ok((
                name.clone(),
                ConstScalarValue::DeclaredInteger {
                    value: value.clone(),
                    integer_type,
                },
            ))
        })
        .collect::<Result<HashMap<_, _>, Vec<Diagnostic>>>()?;
    for (name, parameter_type) in base_info
        .parameter_names
        .iter()
        .zip(&base_info.const_parameter_types)
    {
        if parameter_type.is_none() {
            continue;
        }
        let Some(argument) = substitution.get(name) else {
            continue;
        };
        let Some(value) = syntax.type_references.type_reference(*argument).as_named() else {
            continue;
        };
        if let Some(value) = normalized_boolean_argument(value.as_str(syntax)) {
            const_parameter_scalars.insert(name.clone(), ConstScalarValue::Boolean(value));
            const_expressions.insert(name.clone(), ExpressionNode::Boolean(value));
        }
    }

    let type_identities: HashMap<String, ClosedArgumentIdentity> = {
        let Some(template) = snapshot.root_item(instance.template).as_data() else {
            unreachable!("a discovered template is a data item");
        };
        syntax_items(snapshot)
            .type_parameters(template.type_parameters)
            .iter()
            .zip(instance.argument_identity.iter())
            .filter(|(parameter, _)| matches!(parameter.kind, TypeParameterKind::Type))
            .map(|(parameter, identity)| {
                (parameter.name.as_str(syntax).to_owned(), identity.clone())
            })
            .collect()
    };

    let type_watermark = syntax.type_references.node_count();
    let fact_expression_watermark = syntax.expressions.frontier();
    let template_variant_names: Vec<String> = syntax_items(snapshot)
        .data_members(base_info.members)
        .iter()
        .filter_map(|member| match member {
            DataMember::Variant(variant) => Some(variant.name.as_str(snapshot).to_owned()),
            _ => None,
        })
        .collect();
    let mut impossible_variants: HashSet<String> = HashSet::new();
    let mut copied_fact_roots: Vec<ExpressionHandle> = Vec::new();
    let mut first_fact = Handle::invalid();
    let mut fact_count = 0u32;
    for (offset, fact) in syntax_items(snapshot)
        .proof_facts(base_info.requires_facts)
        .iter()
        .enumerate()
    {
        if base_info
            .type_equations
            .iter()
            .any(|equation| equation.fact_offset == offset)
        {
            continue;
        }
        let fact_warning_start = warnings.len();
        let const_result = match fact {
            ProofFact::Expression(expression) => evaluate_const_fact_expression(
                snapshot,
                *expression,
                &const_parameter_scalars,
                selection,
                warnings,
            )
            .map(|value| match value {
                Some(ConstFactValue::Boolean(value)) => Some(value),
                _ => None,
            }),
            ProofFact::Membership(membership) => evaluate_const_membership_fact(
                snapshot,
                membership,
                &const_parameter_values,
                &const_parameter_types,
                selection,
                warnings,
            ),
        }
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "const fact for generic instance `{}` is invalid: {reason}",
                instance.synthetic_name
            ))]
        })?;
        if const_result == Some(true) {
            continue;
        }
        warnings.truncate(fact_warning_start);
        let source = Handle::from_parts(
            base_info
                .requires_facts
                .start()
                .arena_index()
                .checked_add(u32::try_from(offset).expect("proof fact offset overflow"))
                .expect("proof fact source handle overflow"),
            base_info.requires_facts.start().generation(),
        );
        let handle = syntax.copy_proof_fact_from(snapshot, source);
        let copied_root = match syntax.items.proof_fact(handle) {
            ProofFact::Expression(root) => Some(*root),
            _ => None,
        };
        if let Some(copied_root) = copied_root {
            super::substitution::decide_instance_type_equalities(
                syntax,
                copied_root,
                &type_identities,
                selection,
                &instance.synthetic_name,
            )?;
            copied_fact_roots.push(copied_root);
            let type_name = base_info.origin_name.as_str(syntax);
            for variant_name in &template_variant_names {
                if matches!(
                    variant_fact_verdict(syntax, copied_root, type_name, variant_name,),
                    super::substitution::VariantFactVerdict::Refuted
                ) {
                    impossible_variants.insert(variant_name.clone());
                }
            }
        }
        if fact_count == 0 {
            first_fact = handle;
        }
        fact_count += 1;
    }
    if !impossible_variants.is_empty() {
        let type_name = base_info.origin_name.as_str(syntax).to_owned();
        for copied_root in copied_fact_roots {
            fold_impossible_case_memberships(syntax, copied_root, &type_name, &impossible_variants);
        }
    }
    let requires_facts = HandleSpan::from_parts(first_fact, fact_count);

    let origin_arguments = syntax
        .tables
        .type_references
        .insert_type_reference_handles(instance.argument_handles.iter().copied());
    let generic_instance = syntax
        .tables
        .type_references
        .insert(TypeReferenceNode::Generic {
            base_name: base_info.origin_name.clone(),
            lifetime_arguments: Vec::new(),
            arguments: origin_arguments,
        });

    let members: Vec<DataMember> = syntax.tables.items.data_members(base_info.members).to_vec();
    let properties = base_info.properties;
    let mut first: Handle<DataMember> = Handle::invalid();
    let mut count = 0u32;
    for member in members {
        let substituted = substitute_member(
            syntax,
            snapshot,
            member,
            &substitution,
            &impossible_variants,
            selection,
            warnings,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let handle = syntax.tables.items.append_data_member(substituted);
        if count == 0 {
            first = handle;
        }
        count += 1;
    }
    let copied_types = syntax_type_references(syntax).named_nodes_from(type_watermark, syntax);
    substitute_copied_type_names(syntax, copied_types, &substitution);
    replace_const_expression_names_from(syntax, fact_expression_watermark, &const_expressions);
    let cleanup_selection = base_info.cleanup_selection.clone().map(|cleanup| {
        super::substitution::substitute_cleanup_selection(
            syntax,
            snapshot,
            cleanup,
            &substitution,
            selection,
            warnings,
        )
    });
    let declaration = syntax.push_root_item(Item::Data(DataDefinition {
        name: Identifier::new(
            instance.synthetic_name.as_str(),
            base_info.origin_name.source_span(),
        ),
        is_public: base_info.is_public,
        supply_mode: base_info.supply_mode,
        lifetime_parameters: base_info.lifetime_parameters.clone(),
        type_parameters: HandleSpan::default(),
        conformance_bounds: Vec::new(),
        generic_instance: Some(generic_instance),
        cleanup_selection,
        properties,
        requires_facts,
        members: HandleSpan::from_parts(first, count),
        quotient: None,
    }));

    let mut materialized = instance.clone();
    materialized.declaration = declaration;
    synthesized.push(materialized);

    let Some(machine_items) = attached_machines.get(&instance.template) else {
        return Ok(());
    };
    for &item_index in machine_items {
        let Some(Item::Machine(machine)) = snapshot.root_items().nth(item_index) else {
            continue;
        };
        let type_watermark = syntax.tables.type_references.node_count();
        let expression_watermark = syntax.expressions.frontier();
        let Item::Machine(mut clone) =
            syntax.copy_item_from(snapshot, &Item::Machine(machine.clone()))
        else {
            continue;
        };
        let method_tail = machine
            .name
            .as_str(syntax)
            .rsplit("::")
            .next()
            .unwrap_or(machine.name.as_str(syntax))
            .to_string();
        clone.name = Identifier::new(
            format!("{}::{}", instance.synthetic_name, method_tail),
            machine.name.source_span(),
        );
        clone.attached_data = Some(
            closed_constructor_carrier(
                snapshot,
                selection,
                instance,
                machine.attached_data.as_ref().expect("selected attachment"),
                false,
            )
            .ok_or_else(|| {
                vec![
                    Diagnostic::error(
                        "generic method lost its selected closed carrier lookup context",
                    )
                    .with_source_span(machine.name.source_span()),
                ]
            })?,
        );
        clone.generic_data_template = machine.name.clone();
        clone.type_parameters = HandleSpan::default();
        let runtime_captures = capture_machine_runtime_template_names(
            syntax,
            &clone,
            type_watermark,
            expression_watermark,
        );
        let copied_types = syntax_type_references(syntax)
            .named_nodes_from(type_watermark, syntax)
            .into_iter()
            .filter(|(handle, name)| {
                !const_expressions.contains_key(name)
                    || !runtime_captures.captures_type_reference(*handle)
            });
        substitute_copied_type_names(syntax, copied_types, &substitution);
        for (handle, element_type, name) in
            syntax_type_references(syntax).const_parameter_array_nodes_from(type_watermark, syntax)
        {
            let Some(length) = substitution.get(&name).and_then(|argument| {
                match syntax.tables.type_references.type_reference(*argument) {
                    TypeReferenceNode::Named(value) => value.as_str(syntax).parse::<usize>().ok(),
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
        replace_machine_const_expression_names_from(
            syntax,
            expression_watermark,
            &const_expressions,
            &runtime_captures,
        );
        syntax.push_root_item(Item::Machine(clone));
    }
    Ok(())
}
