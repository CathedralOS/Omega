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
//! case (two `plain-record<plain-Named>` instantiations that previously
//! collided). What it skips (later phases, or the pre-existing poison for a
//! second such instantiation): generic ENUMS (`case` members), non-plain-Named
//! arguments (`Box<i32 in Wrapping>`), and fields that NEST the parameter
//! (`[T; N]`, `&T`, `Other<T>`). FIELD type position only, like plan_laid v0.

use omega_core::arena::{Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{DataDefinition, DataMember, Item};
use omega_syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};
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
            continue; // has methods -> Phase 2
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

    // Scan field type references for generic-data applications. Collection
    // only; mutation happens after the scan so the borrows stay simple.
    let mut rewrites: Vec<PendingRewrite> = Vec::new();
    let mut instantiations: Vec<Instantiation> = Vec::new();
    for item in syntax.root_items() {
        let Item::Data(definition) = item else {
            continue;
        };
        for member in syntax.tables.items.data_members(definition.members) {
            let DataMember::Field(field) = member else {
                continue;
            };
            let TypeReferenceNode::Generic {
                base_name,
                arguments,
            } = syntax
                .tables
                .type_references
                .type_reference(field.type_reference)
            else {
                continue;
            };
            let base = base_name.as_str().to_string();
            let Some(base_info) = generic_data.get(&base) else {
                continue; // non-generic base: plan-laid / existing error paths
            };

            let argument_handles: Vec<TypeReferenceHandle> = syntax
                .tables
                .type_references
                .type_reference_handles(*arguments)
                .to_vec();
            // SKIP (leave for the existing generic path) anything Phase 1 cannot
            // lower completely: wrong arity, a non-plain-Named argument, or a
            // base that is not a plain record whose fields are each exactly the
            // parameter or parameter-free.
            if argument_handles.len() != base_info.parameter_names.len() {
                continue;
            }
            let Some(argument_names) = plain_named_arguments(syntax, &argument_handles) else {
                continue;
            };
            if !base_is_fully_monomorphizable(syntax, base_info) {
                continue;
            }

            let synthetic_name = format!("{base}<{}>", argument_names.join(", "));
            rewrites.push(PendingRewrite {
                type_reference: field.type_reference,
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
    }
    // No Phase-1 diagnostics: unhandled shapes are SKIPPED, not rejected.
    // Synthesize one concrete record per distinct instantiation: the base's
    // members cloned with the type parameters substituted for the arguments.
    for instance in &instantiations {
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
            properties: base_info.properties,
            members: HandleSpan::from_parts(first, count),
        }));
    }

    // Rewrite the field spellings to the synthesized instances' plain names.
    for rewrite in rewrites {
        syntax.tables.type_references.replace_type_reference(
            rewrite.type_reference,
            TypeReferenceNode::Named(Identifier::generated(rewrite.synthetic_name)),
        );
    }

    Ok(())
}

/// The argument names when EVERY argument is a plain concrete `Named` type
/// (the Phase-1 gate); `None` if any argument is composite/domain-typed.
fn plain_named_arguments(
    syntax: &SyntaxTrees,
    argument_handles: &[TypeReferenceHandle],
) -> Option<Vec<String>> {
    argument_handles
        .iter()
        .map(|&argument| match syntax.tables.type_references.type_reference(argument) {
            TypeReferenceNode::Named(name) => Some(name.as_str().to_string()),
            _ => None,
        })
        .collect()
}

/// Whether the base generic is a PLAIN RECORD each of whose fields is either
/// exactly the parameter or parameter-free -- the shape Phase 1 substitutes
/// soundly. A `case`/version member, or a field that nests the parameter,
/// fails (leaving the generic for the existing path).
fn base_is_fully_monomorphizable(syntax: &SyntaxTrees, base_info: &GenericData) -> bool {
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
                // any other node is fine only if it does NOT nest a parameter.
                _ => !type_reference_mentions_parameter(syntax, field.type_reference, &parameters),
            }
        })
}

/// Clone a member with the type parameters substituted. Only reached for a
/// base that `base_is_fully_monomorphizable` accepted, so every member is a
/// plain field whose type is either exactly a parameter or parameter-free.
fn substitute_member(
    syntax: &SyntaxTrees,
    member: DataMember,
    substitution: &HashMap<String, TypeReferenceHandle>,
) -> DataMember {
    let DataMember::Field(mut field) = member else {
        return member;
    };
    if let TypeReferenceNode::Named(name) = syntax
        .tables
        .type_references
        .type_reference(field.type_reference)
        && let Some(&argument) = substitution.get(name.as_str())
    {
        // The field IS the parameter: point it at the argument's type
        // reference (already a concrete type in the same table).
        field.type_reference = argument;
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
