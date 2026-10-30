//! Bind selected cleanup occurrences to their exact source and edge context.
//!
//! Specialization owns the lexical symbol correspondence. Consume its exact
//! generated-from links instead of reconstructing parameter/local identity from
//! spelling, declaration order, or generated machine names. The occurrence and
//! its excluded subtrees still come from the one custody producer.
pub(crate) mod entry_flow;
mod receiver_loans;
mod requirements;
mod sequence;
pub(crate) use receiver_loans::retain_selected_receiver_loans;
pub(crate) use requirements::{
    check_cleanup_requirements, infer_entry_requirements, occurrence_contexts,
};

use arena::Arena;
use checked_trees::{ContextualCleanupApplication, ContextualCleanupFacts, FlowFacts};
use facts::{PlaceRoot, PlaceSegment};
use language_semantics::PermissionEventSource;
use symbols::SymbolHandle;

pub(crate) fn bind_applications(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    cleanup: &mut ContextualCleanupFacts,
) {
    let mut applications = Arena::default();
    for (instance_handle, instance) in cleanup.dispositions.iter() {
        let Some(specialization) = program.machine_specialization(instance.machine) else {
            continue;
        };
        let state = source_symbol(program, instance.state, instance.machine);
        let Some(root) = instance.root.as_symbol() else {
            // A temporary result is established inside this invocation. It is
            // not a lexical input whose source can be obtained from the clone's
            // declaration correspondence.
            continue;
        };
        let root = PlaceRoot::Symbol(source_symbol(program, root, instance.machine));
        let path = source_path(
            program,
            flow.ownership.segments.span_or_empty(instance.path),
            instance.machine,
        );
        let source = source_occurrence(program, instance.source);
        for (template_handle, template) in cleanup.dispositions.iter().filter(|(_, template)| {
            template.machine == specialization.template
                && template.state == state
                && template.root == root
                && template.source == source
                && template.exit.state_symbol
                    == source_symbol(program, instance.exit.state_symbol, instance.machine)
                && template.exit.statement_index == instance.exit.statement_index
                && template.exit.kind == instance.exit.kind
                && flow.ownership.segments.span_or_empty(template.path) == path
        }) {
            let corresponding_regions = [
                (template.exclusions, instance.exclusions),
                (template.inactive_cases, instance.inactive_cases),
            ]
            .into_iter()
            .all(|(template_regions, instance_regions)| {
                let template_exclusions = cleanup.exclusions.span_or_empty(template_regions);
                let instance_exclusions = cleanup.exclusions.span_or_empty(instance_regions);
                template_exclusions.len() == instance_exclusions.len()
                    && template_exclusions.iter().zip(instance_exclusions).all(
                        |(template, instance)| {
                            let Some(instance_root) = instance.root.as_symbol() else {
                                return false;
                            };
                            template.cause == instance.cause
                                && template.root
                                    == PlaceRoot::Symbol(source_symbol(
                                        program,
                                        instance_root,
                                        specialization.instance,
                                    ))
                                && cleanup.segments.span_or_empty(template.segments)
                                    == source_path(
                                        program,
                                        cleanup.segments.span_or_empty(instance.segments),
                                        specialization.instance,
                                    )
                        },
                    )
            });
            if !corresponding_regions {
                continue;
            }
            applications.append(ContextualCleanupApplication {
                template: template_handle,
                instance: instance_handle,
            });
        }
    }
    cleanup.applications = applications;
}

fn source_symbol(
    program: &dyn ::validation::CheckingInput,
    symbol: SymbolHandle,
    instance: SymbolHandle,
) -> SymbolHandle {
    let mut scope = symbol;
    while scope.is_valid() && scope != instance {
        scope = program.symbols.get(scope).parent;
    }
    if scope != instance {
        return symbol;
    }
    let source = program.symbols.get(symbol).generated_from;
    if source.is_valid() && program.symbols.get(source).kind == program.symbols.get(symbol).kind {
        source
    } else {
        symbol
    }
}

fn source_path(
    program: &dyn ::validation::CheckingInput,
    path: &[PlaceSegment],
    instance: SymbolHandle,
) -> Vec<PlaceSegment> {
    path.iter()
        .map(|segment| match *segment {
            PlaceSegment::Field { symbol } => PlaceSegment::Field {
                symbol: source_symbol(program, symbol, instance),
            },
            PlaceSegment::Case { variant } => PlaceSegment::Case {
                variant: source_symbol(program, variant, instance),
            },
            other => other,
        })
        .collect()
}

fn source_occurrence(
    program: &dyn ::validation::CheckingInput,
    source: PermissionEventSource,
) -> PermissionEventSource {
    match source {
        PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => {
            let source = program.symbols.get(target_symbol).generated_from;
            PermissionEventSource::Call {
                statement_index,
                call_ordinal,
                target_symbol: if source.is_valid()
                    && program.symbols.get(source).kind == program.symbols.get(target_symbol).kind
                {
                    source
                } else {
                    target_symbol
                },
            }
        }
        other => other,
    }
}
