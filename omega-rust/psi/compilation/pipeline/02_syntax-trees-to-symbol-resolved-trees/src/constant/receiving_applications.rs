/* Receivers and selected-input joins for constant argument finalization. */

use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::types::TypeReference;
use symbols::SymbolHandle;

#[cfg(test)]
std::thread_local! {
    static RECEIVING_APPLICATION_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/*
    Ordinary symbol assignment publishes into the retained application. Copies
    may share its arguments, but cannot retarget its selected declaration.
    Keep conflicting inputs intact until finalization can report the error.
*/
pub(crate) fn bind_receiving_application(
    references: &mut symbol_resolved_trees::types::TypeReferenceArena,
    retained: symbol_resolved_trees::types::ReceivingApplicationHandle,
    name: &symbol_resolved_trees::name::DiagnosticName,
    selected: SymbolHandle,
    arguments: arena::HandleSpan<TypeReference>,
) {
    let references = &mut references.receiving_applications;
    if !retained.is_valid() || !references.is_valid(retained) {
        return;
    }
    let application = references.get_mut(retained);
    if application.name != *name
        || application.arguments != arguments
        || (application.selected.is_valid() && application.selected != selected)
    {
        application.conflict = Some(symbol_resolved_trees::types::ReceivingApplicationConflict {
            attempted_declaration: selected,
            attempted_arguments: arguments,
        });
    } else {
        application.selected = selected;
    }
}

/*
    Each declaration supplies its binder span once. Only retained applications
    join this declaration walk; finalization reads their published spans directly.
*/
pub(super) fn bind_receiving_parameters(
    program: &mut SymbolResolvedTrees,
    slots: &[crate::resolution::lowerer::PendingConstArgumentSlot],
) {
    if slots.is_empty() {
        return;
    }
    let references = &mut program
        .tables
        .declarations
        .child_type_references
        .receiving_applications;
    let mut applications = slots
        .iter()
        .filter_map(|slot| {
            if !references.is_valid(slot.application) {
                return None;
            }
            let application = references.get(slot.application);
            application
                .selected
                .is_valid()
                .then_some((application.selected, slot.application))
        })
        .collect::<Vec<_>>();
    applications.sort_unstable_by_key(|(symbol, application)| {
        (
            symbol.arena_index(),
            symbol.generation(),
            application.arena_index(),
            application.generation(),
        )
    });
    applications.dedup();
    if applications.is_empty() {
        return;
    }
    let parameters = &program.tables.declarations.data_type_parameters;
    let mut publish =
        |symbol: SymbolHandle,
         binders: arena::HandleSpan<symbol_resolved_trees::data::TypeParameter>| {
            let start = applications
                .partition_point(|(selected, _)| selected.arena_index() < symbol.arena_index());
            for (selected, handle) in applications[start..]
                .iter()
                .take_while(|(selected, _)| selected.arena_index() == symbol.arena_index())
            {
                if *selected == symbol {
                    references.get_mut(*handle).parameters = binders;
                }
            }
        };
    for definition in program.roots.data_definitions.iter() {
        publish(definition.symbol, definition.type_parameters);
    }
    for alias in program.roots.type_aliases.iter() {
        publish(alias.symbol, alias.type_parameters);
    }
    for domain in program.roots.domain_definitions.iter() {
        let mut binders = domain.type_parameters;
        if parameters.span_or_empty(binders).first().is_some_and(|parameter|
            matches!(parameter.kind, symbol_resolved_trees::data::TypeParameterKind::Type)
            && matches!(&domain.target_type, TypeReference::Named { symbol, .. } if *symbol == parameter.symbol))
        {
            binders = arena::HandleSpan::from_parts_or_empty(binders.handle_at(1), binders.count() - 1);
        }
        publish(domain.symbol, binders);
    }
}

#[cfg(test)]
std::thread_local! {
    pub(super) static FINALIZATION_WORK: std::cell::Cell<(usize, usize, usize)> = const { std::cell::Cell::new((0, 0, 0)) };
}

pub(super) struct ConstArgumentInputs {
    pub(super) declarations: Vec<Option<usize>>,
    pub(super) duplicate_declarations: Vec<bool>,
    pub(super) slots: Vec<crate::resolution::lowerer::PendingConstArgumentSlot>,
    pub(super) slot_runs: Vec<std::ops::Range<usize>>,
}

impl ConstArgumentInputs {
    /*
        Join only the selected declaration family once. A selection then reads
        its exact declaration and contiguous slots without repeating either
        reverse search for every occurrence.
    */
    pub(super) fn prepare(
        program: &SymbolResolvedTrees,
        pending: &[crate::resolution::lowerer::PendingConstArgumentSelection],
        slots: &[crate::resolution::lowerer::PendingConstArgumentSlot],
    ) -> Result<Self, Diagnostic> {
        let key = |span: source::SourceSpan| (span.source_id.0, span.span.start, span.span.end);
        let mut selected = pending
            .iter()
            .enumerate()
            .map(|(ordinal, selection)| (key(selection.origin.declaration), ordinal))
            .collect::<Vec<_>>();
        selected.sort_unstable();
        let mut declarations = vec![None; pending.len()];
        let mut duplicate_declarations = vec![false; pending.len()];
        if !selected.is_empty() {
            for (declaration_ordinal, declaration) in program.const_declarations.iter().enumerate()
            {
                #[cfg(test)]
                FINALIZATION_WORK.with(|work| {
                    let (preparation, declarations, slots) = work.get();
                    work.set((preparation + 1, declarations, slots));
                });
                let Some(reference) = program.symbols.symbol_source_span(declaration.symbol) else {
                    continue;
                };
                let reference = key(reference);
                let start = selected.partition_point(|(selected, _)| *selected < reference);
                for (_, selection) in selected[start..]
                    .iter()
                    .take_while(|(selected, _)| *selected == reference)
                {
                    if declarations[*selection]
                        .replace(declaration_ordinal)
                        .is_some()
                    {
                        duplicate_declarations[*selection] = true;
                    }
                }
            }
        }
        let mut slots = slots.to_vec();
        slots.sort_by_key(|slot| slot.selection);
        if slots
            .last()
            .is_some_and(|slot| slot.selection >= pending.len())
        {
            return Err(Diagnostic::error(
                "constant receiving slot names an unknown selection",
            ));
        }
        let mut slot_runs = Vec::with_capacity(pending.len());
        let mut cursor = 0;
        for selection in 0..pending.len() {
            let start = cursor;
            while cursor < slots.len() && slots[cursor].selection == selection {
                cursor += 1;
            }
            slot_runs.push(start..cursor);
        }
        Ok(Self {
            declarations,
            duplicate_declarations,
            slots,
            slot_runs,
        })
    }
}

/* Read the exact application retained by lowering, never its possible containers. */
pub(super) fn receiving_parameters<'program>(
    program: &'program SymbolResolvedTrees,
    slot: &crate::resolution::lowerer::PendingConstArgumentSlot,
) -> Result<&'program [symbol_resolved_trees::data::TypeParameter], Diagnostic> {
    let references = &program
        .tables
        .declarations
        .child_type_references
        .receiving_applications;
    if !references.is_valid(slot.application) {
        return Err(Diagnostic::error(
            "constant receiving application has a stale or foreign handle",
        ));
    }
    #[cfg(test)]
    RECEIVING_APPLICATION_READS.with(|reads| reads.set(reads.get() + 1));
    let application = references.get(slot.application);
    if let Some(conflict) = application.conflict {
        return Err(Diagnostic::error(format!(
            "constant receiving application changed its selected owner or arguments: retained {:?} {:?}, attempted {:?} {:?}",
            application.selected,
            application.arguments,
            conflict.attempted_declaration,
            conflict.attempted_arguments,
        )));
    }
    if application.arguments != slot.arguments {
        return Err(Diagnostic::error(
            "constant receiving slot names a different application argument span",
        ));
    }
    let selected = application.selected;
    if !selected.is_valid() {
        return Err(Diagnostic::error(
            "constant receiving application has no resolved declaration",
        ));
    }
    let parameters = application.parameters;
    if parameters.is_empty()
        || program
            .tables
            .declarations
            .data_type_parameters
            .span(parameters)
            .is_none()
    {
        return Err(Diagnostic::error(
            "constant receiving application has no retained binder correspondence",
        ));
    }
    Ok(program.data_type_parameters(parameters))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolution::lowerer::PendingConstArgumentSlot;
    use symbol_resolved_trees::data::{
        DataDefinition, DataDefinitionStorage, TypeParameter, TypeParameterKind,
    };
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::types::{GenericTypeReference, GenericTypeReferenceStorage};

    fn application(
        program: &mut SymbolResolvedTrees,
        selected: SymbolHandle,
        arguments: arena::HandleSpan<TypeReference>,
    ) -> symbol_resolved_trees::types::ReceivingApplicationHandle {
        program
            .tables
            .declarations
            .child_type_references
            .receiving_applications
            .insert(symbol_resolved_trees::types::ReceivingApplication {
                selected,
                name: DiagnosticName::generated("Indexed"),
                arguments,
                ..Default::default()
            })
    }

    fn declaration(
        program: &mut SymbolResolvedTrees,
        selected: SymbolHandle,
        binder: SymbolHandle,
    ) {
        let parameters = program
            .tables
            .declarations
            .data_type_parameters
            .insert_many([TypeParameter {
                symbol: binder,
                name: DiagnosticName::generated("Selected"),
                kind: TypeParameterKind::Const {
                    type_reference: TypeReference::Unit,
                },
                ..Default::default()
            }]);
        program.roots.data_definitions.push(DataDefinition {
            symbol: selected,
            storage: DataDefinitionStorage {
                type_parameters: parameters,
                ..Default::default()
            },
            ..Default::default()
        });
    }

    #[test]
    fn selected_inputs_keep_all_slots_and_refuse_missing_or_duplicate_declarations() {
        let declaration_span =
            source::SourceSpan::new(source::SourceId(1), source::Span::new(10, 15));
        let mut builder = symbols::SymbolTableBuilder::default();
        let root = builder.insert_root(
            symbols::SymbolKind::Root,
            symbols::SymbolNameRef::Static("root"),
        );
        let selected = builder
            .insert_children(
                root,
                [(
                    symbols::SymbolKind::Const,
                    symbols::SymbolNameRef::OwnedSource {
                        value: "VALUE",
                        source_span: declaration_span,
                    },
                )],
            )
            .start();
        let mut program = SymbolResolvedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        program
            .roots
            .const_declarations
            .push(symbol_resolved_trees::constant::ConstDeclaration {
                symbol: selected,
                evaluated_value: Some(Default::default()),
                ..Default::default()
            });
        let selection = crate::resolution::lowerer::PendingConstArgumentSelection {
            origin: syntax_trees::types::ConstArgumentOrigin {
                declaration: declaration_span,
                ..Default::default()
            },
            exposure: None,
            is_argument_value: false,
        };
        let pending = [selection.clone(), selection];
        let slot = |selection, ordinal| PendingConstArgumentSlot {
            selection,
            ordinal,
            application: Default::default(),
            arguments: Default::default(),
        };
        let slots = [slot(1, 7), slot(0, 2), slot(0, 1)];
        let inputs = ConstArgumentInputs::prepare(&program, &pending, &slots).unwrap();
        assert_eq!(inputs.declarations, vec![Some(0), Some(0)]);
        assert_eq!(inputs.slot_runs, vec![0..2, 2..3]);
        assert_eq!(
            inputs
                .slots
                .iter()
                .map(|slot| slot.ordinal)
                .collect::<Vec<_>>(),
            vec![2, 1, 7]
        );
        let mut missing = pending[0].clone();
        missing.origin.declaration.span.start += 1;
        let error = crate::constant::selections::finalize_const_argument_selections(
            &mut program,
            &[missing],
            &[],
        )
        .unwrap_err();
        assert!(
            error
                .message
                .contains("lost its exact selected declaration")
        );
        program
            .roots
            .const_declarations
            .push(program.const_declarations[0].clone());
        let error = crate::constant::selections::finalize_const_argument_selections(
            &mut program,
            &pending,
            &[],
        )
        .unwrap_err();
        assert!(
            error
                .message
                .contains("declaration or value custody drifted")
        );
    }

    #[test]
    fn receiving_applications_do_not_infer_owners_from_shared_arguments() {
        let mut program = SymbolResolvedTrees::default();
        let first = SymbolHandle::from_arena_index(1);
        let second = SymbolHandle::from_arena_index(2);
        let first_binder = SymbolHandle::from_arena_index(3);
        let second_binder = SymbolHandle::from_arena_index(4);
        declaration(&mut program, first, first_binder);
        declaration(&mut program, second, second_binder);
        let arguments = program
            .tables
            .declarations
            .child_type_references
            .insert_many([TypeReference::Unit]);
        let first_application = application(&mut program, first, arguments);
        let second_application = application(&mut program, second, arguments);
        let slots =
            [first_application, second_application].map(|application| PendingConstArgumentSlot {
                selection: 0,
                application,
                arguments,
                ordinal: 0,
            });
        bind_receiving_parameters(&mut program, &slots);
        assert_eq!(
            receiving_parameters(&program, &slots[0]).unwrap()[0].symbol,
            first_binder
        );
        assert_eq!(
            receiving_parameters(&program, &slots[1]).unwrap()[0].symbol,
            second_binder
        );

        let other_arguments = program
            .tables
            .declarations
            .child_type_references
            .insert_many([TypeReference::Unit]);
        let wrong_slot = PendingConstArgumentSlot {
            arguments: other_arguments,
            ..slots[0]
        };
        assert!(
            receiving_parameters(&program, &wrong_slot)
                .unwrap_err()
                .message
                .contains("different application argument span")
        );
    }

    #[test]
    fn receiving_owner_reads_do_not_grow_with_unrelated_declarations() {
        let mut program = SymbolResolvedTrees::default();
        let selected = SymbolHandle::from_arena_index(1);
        declaration(&mut program, selected, SymbolHandle::from_arena_index(2));
        let arguments = program
            .tables
            .declarations
            .child_type_references
            .insert_many([TypeReference::Unit]);
        let application = application(&mut program, selected, arguments);
        let slot = PendingConstArgumentSlot {
            selection: 0,
            application,
            arguments,
            ordinal: 0,
        };
        bind_receiving_parameters(&mut program, &[slot]);
        for unrelated in [0, 4096] {
            for ordinal in 0..unrelated {
                declaration(
                    &mut program,
                    SymbolHandle::from_arena_index(100 + ordinal),
                    SymbolHandle::from_arena_index(10000 + ordinal),
                );
                program
                    .tables
                    .declarations
                    .child_type_references
                    .insert(TypeReference::Unit);
            }
            RECEIVING_APPLICATION_READS.with(|reads| reads.set(0));
            assert_eq!(receiving_parameters(&program, &slot).unwrap().len(), 1);
            assert_eq!(
                RECEIVING_APPLICATION_READS.with(std::cell::Cell::get),
                1,
                "unrelated declarations: {unrelated}"
            );
        }
    }

    #[test]
    fn receiving_copies_preserve_owner_and_refuse_substituted_owners() {
        let mut program = SymbolResolvedTrees::default();
        let selected = SymbolHandle::from_arena_index(1);
        let substituted = SymbolHandle::from_arena_index(2);
        declaration(&mut program, selected, SymbolHandle::from_arena_index(3));
        declaration(&mut program, substituted, SymbolHandle::from_arena_index(4));
        let arguments = program
            .tables
            .declarations
            .child_type_references
            .insert_many([TypeReference::Unit]);
        let retained = application(&mut program, selected, arguments);
        let source = program
            .tables
            .declarations
            .child_type_references
            .insert_many([TypeReference::Generic(GenericTypeReference {
                storage: GenericTypeReferenceStorage {
                    base_symbol: selected,
                    base_name: DiagnosticName::generated("Indexed"),
                    arguments,
                    receiving_application: retained,
                    ..Default::default()
                },
            })]);
        for substitutions in [
            Vec::new(),
            vec![(
                selected,
                TypeReference::Named {
                    symbol: substituted,
                    name: DiagnosticName::generated("Other"),
                    supplied_operand_owner: Default::default(),
                },
            )],
        ] {
            let copied = program.copy_type_arguments(source, &substitutions);
            let TypeReference::Generic(copy) = program.child_type_reference(copied.start()).clone()
            else {
                panic!("copied application");
            };
            assert_ne!(copy.receiving_application, retained);
            assert_ne!(copy.arguments, arguments);
            let copied_receipt = program
                .tables
                .declarations
                .child_type_references
                .receiving_applications
                .get(copy.receiving_application);
            assert_eq!(
                copied_receipt.conflict.is_some(),
                !substitutions.is_empty(),
                "copy itself must refuse substituted receiving declarations"
            );
            let slot = PendingConstArgumentSlot {
                selection: 0,
                application: copy.receiving_application,
                arguments: copy.arguments,
                ordinal: 0,
            };
            bind_receiving_parameters(&mut program, &[slot]);
            if substitutions.is_empty() {
                assert!(receiving_parameters(&program, &slot).is_ok());
            } else {
                assert!(
                    receiving_parameters(&program, &slot)
                        .unwrap_err()
                        .message
                        .contains("changed its selected owner")
                );
                let again = program.copy_type_arguments(copied, &[]);
                let TypeReference::Generic(again) = program.child_type_reference(again.start())
                else {
                    panic!("copied application");
                };
                assert!(
                    program
                        .tables
                        .declarations
                        .child_type_references
                        .receiving_applications
                        .get(again.receiving_application)
                        .conflict
                        .is_some()
                );
            }
            let original = program
                .tables
                .declarations
                .child_type_references
                .receiving_applications
                .get(retained);
            assert_eq!(original.selected, selected);
            assert!(original.conflict.is_none());
        }
    }

    #[test]
    fn receiving_rebinding_preserves_original_owner_arguments_and_conflicts() {
        let mut program = SymbolResolvedTrees::default();
        let selected = SymbolHandle::from_arena_index(1);
        let other = SymbolHandle::from_arena_index(2);
        let arguments = program
            .tables
            .declarations
            .child_type_references
            .insert_many([TypeReference::Unit]);
        let other_arguments = program
            .tables
            .declarations
            .child_type_references
            .insert_many([TypeReference::Unit]);
        let name = DiagnosticName::generated("Indexed");
        for (attempted, attempted_arguments) in [(other, arguments), (selected, other_arguments)] {
            let application = application(&mut program, selected, arguments);
            bind_receiving_application(
                &mut program.tables.declarations.child_type_references,
                application,
                &name,
                attempted,
                attempted_arguments,
            );
            bind_receiving_application(
                &mut program.tables.declarations.child_type_references,
                application,
                &name,
                selected,
                arguments,
            );
            let retained = program
                .tables
                .declarations
                .child_type_references
                .receiving_applications
                .get(application);
            assert_eq!(retained.selected, selected);
            assert_eq!(retained.arguments, arguments);
            assert_eq!(retained.conflict.unwrap().attempted_declaration, attempted);
            let slot = PendingConstArgumentSlot {
                selection: 0,
                application,
                arguments,
                ordinal: 0,
            };
            assert!(
                receiving_parameters(&program, &slot)
                    .unwrap_err()
                    .message
                    .contains("changed its selected owner or arguments")
            );
        }
    }

    #[test]
    fn receiving_applications_report_unresolved_and_stale_handles() {
        let mut program = SymbolResolvedTrees::default();
        let arguments = program
            .tables
            .declarations
            .child_type_references
            .insert_many([TypeReference::Unit]);
        let application = application(&mut program, SymbolHandle::invalid(), arguments);
        let slot = PendingConstArgumentSlot {
            selection: 0,
            application,
            arguments,
            ordinal: 0,
        };
        assert!(
            receiving_parameters(&program, &slot)
                .unwrap_err()
                .message
                .contains("no resolved declaration")
        );
        program
            .tables
            .declarations
            .child_type_references
            .receiving_applications
            .free(application);
        assert!(
            receiving_parameters(&program, &slot)
                .unwrap_err()
                .message
                .contains("stale or foreign handle")
        );
    }

    #[test]
    fn receiving_scoped_constants_keep_exact_carriers_and_bounded_finalization() {
        let source = include_str!(
            "../../../../../../../tests/fixtures/pass/constants/receiving_application/main.omg"
        );
        let resolve = |text: &str| {
            let mut sources = source::SourceMap::default();
            let source_id = sources
                .add("receiving_application/main.omg".into(), text.to_owned())
                .source_id;
            let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
            let syntax =
                tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
            crate::resolution::resolve(crate::resolution::ResolutionRequest {
                syntax,
                sources: Some(std::sync::Arc::new(sources)),
                top_level_bindings: Vec::new(),
                retained_base: None,
            })
        };
        let measure = |text: &str| {
            RECEIVING_APPLICATION_READS.with(|reads| reads.set(0));
            FINALIZATION_WORK.with(|work| work.set((0, 0, 0)));
            resolve(text).unwrap();
            (
                RECEIVING_APPLICATION_READS.with(std::cell::Cell::get),
                FINALIZATION_WORK.with(std::cell::Cell::get),
            )
        };
        let baseline = measure(source);
        assert!(
            baseline.0 > 0,
            "the program must exercise receiving application finalization: {baseline:?}"
        );
        let mut unrelated = source.to_owned();
        for ordinal in 0..64 {
            unrelated.push_str(&format!("\nconst UNRELATED_{ordinal}: u64 = {ordinal};\n"));
        }
        let enlarged = measure(&unrelated);
        assert_eq!(enlarged.0, baseline.0);
        assert_eq!((enlarged.1.1, enlarged.1.2), (baseline.1.1, baseline.1.2));
        assert!(
            enlarged.1.0 > baseline.1.0,
            "only one-time declaration preparation grows"
        );
        let invalid = include_str!(
            "../../../../../../../tests/fixtures/fail/constants/receiving_application/main.omg"
        );
        assert_eq!(
            source
                .lines()
                .skip(2)
                .collect::<Vec<_>>()
                .join("\n")
                .replace("const Selected: Index", "const Selected: u64"),
            invalid.lines().skip(2).collect::<Vec<_>>().join("\n")
        );
        let diagnostics = resolve(invalid).unwrap_err();
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.message ==
                "machine equation application has a constructor argument with a different declared carrier"),
            "{diagnostics:?}"
        );
    }
}
