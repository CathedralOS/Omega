//! Prepare the complete Terminal checking input while typed source is available.
//! Generic dependency discovery, selected witnesses and source correspondence
//! belong here; the next stage only judges the modules and relationships emitted.

use symbols::SymbolHandle;
use terminal_check_tables::compilation::{GenericForms, PreparedCompilation, TerminalCompilation};
use typed_trees::TypedTrees;

type WitnessRow = (
    SymbolHandle,
    SymbolHandle,
    Vec<semantic_vocabulary::ScalarTerm>,
    SymbolHandle,
);

/// The driver keeps source publication custody while passing only `program`
/// into checking. Both were minted from the same source owner and traversal.
pub struct PreparedSource {
    pub program: TerminalCompilation,
    pub source: crate::SourceCorrespondence,
}

pub fn prepare_compilation(
    typed: &TypedTrees,
    compilation: crate::Compilation<'_>,
    publication: terminal_codec::TerminalPublication,
) -> PreparedSource {
    match crate::check_bound_callable_shapes(typed, compilation.bindings) {
        Ok(failures) if !failures.is_empty() => {
            return PreparedSource {
                program: TerminalCompilation::FormationFailures(failures),
                source: Default::default(),
            };
        }
        Ok(_) => {}
        Err(defect) => {
            return PreparedSource {
                program: TerminalCompilation::FormationDefect(defect),
                source: Default::default(),
            };
        }
    }
    let arrivals = compilation
        .bindings
        .iter()
        .map(|binding| {
            let requirement = binding
                .requirement
                .rejoin(
                    &typed.symbols,
                    typed.traits().iter().flat_map(|owner| {
                        typed
                            .trait_machine_signatures(owner)
                            .iter()
                            .map(|row| row.symbol)
                    }),
                )
                .map_err(|detail| terminal_check_tables::Refusal {
                    reason: terminal_check_tables::RefusalReason::ForeignIdentity,
                    machine: semantic_vocabulary::MachineId::new(1)
                        .expect("nonzero diagnostic identity"),
                    concerns: terminal_check_tables::Construct::Machine,
                    detail,
                })?;
            let (_, required) = typed.trait_machine_signature(requirement).ok_or_else(|| {
                crate::refuse(
                    semantic_vocabulary::MachineId::new(1).expect("nonzero diagnostic identity"),
                    terminal_check_tables::Construct::Machine,
                    crate::Unserved::Contract("bound arrival lost its exact source signature"),
                )
            })?;
            let source = binding.source_span.map_or(
                crate::SourceOrigin::Unpositioned(crate::Unpositioned::ConformanceWitness),
                crate::SourceOrigin::Authored,
            );
            let mut witness = crate::read::logical::projected_arrival(
                typed,
                required,
                binding.implementation,
                &binding.visible_parameters,
                binding.address_model.as_ref(),
                source,
            )?;
            witness.slot = binding.slot.clone();
            Ok(witness)
        })
        .collect();
    let (mut closure, source) = match crate::elaborate_source_for(typed, compilation, publication) {
        Ok(prepared) => (Ok(prepared.program), prepared.source),
        Err(refusal) => (Err(refusal), Default::default()),
    };
    let mut templates = Vec::new();
    let mut rows: Vec<WitnessRow> = Vec::new();
    let mut specializations = semantic_vocabulary::IdTable::default();
    let mut selection_witnesses = Vec::new();
    if let Ok(closure) = &mut closure {
        templates = std::mem::take(&mut closure.generic_templates);
        rows.extend(
            std::mem::take(&mut closure.conformance_witnesses)
                .into_iter()
                .map(|row| {
                    (
                        row.declaring_trait,
                        row.requirement,
                        Vec::new(),
                        row.implementation,
                    )
                }),
        );
        for (_, symbol) in closure.machines.iter() {
            specializations.push(crate::template_of(typed, *symbol));
        }
        for (selection, selected) in closure.selections.iter().enumerate() {
            let Some(implementation) = closure.machines.get(selected.implementation) else {
                continue;
            };
            let row = (
                selected.declaring_trait,
                selected.requirement,
                selected.value_arguments.clone(),
                *implementation,
            );
            let position = rows
                .iter()
                .position(|known| *known == row)
                .unwrap_or_else(|| {
                    rows.push(row);
                    rows.len() - 1
                });
            selection_witnesses.push((selection, position));
        }
    }
    let forms = prepare_generic_forms(typed, templates);
    let mut witnesses = Vec::new();
    let mut next = 0;
    while next < rows.len() {
        let (declaring_trait, requirement, arguments, implementation) = &rows[next];
        let mut witness = crate::elaborate_witness(
            typed,
            *declaring_trait,
            *requirement,
            arguments,
            *implementation,
        );
        if let Ok(witness) = &mut witness {
            for row in std::mem::take(&mut witness.conformance_witnesses) {
                let row = (
                    row.declaring_trait,
                    row.requirement,
                    Vec::new(),
                    row.implementation,
                );
                if !rows.contains(&row) {
                    rows.push(row);
                }
            }
        }
        witnesses.push(witness);
        next += 1;
    }
    PreparedSource {
        source,
        program: TerminalCompilation::Prepared(PreparedCompilation {
            closure,
            arrivals,
            forms,
            witnesses,
            specializations,
            selection_witnesses,
        }),
    }
}

fn prepare_generic_forms(
    typed: &TypedTrees,
    mut templates: Vec<SymbolHandle>,
) -> Vec<GenericForms> {
    let mut forms = Vec::new();
    let mut next = 0;
    while next < templates.len() {
        let template = templates[next];
        let elaborated = crate::elaborate_generic_forms(typed, &[template]);
        let mut applies = Some(Vec::new());
        for form in elaborated.iter().flatten() {
            for application in &form.applications {
                let Some(symbol) = form.machines.get(*application).copied() else {
                    applies = None;
                    continue;
                };
                let position = templates
                    .iter()
                    .position(|template| *template == symbol)
                    .unwrap_or_else(|| {
                        templates.push(symbol);
                        templates.len() - 1
                    });
                if let Some(applies) = &mut applies {
                    applies.push(position);
                }
            }
        }
        forms.push(GenericForms {
            template,
            name: typed.symbols.name(template).to_owned(),
            forms: elaborated,
            applies,
        });
        next += 1;
    }
    forms
}
