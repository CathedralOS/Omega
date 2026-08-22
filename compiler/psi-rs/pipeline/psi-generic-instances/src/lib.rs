//! Closed generic-data synthesis before resolution. A spelled `Box<i32>` is
//! rewritten to a concrete nominal `data Box<i32> { value: i32 }`, so every
//! downstream semantic and native phase sees one exact closed definition
//! rather than a generic definition plus ambient instance bindings.
//!
//! The executable cohort includes fully substitutable records and pure sums.
//! Records and pure sums may have multiple distinct closed instances. Sum
//! constructors selected by an exact destination type, agreeing free-call
//! parameters, or agreeing exact-owner attached-call parameters and destructure
//! paths selected by an exact local subject are relabeled to that closed
//! identity; a sole closed instance remains an unambiguous fallback for other
//! concrete executable contexts.
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
    name: String,
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
    MixedSum,
}

/// Find `Base<Args..>` spellings in FIELD type position where `Base` is a
/// generic data definition, synthesize one concrete instance record per
/// distinct spelling (the parameter substituted for the argument), and rewrite
/// the field spellings to the instances' plain names.
/// Run Psi's target-neutral pre-resolution generic-data normalization and
/// return the only syntax tree downstream stages may consume.
///
/// Taking ownership prevents orchestration code from retaining an unnormalized
/// sibling or reaching into the elaborator as an in-place syntax mutator.
pub fn normalize_pre_resolution(mut syntax: SyntaxTrees) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    desugar_generic_data_instances(&mut syntax)?;
    Ok(syntax)
}

fn desugar_generic_data_instances(syntax: &mut SyntaxTrees) -> Result<(), Vec<Diagnostic>> {
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
                name: definition.name.as_str().to_owned(),
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

/// An assignment target is another explicit destination type. Relabel a bare
/// generic literal only when that type is available directly from a local or
/// an attached data field; computed targets remain fail-closed.
fn relabel_closed_data_uses_in_exact_assignments(
    syntax: &mut SyntaxTrees,
    synthesized_origins: &HashMap<String, String>,
) {
    let concrete_states = syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Machine(machine) if machine.type_parameters.is_empty() => Some(machine),
            _ => None,
        })
        .flat_map(|machine| {
            syntax
                .tables
                .items
                .state_handles(machine.states)
                .iter()
                .map(|state| (*state, machine.attached_data.clone()))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    for (state_handle, attached_data) in concrete_states {
        let state = syntax.tables.items.state(state_handle).clone();
        let mut local_types = HashMap::<String, TypeReferenceHandle>::new();
        for parameter in syntax.tables.items.state_parameters(state.parameters) {
            let parameter = syntax.tables.items.state_parameter(*parameter);
            local_types.insert(parameter.name.as_str().to_owned(), parameter.type_reference);
        }
        let statements = syntax.tables.items.statements(state.statements).to_vec();
        for statement in &statements {
            if let StatementNode::LocalData(local) = syntax.tables.statements.statement(*statement)
            {
                local_types.insert(local.name.as_str().to_owned(), local.type_reference);
            }
        }
        let self_field_types = attached_data
            .as_ref()
            .and_then(|attached| {
                syntax.root_items().find_map(|item| match item {
                    Item::Data(definition) if definition.name == *attached => Some(
                        syntax
                            .tables
                            .items
                            .data_members(definition.members)
                            .iter()
                            .filter_map(|member| match member {
                                DataMember::Field(field) => {
                                    Some((field.name.as_str().to_owned(), field.type_reference))
                                }
                                _ => None,
                            })
                            .collect::<HashMap<_, _>>(),
                    ),
                    _ => None,
                })
            })
            .unwrap_or_default();

        let assignments = statements
            .iter()
            .filter_map(
                |statement| match syntax.tables.statements.statement(*statement) {
                    StatementNode::Assignment(assignment) => Some(*assignment),
                    _ => None,
                },
            )
            .collect::<Vec<_>>();
        for assignment in assignments {
            let expected_type = match syntax.expressions.expression(assignment.target) {
                ExpressionNode::Name(path) => {
                    let [name] = syntax.expressions.identifier_path_members(*path) else {
                        continue;
                    };
                    local_types.get(name.as_str()).copied()
                }
                ExpressionNode::Member(member)
                    if matches!(
                        syntax.expressions.expression(member.receiver),
                        ExpressionNode::SelfValue
                    ) =>
                {
                    self_field_types.get(member.member.as_str()).copied()
                }
                _ => None,
            };
            if let Some(expected_type) = expected_type {
                relabel_data_literal_for_expected_type(
                    syntax,
                    assignment.value,
                    expected_type,
                    synthesized_origins,
                );
            }
        }
    }
}

/// Closed callable signatures provide exact contextual types without inferring
/// from literal fields. Free-machine overloads and concrete attached-machine
/// overloads each contribute a context only when every same-name candidate on
/// the exact owner agrees on one parameter signature. A direct `self.method`
/// statement call has that exact owner from the enclosing attached machine. An
/// explicitly typed local receiver or direct `self.field` receiver also names
/// one exact nominal owner. Computed, chained, and dynamic receiver selection
/// remains resolver-owned and fail closed here.
fn relabel_closed_data_uses_in_exact_calls_and_returns(
    syntax: &mut SyntaxTrees,
    synthesized_origins: &HashMap<String, String>,
) {
    let mut signatures = HashMap::<String, Option<Vec<TypeReferenceHandle>>>::new();
    let mut attached_signatures =
        HashMap::<(String, String), Option<Vec<TypeReferenceHandle>>>::new();
    for item in syntax.root_items() {
        let Item::Machine(machine) = item else {
            continue;
        };
        if !machine.type_parameters.is_empty() {
            continue;
        }
        let Some(entry) = syntax.tables.items.state_handles(machine.states).first() else {
            continue;
        };
        let parameters = syntax
            .tables
            .items
            .state_parameters(syntax.tables.items.state(*entry).parameters)
            .iter()
            .filter_map(|parameter| {
                let parameter = syntax.tables.items.state_parameter(*parameter);
                (!parameter.is_self).then_some(parameter.type_reference)
            })
            .collect::<Vec<_>>();
        let (signature_map, name) = if let Some(attached) = machine.attached_data.as_ref() {
            let method = machine
                .name
                .as_str()
                .rsplit("::")
                .next()
                .unwrap_or(machine.name.as_str())
                .to_owned();
            (
                &mut attached_signatures,
                (attached.as_str().to_owned(), method),
            )
        } else {
            let name = machine.name.as_str().to_owned();
            if let Some(signature) = signatures.get_mut(&name) {
                let agrees = signature.as_ref().is_some_and(|prior| {
                    call_context_parameter_types_agree(syntax, prior, &parameters)
                });
                if !agrees {
                    *signature = None;
                }
            } else {
                signatures.insert(name, Some(parameters));
            }
            continue;
        };
        if let Some(signature) = signature_map.get_mut(&name) {
            let agrees = signature.as_ref().is_some_and(|prior| {
                call_context_parameter_types_agree(syntax, prior, &parameters)
            });
            if !agrees {
                *signature = None;
            }
        } else {
            signature_map.insert(name, Some(parameters));
        }
    }

    let concrete_states = syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Machine(machine) if machine.type_parameters.is_empty() => Some(machine),
            _ => None,
        })
        .flat_map(|machine| {
            syntax
                .tables
                .items
                .state_handles(machine.states)
                .iter()
                .map(|state| (*state, machine.attached_data.clone()))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for (state_handle, attached_data) in concrete_states {
        let state = syntax.tables.items.state(state_handle).clone();
        let statements = syntax.tables.items.statements(state.statements).to_vec();
        let final_statement = statements.last().copied();
        let mut local_owner_types = HashMap::<String, String>::new();
        for parameter in syntax.tables.items.state_parameters(state.parameters) {
            let parameter = syntax.tables.items.state_parameter(*parameter);
            if !parameter.is_self
                && let Some(type_name) = named_type_name(syntax, parameter.type_reference)
            {
                local_owner_types.insert(parameter.name.as_str().to_owned(), type_name);
            }
        }
        for statement in &statements {
            if let StatementNode::LocalData(local) = syntax.tables.statements.statement(*statement)
                && let Some(type_name) = named_type_name(syntax, local.type_reference)
            {
                local_owner_types.insert(local.name.as_str().to_owned(), type_name);
            }
        }
        let self_field_owner_types = attached_data
            .as_ref()
            .and_then(|attached| {
                syntax.root_items().find_map(|item| match item {
                    Item::Data(definition) if definition.name == *attached => Some(
                        syntax
                            .tables
                            .items
                            .data_members(definition.members)
                            .iter()
                            .filter_map(|member| match member {
                                DataMember::Field(field) => {
                                    named_type_name(syntax, field.type_reference)
                                        .map(|owner| (field.name.as_str().to_owned(), owner))
                                }
                                DataMember::Variant(_) | DataMember::Retired(_) => None,
                            })
                            .collect::<HashMap<_, _>>(),
                    ),
                    _ => None,
                })
            })
            .unwrap_or_default();
        for statement in &statements {
            match syntax.tables.statements.statement(*statement) {
                StatementNode::Expression(value)
                    if state.return_type.is_valid() && Some(*statement) == final_statement =>
                {
                    relabel_data_literal_for_expected_type(
                        syntax,
                        *value,
                        state.return_type,
                        synthesized_origins,
                    );
                }
                StatementNode::Call(call) => {
                    let owner = exact_statement_receiver_owner(
                        syntax,
                        call.receiver,
                        call.receiver_starts_at_self,
                        attached_data.as_deref(),
                        &local_owner_types,
                        &self_field_owner_types,
                    );
                    let parameters = if call.receiver.is_empty() {
                        signatures.get(call.target.as_str())
                    } else {
                        let Some(owner) = owner else {
                            continue;
                        };
                        attached_signatures.get(&(owner, call.target.as_str().to_owned()))
                    };
                    let Some(Some(parameters)) = parameters else {
                        continue;
                    };
                    let arguments = syntax
                        .tables
                        .statements
                        .expression_handles(call.arguments)
                        .to_vec();
                    for (argument, expected_type) in arguments.into_iter().zip(parameters) {
                        relabel_data_literal_for_expected_type(
                            syntax,
                            argument,
                            *expected_type,
                            synthesized_origins,
                        );
                    }
                }
                StatementNode::Transition(transition) => {
                    for target in [transition.target, transition.continuation] {
                        if !target.is_valid() {
                            continue;
                        }
                        if let psi_syntax_trees::statement::TransitionTargetNode::Value(value) =
                            syntax.tables.statements.transition_target(target)
                        {
                            relabel_data_literal_for_expected_type(
                                syntax,
                                *value,
                                state.return_type,
                                synthesized_origins,
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        let mut reachable = HashSet::new();
        for statement in statements {
            collect_statement_expression_handles(syntax, statement, &mut reachable);
        }
        let calls = syntax
            .expressions
            .iter_expressions()
            .filter(|(handle, _)| reachable.contains(&handle.arena_index()))
            .filter_map(|(_, expression)| match expression {
                ExpressionNode::Call(call) if !call.receiver.is_valid() => {
                    Some((call.target.clone(), call.arguments, None))
                }
                ExpressionNode::Call(call) => exact_expression_receiver_owner(
                    syntax,
                    call.receiver,
                    attached_data.as_deref(),
                    &local_owner_types,
                    &self_field_owner_types,
                )
                .map(|owner| (call.target.clone(), call.arguments, Some(owner))),
                _ => None,
            })
            .collect::<Vec<_>>();
        for (target, arguments, owner) in calls {
            let parameters = if let Some(owner) = owner {
                attached_signatures.get(&(owner, target.as_str().to_owned()))
            } else {
                signatures.get(target.as_str())
            };
            let Some(Some(parameters)) = parameters else {
                continue;
            };
            let arguments = syntax.expressions.expression_handles(arguments).to_vec();
            for (argument, expected_type) in arguments.into_iter().zip(parameters) {
                relabel_data_literal_for_expected_type(
                    syntax,
                    argument,
                    *expected_type,
                    synthesized_origins,
                );
            }
        }
    }
}

fn exact_statement_receiver_owner(
    syntax: &SyntaxTrees,
    receiver: HandleSpan<Identifier>,
    starts_at_self: bool,
    attached_data: Option<&str>,
    local_owner_types: &HashMap<String, String>,
    self_field_owner_types: &HashMap<String, String>,
) -> Option<String> {
    let members = syntax.tables.statements.identifier_path_members(receiver);
    match members {
        [_] if starts_at_self => attached_data.map(str::to_owned),
        [local] if !starts_at_self => local_owner_types.get(local.as_str()).cloned(),
        [_, field] if starts_at_self => self_field_owner_types.get(field.as_str()).cloned(),
        _ => None,
    }
}

fn exact_expression_receiver_owner(
    syntax: &SyntaxTrees,
    receiver: ExpressionHandle,
    attached_data: Option<&str>,
    local_owner_types: &HashMap<String, String>,
    self_field_owner_types: &HashMap<String, String>,
) -> Option<String> {
    match syntax.expressions.expression(receiver) {
        ExpressionNode::SelfValue => attached_data.map(str::to_owned),
        ExpressionNode::Name(path) => {
            let [local] = syntax.expressions.identifier_path_members(*path) else {
                return None;
            };
            local_owner_types.get(local.as_str()).cloned()
        }
        ExpressionNode::Member(member)
            if matches!(
                syntax.expressions.expression(member.receiver),
                ExpressionNode::SelfValue
            ) =>
        {
            self_field_owner_types.get(member.member.as_str()).cloned()
        }
        _ => None,
    }
}

/// Conservative pre-resolution equality for the expected parameter types that
/// drive generic-literal relabeling. Handles are arena-local occurrences, so
/// comparing the handles themselves would make two textually identical
/// overload signatures look different. Unsupported or expression-bearing
/// details fail closed rather than broadening contextual inference.
fn call_context_parameter_types_agree(
    syntax: &SyntaxTrees,
    left: &[TypeReferenceHandle],
    right: &[TypeReferenceHandle],
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| call_context_types_agree(syntax, *left, *right))
}

fn call_context_types_agree(
    syntax: &SyntaxTrees,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    match (
        syntax.tables.type_references.type_reference(left),
        syntax.tables.type_references.type_reference(right),
    ) {
        (TypeReferenceNode::Named(left), TypeReferenceNode::Named(right)) => left == right,
        (TypeReferenceNode::SelfType, TypeReferenceNode::SelfType)
        | (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => true,
        (
            TypeReferenceNode::Reference {
                referee: left,
                is_mutable: left_mutable,
                lifetime: left_lifetime,
            },
            TypeReferenceNode::Reference {
                referee: right,
                is_mutable: right_mutable,
                lifetime: right_lifetime,
            },
        ) => {
            left_mutable == right_mutable
                && left_lifetime == right_lifetime
                && call_context_types_agree(syntax, *left, *right)
        }
        (
            TypeReferenceNode::Slice { element_type: left },
            TypeReferenceNode::Slice {
                element_type: right,
            },
        ) => call_context_types_agree(syntax, *left, *right),
        (
            TypeReferenceNode::FixedArray {
                element_type: left,
                length: left_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: right,
                length: right_length,
            },
        ) => left_length == right_length && call_context_types_agree(syntax, *left, *right),
        (
            TypeReferenceNode::Generic {
                base_name: left_base,
                lifetime_arguments: left_lifetimes,
                arguments: left_arguments,
            },
            TypeReferenceNode::Generic {
                base_name: right_base,
                lifetime_arguments: right_lifetimes,
                arguments: right_arguments,
            },
        ) => {
            left_base == right_base
                && left_lifetimes == right_lifetimes
                && call_context_parameter_types_agree(
                    syntax,
                    syntax
                        .tables
                        .type_references
                        .type_reference_handles(*left_arguments),
                    syntax
                        .tables
                        .type_references
                        .type_reference_handles(*right_arguments),
                )
        }
        (
            TypeReferenceNode::DynamicTrait {
                name: left_name,
                conformance: left_conformance,
            },
            TypeReferenceNode::DynamicTrait {
                name: right_name,
                conformance: right_conformance,
            },
        ) => left_name == right_name && left_conformance == right_conformance,
        // Constraint expressions and pre-evaluated const expressions use
        // occurrence-specific handles here. Treat them as no consensus until
        // the normalized typed identity is available.
        (TypeReferenceNode::Constrained { .. }, TypeReferenceNode::Constrained { .. })
        | (TypeReferenceNode::ConstExpression(_), TypeReferenceNode::ConstExpression(_)) => false,
        _ => false,
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
    if let ExpressionNode::Name(path) = syntax.expressions.expression(expression).clone() {
        let members = syntax.expressions.identifier_path_members(path);
        let Some(base) = synthesized_origins.get(expected_name.as_str()) else {
            return;
        };
        if let [literal_base, case] = members
            && literal_base.as_str() == base
            && syntax
                .tables
                .items
                .data_members(definition.members)
                .iter()
                .any(|member| matches!(member, DataMember::Variant(variant) if variant.name == *case))
        {
            let case = case.clone();
            let path = closed_sum_path(syntax, expected_name.as_str(), case);
            syntax.expressions.replace_expression(
                expression,
                ExpressionNode::Name(path),
            );
        }
        return;
    }
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

/// Destructure syntax lowers to `subject in Base::Case` before this pass. When
/// the subject is a state parameter or local with an exact synthesized type,
/// that annotation selects the corresponding closed case identity even when
/// another closed instance of the same generic sum exists in the program.
fn relabel_closed_sum_memberships_from_local_types(
    syntax: &mut SyntaxTrees,
    synthesized_origins: &HashMap<String, String>,
) {
    let concrete_states = syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Machine(machine) if machine.type_parameters.is_empty() => Some(machine),
            _ => None,
        })
        .flat_map(|machine| {
            syntax
                .tables
                .items
                .state_handles(machine.states)
                .iter()
                .map(|state| (*state, machine.attached_data.clone()))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    for (state_handle, attached_data) in concrete_states {
        let state = syntax.tables.items.state(state_handle).clone();
        let mut local_types = HashMap::<String, String>::new();
        for parameter in syntax.tables.items.state_parameters(state.parameters) {
            let parameter = syntax.tables.items.state_parameter(*parameter);
            if let Some(type_name) = named_type_name(syntax, parameter.type_reference) {
                local_types.insert(parameter.name.as_str().to_owned(), type_name);
            }
        }
        let statements = syntax.tables.items.statements(state.statements).to_vec();
        for statement in &statements {
            if let StatementNode::LocalData(local) = syntax.tables.statements.statement(*statement)
                && let Some(type_name) = named_type_name(syntax, local.type_reference)
            {
                local_types.insert(local.name.as_str().to_owned(), type_name);
            }
        }
        let self_field_types = attached_data
            .as_ref()
            .and_then(|attached| {
                syntax.root_items().find_map(|item| match item {
                    Item::Data(definition) if definition.name == *attached => Some(
                        syntax
                            .tables
                            .items
                            .data_members(definition.members)
                            .iter()
                            .filter_map(|member| match member {
                                DataMember::Field(field) => {
                                    named_type_name(syntax, field.type_reference).map(|type_name| {
                                        (field.name.as_str().to_owned(), type_name)
                                    })
                                }
                                _ => None,
                            })
                            .collect::<HashMap<_, _>>(),
                    ),
                    _ => None,
                })
            })
            .unwrap_or_default();

        let mut reachable = HashSet::new();
        for statement in statements {
            collect_statement_expression_handles(syntax, statement, &mut reachable);
        }
        let replacements = syntax
            .expressions
            .iter_expressions()
            .filter(|(handle, _)| reachable.contains(&handle.arena_index()))
            .filter_map(|(handle, expression)| {
                let ExpressionNode::Membership(membership) = expression else {
                    return None;
                };
                let closed = match syntax.expressions.expression(membership.value) {
                    ExpressionNode::Name(subject_path) => {
                        let [subject] = syntax.expressions.identifier_path_members(*subject_path)
                        else {
                            return None;
                        };
                        local_types.get(subject.as_str())?
                    }
                    ExpressionNode::Member(member)
                        if matches!(
                            syntax.expressions.expression(member.receiver),
                            ExpressionNode::SelfValue
                        ) =>
                    {
                        self_field_types.get(member.member.as_str())?
                    }
                    _ => return None,
                };
                let base = synthesized_origins.get(closed)?;
                let [domain_base, case] = syntax
                    .expressions
                    .identifier_path_members(membership.domain)
                else {
                    return None;
                };
                (domain_base.as_str() == base)
                    .then(|| (handle, membership.value, closed.clone(), case.clone()))
            })
            .collect::<Vec<_>>();
        for (handle, value, closed, case) in replacements {
            let domain = closed_sum_path(syntax, &closed, case);
            syntax.expressions.replace_expression(
                handle,
                ExpressionNode::Membership(
                    psi_syntax_trees::expression::TableMembershipExpression { value, domain },
                ),
            );
        }
    }
}

fn named_type_name(syntax: &SyntaxTrees, type_reference: TypeReferenceHandle) -> Option<String> {
    let type_reference = match syntax.tables.type_references.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => *base_type,
        TypeReferenceNode::Named(_) => type_reference,
        _ => return None,
    };
    match syntax.tables.type_references.type_reference(type_reference) {
        TypeReferenceNode::Named(name) => Some(name.as_str().to_owned()),
        _ => None,
    }
}

/// When a generic sum has exactly one closed instance, every remaining
/// `Base::Case` constructor and pattern path is unambiguous. Rewrite that
/// fallback cohort to the synthesized nominal identity before symbol
/// resolution. Multiple-instance uses must already have been selected by exact
/// context above. Generic template bodies remain parameterized declarations.
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
        StatementNode::EvidencePackageDestructure(binding) => {
            collect_expression_handles(syntax, binding.call, handles)
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
    fn collect(
        syntax: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
        positions: &mut Vec<TypeReferenceHandle>,
    ) {
        positions.push(type_reference);
        match syntax.tables.type_references.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => collect(syntax, *referee, positions),
            TypeReferenceNode::Constrained { base_type, .. } => {
                collect(syntax, *base_type, positions)
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                collect(syntax, *element_type, positions)
            }
            TypeReferenceNode::Generic { arguments, .. } => {
                for argument in syntax
                    .tables
                    .type_references
                    .type_reference_handles(*arguments)
                {
                    collect(syntax, *argument, positions);
                }
            }
            TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Named(_)
            | TypeReferenceNode::SelfType
            | TypeReferenceNode::Unit => {}
        }
    }

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
                        DataMember::Field(field) => {
                            collect(syntax, field.type_reference, &mut positions)
                        }
                        DataMember::Variant(variant) => {
                            for field in syntax.tables.items.data_payload_fields(variant.payload) {
                                collect(syntax, field.type_reference, &mut positions);
                            }
                        }
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
                    for argument in syntax
                        .tables
                        .type_references
                        .type_reference_handles(conformance.arguments)
                    {
                        collect(syntax, *argument, &mut positions);
                    }
                }
                for state_handle in syntax.tables.items.state_handles(machine.states) {
                    let state = syntax.tables.items.state(*state_handle);
                    collect(syntax, state.return_type, &mut positions);
                    for parameter_handle in syntax.tables.items.state_parameters(state.parameters) {
                        collect(
                            syntax,
                            syntax
                                .tables
                                .items
                                .state_parameter(*parameter_handle)
                                .type_reference,
                            &mut positions,
                        );
                    }
                    for statement_handle in syntax.tables.items.statements(state.statements) {
                        if let StatementNode::LocalData(local) =
                            syntax.tables.statements.statement(*statement_handle)
                        {
                            collect(syntax, local.type_reference, &mut positions);
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

/// Whether every field of a record or sum can be substituted soundly. A
/// field may be exactly the parameter, a concrete Named, a parameter-free
/// composite, or a nested known generic whose arguments are substitutable.
fn base_is_fully_monomorphizable(
    syntax: &SyntaxTrees,
    generic_data: &HashMap<String, GenericData>,
    base_info: &GenericData,
) -> bool {
    // Recursive inline data is proof-only. Keep its generic identity intact so
    // the structural entailment tier continues to see the authored generic
    // constructors and recursive applications; closed-instance synthesis is
    // an executable-layout transform, not a proof-data transform.
    if generic_data_is_recursive(syntax, generic_data, &base_info.name) {
        return false;
    }
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
            DataMember::Field(field)
                if matches!(shape, GenericDataShape::Record | GenericDataShape::MixedSum) =>
            {
                type_reference_is_substitutable(syntax, generic_data, base_info, field, &parameters)
            }
            DataMember::Variant(variant)
                if matches!(
                    shape,
                    GenericDataShape::PureSum | GenericDataShape::MixedSum
                ) =>
            {
                syntax
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
                    })
            }
            DataMember::Retired(_) => true,
            _ => false,
        })
}

fn generic_data_is_recursive(
    syntax: &SyntaxTrees,
    generic_data: &HashMap<String, GenericData>,
    base: &str,
) -> bool {
    fn reaches(
        syntax: &SyntaxTrees,
        generic_data: &HashMap<String, GenericData>,
        current: &str,
        goal: &str,
        visited: &mut HashSet<String>,
    ) -> bool {
        if !visited.insert(current.to_owned()) {
            return false;
        }
        let Some(definition) = generic_data.get(current) else {
            return false;
        };
        generic_inline_data_edges(syntax, definition)
            .into_iter()
            .any(|next| next == goal || reaches(syntax, generic_data, &next, goal, visited))
    }

    reaches(syntax, generic_data, base, base, &mut HashSet::new())
}

fn generic_inline_data_edges(syntax: &SyntaxTrees, definition: &GenericData) -> HashSet<String> {
    fn collect(
        syntax: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
        edges: &mut HashSet<String>,
    ) {
        match syntax.tables.type_references.type_reference(type_reference) {
            TypeReferenceNode::Named(name) => {
                edges.insert(name.as_str().to_owned());
            }
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => {
                edges.insert(base_name.as_str().to_owned());
                for argument in syntax
                    .tables
                    .type_references
                    .type_reference_handles(*arguments)
                {
                    collect(syntax, *argument, edges);
                }
            }
            TypeReferenceNode::Constrained { base_type, .. } => collect(syntax, *base_type, edges),
            TypeReferenceNode::FixedArray { element_type, .. } => {
                collect(syntax, *element_type, edges)
            }
            // References and slices are indirection and therefore break the
            // inline-containment cycle, matching proof-only classification.
            TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::SelfType
            | TypeReferenceNode::Unit => {}
        }
    }

    let mut edges = HashSet::new();
    for member in syntax.tables.items.data_members(definition.members) {
        match member {
            DataMember::Field(field) => collect(syntax, field.type_reference, &mut edges),
            DataMember::Variant(variant) => {
                for field in syntax.tables.items.data_payload_fields(variant.payload) {
                    collect(syntax, field.type_reference, &mut edges);
                }
            }
            DataMember::Retired(_) => {}
        }
    }
    edges
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
        (true, true) => Some(GenericDataShape::MixedSum),
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
mod tests;
