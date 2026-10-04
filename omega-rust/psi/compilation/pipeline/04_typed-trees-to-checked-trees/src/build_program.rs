//! The executable declaration context of one Build activation.
//!
//! Resolution has already given each source instance its Product or Build
//! identity. Preserve the complete Build declaration surface through ordinary
//! specialization: overload, proof, provider and cleanup selection must not be
//! guessed from a preliminary call graph. Product declarations remain frozen for
//! designated selection operations, never ordinary callable lookup. A descriptor
//! operand does not seed host type-work; ordinary signature, storage and value
//! references restore that declaration if Build also uses it. This is a typed
//! input projection, not a checked judgment or post-check product pruning.

use std::collections::HashSet;
use std::sync::Arc;

use crate::authored_selections::static_argument_role;
use diagnostics::Diagnostic;
use source::{DependencyScope, SourceOrigin};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::references::{
    DeclarationSelection, StaticArgumentClassifier, collect_program_references_with_roles,
    collect_selected_program_references,
};

/// Prepare the complete admitted Build context without changing symbol handles.
/// The result must still pass ordinary checking before any machine executes.
pub fn project_build_program(
    original: &TypedTrees,
    entry: SymbolHandle,
) -> Result<TypedTrees, Vec<Diagnostic>> {
    // Instantiation preserves template source provenance for diagnostics, not
    // the purpose of the use that demanded the instance. In particular, a
    // Product application of a toolchain generic is not an independent Build
    // declaration. Exact references and concrete carrier ownership admit it.
    let mut instances = original
        .data_definitions()
        .iter()
        .filter(|data| data.generic_instance.is_some())
        .map(|data| data.symbol)
        .collect::<HashSet<_>>();
    instances.extend(
        original
            .machines()
            .iter()
            .filter(|machine| machine.generic_data_template.is_valid())
            .map(|machine| machine.symbol),
    );
    instances.extend(
        original
            .machine_specializations
            .iter()
            .map(|specialization| specialization.instance),
    );
    let is_generated = |mut symbol: SymbolHandle| {
        while symbol.is_valid() {
            let declaration = original.symbols.get(symbol);
            if instances.contains(&symbol) || declaration.generated_from.is_valid() {
                return true;
            }
            symbol = declaration.parent;
        }
        false
    };
    let in_build_context = |symbol| {
        original
            .symbols
            .symbol_provenance_source_span(symbol)
            .and_then(|span| original.symbols.source_file(span))
            .is_some_and(|file| {
                file.origin == SourceOrigin::Toolchain
                    || file.dependency_scope == DependencyScope::Build
            })
    };
    if !original
        .machines()
        .iter()
        .any(|machine| machine.symbol == entry)
        || !in_build_context(entry)
    {
        return Err(vec![Diagnostic::error(
            "selected Build entry has no exact Build-context declaration custody",
        )]);
    }
    // Purpose selects the checking context, not the executable target member.
    // Every Build sibling still owns constants and proof obligations under
    // Build's provider authority. Dropping siblings here sends their unfinished
    // work to Product completion, which cannot supply that authority. Target
    // preparation has already bound ordinary calls to the execution-profile
    // member; retaining its siblings does not make those calls target-dependent.
    let generated_methods = original
        .machines()
        .iter()
        .filter(|machine| {
            machine.generic_data_template.is_valid() && machine.attached_data_symbol.is_valid()
        })
        .map(|machine| (machine.attached_data_symbol, machine.symbol))
        .collect::<Vec<_>>();
    let descriptors = build_descriptor_declarations(original);
    let machines = original
        .machines()
        .iter()
        .map(|machine| machine.symbol)
        .collect::<HashSet<_>>();
    let mut program = project_declarations(
        original,
        |symbol| {
            in_build_context(symbol)
                && !is_generated(symbol)
                && (machines.contains(&symbol) || !descriptors.contains(&symbol))
        },
        |symbol| {
            is_generated(symbol)
                || (in_build_context(symbol) && descriptors.contains(&symbol))
                || original
                    .symbols
                    .symbol_provenance_source_span(symbol)
                    .is_none()
        },
        &generated_methods,
        Some(static_argument_role),
    );
    program.product_declaration_frontier = Some(Arc::new(original.clone()));
    Ok(program)
}

/// A description can mention an entire carrier/conformance graph. None of that
/// graph becomes host type-work merely through the selection operation. Keep
/// ordinary Build bodies as roots, then restore any described declaration they
/// actually use through the same value/signature/proof reference traversal.
fn build_descriptor_declarations(program: &TypedTrees) -> HashSet<SymbolHandle> {
    let references = collect_program_references_with_roles(
        program,
        &DeclarationSelection::all(),
        Some(static_argument_role),
    );
    let mut pending = references
        .descriptor_symbols
        .into_iter()
        .collect::<Vec<_>>();
    let mut descriptors = HashSet::new();
    while !pending.is_empty() {
        let mut roots = Vec::new();
        for mut symbol in pending.drain(..) {
            while symbol.is_valid() && descriptors.insert(symbol) {
                roots.push(symbol);
                symbol = program.symbols.get(symbol).parent;
            }
        }
        if roots.is_empty() {
            break;
        }
        pending.extend(
            collect_selected_program_references(
                program,
                &DeclarationSelection::from_symbols(roots),
            )
            .symbols,
        );
    }
    descriptors
}

/// Private execution context rooted in exact declarations and their generated
/// children. This selects no target and grants no checking or execution
/// authority. Any frozen Product descriptor already present is preserved.
pub fn project_selected_program(
    original: &TypedTrees,
    selection: &typed_trees::references::DeclarationSelection,
) -> TypedTrees {
    if selection.is_all() {
        return original.clone();
    }
    project_declarations(
        original,
        |symbol| selection.contains(symbol),
        |symbol| {
            original
                .symbols
                .symbol_provenance_source_span(symbol)
                .is_none()
        },
        &[],
        None,
    )
}

fn project_declarations(
    original: &TypedTrees,
    selected_root: impl Fn(SymbolHandle) -> bool,
    retain_reference: impl Fn(SymbolHandle) -> bool,
    declaration_children: &[(SymbolHandle, SymbolHandle)],
    classify_static_arguments: Option<StaticArgumentClassifier>,
) -> TypedTrees {
    let mut program = original.clone();
    // A changed roster is a different program even when backing arena lengths
    // do not change. Program-owned lookup memos must not reuse the original.
    program.identity = Default::default();

    let mut synthesized = HashSet::new();
    let mut removed_declarations = HashSet::new();
    loop {
        let selected = |symbol| selected_root(symbol) || synthesized.contains(&symbol);
        removed_declarations.clear();
        // Retain stable backing handles while selecting declaration roots.
        macro_rules! project_roots {
            ($field:ident, $table:ident) => {{
                let declarations = original.tables.$table.span_or_empty(original.roots.$field);
                removed_declarations.extend(
                    declarations
                        .iter()
                        .filter(|declaration| !selected(declaration.symbol))
                        .map(|declaration| declaration.symbol),
                );
                let roots = declarations
                    .iter()
                    .filter(|declaration| selected(declaration.symbol))
                    .cloned()
                    .collect::<Vec<_>>();
                program.roots.$field = program.tables.$table.insert_many(roots);
            }};
        }
        project_roots!(const_declarations, const_declarations);
        project_roots!(data_definitions, data_definitions);
        project_roots!(domain_definitions, domain_definitions);
        project_roots!(measures, measures);
        project_roots!(operators, operators);
        project_roots!(machine_token_bindings, operators);
        project_roots!(propositions, propositions);
        project_roots!(mathematical_definitions, mathematical_definitions);
        project_roots!(traits, traits);
        project_roots!(conformances, conformances);
        project_roots!(wire_schemas, wire_schemas);
        let machines = original
            .machines()
            .iter()
            .filter(|machine| selected(machine.symbol))
            .cloned()
            .collect::<Vec<_>>();
        program.roots.machines = program.tables.machines.insert_many(machines);

        // Generated declarations may lack a location or retain their template's
        // location. Exact inbound references carry their work ownership.
        let references = collect_program_references_with_roles(
            &program,
            &DeclarationSelection::all(),
            classify_static_arguments,
        );
        let before = synthesized.len();
        for mut symbol in references.symbols {
            while symbol.is_valid() && retain_reference(symbol) {
                synthesized.insert(symbol);
                symbol = original.symbols.get(symbol).parent;
            }
        }
        // A selected concrete data instance owns its generated attached method
        // declarations, including obligations in methods not called by Build.
        for &(owner, child) in declaration_children {
            if selected_root(owner) || synthesized.contains(&owner) {
                synthesized.insert(child);
            }
        }
        if synthesized.len() == before {
            break;
        }
    }
    let retained = program
        .machines()
        .iter()
        .map(|machine| machine.symbol)
        .collect::<HashSet<_>>();
    let removed = original
        .machines()
        .iter()
        .map(|machine| machine.symbol)
        .filter(|symbol| !retained.contains(symbol))
        .collect::<HashSet<_>>();
    removed_declarations.extend(removed.iter().copied());
    crate::product_pruning::rewrite::apply_typed_pruning(&mut program, &retained, &removed);
    crate::product_pruning::rewrite::apply_declaration_sidecar_pruning(
        &mut program,
        &removed_declarations,
    );
    let original_references = typed_trees::references::collect_program_references(original);
    let selected_references = typed_trees::references::collect_program_references(&program);
    // Exact owned occurrences can leave this checking context only when all
    // their declaration-root owners leave. Unknown/orphan ledger IDs remain
    // obligations; shared expression copies retain their original identity.
    program
        .excluded_authored_selection_occurrences
        .retain(|occurrence| {
            !selected_references
                .authored_selection_occurrences
                .contains(occurrence)
        });
    program.excluded_authored_selection_occurrences.extend(
        original_references
            .authored_selection_occurrences
            .difference(&selected_references.authored_selection_occurrences)
            .copied(),
    );
    program
        .excluded_authored_selection_occurrences
        .sort_unstable();
    program.excluded_authored_selection_occurrences.dedup();
    let expressions = selected_references.expressions;
    program
        .expression_table
        .retain_expression_roots(|handle| expressions.contains(&handle));
    program.target_machine_families.retain_mut(|family| {
        family
            .members
            .retain(|member| retained.contains(&member.machine));
        !family.members.is_empty()
    });
    program
}

/// Occurrences outside this exact checking context remain in the ledger for
/// their owners: frozen Product descriptors or declarations removed by private
/// projection. Stable IDs are never erased or reassigned.
pub(crate) fn descriptor_selection_occurrences(
    program: &TypedTrees,
) -> HashSet<typed_trees::AuthoredDeclarationSelectionOccurrenceId> {
    program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| !program.checks_authored_selection(selection))
        .map(|selection| selection.occurrence_id())
        .collect()
}
