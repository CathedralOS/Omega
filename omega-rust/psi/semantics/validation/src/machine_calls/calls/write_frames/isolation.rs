//! Caller-isolated local and aggregate classification for write-frame
//! inference.
//!
//! These queries decide whether an ordinary value is structurally incapable
//! of carrying caller-visible aliasing. They inspect only checked typed shapes.
//! Frame traversal and complete-or-opaque fallback remain in the parent.

use super::type_instantiation::{
    TypeBindings, push_generic_application_bindings, substituted_head,
};
use crate::value_custody::struct_literals::construction_field_type;
use std::cell::RefCell;
use std::collections::HashMap;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::TableStructLiteral;
use typed_trees::name::Identifier;
use typed_trees::type_identity::NormalizedTypeIdentity;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Ordinary normalized identities are a pure function of the handle, and
/// these walks re-query the same handles heavily (the `visiting` back-edge
/// check compares every prefix entry against the same container). The memo
/// keeps each normalization a one-time cost per walk.
pub(super) type TypeIdentityMemo = HashMap<TypeReferenceHandle, NormalizedTypeIdentity>;

/// Write-frame inference asks these same questions once per frame expression:
/// verdicts at empty outer bindings are pure functions of the queried handle,
/// and the definition scans below re-filter the whole table per named type.
/// Cache exact verdicts and ordered definition indexes per program, anchored
/// on the definition-table identity (validation may append definitions).
type IsolationVerdicts = HashMap<(TypeReferenceHandle, bool), bool>;
type StorageMatchVerdicts = HashMap<(TypeReferenceHandle, TypeReferenceHandle), bool>;
// Definition verdicts key on the definition pointer, not its symbol: test
// fixtures can forge duplicate symbols, and two definitions sharing one must
// keep independent verdicts. Within a freshness window the slice is stable.
type DefinitionVerdicts = HashMap<*const typed_trees::data::DataDefinition, bool>;
// Substitution-sensitive verdicts key on the queried handle plus the exact
// binding frame, since the same handle can walk different member graphs
// under different applications.
type BindingsVerdicts = HashMap<
    (
        TypeReferenceHandle,
        Vec<(SymbolHandle, TypeReferenceHandle)>,
    ),
    bool,
>;

pub(super) struct IsolationCache {
    pub(super) isolation: IsolationVerdicts,
    pub(super) storage_match: StorageMatchVerdicts,
    pub(super) definitions: DefinitionVerdicts,
    pub(super) reference_free: BindingsVerdicts,
    pub(super) carry_write: BindingsVerdicts,
    pub(super) by_symbol: HashMap<SymbolHandle, Vec<u32>>,
    pub(super) by_name: HashMap<String, Vec<u32>>,
}

thread_local! {
    static ISOLATION_CACHE: RefCell<
        Option<(typed_trees::ProgramIdentity, usize, IsolationCache)>,
    > = const { RefCell::new(None) };
}

/// Cheap per-call staleness check over the tables these verdicts read.
///
/// This answers "has this program grown since the entry was built", not "is
/// this the same program": `TypedTrees::identity` answers that, and it is the
/// cache's key. The fingerprint mixed arena base pointers before, to tell
/// fixture programs apart when forged handles made their symbol endpoints
/// collide; those blocks are recycled with the program, so a dropped program
/// and its replacement could present the same address and the same
/// fingerprint, and the replacement was then served the dropped program's
/// verdicts. Measured on `-p typed-trees-to-checked-trees --lib -E
/// 'test(/range_/)'`: 2 of 6 multi-threaded runs failed, with different tests
/// failing each time, while 4 of 4 single-threaded runs and 6 of 6 runs with
/// this cache disabled passed.
fn program_fingerprint(program: &TypedTrees) -> usize {
    let definitions = program.data_definitions();
    let machines = program.machines();
    let definition_sample = |index: usize| -> usize {
        definitions
            .get(index)
            .map(|definition| {
                definition.symbol.arena_index() as usize
                    ^ definition.name.as_str().len().rotate_left(7)
                    ^ program.data_members(definition).len().rotate_left(13)
                    ^ program
                        .data_type_parameters(definition)
                        .len()
                        .rotate_left(19)
            })
            .unwrap_or(0)
    };
    let machine_sample = |index: usize| -> usize {
        machines
            .get(index)
            .map(|machine| {
                machine.symbol.arena_index() as usize
                    ^ program.machine_states(machine).len().rotate_left(5)
            })
            .unwrap_or(0)
    };
    let mut fingerprint = definitions.len().rotate_left(17)
        ^ machines.len().rotate_left(31)
        ^ program.plan_laid_layouts.len().rotate_left(9)
        ^ program.placed_view_plans.len().rotate_left(23)
        ^ program.authored_service_reach_rows.len().rotate_left(41);
    fingerprint = fingerprint.rotate_left(11) ^ definition_sample(0);
    fingerprint = fingerprint.rotate_left(11) ^ definition_sample(definitions.len() / 2);
    fingerprint =
        fingerprint.rotate_left(11) ^ definition_sample(definitions.len().saturating_sub(1));
    fingerprint = fingerprint.rotate_left(11) ^ machine_sample(0);
    fingerprint.rotate_left(11) ^ machine_sample(machines.len().saturating_sub(1))
}

pub(super) fn with_isolation_cache<R>(
    program: &TypedTrees,
    run: impl FnOnce(&mut IsolationCache) -> R,
) -> R {
    ISOLATION_CACHE.with(|cell| {
        let mut slot = cell.borrow_mut();
        let fingerprint = program_fingerprint(program);
        let fresh = matches!(&*slot, Some((owner, seen, _))
            if owner.get() == program.identity.get() && *seen == fingerprint);
        if !fresh {
            let mut by_symbol: HashMap<SymbolHandle, Vec<u32>> = HashMap::new();
            let mut by_name: HashMap<String, Vec<u32>> = HashMap::new();
            for (index, definition) in program.data_definitions().iter().enumerate() {
                if definition.symbol.is_valid() {
                    by_symbol
                        .entry(definition.symbol)
                        .or_default()
                        .push(index as u32);
                }
                by_name
                    .entry(definition.name.as_str().to_string())
                    .or_default()
                    .push(index as u32);
            }
            *slot = Some((
                program.identity,
                fingerprint,
                IsolationCache {
                    isolation: HashMap::new(),
                    storage_match: HashMap::new(),
                    definitions: HashMap::new(),
                    reference_free: HashMap::new(),
                    carry_write: HashMap::new(),
                    by_symbol,
                    by_name,
                },
            ));
        }
        run(&mut slot.as_mut().unwrap().2)
    })
}

pub(super) fn definitions_for_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Vec<&typed_trees::data::DataDefinition> {
    let indexes = with_isolation_cache(program, |cache| cache.by_symbol.get(&symbol).cloned());
    indexes
        .map(|indexes| {
            indexes
                .iter()
                .map(|index| &program.data_definitions()[*index as usize])
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn definitions_for_name<'p>(
    program: &'p TypedTrees,
    name: &str,
) -> Vec<&'p typed_trees::data::DataDefinition> {
    let indexes = with_isolation_cache(program, |cache| cache.by_name.get(name).cloned());
    indexes
        .map(|indexes| {
            indexes
                .iter()
                .map(|index| &program.data_definitions()[*index as usize])
                .collect()
        })
        .unwrap_or_default()
}

fn type_identities_match(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
    identities: &mut TypeIdentityMemo,
) -> bool {
    if !actual.is_valid() || !expected.is_valid() {
        return actual.is_valid() == expected.is_valid();
    }
    identities
        .entry(actual)
        .or_insert_with(|| program.normalized_type_identity(actual));
    identities
        .entry(expected)
        .or_insert_with(|| program.normalized_type_identity(expected));
    identities[&actual] == identities[&expected]
}

/// Lifetime applications retain borrow-region checking but do not require
/// type substitution to inspect their declared storage fields.
pub(super) fn concrete_nominal_type(
    reference: &TypeReferenceNode,
) -> Option<(SymbolHandle, &Identifier)> {
    match reference {
        TypeReferenceNode::Named { symbol, name } => Some((*symbol, name)),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } if arguments.is_empty() => Some((*base_symbol, base_name)),
        _ => None,
    }
}

/// Storage-origin transport compares concrete field structure, not borrow
/// regions. An elided nominal use and an explicit lifetime-only application
/// have the same fields; actual type arguments and constraints still require
/// their existing exact type identity. Lifetime validation remains separate.
pub(super) fn aggregate_storage_types_match(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
) -> bool {
    if let Some(verdict) = with_isolation_cache(program, |cache| {
        cache.storage_match.get(&(actual, expected)).copied()
    }) {
        return verdict;
    }
    let mut identities = TypeIdentityMemo::new();
    let verdict = aggregate_storage_types_match_in(program, actual, expected, &[], &mut identities);
    with_isolation_cache(program, |cache| {
        cache.storage_match.insert((actual, expected), verdict);
        cache.storage_match.insert((expected, actual), verdict);
    });
    verdict
}

/// Under an active substitution, a `Named` parameter resolves to its bound
/// actual and two applications of the same generic base match when every
/// argument matches. Anything else keeps the nominal-identity rule.
pub(super) fn aggregate_storage_types_match_in(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
    bindings: &[(SymbolHandle, TypeReferenceHandle)],
    identities: &mut TypeIdentityMemo,
) -> bool {
    let actual = substituted_head(program, actual, bindings);
    let expected = substituted_head(program, expected, bindings);
    if type_identities_match(program, actual, expected, identities) {
        return true;
    }
    if let (
        TypeReferenceNode::Generic {
            base_symbol: actual_base,
            arguments: actual_arguments,
            ..
        },
        TypeReferenceNode::Generic {
            base_symbol: expected_base,
            arguments: expected_arguments,
            ..
        },
    ) = (
        program.type_reference_table.type_reference(actual),
        program.type_reference_table.type_reference(expected),
    ) && actual_base.is_valid()
        && actual_base == expected_base
    {
        let actual_arguments = program
            .type_reference_table
            .type_reference_handles(*actual_arguments);
        let expected_arguments = program
            .type_reference_table
            .type_reference_handles(*expected_arguments);
        return actual_arguments.len() == expected_arguments.len()
            && actual_arguments
                .iter()
                .zip(expected_arguments)
                .all(|(actual, expected)| {
                    aggregate_storage_types_match_in(
                        program, *actual, *expected, bindings, identities,
                    )
                });
    }
    let Some((actual, _)) =
        concrete_nominal_type(program.type_reference_table.type_reference(actual))
    else {
        return false;
    };
    let Some((expected, _)) =
        concrete_nominal_type(program.type_reference_table.type_reference(expected))
    else {
        return false;
    };
    actual.is_valid()
        && actual == expected
        && definitions_for_symbol(program, actual)
            .iter()
            .any(|definition| definition.type_parameters.is_empty())
}

pub(super) fn struct_literal_field_type(
    program: &TypedTrees,
    literal: &TableStructLiteral,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    let mut definitions = definitions_for_name(program, literal.type_name.as_str()).into_iter();
    let definition = definitions.next()?;
    definitions.next().is_none().then_some(())?;
    construction_field_type(
        program,
        definition,
        literal.case_name.as_ref().map(|name| name.as_str()),
        field_name,
    )
}

pub(super) fn struct_literal_matches_expected_type(
    program: &TypedTrees,
    literal: &TableStructLiteral,
    expected_type: TypeReferenceHandle,
) -> bool {
    let Some(expected_type) =
        crate::value_custody::places::unwrapped_type_reference(program, expected_type)
    else {
        return false;
    };
    let Some((symbol, name)) =
        concrete_nominal_type(program.type_reference_table.type_reference(expected_type))
    else {
        return false;
    };
    let mut definitions = definitions_for_name(program, literal.type_name.as_str()).into_iter();
    let Some(definition) = definitions.next() else {
        return false;
    };
    definitions.next().is_none()
        && definition.type_parameters.is_empty()
        && if symbol.is_valid() {
            definition.symbol == symbol
        } else {
            definition.name == *name
        }
}

pub(super) fn type_is_caller_isolated_local(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> bool {
    if let Some(verdict) = with_isolation_cache(program, |cache| {
        cache.isolation.get(&(handle, false)).copied()
    }) {
        return verdict;
    }
    let verdict = type_is_caller_isolated_local_inner(
        program,
        handle,
        &mut Vec::<TypeReferenceHandle>::new(),
        false,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut TypeIdentityMemo::new(),
    );
    with_isolation_cache(program, |cache| {
        cache.isolation.insert((handle, false), verdict);
    });
    verdict
}

/// Under an active substitution, the declared members of a generic
/// application are inspected with its own arguments bound to the
/// definition's `Type` parameters. An unbound parameter keeps the named
/// leaf and stays opaque.
pub(super) fn type_is_caller_isolated_local_in(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
    bindings: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    type_is_caller_isolated_local_inner(
        program,
        handle,
        &mut Vec::<TypeReferenceHandle>::new(),
        false,
        &mut Vec::new(),
        &mut bindings.to_vec(),
        &mut TypeIdentityMemo::new(),
    )
}

/// Erased recursive proof values can have finite constructor terms without a
/// runtime layout. Follow the same storage-shape law as ordinary isolation,
/// but an inline back-edge is not an alias. References still fail closed.
pub(super) fn type_is_caller_isolated_proof_value(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> bool {
    if let Some(verdict) = with_isolation_cache(program, |cache| {
        cache.isolation.get(&(handle, true)).copied()
    }) {
        return verdict;
    }
    let verdict = type_is_caller_isolated_local_inner(
        program,
        handle,
        &mut Vec::<TypeReferenceHandle>::new(),
        true,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut TypeIdentityMemo::new(),
    );
    with_isolation_cache(program, |cache| {
        cache.isolation.insert((handle, true), verdict);
    });
    verdict
}

/// `visiting` records instantiated containers, not bare definition symbols:
/// `Wrap<Wrap<u64>>` is finite while a genuinely recursive instantiation
/// re-encounters an equal container.
fn type_is_caller_isolated_local_inner(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
    visiting: &mut Vec<TypeReferenceHandle>,
    proof_values: bool,
    isolated_parameters: &mut Vec<SymbolHandle>,
    bindings: &mut TypeBindings,
    identities: &mut TypeIdentityMemo,
) -> bool {
    let handle = substituted_head(program, handle, bindings);
    if program.primitive_type_reference(handle).is_some() {
        return true;
    }
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Constrained { base_type, .. } => type_is_caller_isolated_local_inner(
            program,
            *base_type,
            visiting,
            proof_values,
            isolated_parameters,
            bindings,
            identities,
        ),
        // A by-value array or slice reaches exactly what its elements reach;
        // a `[u8]` argument carries no reference that a callee could write
        // through, so it must not turn a boundary frame incomplete.
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => type_is_caller_isolated_local_inner(
            program,
            *element_type,
            visiting,
            proof_values,
            isolated_parameters,
            bindings,
            identities,
        ),
        TypeReferenceNode::Named { symbol, name } => {
            if proof_values && symbol.is_valid() && isolated_parameters.contains(symbol) {
                return true;
            }
            // The unbounded integer atoms are values on every route: a
            // `UInt` field reaches no reference, so a record holding one is
            // its owner's storage exactly like a `u64` field.
            if matches!(
                program.symbols.builtin_type_atom(*symbol),
                Some(symbols::BuiltinTypeAtom::UInt | symbols::BuiltinTypeAtom::Int)
            ) {
                return true;
            }
            let mut definitions = if symbol.is_valid() {
                definitions_for_symbol(program, *symbol).into_iter()
            } else {
                definitions_for_name(program, name).into_iter()
            };
            let Some(definition) = definitions.next() else {
                return false;
            };
            if definitions.next().is_some() {
                return false;
            }
            definition.type_parameters.is_empty()
                && data_definition_is_caller_isolated(
                    program,
                    definition,
                    Some(handle),
                    visiting,
                    proof_values,
                    isolated_parameters,
                    bindings,
                    identities,
                )
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } if proof_values => {
            if !base_symbol.is_valid() {
                return false;
            }
            let Some(definition) = definitions_for_symbol(program, *base_symbol)
                .first()
                .copied()
            else {
                return false;
            };
            let parameters = program.data_type_parameters(definition);
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            if parameters.len() != arguments.len()
                || parameters.iter().any(|parameter| {
                    !parameter.symbol.is_valid()
                        || program.symbols.get(parameter.symbol).kind != symbols::SymbolKind::TypeParameter
                        || program.symbols.get(parameter.symbol).parent != definition.symbol
                        || !matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                })
                // Check actual arguments before consulting the nominal cycle
                // guard: Nest<T> -> Nest<&mut T> introduces authority even
                // though the data-definition symbol is already on the path.
                || !arguments.iter().all(|argument| {
                    type_is_caller_isolated_local_inner(
                        program,
                        *argument,
                        visiting,
                        true,
                        isolated_parameters,
                        bindings,
                        identities,
                    )
                })
            {
                return false;
            }
            // Every actual argument has independently proved the same unary
            // property. Substituting these exact binders therefore preserves
            // isolation without allocating a second tree of substituted types.
            let parameter_count = isolated_parameters.len();
            isolated_parameters.extend(parameters.iter().map(|parameter| parameter.symbol));
            let isolated = data_definition_is_caller_isolated(
                program,
                definition,
                Some(handle),
                visiting,
                true,
                isolated_parameters,
                bindings,
                identities,
            );
            isolated_parameters.truncate(parameter_count);
            isolated
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            // An applied carrier is inspectable exactly when every `Type`
            // parameter of its definition binds to a supplied argument; the
            // substituted member walk then carries the same meaning as a
            // monomorphic field type. A reference-valued actual still fails
            // closed at the substituted `Reference` arm.
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments)
                .to_vec();
            let mark = bindings.len();
            let Some(definition) =
                push_generic_application_bindings(program, *base_symbol, &arguments, bindings)
            else {
                return false;
            };
            let isolated = data_definition_is_caller_isolated(
                program,
                definition,
                Some(handle),
                visiting,
                proof_values,
                isolated_parameters,
                bindings,
                identities,
            );
            bindings.truncate(mark);
            isolated
        }
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}

pub(super) fn struct_literal_type_is_caller_isolated(
    program: &TypedTrees,
    literal: &TableStructLiteral,
) -> bool {
    let mut definitions = definitions_for_name(program, literal.type_name.as_str()).into_iter();
    let Some(definition) = definitions.next() else {
        return false;
    };
    let unique_shape = match literal.case_name.as_ref() {
        None => program
            .data_members(definition)
            .iter()
            .all(|member| matches!(member, DataMember::Field(_))),
        Some(case_name) => {
            let mut variants = program
                .data_members(definition)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Variant(variant) if variant.name == *case_name => Some(variant),
                    _ => None,
                });
            variants.next().is_some() && variants.next().is_none()
        }
    };
    definitions.next().is_none()
        && unique_shape
        && data_definition_is_caller_isolated(
            program,
            definition,
            None,
            &mut Vec::new(),
            false,
            &mut Vec::new(),
            &mut Vec::new(),
            &mut TypeIdentityMemo::new(),
        )
}

pub(super) fn data_definition_has_only_owned_storage(
    program: &TypedTrees,
    definition: &typed_trees::data::DataDefinition,
) -> bool {
    let key = definition as *const typed_trees::data::DataDefinition;
    if let Some(verdict) =
        with_isolation_cache(program, |cache| cache.definitions.get(&key).copied())
    {
        return verdict;
    }
    let verdict = data_definition_is_caller_isolated(
        program,
        definition,
        None,
        &mut Vec::new(),
        false,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut TypeIdentityMemo::new(),
    );
    with_isolation_cache(program, |cache| {
        cache.definitions.insert(key, verdict);
    });
    verdict
}

/// `container` is the instantiated type whose member walk is about to run;
/// a genuinely recursive instantiation revisits an equal container, while a
/// nested application at different arguments is a distinct finite shape. A
/// definition-level entry has no spelling, so it cannot seed the guard.
fn data_definition_is_caller_isolated(
    program: &TypedTrees,
    definition: &typed_trees::data::DataDefinition,
    container: Option<TypeReferenceHandle>,
    visiting: &mut Vec<TypeReferenceHandle>,
    proof_values: bool,
    isolated_parameters: &mut Vec<SymbolHandle>,
    bindings: &mut TypeBindings,
    identities: &mut TypeIdentityMemo,
) -> bool {
    if !definition.type_parameters.is_empty() {
        let parameters_bound = program
            .data_type_parameters(definition)
            .iter()
            .all(|parameter| {
                if proof_values {
                    isolated_parameters.contains(&parameter.symbol)
                } else {
                    // A parameterized definition is inspectable only inside
                    // the generic application that bound those parameters.
                    bindings
                        .iter()
                        .any(|(symbol, _)| *symbol == parameter.symbol)
                }
            });
        if !parameters_bound {
            return false;
        }
    }
    if proof_values && definition.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
        return false;
    }
    if container.is_some_and(|container| {
        visiting.iter().any(|visited| {
            // An equal handle is trivially a storage match; skip the
            // normalized-identity comparison for direct re-encounters.
            *visited == container
                || aggregate_storage_types_match_in(
                    program, *visited, container, bindings, identities,
                )
        })
    }) {
        return proof_values;
    }
    if let Some(container) = container {
        visiting.push(container);
    }
    let isolated = program
        .data_members(definition)
        .iter()
        .all(|member| match member {
            DataMember::Field(field) => type_is_caller_isolated_local_inner(
                program,
                field.type_reference,
                visiting,
                proof_values,
                isolated_parameters,
                bindings,
                identities,
            ),
            DataMember::Variant(variant) => {
                program.data_payload_fields(variant).iter().all(|field| {
                    type_is_caller_isolated_local_inner(
                        program,
                        field.type_reference,
                        visiting,
                        proof_values,
                        isolated_parameters,
                        bindings,
                        identities,
                    )
                })
            }
        });
    if container.is_some() {
        visiting.pop();
    }
    isolated
}
