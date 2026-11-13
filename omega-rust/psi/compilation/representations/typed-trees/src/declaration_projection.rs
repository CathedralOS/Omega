/*
We narrow a typed program to the declarations of its selected sources
(`project_source_declarations`).
*/

/*
A compilation types the toolchain's bundled sources once and then compiles many
programs against them. Each program keeps the declarations written in its own
selected sources, plus the declarations the compiler derived for them (generic
instances, specializations, generated declarations). A declaration of another
source leaves the program instead of travelling on unjudged. The projection
reads references only: it decides nothing about whether a kept declaration is
correct, and it removes every side table row whose owner left.
*/

use crate::references::{
    DeclarationSelection, collect_program_references, collect_selected_program_references,
};
use crate::{AuthoredDeclarationSelectionOccurrenceId, TypedTrees};
use symbols::SymbolHandle;

/// Symbols of one symbol table, stored by table slot. A slot remembers the
/// handle generation it was entered with, so a handle of another generation is
/// never mistaken for a member.
#[derive(Default)]
struct SymbolMembership {
    // `generation + 1` of the member in each slot; zero is an empty slot.
    generations: Vec<u32>,
    count: usize,
}

impl SymbolMembership {
    fn from_symbols(symbols: impl IntoIterator<Item = SymbolHandle>) -> Self {
        let mut membership = Self::default();
        for symbol in symbols {
            membership.insert(symbol);
        }
        membership
    }

    fn contains(&self, symbol: SymbolHandle) -> bool {
        symbol.is_valid()
            && self
                .generations
                .get(symbol.arena_index() as usize)
                .is_some_and(|generation| *generation == symbol.generation().wrapping_add(1))
    }

    fn insert(&mut self, symbol: SymbolHandle) {
        if !symbol.is_valid() || self.contains(symbol) {
            return;
        }
        let slot = symbol.arena_index() as usize;
        if self.generations.len() <= slot {
            self.generations.resize(slot + 1, 0);
        }
        if self.generations[slot] == 0 {
            self.count += 1;
        }
        self.generations[slot] = symbol.generation().wrapping_add(1);
    }

    fn len(&self) -> usize {
        self.count
    }

    fn clear(&mut self) {
        self.generations.clear();
        self.count = 0;
    }
}

/// Keep the declarations written in `sources`' selected files.
///
/// A declaration the compiler derived (a generic instance, a specialization or
/// a generated declaration) has no source of its own to be selected by, so it
/// stays only when a kept declaration refers to it. The methods generated for
/// an instantiated data type stay with that type. An authored declaration of
/// an unselected source leaves even when a kept declaration refers to it; the
/// caller decides what a program that still names one means (source
/// activation rejects that selection).
pub fn project_source_declarations(
    original: &TypedTrees,
    sources: &source::SourceMap,
) -> TypedTrees {
    let mut instances = SymbolMembership::from_symbols(
        original
            .data_definitions()
            .iter()
            .filter(|data| data.generic_instance.is_some())
            .map(|data| data.symbol),
    );
    for machine in original.machines() {
        if machine.generic_data_template.is_valid() {
            instances.insert(machine.symbol);
        }
    }
    for specialization in &original.machine_specializations {
        instances.insert(specialization.instance);
    }
    let derived = |mut symbol: SymbolHandle| {
        while symbol.is_valid() {
            let declaration = original.symbols.get(symbol);
            if instances.contains(symbol) || declaration.generated_from.is_valid() {
                return true;
            }
            symbol = declaration.parent;
        }
        false
    };
    let generated_methods = original
        .machines()
        .iter()
        .filter(|machine| machine.generic_data_template.is_valid())
        .map(|machine| (machine.attached_data_symbol, machine.symbol))
        .collect::<Vec<_>>();
    let mut program = project_declarations(
        original,
        |symbol| {
            !derived(symbol)
                && original
                    .symbols
                    .symbol_provenance_source_span(symbol)
                    .is_none_or(|span| sources.source_is_selected(span.source_id))
        },
        |symbol| {
            derived(symbol)
                || original
                    .symbols
                    .symbol_provenance_source_span(symbol)
                    .is_none()
        },
        &generated_methods,
    );
    // An authored selection written in a source that left, and that no kept
    // declaration reaches, is not this program's to check.
    let references = collect_program_references(&program);
    let mut excluded =
        std::mem::take(&mut program.excluded_authored_selection_occurrences).into_occurrences();
    excluded.extend(
        original
            .authored_declaration_selections()
            .iter()
            .filter_map(|selection| {
                let span = selection.source_span();
                (span.span.start != span.span.end
                    && !sources.source_is_selected(span.source_id)
                    && !references
                        .authored_selection_occurrences
                        .contains(&selection.occurrence_id()))
                .then_some(selection.occurrence_id())
            }),
    );
    excluded.sort_unstable();
    excluded.dedup();
    let is_excluded = |occurrence: &AuthoredDeclarationSelectionOccurrenceId| {
        excluded.binary_search(occurrence).is_ok()
    };
    let expressions = &original.expression_table;
    program.expression_table.retain_expression_roots(|handle| {
        references.expressions.contains(&handle)
            || expressions
                .authored_selection_occurrences(handle)
                .any(|occurrence| !is_excluded(&occurrence))
    });
    program.excluded_authored_selection_occurrences = excluded.into();
    program
}

/// Keep the declarations `selected_root` names and whatever they refer to.
///
/// A referenced declaration joins only when `retain_reference` admits it, and
/// then its enclosing declarations join with it. Each `(owner, declaration)`
/// pair in `declaration_children` keeps the declaration once its owner is
/// kept. A constant whose evaluation entry is kept stays with that entry.
/// Side tables lose the rows owned by a removed declaration.
fn project_declarations(
    original: &TypedTrees,
    selected_root: impl Fn(SymbolHandle) -> bool,
    retain_reference: impl Fn(SymbolHandle) -> bool,
    declaration_children: &[(SymbolHandle, SymbolHandle)],
) -> TypedTrees {
    let mut program = original.clone();
    program.identity = Default::default();

    let mut synthesized = SymbolMembership::default();
    let mut removed_declarations = SymbolMembership::default();
    loop {
        let selected = |symbol| selected_root(symbol) || synthesized.contains(symbol);
        removed_declarations.clear();
        macro_rules! project_roots {
            ($field:ident, $table:ident) => {{
                let declarations = original.tables.$table.span_or_empty(original.roots.$field);
                for declaration in declarations {
                    if !selected(declaration.symbol) {
                        removed_declarations.insert(declaration.symbol);
                    }
                }
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
        project_roots!(type_aliases, type_aliases);
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

        let references = collect_selected_program_references(
            original,
            &DeclarationSelection::from_program(&program),
        );
        let before = synthesized.len();
        for mut symbol in references.symbols {
            while symbol.is_valid() && retain_reference(symbol) {
                synthesized.insert(symbol);
                symbol = original.symbols.get(symbol).parent;
            }
        }
        for symbol in references.descriptor_symbols {
            if symbol.is_valid()
                && original.symbols.get(symbol).kind == symbols::SymbolKind::TypeAlias
            {
                synthesized.insert(symbol);
            }
        }
        for &(owner, child) in declaration_children {
            if selected_root(owner) || synthesized.contains(owner) {
                synthesized.insert(child);
            }
        }
        for declaration in original.const_declarations() {
            if !original
                .tables
                .machines
                .is_valid(declaration.evaluation_entry)
            {
                continue;
            }
            let entry = original
                .tables
                .machines
                .get(declaration.evaluation_entry)
                .symbol;
            if selected_root(entry) || synthesized.contains(entry) {
                synthesized.insert(declaration.symbol);
            }
        }
        if synthesized.len() == before {
            break;
        }
    }
    let retained =
        SymbolMembership::from_symbols(program.machines().iter().map(|machine| machine.symbol));
    let removed = SymbolMembership::from_symbols(
        original
            .machines()
            .iter()
            .map(|machine| machine.symbol)
            .filter(|symbol| !retained.contains(*symbol)),
    );
    for machine in original.machines() {
        if removed.contains(machine.symbol) {
            removed_declarations.insert(machine.symbol);
        }
    }
    remove_machine_rows(&mut program, &removed);
    remove_declaration_rows(&mut program, &removed_declarations);

    let original_references = collect_program_references(original);
    let selected_references = collect_program_references(&program);
    let mut excluded =
        std::mem::take(&mut program.excluded_authored_selection_occurrences).into_occurrences();
    excluded.retain(|occurrence| {
        !selected_references
            .authored_selection_occurrences
            .contains(occurrence)
    });
    excluded.extend(
        original_references
            .authored_selection_occurrences
            .difference(&selected_references.authored_selection_occurrences)
            .copied(),
    );
    excluded.sort_unstable();
    excluded.dedup();
    let is_excluded = |occurrence: &AuthoredDeclarationSelectionOccurrenceId| {
        excluded.binary_search(occurrence).is_ok()
    };
    // An expression no declaration owns can still carry an authored selection
    // that must be checked; it stays unless that selection was excluded.
    let expressions = &original.expression_table;
    program.expression_table.retain_expression_roots(|handle| {
        selected_references.expressions.contains(&handle)
            || (!original_references.expressions.contains(&handle)
                && expressions
                    .authored_selection_occurrences(handle)
                    .any(|occurrence| !is_excluded(&occurrence)))
    });
    program.excluded_authored_selection_occurrences = excluded.into();
    program.target_machine_families.retain_mut(|family| {
        family
            .members
            .retain(|member| retained.contains(member.machine));
        !family.members.is_empty()
    });
    program
}

/// Drop the rows that describe a removed machine.
fn remove_machine_rows(program: &mut TypedTrees, removed: &SymbolMembership) {
    program
        .machine_specializations
        .retain(|row| !removed.contains(row.instance));
    let designated = program
        .tables
        .designated_static_type_arguments
        .iter()
        .filter(|(_, row)| !removed.contains(row.owner))
        .map(|(_, row)| row.clone())
        .collect::<Vec<_>>();
    let mut arguments = arena::Arena::default();
    arguments.insert_many(designated);
    program.tables.designated_static_type_arguments = arguments;
    let captures = program
        .captured_machine_applications
        .iter()
        .filter(|(_, row)| !removed.contains(row.instance))
        .map(|(_, row)| row.clone())
        .collect::<Vec<_>>();
    let mut applications = arena::Arena::default();
    applications.insert_many(captures);
    program.captured_machine_applications = applications;
    program
        .authored_service_reach_rows
        .retain(|row| !removed.contains(row.owner));
    program
        .evidence_forwardings
        .retain(|row| !removed.contains(row.machine_symbol));
    program
        .proof_output_calls
        .retain(|row| !removed.contains(row.machine_symbol));
    program
        .ranking_expression_custody
        .retain(|row| !removed.contains(row.machine));
}

/// Drop the rows owned by a removed declaration or by anything declared
/// inside one.
fn remove_declaration_rows(program: &mut TypedTrees, removed: &SymbolMembership) {
    let symbols = &program.symbols;
    let owner_removed = |mut symbol: SymbolHandle| {
        while symbol.is_valid() {
            if removed.contains(symbol) {
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
        .access_view_plans
        .retain(|row| !owner_removed(row.data_symbol));
    let lifecycle_plans = program
        .access_lifecycle_plans
        .iter()
        .filter(|(_, row)| match row {
            crate::typed_trees::AccessLifecyclePlan::Absent => false,
            crate::typed_trees::AccessLifecyclePlan::Evaluated { callable, .. } => {
                !owner_removed(*callable)
                    && !program
                        .closed_requirement_application(*callable)
                        .is_some_and(|application| {
                            owner_removed(application.declaring_trait)
                                || owner_removed(application.template)
                        })
                    && !program
                        .machine_specialization(*callable)
                        .is_some_and(|application| owner_removed(application.template))
            }
        })
        .map(|(_, row)| row.clone())
        .collect::<Vec<_>>();
    let mut lifecycle_catalog = arena::Arena::default();
    lifecycle_catalog.insert_many(lifecycle_plans);
    program.access_lifecycle_plans = lifecycle_catalog;
    program
        .wire_schema_plans
        .retain(|row| !owner_removed(row.schema));
    program
        .authored_service_reach_rows
        .retain(|row| !owner_removed(row.owner));
    let completions = program
        .boundary_completion_declarations
        .iter()
        .filter(|(_, row)| !owner_removed(row.owner))
        .map(|(_, row)| row.clone())
        .collect::<Vec<_>>();
    let mut declarations = arena::Arena::default();
    declarations.insert_many(completions);
    program.boundary_completion_declarations = declarations;
    // A retained Product selection leaves only when its owner, the machine
    // application that supplied it and the declaration it names were all
    // removed; otherwise it must still be checked.
    let products = program
        .retained_product_application_handles
        .span_or_empty(program.roots.retained_product_applications)
        .iter()
        .copied()
        .filter(|handle| {
            if !program.product_operand_applications.is_valid(*handle) {
                return true;
            }
            let application = program.product_operand_applications.get(*handle);
            let source_owner = if application.supplying_application.is_valid() {
                program
                    .expression_table
                    .static_machine_application(application.supplying_application)
                    .lexical_owner
            } else {
                SymbolHandle::invalid()
            };
            !(owner_removed(application.owner)
                && owner_removed(source_owner)
                && owner_removed(symbols.authored_origin(application.declaration)))
        })
        .collect::<Vec<_>>();
    program.roots.retained_product_applications = program
        .tables
        .retained_product_application_handles
        .insert_many(products);
    program.remove_trait_declaration_applications(|declaration| removed.contains(declaration));
    program.retain_live_trait_implementation_applications(|symbol| removed.contains(symbol));
    program.retain_live_trait_callable_applications();
}
