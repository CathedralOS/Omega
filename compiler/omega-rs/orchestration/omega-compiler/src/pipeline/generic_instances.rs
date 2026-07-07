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
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{DataDefinition, DataMember, Item};
use omega_syntax_trees::statement::StatementNode;
use omega_syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use std::collections::HashMap;

struct GenericData {
    parameter_names: Vec<String>,
    members: HandleSpan<DataMember>,
    properties: omega_syntax_trees::item::DataProperties,
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
    let mut data_with_machines: std::collections::HashSet<String> = std::collections::HashSet::new();
    for item in syntax.root_items() {
        if let Item::Machine(machine) = item
            && let Some(attached) = &machine.attached_data
        {
            data_with_machines.insert(attached.as_str().to_string());
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
        if data_with_machines.contains(definition.name.as_str()) {
            continue; // has methods (a container) -> Phase 2; do NOT fence here --
            // containers are valid-but-unimplemented (used type-check-only, e.g.
            // the stdlib `Vec<T>` in borrow canaries). A desugar-level "reject all
            // container instantiations" is too broad (it pre-empts those checks);
            // the silent-zero of a T-returning container VALUE-CALL at RUNTIME is
            // the narrow #40 concern, for a value-call/codegen fence -- see the
            // CONTAINERS note in TASKS.md and generics-runtime-boundary.md.
        }
        let parameter_names = syntax
            .tables
            .items
            .type_parameters(definition.type_parameters)
            .iter()
            .map(|parameter| parameter.name.as_str().to_string())
            .collect();
        generic_data.insert(
            definition.name.as_str().to_string(),
            GenericData {
                parameter_names,
                members: definition.members,
                properties: definition.properties,
            },
        );
    }
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
    loop {
        let positions = collect_type_reference_positions(syntax);
        let mut rewrites: Vec<PendingRewrite> = Vec::new();
        let mut instantiations: Vec<Instantiation> = Vec::new();
        for position in positions {
            consider_generic_spelling(
                syntax,
                &generic_data,
                position,
                &mut rewrites,
                &mut instantiations,
            );
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

            let members: Vec<DataMember> = syntax
                .tables
                .items
                .data_members(base_info.members)
                .to_vec();
            let properties = base_info.properties;
            let mut first: Handle<DataMember> = Handle::invalid();
            let mut count = 0u32;
            for member in members {
                let substituted = substitute_member(syntax, member, &substitution);
                let handle = syntax.tables.items.append_data_member(substituted);
                if count == 0 {
                    first = handle;
                }
                count += 1;
            }
            syntax.push_root_item(Item::Data(DataDefinition {
                name: Identifier::generated(instance.synthetic_name.as_str()),
                type_parameters: HandleSpan::default(),
                properties,
                members: HandleSpan::from_parts(first, count),
            }));
        }

        // Rewrite this round's spellings to the synthesized instances' plain names.
        for rewrite in rewrites {
            syntax.tables.type_references.replace_type_reference(
                rewrite.type_reference,
                TypeReferenceNode::Named(Identifier::generated(rewrite.synthetic_name)),
            );
        }
    }

    Ok(())
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
    syntax: &SyntaxTrees,
    generic_data: &HashMap<String, GenericData>,
    type_reference: TypeReferenceHandle,
    rewrites: &mut Vec<PendingRewrite>,
    instantiations: &mut Vec<Instantiation>,
) {
    let TypeReferenceNode::Generic {
        base_name,
        arguments,
    } = syntax.tables.type_references.type_reference(type_reference)
    else {
        return;
    };
    let base = base_name.as_str().to_string();
    let Some(base_info) = generic_data.get(&base) else {
        return; // non-generic base: plan-laid / existing error paths
    };

    let argument_handles: Vec<TypeReferenceHandle> = syntax
        .tables
        .type_references
        .type_reference_handles(*arguments)
        .to_vec();
    if argument_handles.len() != base_info.parameter_names.len() {
        return;
    }
    let Some(argument_names) = monomorphizable_argument_slugs(syntax, &argument_handles) else {
        return;
    };
    if !base_is_fully_monomorphizable(syntax, generic_data, base_info) {
        return;
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
            match syntax.tables.type_references.type_reference(field.type_reference) {
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
                                ) || !type_reference_mentions_parameter(
                                    syntax, argument, &parameters,
                                )
                            })
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
            let substituted_arguments: Vec<TypeReferenceHandle> = argument_handles
                .iter()
                .map(|&argument| {
                    match syntax.tables.type_references.type_reference(argument) {
                        TypeReferenceNode::Named(name) => {
                            substitution.get(name.as_str()).copied().unwrap_or(argument)
                        }
                        _ => argument,
                    }
                })
                .collect();
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
            .any(|&argument| {
                type_reference_mentions_parameter(syntax, argument, substitution)
            }),
        // Any other node shape (arrays, references, ...): be conservative --
        // treat it as possibly parameter-bearing so Phase 1 rejects rather than
        // shares a wrong type. Phase 3 handles these precisely.
        _ => true,
    }
}
