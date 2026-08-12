//! Closed generic-data synthesis before resolution. A spelled `Box<i32>` is
//! rewritten to a concrete nominal `data Box<i32> { value: i32 }`, so every
//! downstream semantic and native phase sees one exact closed definition
//! rather than a generic definition plus ambient instance bindings.
//!
//! The executable cohort includes fully substitutable records and pure sums.
//! Records may have multiple distinct closed instances. A pure sum admits one
//! distinct closed instance per generic base until every constructor and
//! pattern occurrence carries its closed identity directly; a second distinct
//! instance rejects instead of falling through to a legacy layout path.
//! Sluggable arguments are a plain concrete `Named` type OR a
//! `Named` carrying only nameable domain constraints (`Box<i32 in Wrapping>`,
//! `Store<u8 in Utf8>`) -- the substitution rides the argument's own type
//! reference, so the domain follows the field for free. What it skips: mixed
//! record/case data, genuinely composite ARGUMENTS (`Box<[i32; 4]>`,
//! `Box<&T>`, a range-bounded arg), and a field that nests the parameter under a
//! NON-generic composite (`[T; N]`, `&T`). A field nesting the parameter under
//! ANOTHER generic (`Pair<T> { a: Box<T> }`) IS handled (Phase 3): the desugar
//! runs to a FIXPOINT, synthesizing the concrete `Box<i32>` a `Pair<i32>`
//! produces. Scans every TYPE-REFERENCE position a generic-data spelling reaches:
//! data FIELDS plus machine-body `let`-local, state PARAMETER, and RETURN type
//! annotations; generic TEMPLATE bodies (defs/machines with type params) are
//! skipped so their param-arg spellings are not mistaken for concrete instances.

use psi_arena::{Handle, HandleSpan};
use psi_diagnostics::Diagnostic;
use psi_language_semantics::const_value::CanonicalConstValue;
use psi_numerics::literals::{IntegerLiteral, IntegerRadix};
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{
    ConstDefinition, DataDefinition, DataMember, Item, ProofFact, TypeParameterKind,
};
use psi_syntax_trees::statement::StatementNode;
use psi_syntax_trees::types::{
    FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use std::collections::{HashMap, HashSet};

struct GenericData {
    lifetime_parameters: Vec<Identifier>,
    parameter_names: Vec<String>,
    const_parameter_types: Vec<Option<TypeReferenceHandle>>,
    where_facts: HandleSpan<ProofFact>,
    members: HandleSpan<DataMember>,
    properties: psi_syntax_trees::item::DataProperties,
    supply_mode: psi_language_semantics::DataSupplyMode,
}

struct PendingRewrite {
    type_reference: TypeReferenceHandle,
    synthetic_name: String,
    lifetime_arguments: Vec<Identifier>,
}

/// One discovered instantiation: the base generic definition and the argument
/// type references spelled for it, plus the plain name of the record to
/// synthesize.
struct Instantiation {
    synthetic_name: String,
    base_name: String,
    argument_handles: Vec<TypeReferenceHandle>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GenericDataShape {
    Record,
    PureSum,
}

/// Find `Base<Args..>` spellings in FIELD type position where `Base` is a
/// generic data definition, synthesize one concrete instance record per
/// distinct spelling (the parameter substituted for the argument), and rewrite
/// the field spellings to the instances' plain names.
pub(crate) fn desugar_generic_data_instances(
    syntax: &mut SyntaxTrees,
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

    canonicalize_closed_domain_indices(syntax, &const_definitions, &const_values)
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
    let mut synthesized_sum_instances: HashMap<String, String> = HashMap::new();
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
            )
            .map_err(|diagnostic| vec![diagnostic])?;
        }
        if rewrites.is_empty() {
            break; // no more monomorphizable generic spellings
        }
        for instance in &instantiations {
            let base_info = &generic_data[&instance.base_name];
            if generic_data_shape(syntax, base_info) != Some(GenericDataShape::PureSum) {
                continue;
            }
            if let Some(existing) = synthesized_sum_instances.get(&instance.base_name)
                && existing != &instance.synthetic_name
            {
                return Err(vec![Diagnostic::error(format!(
                    "generic sum `{}` is used with distinct closed instances `{existing}` and `{}`; this executable slice requires one exact closed instance per generic sum",
                    instance.base_name, instance.synthetic_name
                ))]);
            }
            synthesized_sum_instances
                .insert(instance.base_name.clone(), instance.synthetic_name.clone());
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
            for fact in snapshot.tables.items.proof_facts(base_info.where_facts) {
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
                let copied = syntax.copy_proof_fact_from(&snapshot, fact);
                let handle = syntax.tables.items.append_proof_fact(copied);
                if fact_count == 0 {
                    first_fact = handle;
                }
                fact_count += 1;
            }
            replace_const_expression_names_from(syntax, fact_expression_watermark, &const_literals);
            let where_facts = HandleSpan::from_parts(first_fact, fact_count);

            let members: Vec<DataMember> =
                syntax.tables.items.data_members(base_info.members).to_vec();
            let properties = base_info.properties;
            let mut first: Handle<DataMember> = Handle::invalid();
            let mut count = 0u32;
            for member in members {
                let substituted = substitute_member(syntax, member, &substitution, &const_values);
                let handle = syntax.tables.items.append_data_member(substituted);
                if count == 0 {
                    first = handle;
                }
                count += 1;
            }
            syntax.push_root_item(Item::Data(DataDefinition {
                name: Identifier::generated(instance.synthetic_name.as_str()),
                supply_mode: base_info.supply_mode,
                lifetime_parameters: base_info.lifetime_parameters.clone(),
                type_parameters: HandleSpan::default(),
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
                clone.name =
                    Identifier::generated(format!("{}::{}", instance.synthetic_name, method_tail));
                clone.attached_data = Some(Identifier::generated(instance.synthetic_name.as_str()));
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
            let rewritten = if rewrite.lifetime_arguments.is_empty() {
                TypeReferenceNode::Named(Identifier::generated(rewrite.synthetic_name))
            } else {
                TypeReferenceNode::Generic {
                    base_name: Identifier::generated(rewrite.synthetic_name),
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
    relabel_unique_closed_sum_paths(syntax, &synthesized_sum_instances);

    normalize_generic_template_const_expressions(syntax, &const_values)
        .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}

/// Give a bare generic record literal the exact closed instance selected by an
/// explicitly typed local. This is contextual elaboration, not inference: only
/// `let value: Box<i32> = Box { ... }` (and record literals nested beneath that
/// known destination shape) are rewritten. Calls, returns, assignments, generic
/// sums, and literals without an annotated local destination remain untouched.
fn relabel_closed_data_uses_in_annotated_locals(
    syntax: &mut SyntaxTrees,
    synthesized_origins: &HashMap<String, String>,
) {
    let locals = syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Machine(machine) if machine.type_parameters.is_empty() => Some(machine),
            _ => None,
        })
        .flat_map(|machine| syntax.tables.items.state_handles(machine.states))
        .flat_map(|state| {
            let state = syntax.tables.items.state(*state);
            syntax.tables.items.statements(state.statements)
        })
        .filter_map(
            |statement| match syntax.tables.statements.statement(*statement) {
                StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                    Some((local.type_reference, local.initial_value))
                }
                _ => None,
            },
        )
        .collect::<Vec<_>>();

    for (expected_type, expression) in locals {
        relabel_data_literal_for_expected_type(
            syntax,
            expression,
            expected_type,
            synthesized_origins,
        );
    }
}

fn relabel_data_literal_for_expected_type(
    syntax: &mut SyntaxTrees,
    expression: ExpressionHandle,
    expected_type: TypeReferenceHandle,
    synthesized_origins: &HashMap<String, String>,
) {
    let expected_type = match syntax
        .tables
        .type_references
        .type_reference(expected_type)
        .clone()
    {
        TypeReferenceNode::Constrained { base_type, .. } => base_type,
        TypeReferenceNode::Named(_) => expected_type,
        _ => return,
    };
    let TypeReferenceNode::Named(expected_name) = syntax
        .tables
        .type_references
        .type_reference(expected_type)
        .clone()
    else {
        return;
    };
    let Some(definition) = syntax.root_items().find_map(|item| match item {
        Item::Data(definition) if definition.name.as_str() == expected_name.as_str() => {
            Some(definition.clone())
        }
        _ => None,
    }) else {
        return;
    };
    let ExpressionNode::StructLiteral(mut literal) =
        syntax.expressions.expression(expression).clone()
    else {
        return;
    };

    let literal_names_expected = literal.type_name.as_str() == expected_name.as_str();
    let literal_names_generic_origin = synthesized_origins
        .get(expected_name.as_str())
        .is_some_and(|base| literal.type_name.as_str() == base.as_str());
    if !literal_names_expected && !literal_names_generic_origin {
        return;
    }
    if literal_names_generic_origin {
        literal.type_name = Identifier::generated(expected_name.as_str());
        syntax
            .expressions
            .replace_expression(expression, ExpressionNode::StructLiteral(literal.clone()));
    }

    let mut declared_fields = syntax
        .tables
        .items
        .data_members(definition.members)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => {
                Some((field.name.as_str().to_owned(), field.type_reference))
            }
            DataMember::Variant(_) | DataMember::Retired(_) => None,
        })
        .collect::<Vec<_>>();
    if let Some(case_name) = literal.case_name.as_ref()
        && let Some(variant) = syntax
            .tables
            .items
            .data_members(definition.members)
            .iter()
            .find_map(|member| match member {
                DataMember::Variant(variant) if variant.name.as_str() == case_name.as_str() => {
                    Some(variant)
                }
                _ => None,
            })
    {
        declared_fields.extend(
            syntax
                .tables
                .items
                .data_payload_fields(variant.payload)
                .iter()
                .map(|field| (field.name.as_str().to_owned(), field.type_reference)),
        );
    }
    let authored = syntax.expressions.struct_fields(literal.fields).to_vec();
    for field in authored {
        let Some((_, field_type)) = declared_fields
            .iter()
            .find(|(name, _)| name == field.name.as_str())
        else {
            continue;
        };
        relabel_data_literal_for_expected_type(
            syntax,
            field.value,
            *field_type,
            synthesized_origins,
        );
    }
}

/// A pure generic sum is admitted with exactly one closed instance per base in
/// this slice. That makes every remaining `Base::Case` constructor and pattern
/// path unambiguous: rewrite it to the synthesized nominal identity before
/// symbol resolution. Generic template bodies are excluded because their case
/// paths remain parameterized declarations, not concrete uses.
fn relabel_unique_closed_sum_paths(
    syntax: &mut SyntaxTrees,
    synthesized_sum_instances: &HashMap<String, String>,
) {
    if synthesized_sum_instances.is_empty() {
        return;
    }

    let concrete_expressions = concrete_machine_expression_handles(syntax);
    let variants = generic_sum_variant_names(syntax, synthesized_sum_instances);
    let replacements = syntax
        .expressions
        .iter_expressions()
        .filter(|(handle, _)| concrete_expressions.contains(&handle.arena_index()))
        .filter_map(|(handle, expression)| {
            let (path, kind) = match expression {
                ExpressionNode::Name(path) => (*path, SumPathExpressionKind::Name),
                ExpressionNode::Membership(membership) => (
                    membership.domain,
                    SumPathExpressionKind::Membership(membership.value),
                ),
                ExpressionNode::StructLiteral(literal) if literal.case_name.is_some() => {
                    let case = literal.case_name.as_ref().expect("case literal");
                    if !variants
                        .get(literal.type_name.as_str())
                        .is_some_and(|names| names.contains(case.as_str()))
                    {
                        return None;
                    }
                    let closed = synthesized_sum_instances.get(literal.type_name.as_str())?;
                    return Some((
                        handle,
                        SumPathExpressionKind::StructLiteral(literal.clone()),
                        closed.clone(),
                        case.clone(),
                    ));
                }
                _ => return None,
            };
            let [base, case] = syntax.expressions.identifier_path_members(path) else {
                return None;
            };
            let closed = synthesized_sum_instances.get(base.as_str())?;
            if !variants
                .get(base.as_str())
                .is_some_and(|names| names.contains(case.as_str()))
            {
                return None;
            }
            Some((handle, kind, closed.clone(), case.clone()))
        })
        .collect::<Vec<_>>();

    for (handle, kind, closed, case) in replacements {
        let replacement = match kind {
            SumPathExpressionKind::Name => {
                ExpressionNode::Name(closed_sum_path(syntax, &closed, case))
            }
            SumPathExpressionKind::Membership(value) => ExpressionNode::Membership(
                psi_syntax_trees::expression::TableMembershipExpression {
                    value,
                    domain: closed_sum_path(syntax, &closed, case),
                },
            ),
            SumPathExpressionKind::StructLiteral(mut literal) => {
                literal.type_name = Identifier::generated(closed);
                ExpressionNode::StructLiteral(literal)
            }
        };
        syntax.expressions.replace_expression(handle, replacement);
    }
}

fn generic_sum_variant_names(
    syntax: &SyntaxTrees,
    instances: &HashMap<String, String>,
) -> HashMap<String, HashSet<String>> {
    syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Data(definition) if instances.contains_key(definition.name.as_str()) => Some((
                definition.name.as_str().to_owned(),
                syntax
                    .items
                    .data_members(definition.members)
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Variant(variant) => Some(variant.name.as_str().to_owned()),
                        _ => None,
                    })
                    .collect(),
            )),
            _ => None,
        })
        .collect()
}

fn closed_sum_path(
    syntax: &mut SyntaxTrees,
    closed: &str,
    case: Identifier,
) -> HandleSpan<Identifier> {
    let mut path = HandleSpan::empty();
    syntax
        .expressions
        .append_identifier_path_member_to_span(&mut path, Identifier::generated(closed));
    syntax
        .expressions
        .append_identifier_path_member_to_span(&mut path, case);
    path
}

#[derive(Clone)]
enum SumPathExpressionKind {
    Name,
    Membership(ExpressionHandle),
    StructLiteral(psi_syntax_trees::expression::TableStructLiteral),
}

fn concrete_machine_expression_handles(syntax: &SyntaxTrees) -> HashSet<u32> {
    let mut handles = HashSet::new();
    for item in syntax.root_items() {
        let Item::Machine(machine) = item else {
            continue;
        };
        if !machine.type_parameters.is_empty()
            || machine.attached_data.as_ref().is_some_and(|attached| {
                syntax.root_items().any(|item| {
                    matches!(item, Item::Data(definition)
                        if definition.name == *attached && !definition.type_parameters.is_empty())
                })
            })
        {
            continue;
        }
        for state in syntax.tables.items.state_handles(machine.states) {
            let state = syntax.tables.items.state(*state);
            for statement in syntax.tables.items.statements(state.statements) {
                collect_statement_expression_handles(syntax, *statement, &mut handles);
            }
        }
    }
    handles
}

fn collect_statement_expression_handles(
    syntax: &SyntaxTrees,
    statement: psi_syntax_trees::statement::StatementHandle,
    handles: &mut HashSet<u32>,
) {
    use psi_syntax_trees::statement::{TransitionGuardNode, TransitionTargetNode};
    match syntax.tables.statements.statement(statement) {
        StatementNode::AssemblyFact(fact) => {
            collect_expression_handles(syntax, fact.expression, handles)
        }
        StatementNode::Assignment(assignment) => {
            collect_expression_handles(syntax, assignment.target, handles);
            collect_expression_handles(syntax, assignment.value, handles);
        }
        StatementNode::Call(call) => {
            for argument in syntax.tables.statements.expression_handles(call.arguments) {
                collect_expression_handles(syntax, *argument, handles);
            }
        }
        StatementNode::Expression(expression) => {
            collect_expression_handles(syntax, *expression, handles)
        }
        StatementNode::LocalData(local) => {
            collect_expression_handles(syntax, local.initial_value, handles)
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                collect_expression_handles(syntax, guard, handles);
            }
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match syntax.tables.statements.transition_target(target) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        for argument in syntax.tables.statements.expression_handles(*arguments) {
                            collect_expression_handles(syntax, *argument, handles);
                        }
                    }
                    TransitionTargetNode::Value(value) => {
                        collect_expression_handles(syntax, *value, handles)
                    }
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

fn collect_expression_handles(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    handles: &mut HashSet<u32>,
) {
    if !expression.is_valid() || !handles.insert(expression.arena_index()) {
        return;
    }
    match syntax.expressions.expression(expression) {
        ExpressionNode::ArrayLiteral(expressions) => {
            for expression in syntax.expressions.expression_handles(*expressions) {
                collect_expression_handles(syntax, *expression, handles);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            collect_expression_handles(syntax, atomic.value, handles);
            collect_expression_handles(syntax, atomic.result, handles);
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_handles(syntax, binary.left, handles);
            collect_expression_handles(syntax, binary.right, handles);
        }
        ExpressionNode::Cast(cast) => collect_expression_handles(syntax, cast.value, handles),
        ExpressionNode::Call(call) => {
            collect_expression_handles(syntax, call.receiver, handles);
            for argument in syntax.expressions.expression_handles(call.arguments) {
                collect_expression_handles(syntax, *argument, handles);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            collect_expression_handles(syntax, indexed.collection, handles);
            collect_expression_handles(syntax, indexed.index, handles);
        }
        ExpressionNode::Membership(membership) => {
            collect_expression_handles(syntax, membership.value, handles)
        }
        ExpressionNode::Member(member) => {
            collect_expression_handles(syntax, member.receiver, handles)
        }
        ExpressionNode::Mutable(inner) => collect_expression_handles(syntax, *inner, handles),
        ExpressionNode::Range(range) => {
            collect_expression_handles(syntax, range.start, handles);
            collect_expression_handles(syntax, range.end, handles);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in syntax.expressions.struct_fields(literal.fields) {
                collect_expression_handles(syntax, field.value, handles);
            }
        }
        ExpressionNode::Unary(unary) => collect_expression_handles(syntax, unary.operand, handles),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::SelfValue
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

#[derive(Clone, Copy)]
enum ConstFactValue {
    Integer(i128),
    Boolean(bool),
}

/// Evaluate a proof expression exactly when every operand is known at generic
/// instantiation time. `None` means the fact still depends on a runtime field
/// and must remain on the synthesized record.
fn evaluate_const_fact_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, i128>,
    self_value: Option<i128>,
) -> Result<Option<ConstFactValue>, String> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Integer(value) => integer_literal_value(value)
            .map(ConstFactValue::Integer)
            .map(Some)
            .ok_or_else(|| {
                "integer operand must fit the signed/unsigned 64-bit envelope".to_string()
            }),
        ExpressionNode::Boolean(value) => Ok(Some(ConstFactValue::Boolean(*value))),
        ExpressionNode::Name(path) => {
            let name = syntax
                .expressions
                .identifier_path_members(*path)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            Ok(parameter_values
                .get(&name)
                .or_else(|| const_values.get(&name))
                .copied()
                .map(ConstFactValue::Integer))
        }
        ExpressionNode::SelfValue => Ok(self_value.map(ConstFactValue::Integer)),
        ExpressionNode::Binary(binary) => {
            let Some(left) = evaluate_const_fact_expression(
                syntax,
                binary.left,
                const_values,
                parameter_values,
                self_value,
            )?
            else {
                return Ok(None);
            };
            let Some(right) = evaluate_const_fact_expression(
                syntax,
                binary.right,
                const_values,
                parameter_values,
                self_value,
            )?
            else {
                return Ok(None);
            };
            evaluate_const_fact_binary(binary.operator, left, right).map(Some)
        }
        _ => Ok(None),
    }
}

/// Discharge `N in Domain` when `N` is a concrete const parameter and the
/// domain is defined by evaluable boolean facts over `self`. Machine-call facts
/// stay on the concrete record for typed build-time evaluation.
fn evaluate_const_membership_fact(
    syntax: &SyntaxTrees,
    membership: &psi_syntax_trees::item::ProofMembershipFact,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, i128>,
    parameter_type_names: &HashMap<String, String>,
) -> Result<Option<bool>, String> {
    let ExpressionNode::Name(value_path) = syntax.expressions.expression(membership.value) else {
        return Ok(None);
    };
    let [parameter_name] = syntax.expressions.identifier_path_members(*value_path) else {
        return Ok(None);
    };
    let Some(value) = parameter_values.get(parameter_name.as_str()).copied() else {
        return Ok(None);
    };
    let Some(parameter_type) = parameter_type_names.get(parameter_name.as_str()) else {
        return Ok(None);
    };
    let domain_path = syntax
        .items
        .identifier_path_members(membership.domain)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let domain_name = if domain_path.contains("::") {
        domain_path
    } else {
        format!("{parameter_type}::{domain_path}")
    };
    evaluate_named_const_domain(
        syntax,
        &domain_name,
        parameter_type,
        value,
        const_values,
        &mut Vec::new(),
    )
}

fn evaluate_named_const_domain(
    syntax: &SyntaxTrees,
    domain_name: &str,
    carrier: &str,
    value: i128,
    const_values: &HashMap<String, i128>,
    visiting: &mut Vec<String>,
) -> Result<Option<bool>, String> {
    if visiting.iter().any(|name| name == domain_name) {
        return Ok(None);
    }
    let Some(domain) = syntax.root_items().find_map(|item| {
        let Item::Domain(domain) = item else {
            return None;
        };
        (domain.name.as_str() == domain_name).then_some(domain)
    }) else {
        return Ok(None);
    };
    let TypeReferenceNode::Named(domain_target) =
        syntax.type_references.type_reference(domain.target_type)
    else {
        return Ok(None);
    };
    if domain_target.as_str() != carrier {
        return Err(format!(
            "domain `{domain_name}` has carrier `{}`, but the const value has carrier `{carrier}`",
            domain_target.as_str(),
        ));
    }
    visiting.push(domain_name.to_owned());
    for fact in syntax.items.proof_facts(domain.facts) {
        let holds = match fact {
            ProofFact::Expression(expression) => evaluate_const_domain_expression(
                syntax,
                *expression,
                const_values,
                value,
                carrier,
                visiting,
            )?,
            ProofFact::Membership(membership) => {
                let Some(ConstFactValue::Integer(nested_value)) = evaluate_const_fact_expression(
                    syntax,
                    membership.value,
                    const_values,
                    &HashMap::new(),
                    Some(value),
                )?
                else {
                    visiting.pop();
                    return Ok(None);
                };
                let path = syntax
                    .items
                    .identifier_path_members(membership.domain)
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::");
                let nested_domain = if path.contains("::") {
                    path
                } else {
                    format!("{carrier}::{path}")
                };
                evaluate_named_const_domain(
                    syntax,
                    &nested_domain,
                    carrier,
                    nested_value,
                    const_values,
                    visiting,
                )?
                .map(ConstFactValue::Boolean)
            }
        };
        let Some(ConstFactValue::Boolean(holds)) = holds else {
            visiting.pop();
            return Ok(None);
        };
        if !holds {
            visiting.pop();
            return Ok(Some(false));
        }
    }
    visiting.pop();
    Ok(Some(true))
}

fn evaluate_const_domain_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, i128>,
    self_value: i128,
    carrier: &str,
    visiting: &mut Vec<String>,
) -> Result<Option<ConstFactValue>, String> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Membership(membership) => {
            let Some(ConstFactValue::Integer(value)) = evaluate_const_fact_expression(
                syntax,
                membership.value,
                const_values,
                &HashMap::new(),
                Some(self_value),
            )?
            else {
                return Ok(None);
            };
            let path = syntax
                .expressions
                .identifier_path_members(membership.domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            let domain_name = if path.contains("::") {
                path
            } else {
                format!("{carrier}::{path}")
            };
            evaluate_named_const_domain(
                syntax,
                &domain_name,
                carrier,
                value,
                const_values,
                visiting,
            )
            .map(|result| result.map(ConstFactValue::Boolean))
        }
        ExpressionNode::Binary(binary) => {
            let Some(left) = evaluate_const_domain_expression(
                syntax,
                binary.left,
                const_values,
                self_value,
                carrier,
                visiting,
            )?
            else {
                return Ok(None);
            };
            let Some(right) = evaluate_const_domain_expression(
                syntax,
                binary.right,
                const_values,
                self_value,
                carrier,
                visiting,
            )?
            else {
                return Ok(None);
            };
            evaluate_const_fact_binary(binary.operator, left, right).map(Some)
        }
        _ => evaluate_const_fact_expression(
            syntax,
            expression,
            const_values,
            &HashMap::new(),
            Some(self_value),
        ),
    }
}

fn evaluate_const_fact_binary(
    operator: BinaryOperator,
    left: ConstFactValue,
    right: ConstFactValue,
) -> Result<ConstFactValue, String> {
    use BinaryOperator::*;
    match (left, right) {
        (ConstFactValue::Integer(left), ConstFactValue::Integer(right)) => match operator {
            Add => checked_fact_integer(left.checked_add(right), "addition"),
            Subtract => checked_fact_integer(left.checked_sub(right), "subtraction"),
            Multiply => checked_fact_integer(left.checked_mul(right), "multiplication"),
            Divide => left
                .checked_div(right)
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "division by zero is invalid".to_string()),
            Modulo => left
                .checked_rem(right)
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "remainder by zero is invalid".to_string()),
            ShiftLeft if left >= 0 => u32::try_from(right)
                .ok()
                .filter(|amount| *amount < u64::BITS)
                .and_then(|amount| left.checked_shl(amount))
                .and_then(const_integer_in_envelope)
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "left shift exceeds the `u64` width".to_string()),
            ShiftRight if left >= 0 => u32::try_from(right)
                .ok()
                .filter(|amount| *amount < u64::BITS)
                .and_then(|amount| left.checked_shr(amount))
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "right shift exceeds the `u64` width".to_string()),
            BitwiseAnd if left >= 0 && right >= 0 => Ok(ConstFactValue::Integer(left & right)),
            BitwiseOr if left >= 0 && right >= 0 => Ok(ConstFactValue::Integer(left | right)),
            BitwiseXor if left >= 0 && right >= 0 => Ok(ConstFactValue::Integer(left ^ right)),
            Equal => Ok(ConstFactValue::Boolean(left == right)),
            NotEqual => Ok(ConstFactValue::Boolean(left != right)),
            Greater => Ok(ConstFactValue::Boolean(left > right)),
            GreaterOrEqual => Ok(ConstFactValue::Boolean(left >= right)),
            Less => Ok(ConstFactValue::Boolean(left < right)),
            LessOrEqual => Ok(ConstFactValue::Boolean(left <= right)),
            And | Or => Err("logical operators require boolean operands".to_string()),
            ShiftLeft | ShiftRight | BitwiseAnd | BitwiseOr | BitwiseXor => Err(
                "signed shifts and bitwise operators require declared-width semantics".to_string(),
            ),
        },
        (ConstFactValue::Boolean(left), ConstFactValue::Boolean(right)) => match operator {
            And => Ok(ConstFactValue::Boolean(left && right)),
            Or => Ok(ConstFactValue::Boolean(left || right)),
            Equal => Ok(ConstFactValue::Boolean(left == right)),
            NotEqual => Ok(ConstFactValue::Boolean(left != right)),
            _ => Err("arithmetic and ordering operators require integer operands".to_string()),
        },
        _ => Err("const fact operands have incompatible types".to_string()),
    }
}

fn integer_literal_value(value: &IntegerLiteral) -> Option<i128> {
    value
        .value_i64()
        .map(i128::from)
        .or_else(|| value.value_u64().map(i128::from))
}

fn qualified_const_name(definition: &ConstDefinition) -> String {
    if definition.scope.as_str().is_empty() {
        definition.name.as_str().to_owned()
    } else {
        format!(
            "{}::{}",
            definition.scope.as_str(),
            definition.name.as_str()
        )
    }
}

#[derive(Clone)]
struct ClosedDomainFamily {
    parameters: Vec<(String, TypeReferenceHandle)>,
}

/// PDI2's closed-index precursor runs beside generic-data canonicalization but
/// does not monomorphize the domain: the family remains nominal and erased.
/// Only its const arguments are rewritten to the same canonical leaves used by
/// PDI1 generic identity.
fn canonicalize_closed_domain_indices(
    syntax: &mut SyntaxTrees,
    const_definitions: &HashMap<String, ConstDefinition>,
    const_values: &HashMap<String, i128>,
) -> Result<(), Diagnostic> {
    let mut families = HashMap::<String, ClosedDomainFamily>::new();

    for item in syntax.root_items() {
        let Item::Domain(definition) = item else {
            continue;
        };
        let parameters = syntax
            .tables
            .items
            .type_parameters(definition.type_parameters);
        let header_arguments = syntax
            .tables
            .type_references
            .type_reference_handles(definition.index_arguments);

        if parameters.is_empty() {
            if !header_arguments.is_empty() {
                return Err(Diagnostic::error(format!(
                    "domain `{}` supplies index arguments but declares no generic carrier/const telescope",
                    definition.name
                )));
            }
            continue;
        }

        let Some(carrier) = parameters.first() else {
            unreachable!();
        };
        if !matches!(carrier.kind, TypeParameterKind::Type) {
            return Err(Diagnostic::error(format!(
                "indexed domain `{}` must declare its carrier type parameter first",
                definition.name
            )));
        }
        let TypeReferenceNode::Named(target) = syntax
            .tables
            .type_references
            .type_reference(definition.target_type)
        else {
            return Err(Diagnostic::error(format!(
                "indexed domain `{}` must use its carrier binder directly before `::{}`",
                definition.name, definition.name
            )));
        };
        if target.as_str() != carrier.name.as_str() {
            return Err(Diagnostic::error(format!(
                "indexed domain `{}` binds carrier `{}` but declares the family over `{target}`",
                definition.name, carrier.name
            )));
        }

        let mut const_parameters = Vec::new();
        for parameter in &parameters[1..] {
            let TypeParameterKind::Const { type_reference } = parameter.kind else {
                return Err(Diagnostic::error(format!(
                    "indexed domain `{}` may declare only one carrier type followed by proof-static const parameters",
                    definition.name
                )));
            };
            validate_const_index_type(syntax, type_reference, &mut HashSet::new()).map_err(
                |reason| {
                    Diagnostic::error(format!(
                        "indexed domain `{}::{}` has an ineligible index type: {reason}",
                        definition.name, parameter.name
                    ))
                },
            )?;
            const_parameters.push((parameter.name.as_str().to_owned(), type_reference));
        }
        if const_parameters.is_empty() {
            return Err(Diagnostic::error(format!(
                "generic domain `{}` must declare at least one proof-static const index after its carrier",
                definition.name
            )));
        }
        if header_arguments.len() != const_parameters.len() {
            return Err(Diagnostic::error(format!(
                "indexed domain `{}` declares {} const parameter(s) but selects {} index argument(s) in its family header",
                definition.name,
                const_parameters.len(),
                header_arguments.len()
            )));
        }
        for ((parameter_name, _), argument) in const_parameters.iter().zip(header_arguments) {
            let TypeReferenceNode::Named(argument_name) =
                syntax.tables.type_references.type_reference(*argument)
            else {
                return Err(Diagnostic::error(format!(
                    "indexed domain `{}` must select each const binder directly in its family header",
                    definition.name
                )));
            };
            if argument_name.as_str() != parameter_name {
                return Err(Diagnostic::error(format!(
                    "indexed domain `{}` must select const binder `{parameter_name}` in declaration order, not `{argument_name}`",
                    definition.name
                )));
            }
        }
        if families
            .insert(
                definition.name.as_str().to_owned(),
                ClosedDomainFamily {
                    parameters: const_parameters,
                },
            )
            .is_some()
        {
            return Err(Diagnostic::error(format!(
                "indexed domain family `{}` is declared more than once",
                definition.name
            )));
        }
    }

    let mut applications = syntax
        .tables
        .type_references
        .domain_constraints()
        .into_iter()
        .map(|constraint| (constraint.name.as_str().to_owned(), constraint.arguments))
        .collect::<Vec<_>>();
    applications.extend(
        syntax
            .expressions
            .iter_expressions()
            .filter_map(|(_, expression)| {
                let ExpressionNode::Cast(cast) = expression else {
                    return None;
                };
                if cast.semantic_domain.is_empty() {
                    return None;
                }
                let name = syntax
                    .expressions
                    .identifier_path_members(cast.semantic_domain)
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::");
                Some((name, cast.semantic_domain_arguments))
            }),
    );

    for (name, argument_span) in applications {
        let Some(family) = families.get(&name) else {
            continue;
        };
        let arguments = syntax
            .tables
            .type_references
            .type_reference_handles(argument_span)
            .to_vec();
        canonicalize_closed_domain_application(
            syntax,
            &name,
            family,
            arguments,
            const_definitions,
            const_values,
        )?;
    }
    Ok(())
}

fn canonicalize_closed_domain_application(
    syntax: &mut SyntaxTrees,
    family_name: &str,
    family: &ClosedDomainFamily,
    arguments: Vec<TypeReferenceHandle>,
    const_definitions: &HashMap<String, ConstDefinition>,
    const_values: &HashMap<String, i128>,
) -> Result<(), Diagnostic> {
    if arguments.len() != family.parameters.len() {
        return Err(Diagnostic::error(format!(
            "indexed domain `{}` requires {} closed const argument(s), but {} were supplied",
            family_name,
            family.parameters.len(),
            arguments.len()
        )));
    }
    for ((parameter_name, parameter_type), argument) in family.parameters.iter().zip(arguments) {
        let node = syntax
            .tables
            .type_references
            .type_reference(argument)
            .clone();
        match node {
            TypeReferenceNode::Named(name) => {
                if let Some(value) = CanonicalConstValue::from_atom(name.as_str()) {
                    let required =
                        syntax_type_identity(syntax, *parameter_type).map_err(Diagnostic::error)?;
                    if value.type_name != required {
                        return Err(Diagnostic::error(format!(
                            "index argument for `{}::{parameter_name}` has canonical type `{}`, expected `{required}`",
                            family_name, value.type_name
                        )));
                    }
                    continue;
                }
                if let Some(value) = const_values.get(name.as_str()) {
                    syntax.tables.type_references.replace_type_reference(
                        argument,
                        TypeReferenceNode::Named(Identifier::generated(value.to_string())),
                    );
                    continue;
                }
                let Some(definition) = const_definitions.get(name.as_str()) else {
                    // A direct generic const binder is resolved and checked
                    // later in its declaration context. Unknown names fail
                    // there as well; never guess that a type is a value.
                    continue;
                };
                let value = canonicalize_const_definition(syntax, definition, *parameter_type)
                    .map_err(|reason| {
                        Diagnostic::error(format!(
                            "index argument for `{}::{parameter_name}` is invalid: {reason}",
                            family_name
                        ))
                    })?;
                syntax.tables.type_references.replace_type_reference(
                    argument,
                    TypeReferenceNode::Named(Identifier::generated(value.atom())),
                );
            }
            TypeReferenceNode::ConstExpression(expression) => {
                // PDI3 open indexed-domain expressions must survive this
                // pre-resolution pass so binder names and selected operators
                // can acquire exact symbols later. Closed integer arithmetic
                // keeps the existing eager fold and diagnostics.
                if const_expression_contains_name(syntax, expression) {
                    continue;
                }
                let value = evaluate_const_argument_expression(
                    syntax,
                    expression,
                    const_values,
                    &HashMap::new(),
                    &HashSet::new(),
                    const_integer_type(syntax, *parameter_type),
                )
                .and_then(EvaluatedConst::into_concrete)
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "index argument expression for `{}` is invalid: {reason}",
                        family_name
                    ))
                })?;
                syntax.tables.type_references.replace_type_reference(
                    argument,
                    TypeReferenceNode::Named(Identifier::generated(value.to_string())),
                );
            }
            _ => {
                return Err(Diagnostic::error(format!(
                    "indexed domain `{}::{parameter_name}` requires a closed const value or direct const binder",
                    family_name
                )));
            }
        }
    }
    Ok(())
}

fn const_expression_contains_name(syntax: &SyntaxTrees, expression: ExpressionHandle) -> bool {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Name(_) => true,
        ExpressionNode::Binary(binary) => {
            const_expression_contains_name(syntax, binary.left)
                || const_expression_contains_name(syntax, binary.right)
        }
        ExpressionNode::Unary(unary) => const_expression_contains_name(syntax, unary.operand),
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CanonicalConstNode {
    Integer {
        type_name: String,
        value: i128,
    },
    Boolean(bool),
    Array {
        type_name: String,
        values: Vec<CanonicalConstNode>,
    },
    Record {
        type_name: String,
        fields: Vec<(String, CanonicalConstNode)>,
    },
    Variant {
        type_name: String,
        case_name: String,
        fields: Vec<(String, CanonicalConstNode)>,
    },
}

impl CanonicalConstNode {
    fn encoding(&self) -> String {
        match self {
            Self::Integer { type_name, value } => {
                framed("integer", [type_name.clone(), value.to_string()])
            }
            Self::Boolean(value) => framed("boolean", [if *value { "true" } else { "false" }]),
            Self::Array { type_name, values } => framed(
                "array",
                std::iter::once(type_name.as_str().to_owned())
                    .chain(values.iter().map(Self::encoding)),
            ),
            Self::Record { type_name, fields } => framed(
                "record",
                std::iter::once(type_name.clone()).chain(
                    fields
                        .iter()
                        .flat_map(|(name, value)| [name.clone(), value.encoding()]),
                ),
            ),
            Self::Variant {
                type_name,
                case_name,
                fields,
            } => framed(
                "variant",
                [type_name.clone(), case_name.clone()].into_iter().chain(
                    fields
                        .iter()
                        .flat_map(|(name, value)| [name.clone(), value.encoding()]),
                ),
            ),
        }
    }

    fn display(&self) -> String {
        match self {
            Self::Integer { value, .. } => value.to_string(),
            Self::Boolean(value) => value.to_string(),
            Self::Array { values, .. } => format!(
                "[{}]",
                values
                    .iter()
                    .map(Self::display)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Record { type_name, fields } => format!(
                "{type_name} {{ {} }}",
                fields
                    .iter()
                    .map(|(name, value)| format!("{name}: {}", value.display()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Variant {
                type_name,
                case_name,
                fields,
            } if fields.is_empty() => format!("{type_name}::{case_name}"),
            Self::Variant {
                type_name,
                case_name,
                fields,
            } => format!(
                "{type_name}::{case_name} {{ {} }}",
                fields
                    .iter()
                    .map(|(name, value)| format!("{name}: {}", value.display()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

fn framed(tag: &str, pieces: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let mut encoded = tag.to_owned();
    for piece in pieces {
        let piece = piece.as_ref();
        encoded.push_str(&piece.len().to_string());
        encoded.push(':');
        encoded.push_str(piece);
    }
    encoded
}

fn canonicalize_const_definition(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    parameter_type: TypeReferenceHandle,
) -> Result<CanonicalConstValue, String> {
    let declared = syntax_type_identity(syntax, definition.type_reference)?;
    let required = syntax_type_identity(syntax, parameter_type)?;
    if declared != required {
        return Err(format!(
            "const `{}` declares type `{declared}`, but the parameter requires `{required}`",
            qualified_const_name(definition)
        ));
    }
    validate_const_index_type(syntax, parameter_type, &mut HashSet::new())?;
    let node = canonicalize_const_expression(syntax, parameter_type, definition.value)?;
    if required == "Rat" {
        validate_canonical_rat(&node)?;
    }
    Ok(CanonicalConstValue::new(
        required,
        node.encoding(),
        node.display(),
    ))
}

fn syntax_type_identity(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
) -> Result<String, String> {
    Ok(
        match syntax.tables.type_references.type_reference(type_reference) {
            TypeReferenceNode::Named(name) => name.as_str().to_owned(),
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => format!(
                "[{}; {length}]",
                syntax_type_identity(syntax, *element_type)?
            ),
            TypeReferenceNode::Constrained { base_type, .. } => {
                syntax_type_identity(syntax, *base_type)?
            }
            TypeReferenceNode::Unit => "()".to_owned(),
            _ => {
                return Err(
                "structured const parameter types must be a canonical scalar, fixed array, or declared data value"
                    .to_owned(),
            );
            }
        },
    )
}

fn validate_const_index_type(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut HashSet<String>,
) -> Result<(), String> {
    match syntax.tables.type_references.type_reference(type_reference) {
        TypeReferenceNode::Named(name) => {
            if matches!(
                name.as_str(),
                "bool" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64"
                    | "addr"
            ) {
                return Ok(());
            }
            if matches!(name.as_str(), "f32" | "f64" | "string") {
                return Err(format!(
                    "`{name}` is not eligible as a const index: runtime floating/text identity is not canonical structural data"
                ));
            }
            if !visiting.insert(name.as_str().to_owned()) {
                return Ok(());
            }
            let definition = syntax
                .root_items()
                .find_map(|item| match item {
                    Item::Data(definition) if definition.name.as_str() == name.as_str() => {
                        Some(definition)
                    }
                    _ => None,
                })
                .ok_or_else(|| format!("`{name}` is not a declared canonical data type"))?;
            if definition.supply_mode == psi_language_semantics::DataSupplyMode::BoundaryOpaque {
                return Err(format!(
                    "boundary-opaque data `{name}` is not eligible as a const index"
                ));
            }
            if definition.quotient.is_some() {
                return Err(format!(
                    "quotient data `{name}` is not eligible as a structural const index until quotient-backed canonical representatives land"
                ));
            }
            if !definition.where_facts.is_empty() {
                return Err(format!(
                    "data `{name}` has default-domain facts whose index-site proof is not implemented; it is not yet eligible as a const index"
                ));
            }
            for member in syntax.tables.items.data_members(definition.members) {
                match member {
                    DataMember::Field(field) => validate_const_index_type(
                        syntax,
                        field.type_reference,
                        visiting,
                    )?,
                    DataMember::Variant(variant) => {
                        for field in syntax.tables.items.data_payload_fields(variant.payload) {
                            validate_const_index_type(syntax, field.type_reference, visiting)?;
                        }
                    }
                    DataMember::Retired(_) => {}
                }
            }
            visiting.remove(name.as_str());
            Ok(())
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(_),
        } => validate_const_index_type(syntax, *element_type, visiting),
        TypeReferenceNode::Constrained { base_type, .. } => {
            validate_const_index_type(syntax, *base_type, visiting)
        }
        TypeReferenceNode::Unit => Ok(()),
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::SelfType => Err(
            "const index types require finite structural values with decidable equality and one canonical form"
                .to_owned(),
        ),
    }
}

fn canonicalize_const_expression(
    syntax: &SyntaxTrees,
    expected_type: TypeReferenceHandle,
    expression: ExpressionHandle,
) -> Result<CanonicalConstNode, String> {
    match syntax.tables.type_references.type_reference(expected_type) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            canonicalize_const_expression(syntax, *base_type, expression)
        }
        TypeReferenceNode::Named(type_name)
            if matches!(
                type_name.as_str(),
                "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "addr"
            ) =>
        {
            let ExpressionNode::Integer(literal) = syntax.expressions.expression(expression) else {
                return Err(format!("expected an integer literal for `{type_name}`"));
            };
            let value = integer_literal_value(literal)
                .ok_or_else(|| "integer literal exceeds the const-value envelope".to_owned())?;
            validate_syntax_integer_range(type_name.as_str(), value)?;
            Ok(CanonicalConstNode::Integer {
                type_name: type_name.as_str().to_owned(),
                value,
            })
        }
        TypeReferenceNode::Named(type_name) if type_name.as_str() == "bool" => {
            let ExpressionNode::Boolean(value) = syntax.expressions.expression(expression) else {
                return Err("expected a boolean literal for `bool`".to_owned());
            };
            Ok(CanonicalConstNode::Boolean(*value))
        }
        TypeReferenceNode::Named(type_name) => {
            canonicalize_data_const_expression(syntax, type_name.as_str(), expression)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => {
            let ExpressionNode::ArrayLiteral(values) = syntax.expressions.expression(expression)
            else {
                return Err("expected an array literal for fixed-array const value".to_owned());
            };
            let values = syntax.expressions.expression_handles(*values);
            if values.len() != *length {
                return Err(format!(
                    "fixed-array const value requires {length} elements but has {}",
                    values.len()
                ));
            }
            let values = values
                .iter()
                .map(|value| canonicalize_const_expression(syntax, *element_type, *value))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(CanonicalConstNode::Array {
                type_name: syntax_type_identity(syntax, expected_type)?,
                values,
            })
        }
        TypeReferenceNode::Unit => Err(
            "unit const values do not yet have a source literal; use an empty declared record"
                .to_owned(),
        ),
        _ => Err("const value expression has an ineligible parameter type".to_owned()),
    }
}

fn canonicalize_data_const_expression(
    syntax: &SyntaxTrees,
    type_name: &str,
    expression: ExpressionHandle,
) -> Result<CanonicalConstNode, String> {
    let definition = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == type_name => Some(definition),
            _ => None,
        })
        .ok_or_else(|| format!("`{type_name}` is not a declared data type"))?;
    match syntax.expressions.expression(expression) {
        ExpressionNode::StructLiteral(literal) if literal.type_name.as_str() == type_name => {
            if let Some(case_name) = &literal.case_name {
                let variant = syntax
                    .tables
                    .items
                    .data_members(definition.members)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(variant)
                            if variant.name.as_str() == case_name.as_str() =>
                        {
                            Some(variant)
                        }
                        _ => None,
                    })
                    .ok_or_else(|| format!("`{type_name}` has no case `{}`", case_name.as_str()))?;
                let declared_fields = syntax
                    .tables
                    .items
                    .data_payload_fields(variant.payload)
                    .iter()
                    .collect::<Vec<_>>();
                let fields = canonicalize_named_fields(syntax, &declared_fields, literal.fields)?;
                Ok(CanonicalConstNode::Variant {
                    type_name: type_name.to_owned(),
                    case_name: case_name.as_str().to_owned(),
                    fields,
                })
            } else {
                if syntax
                    .tables
                    .items
                    .data_members(definition.members)
                    .iter()
                    .any(|member| matches!(member, DataMember::Variant(_)))
                {
                    return Err(format!(
                        "`{type_name}` is case data; its const value must name one case"
                    ));
                }
                let declared_fields = syntax
                    .tables
                    .items
                    .data_members(definition.members)
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Field(field) => Some(field),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let fields = canonicalize_named_fields(syntax, &declared_fields, literal.fields)?;
                Ok(CanonicalConstNode::Record {
                    type_name: type_name.to_owned(),
                    fields,
                })
            }
        }
        ExpressionNode::Name(path) => {
            let path = syntax.expressions.identifier_path_members(*path);
            let [head, case_name] = path else {
                return Err(format!("expected a `{type_name}` structural literal"));
            };
            if head.as_str() != type_name {
                return Err(format!(
                    "expected a `{type_name}` value, got `{}`",
                    head.as_str()
                ));
            }
            let variant = syntax
                .tables
                .items
                .data_members(definition.members)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.name.as_str() == case_name.as_str() => {
                        Some(variant)
                    }
                    _ => None,
                })
                .ok_or_else(|| format!("`{type_name}` has no case `{case_name}`"))?;
            if !variant.payload.is_empty() {
                return Err(format!(
                    "case `{type_name}::{case_name}` requires named payload fields"
                ));
            }
            Ok(CanonicalConstNode::Variant {
                type_name: type_name.to_owned(),
                case_name: case_name.as_str().to_owned(),
                fields: Vec::new(),
            })
        }
        _ => Err(format!("expected a `{type_name}` structural literal")),
    }
}

fn canonicalize_named_fields(
    syntax: &SyntaxTrees,
    declared_fields: &[&psi_syntax_trees::item::DataField],
    literal_fields: HandleSpan<psi_syntax_trees::expression::TableStructLiteralField>,
) -> Result<Vec<(String, CanonicalConstNode)>, String> {
    let authored = syntax.expressions.struct_fields(literal_fields);
    let mut canonical = Vec::with_capacity(declared_fields.len());
    for declared in declared_fields {
        let matches = authored
            .iter()
            .filter(|field| field.name.as_str() == declared.name.as_str())
            .collect::<Vec<_>>();
        let [field] = matches.as_slice() else {
            return Err(if matches.is_empty() {
                format!("missing const field `{}`", declared.name.as_str())
            } else {
                format!("duplicate const field `{}`", declared.name.as_str())
            });
        };
        canonical.push((
            declared.name.as_str().to_owned(),
            canonicalize_const_expression(syntax, declared.type_reference, field.value)?,
        ));
    }
    for field in authored {
        if !declared_fields
            .iter()
            .any(|declared| declared.name.as_str() == field.name.as_str())
        {
            return Err(format!("unknown const field `{}`", field.name.as_str()));
        }
    }
    Ok(canonical)
}

fn validate_syntax_integer_range(type_name: &str, value: i128) -> Result<(), String> {
    let (minimum, maximum) = match type_name {
        "i8" => (i128::from(i8::MIN), i128::from(i8::MAX)),
        "i16" => (i128::from(i16::MIN), i128::from(i16::MAX)),
        "i32" => (i128::from(i32::MIN), i128::from(i32::MAX)),
        "i64" => (i128::from(i64::MIN), i128::from(i64::MAX)),
        "u8" => (0, i128::from(u8::MAX)),
        "u16" => (0, i128::from(u16::MAX)),
        "u32" => (0, i128::from(u32::MAX)),
        "u64" | "addr" => (0, i128::from(u64::MAX)),
        _ => return Err(format!("`{type_name}` is not an integer const type")),
    };
    if value < minimum || value > maximum {
        Err(format!("const value `{value}` does not fit `{type_name}`"))
    } else {
        Ok(())
    }
}

fn validate_canonical_rat(value: &CanonicalConstNode) -> Result<(), String> {
    let CanonicalConstNode::Record { fields, .. } = value else {
        return Err("`Rat` index value must be a structural record".to_owned());
    };
    let numerator = fields
        .iter()
        .find(|(name, _)| name == "num")
        .map(|(_, value)| value)
        .ok_or_else(|| "`Rat` index value is missing `num`".to_owned())?;
    let denominator = fields
        .iter()
        .find(|(name, _)| name == "den")
        .map(|(_, value)| nat_value(value))
        .transpose()?
        .ok_or_else(|| "`Rat` index value is missing `den`".to_owned())?;
    let CanonicalConstNode::Record { fields, .. } = numerator else {
        return Err("`Rat.num` must be an `IntPair` record".to_owned());
    };
    let negative = fields
        .iter()
        .find(|(name, _)| name == "neg")
        .map(|(_, value)| nat_value(value))
        .transpose()?
        .ok_or_else(|| "`Rat.num` is missing `neg`".to_owned())?;
    let positive = fields
        .iter()
        .find(|(name, _)| name == "pos")
        .map(|(_, value)| nat_value(value))
        .transpose()?
        .ok_or_else(|| "`Rat.num` is missing `pos`".to_owned())?;
    if denominator == 0 {
        return Err("`Rat` index denominator must be positive".to_owned());
    }
    if negative != 0 && positive != 0 {
        return Err(
            "`Rat` index signed coordinates must be cancelled (at least one of `num.neg` and `num.pos` must be zero)"
                .to_owned(),
        );
    }
    let magnitude = negative.max(positive);
    if gcd_usize(magnitude, denominator) != 1 {
        return Err(
            "`Rat` index numerator magnitude and denominator must be gcd-reduced".to_owned(),
        );
    }
    Ok(())
}

fn nat_value(value: &CanonicalConstNode) -> Result<usize, String> {
    match value {
        CanonicalConstNode::Variant {
            type_name,
            case_name,
            fields,
        } if type_name == "Nat" && case_name == "Zero" && fields.is_empty() => Ok(0),
        CanonicalConstNode::Variant {
            type_name,
            case_name,
            fields,
        } if type_name == "Nat" && case_name == "Succ" => {
            let previous = fields
                .iter()
                .find(|(name, _)| name == "prev")
                .map(|(_, value)| nat_value(value))
                .transpose()?
                .ok_or_else(|| "`Nat::Succ` is missing `prev`".to_owned())?;
            previous
                .checked_add(1)
                .ok_or_else(|| "`Nat` const value is too large".to_owned())
        }
        _ => Err("`Rat` canonicality requires structural core `Nat` fields".to_owned()),
    }
}

fn gcd_usize(mut left: usize, mut right: usize) -> usize {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn const_integer_in_envelope(value: i128) -> Option<i128> {
    (value >= i128::from(i64::MIN) && value <= i128::from(u64::MAX)).then_some(value)
}

fn checked_fact_integer(value: Option<i128>, operation: &str) -> Result<ConstFactValue, String> {
    value
        .and_then(const_integer_in_envelope)
        .map(ConstFactValue::Integer)
        .ok_or_else(|| format!("{operation} exceeds the signed/unsigned 64-bit envelope"))
}

fn replace_const_expression_names_from(
    syntax: &mut SyntaxTrees,
    expression_watermark: u32,
    const_literals: &HashMap<String, IntegerLiteral>,
) {
    let replacements = syntax
        .expressions
        .iter_expressions()
        .filter(|(handle, _)| handle.arena_index() >= expression_watermark)
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Name(path) = expression else {
                return None;
            };
            let [name] = syntax.expressions.identifier_path_members(*path) else {
                return None;
            };
            const_literals
                .get(name.as_str())
                .cloned()
                .map(|literal| (handle, literal))
        })
        .collect::<Vec<_>>();
    for (handle, literal) in replacements {
        syntax
            .expressions
            .replace_expression(handle, ExpressionNode::Integer(literal));
    }
}

/// Generic definitions remain in the tree after their concrete records are
/// synthesized so the normal frontend can validate the template. A symbolic
/// const expression cannot cross that boundary yet, so reduce each template
/// expression to either its concrete value or one declared const-parameter
/// dependency. The concrete clones already carry the fully evaluated value;
/// this placeholder exists only to preserve the established generic type/kind
/// checks on the source template.
fn normalize_generic_template_const_expressions(
    syntax: &mut SyntaxTrees,
    const_values: &HashMap<String, i128>,
) -> Result<(), Diagnostic> {
    let templates: Vec<(String, HashSet<String>, Vec<TypeReferenceHandle>)> = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Data(definition) = item else {
                return None;
            };
            if definition.type_parameters.is_empty() {
                return None;
            }
            let symbolic_parameters = syntax
                .tables
                .items
                .type_parameters(definition.type_parameters)
                .iter()
                .filter_map(|parameter| {
                    matches!(parameter.kind, TypeParameterKind::Const { .. })
                        .then(|| parameter.name.as_str().to_string())
                })
                .collect();
            let fields = syntax
                .tables
                .items
                .data_members(definition.members)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some(field.type_reference),
                    DataMember::Variant(_) => None,
                    DataMember::Retired(_) => None,
                })
                .collect();
            Some((
                definition.name.as_str().to_string(),
                symbolic_parameters,
                fields,
            ))
        })
        .collect();

    for (template, symbolic_parameters, fields) in templates {
        for field in fields {
            normalize_template_type_reference(
                syntax,
                field,
                const_values,
                &symbolic_parameters,
            )
            .map_err(|reason| {
                Diagnostic::error(format!(
                    "const argument expression in generic data `{template}` is invalid: {reason}"
                ))
            })?;
        }
    }
    Ok(())
}

fn normalize_template_type_reference(
    syntax: &mut SyntaxTrees,
    type_reference: TypeReferenceHandle,
    const_values: &HashMap<String, i128>,
    symbolic_parameters: &HashSet<String>,
) -> Result<(), String> {
    let node = syntax
        .tables
        .type_references
        .type_reference(type_reference)
        .clone();
    match node {
        TypeReferenceNode::Reference { referee, .. } => {
            normalize_template_type_reference(syntax, referee, const_values, symbolic_parameters)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            normalize_template_type_reference(syntax, base_type, const_values, symbolic_parameters)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => normalize_template_type_reference(
            syntax,
            element_type,
            const_values,
            symbolic_parameters,
        ),
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            let arguments = syntax
                .tables
                .type_references
                .type_reference_handles(arguments)
                .to_vec();
            let integer_types = generic_const_integer_types(syntax, base_name.as_str());
            for (index, argument) in arguments.into_iter().enumerate() {
                let node = syntax
                    .tables
                    .type_references
                    .type_reference(argument)
                    .clone();
                if let TypeReferenceNode::ConstExpression(expression) = node {
                    let placeholder = evaluate_const_argument_expression(
                        syntax,
                        expression,
                        const_values,
                        &HashMap::new(),
                        symbolic_parameters,
                        integer_types.get(index).copied().flatten(),
                    )?;
                    let name = match placeholder {
                        EvaluatedConst::Concrete(value) => value.to_string(),
                        EvaluatedConst::Symbolic(name) => name,
                    };
                    syntax.tables.type_references.replace_type_reference(
                        argument,
                        TypeReferenceNode::Named(Identifier::generated(name)),
                    );
                } else {
                    normalize_template_type_reference(
                        syntax,
                        argument,
                        const_values,
                        symbolic_parameters,
                    )?;
                }
            }
            Ok(())
        }
        TypeReferenceNode::ConstExpression(expression) => {
            let placeholder = evaluate_const_argument_expression(
                syntax,
                expression,
                const_values,
                &HashMap::new(),
                symbolic_parameters,
                None,
            )?;
            let name = match placeholder {
                EvaluatedConst::Concrete(value) => value.to_string(),
                EvaluatedConst::Symbolic(name) => name,
            };
            syntax.tables.type_references.replace_type_reference(
                type_reference,
                TypeReferenceNode::Named(Identifier::generated(name)),
            );
            Ok(())
        }
        TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named(_)
        | TypeReferenceNode::SelfType
        | TypeReferenceNode::Unit => Ok(()),
    }
}

/// Every TYPE-REFERENCE position a generic-data spelling can appear in: data
/// FIELDS plus machine-body `let`-local, state PARAMETER, and RETURN types. Run
/// afresh each fixpoint round so newly-synthesized records' fields are seen.
fn collect_type_reference_positions(syntax: &SyntaxTrees) -> Vec<TypeReferenceHandle> {
    let mut positions: Vec<TypeReferenceHandle> = Vec::new();
    for item in syntax.root_items() {
        match item {
            // SKIP the bodies of GENERIC TEMPLATES (defs/machines with type
            // parameters): their `Box<T>` fields carry the type PARAMETER as an
            // argument, not a concrete instantiation -- monomorphizing them would
            // synthesize a bogus `Box<T>` record and corrupt the template. Only
            // concrete records (incl. synthesized instances) and non-generic
            // machine bodies hold real `Box<i32>` spellings.
            Item::Data(definition) if definition.type_parameters.is_empty() => {
                for member in syntax.tables.items.data_members(definition.members) {
                    match member {
                        DataMember::Field(field) => positions.push(field.type_reference),
                        DataMember::Variant(variant) => positions.extend(
                            syntax
                                .tables
                                .items
                                .data_payload_fields(variant.payload)
                                .iter()
                                .map(|field| field.type_reference),
                        ),
                        DataMember::Retired(_) => {}
                    }
                }
            }
            Item::Machine(machine) if machine.type_parameters.is_empty() => {
                // Conformance arguments participate in the same concrete
                // generic-data identity as the machine signature. Rewriting
                // `-> Algebra<Unit>` while leaving
                // `satisfies Trait<Algebra<Unit>>` generic makes an otherwise
                // exact requirement mismatch after instance synthesis.
                for conformance in syntax.tables.items.satisfies_clauses(machine.satisfies) {
                    positions.extend_from_slice(
                        syntax
                            .tables
                            .type_references
                            .type_reference_handles(conformance.arguments),
                    );
                }
                for state_handle in syntax.tables.items.state_handles(machine.states) {
                    let state = syntax.tables.items.state(*state_handle);
                    positions.push(state.return_type);
                    for parameter_handle in syntax.tables.items.state_parameters(state.parameters) {
                        positions.push(
                            syntax
                                .tables
                                .items
                                .state_parameter(*parameter_handle)
                                .type_reference,
                        );
                    }
                    for statement_handle in syntax.tables.items.statements(state.statements) {
                        if let StatementNode::LocalData(local) =
                            syntax.tables.statements.statement(*statement_handle)
                        {
                            positions.push(local.type_reference);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    positions
}

/// If `type_reference` is a `Base<Args..>` spelling of a fully-monomorphizable
/// generic data definition, record the rewrite-to-plain-name and the (deduped)
/// instantiation. Anything Phase 1 cannot lower completely -- a non-generic base,
/// wrong arity, a non-sluggable argument, or a base that is not a plain record
/// whose fields are each exactly the parameter or parameter-free -- is left
/// UNTOUCHED for the existing type-check-only path (skip, never reject).
fn consider_generic_spelling(
    syntax: &mut SyntaxTrees,
    generic_data: &HashMap<String, GenericData>,
    const_definitions: &HashMap<String, ConstDefinition>,
    const_values: &HashMap<String, i128>,
    type_reference: TypeReferenceHandle,
    rewrites: &mut Vec<PendingRewrite>,
    instantiations: &mut Vec<Instantiation>,
) -> Result<(), Diagnostic> {
    let (base_name, lifetime_arguments, arguments) =
        match syntax.tables.type_references.type_reference(type_reference) {
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } => (base_name.clone(), lifetime_arguments.clone(), *arguments),
            _ => return Ok(()),
        };
    let base = base_name.as_str().to_string();
    let Some(base_info) = generic_data.get(&base) else {
        return Ok(()); // non-generic base: plan-laid / existing error paths
    };

    let argument_handles: Vec<TypeReferenceHandle> = syntax
        .tables
        .type_references
        .type_reference_handles(arguments)
        .to_vec();
    if argument_handles.len() != base_info.parameter_names.len() {
        return Ok(());
    }
    for ((parameter_name, parameter_type), argument) in base_info
        .parameter_names
        .iter()
        .zip(&base_info.const_parameter_types)
        .zip(&argument_handles)
    {
        let Some(parameter_type) = *parameter_type else {
            if matches!(
                syntax.tables.type_references.type_reference(*argument),
                TypeReferenceNode::ConstExpression(_)
            ) {
                return Err(Diagnostic::error(format!(
                    "generic argument expression for `{base}` is only valid for a const parameter"
                )));
            }
            continue;
        };
        match syntax
            .tables
            .type_references
            .type_reference(*argument)
            .clone()
        {
            TypeReferenceNode::Named(name) => {
                if CanonicalConstValue::from_atom(name.as_str()).is_some() {
                    continue;
                }
                if let Some(value) = const_values.get(name.as_str()) {
                    syntax.tables.type_references.replace_type_reference(
                        *argument,
                        TypeReferenceNode::Named(Identifier::generated(value.to_string())),
                    );
                    continue;
                }
                let Some(definition) = const_definitions.get(name.as_str()) else {
                    continue;
                };
                let value = canonicalize_const_definition(syntax, definition, parameter_type)
                    .map_err(|reason| {
                        Diagnostic::error(format!(
                            "const argument for `{base}::{parameter_name}` is invalid at this index site: {reason}"
                        ))
                    })?;
                syntax.tables.type_references.replace_type_reference(
                    *argument,
                    TypeReferenceNode::Named(Identifier::generated(value.atom())),
                );
            }
            TypeReferenceNode::ConstExpression(expression) => {
                let value = evaluate_const_argument_expression(
                    syntax,
                    expression,
                    const_values,
                    &HashMap::new(),
                    &HashSet::new(),
                    const_integer_type(syntax, parameter_type),
                )
                .and_then(EvaluatedConst::into_concrete)
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "const argument expression for `{base}` is invalid: {reason}"
                    ))
                })?;
                syntax.tables.type_references.replace_type_reference(
                    *argument,
                    TypeReferenceNode::Named(Identifier::generated(value.to_string())),
                );
            }
            _ => continue,
        }
    }
    let Some(argument_names) = monomorphizable_argument_slugs(syntax, &argument_handles) else {
        return Ok(());
    };
    if !const_arguments_fit_declarations(syntax, base_info, &argument_handles) {
        // Leave malformed/out-of-range const applications intact so the normal
        // declaration-aware validator emits its precise diagnostic.
        return Ok(());
    }
    if generic_data_shape(syntax, base_info).is_none() {
        return Err(Diagnostic::error(format!(
            "generic data `{base}` mixes common fields with cases; closed mixed generic instances are not implemented"
        )));
    }
    if !base_is_fully_monomorphizable(syntax, generic_data, base_info) {
        return Ok(());
    }

    let synthetic_name = format!("{base}<{}>", argument_names.join(", "));
    rewrites.push(PendingRewrite {
        type_reference,
        synthetic_name: synthetic_name.clone(),
        lifetime_arguments,
    });
    if !instantiations
        .iter()
        .any(|instance| instance.synthetic_name == synthetic_name)
    {
        instantiations.push(Instantiation {
            synthetic_name,
            base_name: base,
            argument_handles,
        });
    }
    Ok(())
}

fn const_arguments_fit_declarations(
    syntax: &SyntaxTrees,
    base_info: &GenericData,
    arguments: &[TypeReferenceHandle],
) -> bool {
    base_info
        .const_parameter_types
        .iter()
        .zip(arguments)
        .all(|(parameter_type, argument)| {
            let Some(parameter_type) = parameter_type else {
                return true;
            };
            let TypeReferenceNode::Named(value) =
                syntax.tables.type_references.type_reference(*argument)
            else {
                return false;
            };
            let TypeReferenceNode::Named(type_name) = syntax
                .tables
                .type_references
                .type_reference(*parameter_type)
            else {
                return false;
            };
            if let Some(value) = CanonicalConstValue::from_atom(value.as_str()) {
                return value.type_name == type_name.as_str();
            }
            let Ok(value) = value.as_str().parse::<i128>() else {
                return false;
            };
            let (minimum, maximum) = match type_name.as_str() {
                "i8" => (i128::from(i8::MIN), i128::from(i8::MAX)),
                "i16" => (i128::from(i16::MIN), i128::from(i16::MAX)),
                "i32" => (i128::from(i32::MIN), i128::from(i32::MAX)),
                "i64" => (i128::from(i64::MIN), i128::from(i64::MAX)),
                "u8" => (0, i128::from(u8::MAX)),
                "u16" => (0, i128::from(u16::MAX)),
                "u32" => (0, i128::from(u32::MAX)),
                "u64" | "addr" => (0, i128::from(u64::MAX)),
                _ => return false,
            };
            value >= minimum && value <= maximum
        })
}

/// Evaluate the symbolic integer subset retained in a const-generic argument.
/// Names resolve to literal scoped const declarations collected above.
/// Arithmetic deliberately matches the closed-expression parser fold over the
/// current signed/unsigned 64-bit envelope. Shifts and bitwise operations use
/// the matched const parameter's declared width and signedness.
fn evaluate_const_argument_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, i128>,
    symbolic_parameters: &HashSet<String>,
    integer_type: Option<ConstIntegerType>,
) -> Result<EvaluatedConst, String> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Integer(value) => integer_literal_value(value)
            .map(EvaluatedConst::Concrete)
            .ok_or_else(|| {
                "integer operand must fit the signed/unsigned 64-bit envelope".to_string()
            }),
        ExpressionNode::Name(path) => {
            let name = syntax
                .expressions
                .identifier_path_members(*path)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            if let Some(value) = parameter_values
                .get(&name)
                .or_else(|| const_values.get(&name))
            {
                Ok(EvaluatedConst::Concrete(*value))
            } else if symbolic_parameters.contains(&name) {
                Ok(EvaluatedConst::Symbolic(name))
            } else {
                Err(format!("`{name}` is not a scoped integer const"))
            }
        }
        ExpressionNode::Binary(binary) => {
            let left = evaluate_const_argument_expression(
                syntax,
                binary.left,
                const_values,
                parameter_values,
                symbolic_parameters,
                integer_type,
            )?;
            let right = evaluate_const_argument_expression(
                syntax,
                binary.right,
                const_values,
                parameter_values,
                symbolic_parameters,
                integer_type,
            )?;
            match (binary.operator, &right) {
                (BinaryOperator::Divide | BinaryOperator::Modulo, EvaluatedConst::Concrete(0)) => {
                    return Err(match binary.operator {
                        BinaryOperator::Divide => "division by zero is invalid".to_string(),
                        _ => "remainder by zero is invalid".to_string(),
                    });
                }
                (
                    BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight,
                    EvaluatedConst::Concrete(amount),
                ) if *amount < 0 || *amount >= i128::from(u64::BITS) => {
                    return Err(match binary.operator {
                        BinaryOperator::ShiftLeft => {
                            "left shift exceeds the `u64` width".to_string()
                        }
                        _ => "right shift exceeds the `u64` width".to_string(),
                    });
                }
                (
                    BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor,
                    _,
                ) => {}
                _ => {
                    return Err(
                        "only integer arithmetic, shifts, and bitwise operators are supported"
                            .to_string(),
                    );
                }
            }
            let (EvaluatedConst::Concrete(left), EvaluatedConst::Concrete(right)) = (&left, &right)
            else {
                return Ok(left.or_symbolic(right));
            };
            let (left, right) = (*left, *right);
            match binary.operator {
                BinaryOperator::Add => checked_evaluated_const(left.checked_add(right), "addition"),
                BinaryOperator::Subtract => {
                    checked_evaluated_const(left.checked_sub(right), "subtraction")
                }
                BinaryOperator::Multiply => {
                    checked_evaluated_const(left.checked_mul(right), "multiplication")
                }
                BinaryOperator::Divide => left
                    .checked_div(right)
                    .map(EvaluatedConst::Concrete)
                    .ok_or_else(|| "division by zero is invalid".to_string()),
                BinaryOperator::Modulo => left
                    .checked_rem(right)
                    .map(EvaluatedConst::Concrete)
                    .ok_or_else(|| "remainder by zero is invalid".to_string()),
                BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor => {
                    evaluate_declared_width_operation(binary.operator, left, right, integer_type)
                        .map(EvaluatedConst::Concrete)
                }
                _ => unreachable!("const integer operator was validated above"),
            }
        }
        _ => Err("expression is not a symbolic integer const expression".to_string()),
    }
}

#[derive(Clone, Copy)]
struct ConstIntegerType {
    name: &'static str,
    bits: u32,
    signed: bool,
}

fn const_integer_type(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
) -> Option<ConstIntegerType> {
    let TypeReferenceNode::Named(name) =
        syntax.tables.type_references.type_reference(type_reference)
    else {
        return None;
    };
    Some(match name.as_str() {
        "i8" => ConstIntegerType {
            name: "i8",
            bits: 8,
            signed: true,
        },
        "i16" => ConstIntegerType {
            name: "i16",
            bits: 16,
            signed: true,
        },
        "i32" => ConstIntegerType {
            name: "i32",
            bits: 32,
            signed: true,
        },
        "i64" => ConstIntegerType {
            name: "i64",
            bits: 64,
            signed: true,
        },
        "u8" => ConstIntegerType {
            name: "u8",
            bits: 8,
            signed: false,
        },
        "u16" => ConstIntegerType {
            name: "u16",
            bits: 16,
            signed: false,
        },
        "u32" => ConstIntegerType {
            name: "u32",
            bits: 32,
            signed: false,
        },
        "u64" => ConstIntegerType {
            name: "u64",
            bits: 64,
            signed: false,
        },
        "addr" => ConstIntegerType {
            name: "addr",
            bits: 64,
            signed: false,
        },
        _ => return None,
    })
}

fn generic_const_integer_types(
    syntax: &SyntaxTrees,
    generic_name: &str,
) -> Vec<Option<ConstIntegerType>> {
    syntax
        .root_items()
        .find_map(|item| {
            let Item::Data(definition) = item else {
                return None;
            };
            (definition.name.as_str() == generic_name).then(|| {
                syntax
                    .tables
                    .items
                    .type_parameters(definition.type_parameters)
                    .iter()
                    .map(|parameter| match parameter.kind {
                        TypeParameterKind::Const { type_reference } => {
                            const_integer_type(syntax, type_reference)
                        }
                        _ => None,
                    })
                    .collect()
            })
        })
        .unwrap_or_default()
}

fn evaluate_declared_width_operation(
    operator: BinaryOperator,
    left: i128,
    right: i128,
    integer_type: Option<ConstIntegerType>,
) -> Result<i128, String> {
    let Some(integer_type) = integer_type else {
        return Err(
            "shifts and bitwise operators require a declared integer const type".to_string(),
        );
    };
    let modulus = 1i128 << integer_type.bits;
    let maximum = if integer_type.signed {
        (modulus >> 1) - 1
    } else {
        modulus - 1
    };
    let minimum = if integer_type.signed {
        -(modulus >> 1)
    } else {
        0
    };
    if left < minimum || left > maximum {
        return Err(format!(
            "left operand `{left}` is outside the declared `{}` range",
            integer_type.name
        ));
    }

    match operator {
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
            let amount = u32::try_from(right)
                .ok()
                .filter(|amount| *amount < integer_type.bits)
                .ok_or_else(|| {
                    format!(
                        "shift count must be non-negative and below the declared `{}` width",
                        integer_type.name
                    )
                })?;
            if operator == BinaryOperator::ShiftRight {
                // `i128 >>` sign-extends negative signed operands. Unsigned
                // operands were range-checked non-negative, for which the same
                // operation is the required logical shift.
                return Ok(left >> amount);
            }
            let shifted = left.checked_shl(amount).ok_or_else(|| {
                format!(
                    "left shift exceeds the declared `{}` range",
                    integer_type.name
                )
            })?;
            if shifted < minimum || shifted > maximum {
                return Err(format!(
                    "left shift exceeds the declared `{}` range",
                    integer_type.name
                ));
            }
            Ok(shifted)
        }
        BinaryOperator::BitwiseAnd | BinaryOperator::BitwiseOr | BinaryOperator::BitwiseXor => {
            if right < minimum || right > maximum {
                return Err(format!(
                    "right operand `{right}` is outside the declared `{}` range",
                    integer_type.name
                ));
            }
            let mask = modulus - 1;
            let left_bits = left & mask;
            let right_bits = right & mask;
            let result_bits = match operator {
                BinaryOperator::BitwiseAnd => left_bits & right_bits,
                BinaryOperator::BitwiseOr => left_bits | right_bits,
                BinaryOperator::BitwiseXor => left_bits ^ right_bits,
                _ => unreachable!(),
            };
            if integer_type.signed && result_bits >= modulus >> 1 {
                Ok(result_bits - modulus)
            } else {
                Ok(result_bits)
            }
        }
        _ => unreachable!("caller provides only shifts and bitwise operators"),
    }
}

#[derive(Debug)]
enum EvaluatedConst {
    Concrete(i128),
    Symbolic(String),
}

fn checked_evaluated_const(value: Option<i128>, operation: &str) -> Result<EvaluatedConst, String> {
    value
        .and_then(const_integer_in_envelope)
        .map(EvaluatedConst::Concrete)
        .ok_or_else(|| format!("{operation} exceeds the signed/unsigned 64-bit envelope"))
}

impl EvaluatedConst {
    fn into_concrete(self) -> Result<i128, String> {
        match self {
            Self::Concrete(value) => Ok(value),
            Self::Symbolic(name) => Err(format!(
                "`{name}` is a const parameter that has no binding at this use"
            )),
        }
    }

    fn or_symbolic(self, other: Self) -> Self {
        match self {
            Self::Symbolic(_) => self,
            Self::Concrete(_) => other,
        }
    }
}

/// A distinguishing slug for each argument -- the Phase-1 gate. `Some` when
/// EVERY argument is either a plain concrete `Named` type or a `Named` carrying
/// only nameable constraints (an arithmetic/carrier domain, `Box<i32 in
/// Wrapping>` / `Store<u8 in Utf8>`); `None` if any argument is genuinely
/// composite (a nested generic, array, slice, reference, or a range-bounded
/// type whose bound is an expression). The slug is used only to name the
/// synthetic record -- the SUBSTITUTION points the field at the argument's own
/// type reference, so a domain constraint on the argument rides along
/// unchanged. Distinct spellings must slug distinctly (`i32 in Wrapping` vs
/// `i32 in Saturating`); identical spellings share one instance.
fn monomorphizable_argument_slugs(
    syntax: &SyntaxTrees,
    argument_handles: &[TypeReferenceHandle],
) -> Option<Vec<String>> {
    argument_handles
        .iter()
        .map(|&argument| type_reference_slug(syntax, argument))
        .collect()
}

/// The naming slug for an argument type, or `None` for a shape Phase 1 leaves
/// to the existing generic path. Plain `Named` and `Named in Domain...` only.
fn type_reference_slug(syntax: &SyntaxTrees, handle: TypeReferenceHandle) -> Option<String> {
    match syntax.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) => Some(name.as_str().to_string()),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let base = type_reference_slug(syntax, *base_type)?;
            let mut rendered = Vec::new();
            for constraint in syntax.tables.type_references.constraints(*constraints) {
                rendered.push(constraint_slug(constraint)?);
            }
            if rendered.is_empty() {
                return Some(base);
            }
            Some(format!("{base} in {}", rendered.join(" + ")))
        }
        _ => None,
    }
}

/// The naming slug for a constraint, or `None` for a range bound (an expression
/// -- Phase 3). Only the nameable behaviour/domain tags slug here.
fn constraint_slug(constraint: &TypeConstraintNode) -> Option<String> {
    match constraint {
        TypeConstraintNode::Named(name) => Some(name.as_str().to_string()),
        TypeConstraintNode::Domain(domain) if domain.arguments.is_empty() => {
            Some(domain.name.as_str().to_string())
        }
        TypeConstraintNode::Domain(_) => None,
        TypeConstraintNode::ArithmeticDomain(domain) => Some(domain.name().to_string()),
        TypeConstraintNode::Range { .. } => None,
    }
}

/// Whether every field of a record or pure sum can be substituted soundly. A
/// field may be exactly the parameter, a concrete Named, a parameter-free
/// composite, or a nested known generic whose arguments are substitutable.
/// Mixed common-field/case shapes remain outside this closed cohort.
fn base_is_fully_monomorphizable(
    syntax: &SyntaxTrees,
    generic_data: &HashMap<String, GenericData>,
    base_info: &GenericData,
) -> bool {
    let parameters: HashMap<String, TypeReferenceHandle> = base_info
        .parameter_names
        .iter()
        .map(|name| (name.clone(), TypeReferenceHandle::default()))
        .collect();
    let Some(shape) = generic_data_shape(syntax, base_info) else {
        return false;
    };
    syntax
        .tables
        .items
        .data_members(base_info.members)
        .iter()
        .all(|member| match member {
            DataMember::Field(field) if shape == GenericDataShape::Record => {
                type_reference_is_substitutable(syntax, generic_data, base_info, field, &parameters)
            }
            DataMember::Variant(variant) if shape == GenericDataShape::PureSum => syntax
                .tables
                .items
                .data_payload_fields(variant.payload)
                .iter()
                .all(|field| {
                    type_reference_is_substitutable(
                        syntax,
                        generic_data,
                        base_info,
                        field,
                        &parameters,
                    )
                }),
            DataMember::Retired(_) => true,
            _ => false,
        })
}

fn generic_data_shape(syntax: &SyntaxTrees, base_info: &GenericData) -> Option<GenericDataShape> {
    let mut has_fields = false;
    let mut has_variants = false;
    for member in syntax.tables.items.data_members(base_info.members) {
        match member {
            DataMember::Field(_) => has_fields = true,
            DataMember::Variant(_) => has_variants = true,
            DataMember::Retired(_) => {}
        }
    }
    match (has_fields, has_variants) {
        (true, false) | (false, false) => Some(GenericDataShape::Record),
        (false, true) => Some(GenericDataShape::PureSum),
        (true, true) => None,
    }
}

fn type_reference_is_substitutable(
    syntax: &SyntaxTrees,
    generic_data: &HashMap<String, GenericData>,
    base_info: &GenericData,
    field: &psi_syntax_trees::item::DataField,
    parameters: &HashMap<String, TypeReferenceHandle>,
) -> bool {
    match syntax
        .tables
        .type_references
        .type_reference(field.type_reference)
    {
        TypeReferenceNode::Named(_) => true,
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            generic_data.contains_key(base_name.as_str())
                && syntax
                    .tables
                    .type_references
                    .type_reference_handles(*arguments)
                    .iter()
                    .all(|&argument| {
                        matches!(
                            syntax.tables.type_references.type_reference(argument),
                            TypeReferenceNode::Named(_) | TypeReferenceNode::ConstExpression(_)
                        ) || !type_reference_mentions_parameter(syntax, argument, parameters)
                    })
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let element_is_substitutable =
                matches!(
                    syntax.tables.type_references.type_reference(*element_type),
                    TypeReferenceNode::Named(_)
                ) || !type_reference_mentions_parameter(syntax, *element_type, parameters);
            let length_is_substitutable = match length {
                FixedArrayLength::Literal(_) | FixedArrayLength::ConstCall(_) => true,
                FixedArrayLength::ConstParameter(name) => base_info
                    .parameter_names
                    .iter()
                    .zip(&base_info.const_parameter_types)
                    .any(|(parameter_name, parameter_type)| {
                        parameter_type.is_some() && parameter_name == name.as_str()
                    }),
            };
            element_is_substitutable && length_is_substitutable
        }
        _ => !type_reference_mentions_parameter(syntax, field.type_reference, parameters),
    }
}

/// Clone a member with the type parameters substituted. Only reached for a base
/// `base_is_fully_monomorphizable` accepted. A field that IS a parameter points
/// at the argument; a NESTED generic (`a: Box<T>`) becomes a fresh concrete
/// spelling (`Box<i32>`) the fixpoint monomorphizes; a parameter-free field is
/// shared unchanged.
fn substitute_member(
    syntax: &mut SyntaxTrees,
    member: DataMember,
    substitution: &HashMap<String, TypeReferenceHandle>,
    const_values: &HashMap<String, i128>,
) -> DataMember {
    match member {
        DataMember::Field(field) => DataMember::Field(substitute_data_field(
            syntax,
            field,
            substitution,
            const_values,
        )),
        DataMember::Variant(mut variant) => {
            let payload = syntax
                .tables
                .items
                .data_payload_fields(variant.payload)
                .to_vec();
            let mut first = Handle::invalid();
            let mut count = 0u32;
            for field in payload {
                let field = substitute_data_field(syntax, field, substitution, const_values);
                let handle = syntax.tables.items.append_data_payload_field(field);
                if count == 0 {
                    first = handle;
                }
                count = count
                    .checked_add(1)
                    .expect("generic sum payload field count overflow");
            }
            variant.payload = HandleSpan::from_parts(first, count);
            DataMember::Variant(variant)
        }
        DataMember::Retired(identity) => DataMember::Retired(identity),
    }
}

fn substitute_data_field(
    syntax: &mut SyntaxTrees,
    mut field: psi_syntax_trees::item::DataField,
    substitution: &HashMap<String, TypeReferenceHandle>,
    const_values: &HashMap<String, i128>,
) -> psi_syntax_trees::item::DataField {
    let node = syntax
        .tables
        .type_references
        .type_reference(field.type_reference)
        .clone();
    match node {
        TypeReferenceNode::Named(name) => {
            if let Some(&argument) = substitution.get(name.as_str()) {
                // The field IS the parameter: point it at the argument's type
                // reference (already a concrete type in the same table).
                field.type_reference = argument;
            }
        }
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            let argument_handles: Vec<TypeReferenceHandle> = syntax
                .tables
                .type_references
                .type_reference_handles(arguments)
                .to_vec();
            let integer_types = generic_const_integer_types(syntax, base_name.as_str());
            let const_bindings: HashMap<String, i128> = substitution
                .iter()
                .filter_map(|(name, argument)| {
                    let TypeReferenceNode::Named(value) =
                        syntax.tables.type_references.type_reference(*argument)
                    else {
                        return None;
                    };
                    Some((name.clone(), value.as_str().parse::<i128>().ok()?))
                })
                .collect();
            let mut substituted_arguments = Vec::with_capacity(argument_handles.len());
            for (index, argument) in argument_handles.into_iter().enumerate() {
                let node = syntax
                    .tables
                    .type_references
                    .type_reference(argument)
                    .clone();
                let substituted = match node {
                    TypeReferenceNode::Named(name) => {
                        substitution.get(name.as_str()).copied().unwrap_or(argument)
                    }
                    TypeReferenceNode::ConstExpression(expression) => {
                        match evaluate_const_argument_expression(
                            syntax,
                            expression,
                            const_values,
                            &const_bindings,
                            &HashSet::new(),
                            integer_types.get(index).copied().flatten(),
                        )
                        .and_then(EvaluatedConst::into_concrete)
                        {
                            Ok(value) => syntax
                                .tables
                                .type_references
                                .insert_named(Identifier::generated(value.to_string())),
                            Err(_) => argument,
                        }
                    }
                    _ => argument,
                };
                substituted_arguments.push(substituted);
            }
            let new_span = syntax
                .tables
                .type_references
                .insert_type_reference_handles(substituted_arguments);
            field.type_reference =
                syntax
                    .tables
                    .type_references
                    .insert(TypeReferenceNode::Generic {
                        base_name,
                        lifetime_arguments,
                        arguments: new_span,
                    });
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let substituted_element =
                match syntax.tables.type_references.type_reference(element_type) {
                    TypeReferenceNode::Named(name) => substitution
                        .get(name.as_str())
                        .copied()
                        .unwrap_or(element_type),
                    _ => element_type,
                };
            let substituted_length = match length {
                FixedArrayLength::ConstParameter(name) => substitution
                    .get(name.as_str())
                    .and_then(|argument| {
                        match syntax.tables.type_references.type_reference(*argument) {
                            TypeReferenceNode::Named(value) => value.as_str().parse::<usize>().ok(),
                            _ => None,
                        }
                    })
                    .map(FixedArrayLength::Literal)
                    .unwrap_or(FixedArrayLength::ConstParameter(name)),
                length => length,
            };
            field.type_reference =
                syntax
                    .tables
                    .type_references
                    .insert(TypeReferenceNode::FixedArray {
                        element_type: substituted_element,
                        length: substituted_length,
                    });
        }
        _ => {} // parameter-free composite: shared unchanged
    }
    field
}

/// Whether a type reference mentions any of the substituted parameter names
/// (recursively through composite nodes). Conservative: on an unhandled node
/// shape it returns `true` so the caller rejects rather than silently sharing a
/// parameter-bearing type.
fn type_reference_mentions_parameter(
    syntax: &SyntaxTrees,
    handle: TypeReferenceHandle,
    substitution: &HashMap<String, TypeReferenceHandle>,
) -> bool {
    match syntax.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) => substitution.contains_key(name.as_str()),
        TypeReferenceNode::Generic { arguments, .. } => syntax
            .tables
            .type_references
            .type_reference_handles(*arguments)
            .iter()
            .any(|&argument| type_reference_mentions_parameter(syntax, argument, substitution)),
        // The common composite shells recurse precisely, so a parameter-FREE
        // field like `touched: i32 in Wrapping` (Constrained) or
        // `tags: [u8; 4]` shares unchanged instead of refusing the whole
        // container (constraints carry domain names, not type references).
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_mentions_parameter(syntax, *base_type, substitution)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_reference_mentions_parameter(syntax, *element_type, substitution)
        }
        TypeReferenceNode::Slice { element_type } => {
            type_reference_mentions_parameter(syntax, *element_type, substitution)
        }
        TypeReferenceNode::Reference { referee, .. } => {
            type_reference_mentions_parameter(syntax, *referee, substitution)
        }
        // Anything else: conservative -- possibly parameter-bearing, refuse
        // rather than share a wrong type.
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::desugar_generic_data_instances;
    use omega_layout::{DataShape, build_layout_plan};
    use omega_target::NativeTarget;
    use psi_checked_trees::CheckedTrees;
    use psi_diagnostics::Diagnostic;
    use psi_source_files_to_tokens::Lexer;
    use psi_syntax_trees::expression::ExpressionNode;
    use psi_syntax_trees::item::{DataMember, Item};
    use psi_syntax_trees::statement::StatementNode;
    use psi_syntax_trees::types::TypeReferenceNode;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;

    fn checked(source: &str) -> Result<CheckedTrees, Vec<Diagnostic>> {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let mut syntax = parse_syntax_trees(&tokens).expect("parse");
        desugar_generic_data_instances(&mut syntax)?;
        let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .map_err(|diagnostic| vec![diagnostic])?;
        let typed =
            psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
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
        let ExpressionNode::StructLiteral(boxed) = syntax.expressions.expression(boxed.value)
        else {
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

        let layout = build_layout_plan(&checked, NativeTarget::host()).expect("layout");
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

        let graph = omega_checked_trees_to_state_graph::build_state_graph(&checked)
            .expect("runtime state graph");
        assert!(graph.expressions.iter_expressions().all(|(_, expression)| {
            !matches!(
                expression,
                psi_checked_trees::expression::ExpressionNode::Name(path)
                    if path.symbol == only_symbol
            )
        }));
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
                Item::Data(definition) if definition.name.as_str() == "Maybe<i32>" => {
                    Some(definition)
                }
                _ => None,
            })
            .expect("closed Maybe definition");
        assert!(definition.type_parameters.is_empty());
        let members = syntax.items.data_members(definition.members);
        assert!(
            matches!(members[0], DataMember::Variant(ref variant) if variant.identity == Some(1))
        );
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

        let layout = build_layout_plan(&checked, NativeTarget::host()).expect("layout");
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
    fn mixed_generic_sum_remains_fail_closed() {
        rejected(
            r#"
            data Mixed<T> { common: i32; case None; case Some(value: T); }
            machine run() -> i32 {
                let mixed: Mixed<i32> = Mixed::Some { common: 1, value: 7 };
                0
            }
            "#,
            "mixes common fields with cases",
        );
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
    fn distinct_closed_instances_of_one_generic_sum_reject() {
        let source = r#"
            data Maybe<T> { case None; case Some(value: T); }
            data Holder { integer: Maybe<i32>; boolean: Maybe<bool>; }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let mut syntax = parse_syntax_trees(&tokens).expect("parse");
        let diagnostics = desugar_generic_data_instances(&mut syntax)
            .expect_err("two closed sum instances must reject in this slice");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("requires one exact closed instance per generic sum")
        }));
    }

    #[test]
    fn bare_erased_generic_literal_in_return_context_is_rejected() {
        rejected(
            r#"
            data Evidence { case Only; }
            data Box<T> { value: T; proof [erased]: Evidence; }
            machine make() -> Box<i32> {
                Box { value: 7 }
            }
            "#,
            "construction of erased generic data `Box` is unsupported in this context",
        );
    }

    #[test]
    fn bare_erased_generic_literal_in_call_context_is_rejected() {
        rejected(
            r#"
            data Evidence { case Only; }
            data Box<T> { value: T; proof [erased]: Evidence; }
            machine consume(boxed: Box<i32>) -> i32 { boxed.value }
            machine run() -> i32 {
                consume(Box { value: 7 })
            }
            "#,
            "construction of erased generic data `Box` is unsupported in this context",
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
        let TypeReferenceNode::Named(left) =
            syntax.type_references.type_reference(left.type_reference)
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
                Item::Machine(machine) if machine.name.as_str().ends_with("::project") => {
                    Some(machine)
                }
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

        let TypeReferenceNode::Named(result) =
            syntax.type_references.type_reference(state.return_type)
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
}
