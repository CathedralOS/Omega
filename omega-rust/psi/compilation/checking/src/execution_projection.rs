//! Executable projections of admitted typed declarations.
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

use crate::compiler_operations::static_argument_role;
use diagnostics::Diagnostic;
use source::{DependencyScope, SourceOrigin};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::references::{
    DeclarationSelection, StaticArgumentClassifier, collect_program_references_with_roles,
    collect_selected_program_references,
};

/// Prepare the complete admitted Build context without changing symbol handles.
/// The result must still pass ordinary checking before any machine executes.
pub fn project_build_program(
    original: &dyn crate::CheckingInput,
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
    if !original.machine(entry).is_some() || !in_build_context(entry) {
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
    program.product_declaration_frontier = Some(Arc::new(original.typed_program().clone()));
    Ok(program)
}

/// A description can mention an entire carrier/conformance graph. None of that
/// graph becomes host type-work merely through the selection operation. Keep
/// ordinary Build bodies as roots, then restore any described declaration they
/// actually use through the same value/signature/proof reference traversal.
fn build_descriptor_declarations(program: &dyn crate::CheckingInput) -> HashSet<SymbolHandle> {
    let references = collect_program_references_with_roles(
        program,
        &DeclarationSelection::all(),
        Some(static_argument_role),
    );
    let mut pending = references
        .descriptor_symbols
        .into_iter()
        .collect::<Vec<_>>();
    // Every authored `satisfies` conformance is description machinery: an
    // unselected carrier alternative must not root its opaque into the
    // projection any more than the selected one does.
    pending.extend(
        program
            .conformances()
            .iter()
            .map(|conformance| conformance.symbol),
    );
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
    original: &dyn crate::CheckingInput,
    selection: &typed_trees::references::DeclarationSelection,
) -> TypedTrees {
    if selection.is_all() {
        return original.typed_program().clone();
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
    original: &dyn crate::CheckingInput,
    selected_root: impl Fn(SymbolHandle) -> bool,
    retain_reference: impl Fn(SymbolHandle) -> bool,
    declaration_children: &[(SymbolHandle, SymbolHandle)],
    classify_static_arguments: Option<StaticArgumentClassifier>,
) -> TypedTrees {
    let mut program = original.typed_program().clone();
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
    retain_machine_roster(&mut program, &retained, &removed);
    apply_declaration_sidecar_pruning(&mut program, &removed_declarations);
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
    // An expression no declaration root reaches in the original program is
    // an orphan: a whole-bound fold replaced its parent with a literal, and
    // the orphan alone carries that subexpression's selection occurrence.
    // Its ID stayed an obligation above, so the orphan must stay with it as
    // that obligation's discharge site, exactly as in the original program.
    let expressions = selected_references.expressions;
    let excluded = &program.excluded_authored_selection_occurrences;
    let orphan_obligations = original
        .expression_table
        .iter_expressions()
        .map(|(handle, _)| handle)
        .filter(|handle| {
            !original_references.expressions.contains(handle)
                && original
                    .expression_table
                    .authored_selection_occurrences(*handle)
                    .any(|occurrence| excluded.binary_search(&occurrence).is_err())
        })
        .collect::<HashSet<_>>();
    program.expression_table.retain_expression_roots(|handle| {
        expressions.contains(&handle) || orphan_obligations.contains(&handle)
    });
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
pub fn descriptor_selection_occurrences(
    program: &dyn crate::CheckingInput,
) -> HashSet<typed_trees::AuthoredDeclarationSelectionOccurrenceId> {
    program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| !program.checks_authored_selection(selection))
        .map(|selection| selection.occurrence_id())
        .collect()
}

/// Rebuild machine roots and machine-keyed sidecars. Backing declarations stay
/// available to exact authored-selection and visibility evidence; only roots
/// determine which implementations belong to this executable program.
pub fn retain_machine_roster(
    program: &mut TypedTrees,
    retained: &HashSet<SymbolHandle>,
    pruned: &HashSet<SymbolHandle>,
) {
    // Append the retained roster without invalidating existing declaration
    // handles or erasing payloads still named by authored-selection evidence.
    let retained_machines: Vec<Machine> = program
        .machines()
        .iter()
        .filter(|machine| retained.contains(&machine.symbol))
        .cloned()
        .collect();
    program.roots.machines = program.tables.machines.insert_many(retained_machines);

    // Typed sidecars keyed by a machine identity.
    program
        .machine_specializations
        .retain(|row| retained.contains(&row.instance));
    // Trait requirement signatures own reach rows too; only rows owned by a
    // pruned machine leave.
    program
        .authored_service_reach_rows
        .retain(|row| !pruned.contains(&row.owner));
    program
        .evidence_forwardings
        .retain(|row| retained.contains(&row.machine_symbol));
    program
        .proof_output_calls
        .retain(|row| retained.contains(&row.machine_symbol));
    program
        .ranking_expression_custody
        .retain(|row| retained.contains(&row.machine));
}

/// Project sidecars whose exact declaration owner left the root roster.
/// Unknown/orphan owners are deliberately retained for ordinary validation:
/// absence is not authorization to discard malformed evidence.
fn apply_declaration_sidecar_pruning(program: &mut TypedTrees, removed: &HashSet<SymbolHandle>) {
    program.remove_trait_declaration_applications(|declaration| removed.contains(&declaration));
    let symbols = &program.symbols;
    let owner_removed = |mut symbol: SymbolHandle| {
        while symbol.is_valid() {
            if removed.contains(&symbol) {
                return true;
            }
            symbol = symbols.get(symbol).parent;
        }
        false
    };
    program
        .plan_laid_layouts
        .retain(|row| !owner_removed(row.data_symbol));
    program
        .placed_view_plans
        .retain(|row| !owner_removed(row.data_symbol));
    program
        .wire_schema_plans
        .retain(|row| !owner_removed(row.schema));
    program
        .authored_service_reach_rows
        .retain(|row| !owner_removed(row.owner));
    // This roster has no inbound handles; source roles join by declaration
    // symbols. Wire placement arenas and other referenced backing rows stay.
    let completions = program
        .boundary_completion_declarations
        .iter()
        .filter(|(_, row)| !owner_removed(row.owner))
        .map(|(_, row)| row.clone())
        .collect::<Vec<_>>();
    let mut declarations = arena::Arena::default();
    declarations.insert_many(completions);
    program.boundary_completion_declarations = declarations;
}
