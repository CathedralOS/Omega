//! GENERIC DATA MONOMORPHIZATION -- Phase 1 (per-instance layout via
//! pre-resolution desugar). A field `b: Box<i32>;` where `data Box<T> { value:
//! T }` is a genuine generic definition is rewritten to a synthesized concrete
//! record `data Box<i32> { value: i32 }` -- the type parameter substituted for
//! the spelled argument. `Box<i32>` and `Box<bool>` become DISTINCT plain
//! types, so symbol resolution, typing, validation, and the native layout
//! builder all see ordinary records: two coexisting instances instead of the
//! layout builder's one-slot poison. Per-instance monomorphization, no
//! unification (the argument is always spelled) -- Zach's settled design.
//!
//! This is the same shape as `plan_laid`'s desugar (synthesize a per-spelling
//! instance definition, rewrite the field's type reference to its plain name),
//! plus the one addition generics need: SUBSTITUTE the type parameter inside
//! the copied members.
//!
//! PURELY ADDITIVE. Phase 1 monomorphizes only the cases it can lower
//! completely; every other generic spelling is LEFT UNTOUCHED for the existing
//! type-check-only path (which handles single instantiations, generic enums,
//! and domain-typed arguments today). So this never regresses a working
//! program -- it only lifts the layout builder's one-slot POISON for the clean
//! case (two `plain-record<sluggable-arg>` instantiations that previously
//! collided). Sluggable arguments are a plain concrete `Named` type OR a
//! `Named` carrying only nameable domain constraints (`Box<i32 in Wrapping>`,
//! `Store<u8 in Utf8>`) -- the substitution rides the argument's own type
//! reference, so the domain follows the field for free. What it skips (later
//! phases, or the pre-existing poison for a second such instantiation): generic
//! ENUMS (`case` members), genuinely composite ARGUMENTS (`Box<[i32; 4]>`,
//! `Box<&T>`, a range-bounded arg), and a field that nests the parameter under a
//! NON-generic composite (`[T; N]`, `&T`). A field nesting the parameter under
//! ANOTHER generic (`Pair<T> { a: Box<T> }`) IS handled (Phase 3): the desugar
//! runs to a FIXPOINT, synthesizing the concrete `Box<i32>` a `Pair<i32>`
//! produces. Scans every TYPE-REFERENCE position a generic-data spelling reaches:
//! data FIELDS plus machine-body `let`-local, state PARAMETER, and RETURN type
//! annotations; generic TEMPLATE bodies (defs/machines with type params) are
//! skipped so their param-arg spellings are not mistaken for concrete instances.

use omega_core::arena::{Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::literals::{IntegerLiteral, IntegerRadix};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{DataDefinition, DataMember, Item, ProofFact, TypeParameterKind};
use omega_syntax_trees::statement::StatementNode;
use omega_syntax_trees::types::{
    FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use std::collections::{HashMap, HashSet};

struct GenericData {
    parameter_names: Vec<String>,
    const_parameter_types: Vec<Option<TypeReferenceHandle>>,
    where_facts: HandleSpan<ProofFact>,
    members: HandleSpan<DataMember>,
    properties: omega_syntax_trees::item::DataProperties,
    supply_mode: omega_core::semantics::DataSupplyMode,
}

struct PendingRewrite {
    type_reference: TypeReferenceHandle,
    synthetic_name: String,
}

/// One discovered instantiation: the base generic definition and the argument
/// type references spelled for it, plus the plain name of the record to
/// synthesize.
struct Instantiation {
    synthetic_name: String,
    base_name: String,
    argument_handles: Vec<TypeReferenceHandle>,
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
                parameter_names,
                const_parameter_types,
                where_facts: definition.where_facts,
                members: definition.members,
                properties: definition.properties,
                supply_mode: definition.supply_mode,
            },
        );
    }
    if generic_data.is_empty() {
        return Ok(());
    }

    // Const-v0 declarations disappear during symbol resolution, but generic
    // data instances are selected before that stage. Resolve their literal
    // integer values here so `Buffer<Limits::WIDTH>` follows the same path as
    // `Buffer<4>` while leaving non-integer and negative consts to the normal
    // declaration/use diagnostics.
    let const_values: HashMap<String, u64> = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Const(definition) = item else {
                return None;
            };
            let ExpressionNode::Integer(value) = syntax.expressions.expression(definition.value)
            else {
                return None;
            };
            let value = value.value_u64()?;
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

    // FIXPOINT. Each round scans every type-reference position for a
    // `Base<Args..>` spelling, synthesizes one concrete record per new distinct
    // spelling, and rewrites the spellings to the instances' plain names. A
    // NESTED generic (`Pair<T> { a: Box<T> }` used as `Pair<i32>`) synthesizes a
    // `Pair<i32>` record whose `a` field is a fresh `Box<i32>` spelling -- picked
    // up and monomorphized by the NEXT round. Terminates: each round rewrites
    // >=1 Generic node to Named (permanent) or stops, and the distinct concrete
    // spellings are finite.
    let mut synthesized: std::collections::HashSet<String> = std::collections::HashSet::new();
    loop {
        let positions = collect_type_reference_positions(syntax);
        let mut rewrites: Vec<PendingRewrite> = Vec::new();
        let mut instantiations: Vec<Instantiation> = Vec::new();
        for position in positions {
            consider_generic_spelling(
                syntax,
                &generic_data,
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
        // Synthesize each not-yet-built instance: the base's members cloned with
        // the type parameters substituted for the arguments.
        for instance in &instantiations {
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
            let const_parameter_values: HashMap<String, u64> = base_info
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
            let const_literals: HashMap<String, IntegerLiteral> = const_parameter_values
                .iter()
                .map(|(name, value)| {
                    let literal = IntegerLiteral::from_parts(
                        false,
                        IntegerRadix::Decimal,
                        value.to_string().as_str(),
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
            let fact_expression_watermark = syntax.expressions.expression_count() as u32;
            let mut first_fact = Handle::invalid();
            let mut fact_count = 0u32;
            for fact in snapshot.tables.items.proof_facts(base_info.where_facts) {
                if let ProofFact::Expression(expression) = fact
                    && let Some(ConstFactValue::Boolean(value)) = evaluate_const_fact_expression(
                        &snapshot,
                        *expression,
                        &const_values,
                        &const_parameter_values,
                    )
                    .map_err(|reason| {
                        vec![Diagnostic::error(format!(
                            "const fact for generic instance `{}` is invalid: {reason}",
                            instance.synthetic_name
                        ))]
                    })?
                {
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
                type_parameters: HandleSpan::default(),
                properties,
                where_facts,
                members: HandleSpan::from_parts(first, count),
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
                let expression_watermark = syntax.expressions.expression_count() as u32;
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
            syntax.tables.type_references.replace_type_reference(
                rewrite.type_reference,
                TypeReferenceNode::Named(Identifier::generated(rewrite.synthetic_name)),
            );
        }
    }

    normalize_generic_template_const_expressions(syntax, &const_values)
        .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}

#[derive(Clone, Copy)]
enum ConstFactValue {
    Integer(u64),
    Boolean(bool),
}

/// Evaluate a proof expression exactly when every operand is known at generic
/// instantiation time. `None` means the fact still depends on a runtime field
/// and must remain on the synthesized record.
fn evaluate_const_fact_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, u64>,
    parameter_values: &HashMap<String, u64>,
) -> Result<Option<ConstFactValue>, String> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Integer(value) => value
            .value_u64()
            .map(ConstFactValue::Integer)
            .map(Some)
            .ok_or_else(|| "integer operand must be non-negative and fit `u64`".to_string()),
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
        ExpressionNode::Binary(binary) => {
            let Some(left) = evaluate_const_fact_expression(
                syntax,
                binary.left,
                const_values,
                parameter_values,
            )?
            else {
                return Ok(None);
            };
            let Some(right) = evaluate_const_fact_expression(
                syntax,
                binary.right,
                const_values,
                parameter_values,
            )?
            else {
                return Ok(None);
            };
            evaluate_const_fact_binary(binary.operator, left, right).map(Some)
        }
        _ => Ok(None),
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
            Add => left
                .checked_add(right)
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "addition overflows `u64`".to_string()),
            Subtract => left
                .checked_sub(right)
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "subtraction produces a negative value".to_string()),
            Multiply => left
                .checked_mul(right)
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "multiplication overflows `u64`".to_string()),
            Divide => left
                .checked_div(right)
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "division by zero is invalid".to_string()),
            Modulo => left
                .checked_rem(right)
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "remainder by zero is invalid".to_string()),
            ShiftLeft => u32::try_from(right)
                .ok()
                .and_then(|amount| left.checked_shl(amount))
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "left shift exceeds the `u64` width".to_string()),
            ShiftRight => u32::try_from(right)
                .ok()
                .and_then(|amount| left.checked_shr(amount))
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "right shift exceeds the `u64` width".to_string()),
            BitwiseAnd => Ok(ConstFactValue::Integer(left & right)),
            BitwiseOr => Ok(ConstFactValue::Integer(left | right)),
            BitwiseXor => Ok(ConstFactValue::Integer(left ^ right)),
            Equal => Ok(ConstFactValue::Boolean(left == right)),
            NotEqual => Ok(ConstFactValue::Boolean(left != right)),
            Greater => Ok(ConstFactValue::Boolean(left > right)),
            GreaterOrEqual => Ok(ConstFactValue::Boolean(left >= right)),
            Less => Ok(ConstFactValue::Boolean(left < right)),
            LessOrEqual => Ok(ConstFactValue::Boolean(left <= right)),
            And | Or => Err("logical operators require boolean operands".to_string()),
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
    const_values: &HashMap<String, u64>,
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
    const_values: &HashMap<String, u64>,
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
        TypeReferenceNode::Generic { arguments, .. } => {
            let arguments = syntax
                .tables
                .type_references
                .type_reference_handles(arguments)
                .to_vec();
            for argument in arguments {
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
        TypeReferenceNode::DynamicTrait(_)
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
                    if let DataMember::Field(field) = member {
                        positions.push(field.type_reference);
                    }
                }
            }
            Item::Machine(machine) if machine.type_parameters.is_empty() => {
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
    const_values: &HashMap<String, u64>,
    type_reference: TypeReferenceHandle,
    rewrites: &mut Vec<PendingRewrite>,
    instantiations: &mut Vec<Instantiation>,
) -> Result<(), Diagnostic> {
    let TypeReferenceNode::Generic {
        base_name,
        arguments,
    } = syntax.tables.type_references.type_reference(type_reference)
    else {
        return Ok(());
    };
    let base = base_name.as_str().to_string();
    let Some(base_info) = generic_data.get(&base) else {
        return Ok(()); // non-generic base: plan-laid / existing error paths
    };

    let argument_handles: Vec<TypeReferenceHandle> = syntax
        .tables
        .type_references
        .type_reference_handles(*arguments)
        .to_vec();
    if argument_handles.len() != base_info.parameter_names.len() {
        return Ok(());
    }
    for (parameter_type, argument) in base_info
        .const_parameter_types
        .iter()
        .zip(&argument_handles)
    {
        if parameter_type.is_none() {
            if matches!(
                syntax.tables.type_references.type_reference(*argument),
                TypeReferenceNode::ConstExpression(_)
            ) {
                return Err(Diagnostic::error(format!(
                    "generic argument expression for `{base}` is only valid for a const parameter"
                )));
            }
            continue;
        }
        match syntax.tables.type_references.type_reference(*argument) {
            TypeReferenceNode::Named(name) => {
                let Some(value) = const_values.get(name.as_str()) else {
                    continue;
                };
                syntax.tables.type_references.replace_type_reference(
                    *argument,
                    TypeReferenceNode::Named(Identifier::generated(value.to_string())),
                );
            }
            TypeReferenceNode::ConstExpression(expression) => {
                let value = evaluate_const_argument_expression(
                    syntax,
                    *expression,
                    const_values,
                    &HashMap::new(),
                    &HashSet::new(),
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
            let Ok(value) = value.as_str().parse::<u64>() else {
                return false;
            };
            let TypeReferenceNode::Named(type_name) = syntax
                .tables
                .type_references
                .type_reference(*parameter_type)
            else {
                return false;
            };
            let maximum = match type_name.as_str() {
                "i8" => i8::MAX as u64,
                "i16" => i16::MAX as u64,
                "i32" => i32::MAX as u64,
                "i64" => i64::MAX as u64,
                "u8" => u8::MAX as u64,
                "u16" => u16::MAX as u64,
                "u32" => u32::MAX as u64,
                "u64" | "addr" => u64::MAX,
                _ => return false,
            };
            value <= maximum
        })
}

/// Evaluate the symbolic integer subset retained in a const-generic argument.
/// Names resolve to literal scoped const declarations collected above.
/// Arithmetic deliberately matches the closed-expression parser fold:
/// non-negative checked `u64` only;
/// signed/domain semantics remain a separate language decision.
fn evaluate_const_argument_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, u64>,
    parameter_values: &HashMap<String, u64>,
    symbolic_parameters: &HashSet<String>,
) -> Result<EvaluatedConst, String> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Integer(value) => value
            .value_u64()
            .map(EvaluatedConst::Concrete)
            .ok_or_else(|| "integer operand must be non-negative and fit `u64`".to_string()),
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
            )?;
            let right = evaluate_const_argument_expression(
                syntax,
                binary.right,
                const_values,
                parameter_values,
                symbolic_parameters,
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
                ) if *amount >= u64::BITS as u64 => {
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
                BinaryOperator::Add => left
                    .checked_add(right)
                    .map(EvaluatedConst::Concrete)
                    .ok_or_else(|| "addition overflows `u64`".to_string()),
                BinaryOperator::Subtract => left
                    .checked_sub(right)
                    .map(EvaluatedConst::Concrete)
                    .ok_or_else(|| "subtraction produces a negative value".to_string()),
                BinaryOperator::Multiply => left
                    .checked_mul(right)
                    .map(EvaluatedConst::Concrete)
                    .ok_or_else(|| "multiplication overflows `u64`".to_string()),
                BinaryOperator::Divide => left
                    .checked_div(right)
                    .map(EvaluatedConst::Concrete)
                    .ok_or_else(|| "division by zero is invalid".to_string()),
                BinaryOperator::Modulo => left
                    .checked_rem(right)
                    .map(EvaluatedConst::Concrete)
                    .ok_or_else(|| "remainder by zero is invalid".to_string()),
                BinaryOperator::ShiftLeft => u32::try_from(right)
                    .ok()
                    .and_then(|amount| left.checked_shl(amount))
                    .map(EvaluatedConst::Concrete)
                    .ok_or_else(|| "left shift exceeds the `u64` width".to_string()),
                BinaryOperator::ShiftRight => u32::try_from(right)
                    .ok()
                    .and_then(|amount| left.checked_shr(amount))
                    .map(EvaluatedConst::Concrete)
                    .ok_or_else(|| "right shift exceeds the `u64` width".to_string()),
                BinaryOperator::BitwiseAnd => Ok(EvaluatedConst::Concrete(left & right)),
                BinaryOperator::BitwiseOr => Ok(EvaluatedConst::Concrete(left | right)),
                BinaryOperator::BitwiseXor => Ok(EvaluatedConst::Concrete(left ^ right)),
                _ => Err(
                    "only integer arithmetic, shifts, and bitwise operators are supported"
                        .to_string(),
                ),
            }
        }
        _ => Err("expression is not a symbolic integer const expression".to_string()),
    }
}

#[derive(Debug)]
enum EvaluatedConst {
    Concrete(u64),
    Symbolic(String),
}

impl EvaluatedConst {
    fn into_concrete(self) -> Result<u64, String> {
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
        TypeConstraintNode::Domain(name) => Some(name.as_str().to_string()),
        TypeConstraintNode::ArithmeticDomain(domain) => Some(domain.name().to_string()),
        TypeConstraintNode::Range { .. } => None,
    }
}

/// Whether the base generic is a PLAIN RECORD each of whose fields Phase 1/3 can
/// substitute soundly. A `case`/version member fails. A field may be exactly the
/// parameter, a concrete Named, a parameter-free composite, or a NESTED generic
/// `Base<Args..>` of a KNOWN generic whose arguments are each a parameter or
/// parameter-free (`Pair<T> { a: Box<T> }`) -- the fixpoint monomorphizes the
/// concrete `Box<i32>` the substitution produces.
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
    syntax
        .tables
        .items
        .data_members(base_info.members)
        .iter()
        .all(|member| {
            let DataMember::Field(field) = member else {
                return false; // case/version member
            };
            match syntax
                .tables
                .type_references
                .type_reference(field.type_reference)
            {
                // exactly the parameter, or a concrete Named -> fine.
                TypeReferenceNode::Named(_) => true,
                // a nested generic of a KNOWN base whose args are each the
                // parameter or parameter-free -> substitution yields a concrete
                // `Base<concretes>` the fixpoint picks up.
                TypeReferenceNode::Generic {
                    base_name,
                    arguments,
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
                                    TypeReferenceNode::Named(_)
                                        | TypeReferenceNode::ConstExpression(_)
                                ) || !type_reference_mentions_parameter(
                                    syntax,
                                    argument,
                                    &parameters,
                                )
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
                        ) || !type_reference_mentions_parameter(syntax, *element_type, &parameters);
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
                // any other node is fine only if it does NOT nest a parameter.
                _ => !type_reference_mentions_parameter(syntax, field.type_reference, &parameters),
            }
        })
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
    const_values: &HashMap<String, u64>,
) -> DataMember {
    let DataMember::Field(mut field) = member else {
        return member;
    };
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
            arguments,
        } => {
            let argument_handles: Vec<TypeReferenceHandle> = syntax
                .tables
                .type_references
                .type_reference_handles(arguments)
                .to_vec();
            let const_bindings: HashMap<String, u64> = substitution
                .iter()
                .filter_map(|(name, argument)| {
                    let TypeReferenceNode::Named(value) =
                        syntax.tables.type_references.type_reference(*argument)
                    else {
                        return None;
                    };
                    Some((name.clone(), value.as_str().parse::<u64>().ok()?))
                })
                .collect();
            let mut substituted_arguments = Vec::with_capacity(argument_handles.len());
            for argument in argument_handles {
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
    DataMember::Field(field)
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
