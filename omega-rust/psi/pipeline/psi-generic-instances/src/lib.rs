//! Closed generic-data synthesis before resolution. A spelled `Box<i32>` is
//! rewritten to a concrete nominal `data Box<i32> { value: i32 }`, so every
//! downstream semantic and native phase sees one exact closed definition
//! rather than a generic definition plus ambient instance bindings.
//!
//! The executable cohort includes fully substitutable records, pure sums, and
//! mixed field/case data. Those shapes may have multiple distinct closed
//! instances. Sum constructors selected by an exact destination type, agreeing free-call
//! parameters, or agreeing exact-owner attached-call parameters and destructure
//! paths selected by an exact local subject are relabeled to that closed
//! identity; a sole closed instance remains an unambiguous fallback for other
//! concrete executable contexts.
//! Sluggable arguments are a plain concrete `Named` type OR a
//! `Named` carrying only nameable domain constraints (`Box<i32 in Wrapping>`,
//! `Store<u8 in Utf8>`), or a recursively nonzero literal fixed array of a
//! sluggable type. The substitution rides the argument's own type reference,
//! so its exact closed shape follows the field for free. What it skips: other
//! composite ARGUMENTS (`Box<&T>`, a range-bounded arg), and constrained or
//! dynamic parameter-bearing composites. References,
//! slices, and literal/const fixed arrays recursively substitute their element
//! or referee. A field nesting the parameter under
//! ANOTHER generic (`Pair<T> { a: Box<T> }`) IS handled (Phase 3): the desugar
//! runs to a FIXPOINT, synthesizing the concrete `Box<i32>` a `Pair<i32>`
//! produces. Scans every TYPE-REFERENCE position a generic-data spelling reaches:
//! data FIELDS plus concrete-machine `let` locals, state PARAMETERS, RETURN
//! annotations, and cast TARGETS; generic TEMPLATE bodies (defs/machines with
//! type params) are skipped so their param-arg spellings are not mistaken for
//! concrete instances.

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

mod const_evaluation;

use const_evaluation::*;

/// Canonicalize one source const declaration against its own declared type.
///
/// This is the narrow handoff used by declaration/API retention. The returned
/// value's structural encoding is semantic material; its display text remains
/// diagnostic-only. Constrained public constants stay unsupported until their
/// declaration-site proof obligations are checked rather than erased here.
pub fn canonicalize_declared_const_definition(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
) -> Result<CanonicalConstValue, String> {
    if matches!(
        syntax
            .tables
            .type_references
            .type_reference(definition.type_reference),
        TypeReferenceNode::Constrained { .. }
    ) {
        return Err(
            "constrained const declarations require declaration-site proof checking before they can publish compatibility identity"
                .to_owned(),
        );
    }
    canonicalize_const_definition(syntax, definition, definition.type_reference)
}

struct GenericData {
    name: String,
    origin_name: Identifier,
    is_public: bool,
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
                psi_syntax_trees::types::TypeReferenceNode::Generic {
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
                let substituted = substitute_member(syntax, member, &substitution, &const_values);
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
                access: left_access,
                lifetime: left_lifetime,
            },
            TypeReferenceNode::Reference {
                referee: right,
                access: right_access,
                lifetime: right_lifetime,
            },
        ) => {
            left_access == right_access
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
        StatementNode::ProofOutputBindingStatement(binding) => {
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
        ExpressionNode::Borrow(inner) => collect_expression_handles(syntax, inner.target, handles),
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

/// A distinguishing slug for each argument -- the Phase-1 gate. `Some` when
/// EVERY argument is either a plain concrete `Named` type, a recursively
/// nonzero literal fixed array of one, or a `Named` carrying only nameable
/// constraints (an arithmetic/carrier domain, `Box<i32 in Wrapping>` /
/// `Store<u8 in Utf8>`); `None` if any argument is a nested generic, zero or
/// nonliteral array, slice, reference, or a range-bounded type whose bound is
/// an expression. The slug is used only to name the
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

/// Rebind erased lifetimes carried by an already-synthesized local instance
/// from one concrete outer use to the outer template's own binder roster.
///
/// This first exact cohort is deliberately positional: the nested instance
/// must forward the complete outer lifetime application in the same order.
/// That preserves one stable synthesized definition across differently named
/// use-site lifetimes without inventing binders or choosing an alias/routing
/// policy. Broader permutations remain on the unnormalized path.
fn canonicalize_monomorphizable_argument_handles(
    syntax: &mut SyntaxTrees,
    base_info: &GenericData,
    outer_lifetime_arguments: &[Identifier],
    argument_handles: &[TypeReferenceHandle],
) -> Option<Vec<TypeReferenceHandle>> {
    base_info
        .const_parameter_types
        .iter()
        .zip(argument_handles)
        .map(|(const_parameter_type, argument)| {
            if const_parameter_type.is_some() {
                Some(*argument)
            } else {
                canonicalize_lifetime_bearing_type_argument(
                    syntax,
                    *argument,
                    &base_info.lifetime_parameters,
                    outer_lifetime_arguments,
                )
            }
        })
        .collect()
}

fn canonicalize_lifetime_bearing_type_argument(
    syntax: &mut SyntaxTrees,
    type_reference: TypeReferenceHandle,
    outer_lifetime_parameters: &[Identifier],
    outer_lifetime_arguments: &[Identifier],
) -> Option<TypeReferenceHandle> {
    let node = syntax
        .tables
        .type_references
        .type_reference(type_reference)
        .clone();
    match node {
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } if !lifetime_arguments.is_empty()
            && syntax
                .tables
                .type_references
                .type_reference_handles(arguments)
                .is_empty()
            && exact_synthesized_lifetime_instance(
                syntax,
                base_name.as_str(),
                lifetime_arguments.len(),
            ) =>
        {
            if outer_lifetime_parameters.len() != outer_lifetime_arguments.len()
                || lifetime_arguments.len() != outer_lifetime_arguments.len()
                || !lifetime_arguments
                    .iter()
                    .zip(outer_lifetime_arguments)
                    .all(|(nested, outer)| nested.as_str() == outer.as_str())
            {
                return None;
            }
            Some(
                syntax
                    .tables
                    .type_references
                    .insert(TypeReferenceNode::Generic {
                        base_name,
                        lifetime_arguments: outer_lifetime_parameters.to_vec(),
                        arguments: HandleSpan::empty(),
                    }),
            )
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => {
            let element_type = canonicalize_lifetime_bearing_type_argument(
                syntax,
                element_type,
                outer_lifetime_parameters,
                outer_lifetime_arguments,
            )?;
            Some(
                syntax
                    .tables
                    .type_references
                    .insert(TypeReferenceNode::FixedArray {
                        element_type,
                        length: FixedArrayLength::Literal(length),
                    }),
            )
        }
        _ => Some(type_reference),
    }
}

fn exact_synthesized_lifetime_instance(
    syntax: &SyntaxTrees,
    name: &str,
    lifetime_arity: usize,
) -> bool {
    lifetime_arity > 0
        && syntax.root_items().any(|item| {
            matches!(
                item,
                Item::Data(definition)
                    if definition.name.as_str() == name
                        && definition.generic_instance.is_some()
                        && definition.type_parameters.is_empty()
                        && definition.lifetime_parameters.len() == lifetime_arity
            )
        })
}

/// The naming slug for an argument type, or `None` for a shape Phase 1 leaves
/// to the existing generic path. Plain `Named`, recursively nonzero literal
/// fixed arrays, and `Named in Domain...` only.
fn type_reference_slug(syntax: &SyntaxTrees, handle: TypeReferenceHandle) -> Option<String> {
    match syntax.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) => Some(name.as_str().to_string()),
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } if syntax
            .tables
            .type_references
            .type_reference_handles(*arguments)
            .is_empty()
            && exact_synthesized_lifetime_instance(
                syntax,
                base_name.as_str(),
                lifetime_arguments.len(),
            ) =>
        {
            Some(base_name.as_str().to_owned())
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } if *length > 0 => Some(format!(
            "[{}; {length}]",
            type_reference_slug(syntax, *element_type)?
        )),
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
    type_reference_handle_is_substitutable(
        syntax,
        generic_data,
        base_info,
        field.type_reference,
        parameters,
    )
}

fn type_reference_handle_is_substitutable(
    syntax: &SyntaxTrees,
    generic_data: &HashMap<String, GenericData>,
    base_info: &GenericData,
    type_reference: TypeReferenceHandle,
    parameters: &HashMap<String, TypeReferenceHandle>,
) -> bool {
    match syntax.tables.type_references.type_reference(type_reference) {
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
            let element_is_substitutable = type_reference_handle_is_substitutable(
                syntax,
                generic_data,
                base_info,
                *element_type,
                parameters,
            );
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
        TypeReferenceNode::Reference { referee, .. } => type_reference_handle_is_substitutable(
            syntax,
            generic_data,
            base_info,
            *referee,
            parameters,
        ),
        TypeReferenceNode::Slice { element_type } => type_reference_handle_is_substitutable(
            syntax,
            generic_data,
            base_info,
            *element_type,
            parameters,
        ),
        _ => !type_reference_mentions_parameter(syntax, type_reference, parameters),
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
    field.type_reference =
        substitute_type_reference(syntax, field.type_reference, substitution, const_values);
    field
}

fn substitute_type_reference(
    syntax: &mut SyntaxTrees,
    type_reference: TypeReferenceHandle,
    substitution: &HashMap<String, TypeReferenceHandle>,
    const_values: &HashMap<String, i128>,
) -> TypeReferenceHandle {
    let node = syntax
        .tables
        .type_references
        .type_reference(type_reference)
        .clone();
    match node {
        TypeReferenceNode::Named(name) => substitution
            .get(name.as_str())
            .copied()
            .unwrap_or(type_reference),
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
                    _ => substitute_type_reference(syntax, argument, substitution, const_values),
                };
                substituted_arguments.push(substituted);
            }
            let new_span = syntax
                .tables
                .type_references
                .insert_type_reference_handles(substituted_arguments);
            syntax
                .tables
                .type_references
                .insert(TypeReferenceNode::Generic {
                    base_name,
                    lifetime_arguments,
                    arguments: new_span,
                })
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let substituted_element =
                substitute_type_reference(syntax, element_type, substitution, const_values);
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
            syntax
                .tables
                .type_references
                .insert(TypeReferenceNode::FixedArray {
                    element_type: substituted_element,
                    length: substituted_length,
                })
        }
        TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        } => {
            let referee = substitute_type_reference(syntax, referee, substitution, const_values);
            syntax
                .tables
                .type_references
                .insert(TypeReferenceNode::Reference {
                    referee,
                    access,
                    lifetime,
                })
        }
        TypeReferenceNode::Slice { element_type } => {
            let element_type =
                substitute_type_reference(syntax, element_type, substitution, const_values);
            syntax
                .tables
                .type_references
                .insert(TypeReferenceNode::Slice { element_type })
        }
        _ => type_reference,
    }
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
