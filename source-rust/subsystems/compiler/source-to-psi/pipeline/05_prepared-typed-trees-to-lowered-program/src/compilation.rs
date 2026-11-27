//! Prepare the complete Terminal checking input while typed source is available.
//! Generic dependency discovery, selected witnesses and source correspondence
//! belong here; the next stage only judges the modules and relationships emitted.

pub mod source_roster;

use crate::source_state::TypedTrees;
use lowered_source_publication::{LoweredSourcePublication, PublicationSource};
use lowered_source_publication_issuer::LOWERING;
use symbols::SymbolHandle;
use terminal_check_tables::compilation::{GenericForms, PreparedCompilation, TerminalCompilation};

type WitnessRow<'source> = (
    SymbolHandle,
    SymbolHandle,
    Vec<semantic_vocabulary::ScalarTerm>,
    SymbolHandle,
    Option<&'source crate::source_state::signature::StateSignature>,
);

/// The driver keeps source publication custody while passing only `program`
/// into checking. Both were minted from the same source owner and traversal.
pub struct LoweredCompilationWithSource {
    pub program: TerminalCompilation,
    pub source: crate::SourceCorrespondence,
}

/// Lower one selected compilation of the source that completed typed
/// preparation, and bind the result to that source for the checker and for
/// publication. A snapshot stage 04 did not issue is not accepted:
///
/// ```compile_fail,E0308
/// use std::sync::Arc;
/// use typed_trees::source_state::FrozenTypedTrees;
/// use prepared_typed_trees_to_lowered_program::{Compilation, lower_for_publication};
/// fn bypass(snapshot: Arc<FrozenTypedTrees>, roots: Compilation<'_>) {
///     let _ = lower_for_publication(snapshot, roots, terminal_codec::TerminalPublication::Portable);
/// }
/// ```
///
/// Lowering cannot issue its own input either. Only stage 04 links the crate
/// that supplies the capability `PreparedTypedTrees::new` asks for:
///
/// ```compile_fail,E0433
/// let _ = prepared_source_issuer::PREPARATION;
/// ```
pub fn lower_for_publication(
    owner: crate::PreparedTypedTrees,
    compilation: crate::Compilation<'_>,
    publication: terminal_codec::TerminalPublication,
) -> Result<LoweredSourcePublication, Vec<diagnostics::Diagnostic>> {
    let prepared = prepare_compilation(&owner, compilation, publication)?;
    Ok(bind_source(owner, publication, prepared, None))
}

/// Bind what `prepare_compilation` derived from `owner` to that owner.
/// `roster` is the Product roster the roots were borrowed from, when there is one.
pub(super) fn bind_source(
    owner: crate::PreparedTypedTrees,
    publication: terminal_codec::TerminalPublication,
    prepared: LoweredCompilationWithSource,
    roster: Option<crate::compilation::source_roster::SourceCompilationRoster>,
) -> LoweredSourcePublication {
    LoweredSourcePublication::from_lowering(
        LOWERING,
        prepared.program,
        PublicationSource {
            owner,
            publication,
            correspondence: prepared.source,
            roster,
        },
    )
}

/// Lower the compilation from stage 04's completed output.
///
/// ```compile_fail,E0308
/// use typed_trees::source_state::FrozenTypedTrees;
/// use prepared_typed_trees_to_lowered_program::{Compilation, prepare_compilation};
/// fn bypass(snapshot: &FrozenTypedTrees, roots: Compilation<'_>) {
///     let _ = prepare_compilation(snapshot, roots, terminal_codec::TerminalPublication::Portable);
/// }
/// ```
pub fn prepare_compilation(
    typed: &crate::PreparedTypedTrees,
    compilation: crate::Compilation<'_>,
    publication: terminal_codec::TerminalPublication,
) -> Result<LoweredCompilationWithSource, Vec<diagnostics::Diagnostic>> {
    Ok(prepare_closure(typed, compilation, publication)?.complete())
}

/// One compilation lowered as far as its closure: the closure is read,
/// specialized and finished, its generic forms are prepared, and the
/// conformance rows it owes are listed. A Product's provider settlement reads
/// only the closure, and a settlement that asks for other providers or
/// layouts discards it, so the conformance witnesses and the mathematical
/// signature are prepared by `complete`, for the compilation that is judged.
/// `complete` yields exactly what preparing everything in one pass yields.
pub(crate) struct PreparedClosure<'source> {
    typed: &'source TypedTrees,
    /// The compilation's subjects and purpose. The mathematical signature
    /// is read for them, and each witness reports its outcome to them.
    roots: (
        Vec<Option<semantic_vocabulary::PackageKeyIdentity>>,
        source_provenance::DependencyScope,
    ),
    reading: ClosureReading<'source>,
}

enum ClosureReading<'source> {
    /// The bound callable shapes refused before any closure was read.
    Unformed(TerminalCompilation),
    Read(Box<ReadClosure<'source>>),
}

fn collect_callee_summaries(
    prepared: &crate::publication::PreparedProgram<'_>,
    summaries: &mut crate::CalleeSummaries,
) {
    if prepared.has_type_alternative {
        return;
    }
    let mut requirements = prepared.program.requirements.clone();
    requirements.sort_unstable();
    for (id, outcome) in prepared.outcomes.iter() {
        if requirements.binary_search(&id).is_ok() {
            continue;
        }
        let Some(&symbol) = prepared.program.machines.get(id) else {
            continue;
        };
        let Some(envelope) = prepared.source_envelopes.get(id).and_then(Option::as_ref) else {
            continue;
        };
        summaries.insert(
            symbol,
            crate::CalleeSummary {
                envelope: envelope.clone(),
                crash_routes: outcome.crash_routes.clone(),
                completes_normally: outcome.completes_normally,
            },
        );
    }
}

struct ReadClosure<'source> {
    captures: crate::SharedReading,
    closure: terminal_check_tables::Checked<crate::Elaboration>,
    further: Vec<terminal_check_tables::Refusal>,
    arrivals: Vec<terminal_check_tables::Checked<crate::CallableArrivalWitness>>,
    source: crate::SourceCorrespondence,
    forms: Vec<GenericForms>,
    rows: Vec<WitnessRow<'source>>,
    specializations: arena::IdTable<semantic_vocabulary::MachineId, Option<SymbolHandle>>,
    selection_witnesses: Vec<(usize, usize)>,
    callee_summaries: crate::CalleeSummaries,
}

impl<'source> PreparedClosure<'source> {
    /// The lowered closure, when the bound shapes formed and reading it
    /// refused nothing.
    pub(crate) fn closure(&self) -> Option<&crate::Elaboration> {
        match &self.reading {
            ClosureReading::Read(read) => read.closure.as_ref().ok(),
            ClosureReading::Unformed(_) => None,
        }
    }

    /// Prepare the mathematical signature and every conformance witness the
    /// closure and its generic forms owe, including the rows those witnesses
    /// discover in turn.
    pub(crate) fn complete(self) -> LoweredCompilationWithSource {
        let read = match self.reading {
            ClosureReading::Unformed(program) => {
                return LoweredCompilationWithSource {
                    program,
                    source: Default::default(),
                };
            }
            ClosureReading::Read(read) => *read,
        };
        let ReadClosure {
            captures,
            closure,
            mut further,
            arrivals,
            mut source,
            forms,
            mut rows,
            specializations,
            selection_witnesses,
            callee_summaries,
        } = read;
        let typed = self.typed;
        let (subjects, purpose) = &self.roots;
        // The declarations over data that contains itself are read apart from
        // the closure: they become one kernel signature and no operations.
        let (mathematics_result, witnesses) = if worker_budget::current_workers() > 1 {
            rayon::join(
                || crate::mathematics::read(typed, &self.roots, captures.clone()),
                || {
                    prepare_witnesses(
                        typed,
                        &mut rows,
                        &forms,
                        &callee_summaries,
                        &captures,
                        *purpose,
                        subjects,
                        &mut source,
                    )
                },
            )
        } else {
            (
                crate::mathematics::read(typed, &self.roots, captures.clone()),
                prepare_witnesses(
                    typed,
                    &mut rows,
                    &forms,
                    &callee_summaries,
                    &captures,
                    *purpose,
                    subjects,
                    &mut source,
                ),
            )
        };
        let mut mathematics: terminal_check_tables::mathematics::MathematicalSignature =
            match mathematics_result {
                Ok(mathematics) => mathematics,
                Err(mut refused) => {
                    further.append(&mut refused);
                    Default::default()
                }
            };
        mathematics.identities = crate::identity_tables::of_forms(&forms);
        LoweredCompilationWithSource {
            source,
            program: TerminalCompilation::Prepared(PreparedCompilation {
                closure,
                further,
                arrivals,
                forms,
                witnesses,
                specializations,
                selection_witnesses,
                mathematics,
            }),
        }
    }
}

/// Lower the closure of `compilation` (`PreparedClosure`).
pub(crate) fn prepare_closure<'source>(
    owner: &'source crate::PreparedTypedTrees,
    compilation: crate::Compilation<'_>,
    publication: terminal_codec::TerminalPublication,
) -> Result<PreparedClosure<'source>, Vec<diagnostics::Diagnostic>> {
    let domain_schemas = owner.domain_schemas();
    let typed = owner.trees();
    let roots = (compilation.subjects.to_vec(), compilation.purpose);
    let unformed = |program| PreparedClosure {
        typed,
        roots: roots.clone(),
        reading: ClosureReading::Unformed(program),
    };
    match crate::check_bound_callable_shapes(typed, compilation.bindings) {
        Ok(failures) if !failures.is_empty() => {
            return Ok(unformed(TerminalCompilation::FormationFailures(failures)));
        }
        Ok(_) => {}
        Err(defect) => {
            return Ok(unformed(TerminalCompilation::FormationDefect(defect)));
        }
    }
    let captures = crate::shared_reading(typed);
    let build_arrivals = || {
        compilation
            .bindings
            .iter()
            .map(|binding| {
                let requirement = crate::source_symbols::rejoin_declaration(
                    &binding.requirement,
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
                    site: terminal_check_tables::RefusalSite::Program,
                    source_span: None,
                    detail: detail.to_owned(),
                })?;
                let (_, required) =
                    typed.trait_machine_signature(requirement).ok_or_else(|| {
                        crate::refuse(
                            semantic_vocabulary::MachineId::new(1)
                                .expect("nonzero diagnostic identity"),
                            terminal_check_tables::Construct::Machine,
                            crate::Unserved::Contract(
                                "bound arrival lost its exact source signature",
                            ),
                        )
                    })?;
                let source = binding.source_span.map_or(
                    crate::SourceOrigin::Unpositioned(crate::Unpositioned::ConformanceWitness),
                    crate::SourceOrigin::Authored,
                );
                let mut witness = crate::read::logical::projected_arrival(
                    typed,
                    compilation.purpose,
                    required,
                    binding.implementation,
                    &binding.visible_parameters,
                    binding.address_model.as_ref(),
                    source,
                    captures.clone(),
                )?;
                witness.slot = binding.slot.clone();
                Ok(witness)
            })
            .collect::<Vec<_>>()
    };
    let elaborate_primary = || {
        crate::elaborate_source_for(
            typed,
            compilation,
            publication,
            captures.clone(),
            domain_schemas,
        )
    };
    let (arrivals, elaborated_primary) =
        if worker_budget::current_workers() > 1 && !compilation.bindings.is_empty() {
            rayon::join(build_arrivals, elaborate_primary)
        } else {
            (build_arrivals(), elaborate_primary())
        };
    // Every refusal reading met is reported: the first as the closure's
    // verdict, the rest beside it (`PreparedCompilation::further`).
    let mut further = Vec::new();
    let mut callee_summaries = crate::CalleeSummaries::default();
    let (mut closure, primary_source, signatures, prepared_applications, pending_publication) =
        match elaborated_primary {
            Ok(prepared) => {
                collect_callee_summaries(&prepared, &mut callee_summaries);
                (
                    Ok(prepared.program),
                    Some(prepared.source),
                    prepared.selected_calls,
                    prepared.prepared_applications,
                    prepared.pending_publication,
                )
            }
            Err(mut refused) => {
                let first = refused.remove(0);
                further = refused;
                (Err(first), None, Vec::new(), Vec::new(), None)
            }
        };
    let mut source = crate::SourceCorrespondence {
        forms: vec![primary_source],
    };
    let mut templates = Vec::new();
    let mut rows: Vec<WitnessRow<'_>> = Vec::new();
    let mut selected_failure = None;
    let mut specializations = arena::IdTable::default();
    let mut selection_witnesses = Vec::new();
    if let Ok(closure) = &mut closure {
        templates = std::mem::take(&mut closure.generic_templates);
        let mut indexed_templates = templates
            .iter()
            .copied()
            .map(|template| (template, ()))
            .collect::<arena::HandleKeyMap<SymbolHandle, _>>();
        rows.extend(
            std::mem::take(&mut closure.conformance_witnesses)
                .into_iter()
                .map(|row| {
                    (
                        row.declaring_trait,
                        row.requirement,
                        row.value_arguments,
                        row.implementation,
                        None,
                    )
                }),
        );
        for (_, symbol) in closure.machines.iter() {
            let template = crate::template_of(typed, *symbol);
            // A closed invocation still owes the generic source declaration's
            // universal check, even when no whole-package subject was requested.
            if let Some(template) = template
                && indexed_templates.insert(template, ()).is_none()
                && typed
                    .machine_by_symbol(template)
                    .is_some_and(|machine| crate::is_generic_template(typed, machine))
            {
                templates.push(template);
            }
            specializations.push(template);
        }
    }
    // Template proof forms retain both the checked source context and the
    // actual publication mode of the invocation using their captures.
    let (mut forms, form_summaries) = if let Ok(closure_ref) = &closure {
        let mut resolve_selections = || {
            for (selection, selected) in closure_ref.selections.iter().enumerate() {
                if selected_failure.is_some() {
                    break;
                }
                let Some(implementation) = closure_ref.machines.get(selected.implementation) else {
                    continue;
                };
                let signature = match selected_signature_key(selected).and_then(|key| {
                    signatures
                        .binary_search_by_key(&key, |row| row.key)
                        .map(|position| signatures[position].signature)
                        .map_err(|_| {
                            crate::refuse(
                                selected.caller,
                                terminal_check_tables::Construct::Machine,
                                crate::Unserved::Declaration(
                                    "selected witness has no exact source call correspondence",
                                ),
                            )
                        })
                }) {
                    Ok(signature) => signature,
                    Err(reason) => {
                        selected_failure = Some(reason);
                        break;
                    }
                };
                let row = (
                    selected.declaring_trait,
                    selected.requirement,
                    selected.value_arguments.clone(),
                    *implementation,
                    Some(signature),
                );
                // A conformance row of the same implementation, read without the
                // selected header, already owes the witness this call would. The
                // call shares it when its header passes the row's checks and
                // supplies no contract of its own; otherwise it is proved alone.
                let position = rows
                    .iter()
                    .position(|known| *known == row)
                    .or_else(|| {
                        rows.iter()
                            .position(|known| {
                                known.4.is_none()
                                    && (known.0, known.1, &known.2, known.3)
                                        == (row.0, row.1, &row.2, row.3)
                            })
                            .filter(|_| {
                                crate::selected_witness_is_declared(
                                    typed,
                                    row.0,
                                    row.1,
                                    &row.2,
                                    row.3,
                                    signature,
                                    crate::WitnessInputs {
                                        captures: captures.clone(),
                                        purpose: compilation.purpose,
                                        outcome_subjects: Some(compilation.subjects),
                                        callee_summaries: None,
                                        parallel_machines: false,
                                    },
                                )
                            })
                    })
                    .unwrap_or_else(|| {
                        rows.push(row);
                        rows.len() - 1
                    });
                selection_witnesses.push((selection, position));
            }
        };
        if worker_budget::current_workers() > 1
            && !templates.is_empty()
            && !closure_ref.selections.is_empty()
        {
            let (_, forms) = rayon::join(resolve_selections, || {
                prepare_generic_forms(
                    typed,
                    compilation.purpose,
                    templates,
                    &captures,
                    publication,
                    &mut source,
                )
            });
            forms
        } else {
            resolve_selections();
            prepare_generic_forms(
                typed,
                compilation.purpose,
                templates,
                &captures,
                publication,
                &mut source,
            )
        }
    } else {
        prepare_generic_forms(
            typed,
            compilation.purpose,
            templates,
            &captures,
            publication,
            &mut source,
        )
    };
    callee_summaries.merge(form_summaries);
    if let Some(reason) = selected_failure {
        closure = Err(reason);
    }
    // Policies discovered inside a check form owe their own conformance
    // witnesses too. Move those exact source rows into the existing worklist.
    for generic in &mut forms {
        for form in generic.forms.iter_mut().flatten() {
            for witness in std::mem::take(&mut form.conformance_witnesses) {
                let row = (
                    witness.declaring_trait,
                    witness.requirement,
                    witness.value_arguments,
                    witness.implementation,
                    None,
                );
                if !rows.contains(&row) {
                    rows.push(row);
                }
            }
        }
    }
    closure = closure.and_then(|mut elaboration| {
        crate::declare::close_specializations(&forms, prepared_applications, &mut elaboration)
            .map_err(|(owner, reason)| {
                crate::refuse(
                    owner,
                    terminal_check_tables::Construct::Machine,
                    crate::Unserved::Declaration(reason),
                )
            })?;
        // Proof scope retains the completed callable headers for later
        // admission. Capture commitments are part of those headers.
        crate::publication::finalize_source(
            typed,
            &mut elaboration,
            &signatures,
            pending_publication,
        )?;
        Ok(elaboration)
    });
    if let Ok(elaboration) = &mut closure {
        retain_installation_reach(&mut elaboration.module)?;
    }
    Ok(PreparedClosure {
        typed,
        roots,
        reading: ClosureReading::Read(Box::new(ReadClosure {
            captures,
            closure,
            further,
            arrivals,
            source,
            forms,
            rows,
            specializations,
            selection_witnesses,
            callee_summaries,
        })),
    })
}

/// Prepare the conformance witness of every row in order, and of every row
/// those witnesses owe in turn, appending each one's source form to `source`.
/// Each wave of newly discovered rows is prepared on the invocation's workers.
fn prepare_witnesses<'source>(
    typed: &'source TypedTrees,
    rows: &mut Vec<WitnessRow<'source>>,
    forms: &[GenericForms],
    callee_summaries: &crate::CalleeSummaries,
    captures: &crate::SharedReading,
    purpose: source_provenance::DependencyScope,
    subjects: &[Option<semantic_vocabulary::PackageKeyIdentity>],
    source: &mut crate::SourceCorrespondence,
) -> Vec<terminal_check_tables::Checked<crate::Elaboration>> {
    let mut witnesses = Vec::new();
    let mut next = 0;
    while next < rows.len() {
        let wave_end = rows.len();
        let prepare_one =
            |(declaring_trait, requirement, arguments, implementation, signature): &(
                SymbolHandle,
                SymbolHandle,
                Vec<semantic_vocabulary::ScalarTerm>,
                SymbolHandle,
                Option<&_>,
            )| {
                let mut prepared = crate::prepare_witness(
                    typed,
                    *declaring_trait,
                    *requirement,
                    arguments,
                    *implementation,
                    *signature,
                    crate::WitnessInputs {
                        captures: captures.clone(),
                        purpose,
                        outcome_subjects: Some(subjects),
                        callee_summaries: Some(callee_summaries),
                        parallel_machines: worker_budget::current_workers() > 1
                            && wave_end - next < 2,
                    },
                )?;
                crate::declare::close_specializations(
                    forms,
                    std::mem::take(&mut prepared.prepared_applications),
                    &mut prepared.program,
                )
                .map_err(|(owner, reason)| {
                    crate::refuse(
                        owner,
                        terminal_check_tables::Construct::Machine,
                        crate::Unserved::Declaration(reason),
                    )
                })?;
                prepared
                    .finalize_check_form()
                    .map(|prepared| (prepared.source, prepared.program))
            };
        let wave_results: Vec<_> = if worker_budget::current_workers() == 1 || wave_end - next < 2 {
            rows[next..wave_end].iter().map(prepare_one).collect()
        } else {
            use rayon::prelude::*;
            rows[next..wave_end]
                .par_iter()
                .with_max_len(1)
                .map(prepare_one)
                .collect()
        };
        for prepared in wave_results {
            let mut witness = match prepared {
                Ok((witness_source, program)) => {
                    source.forms.push(Some(witness_source));
                    Ok(program)
                }
                Err(refusal) => {
                    source.forms.push(None);
                    Err(refusal)
                }
            };
            if let Ok(witness) = &mut witness {
                for row in std::mem::take(&mut witness.conformance_witnesses) {
                    let row = (
                        row.declaring_trait,
                        row.requirement,
                        row.value_arguments,
                        row.implementation,
                        None,
                    );
                    if !rows.contains(&row) {
                        rows.push(row);
                    }
                }
            }
            witnesses.push(witness);
        }
        next = wave_end;
    }
    witnesses
}

/// Record which installed services the entry can reach before the checker judges it.
fn retain_installation_reach(
    module: &mut terminal_psi::TerminalModule,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let Some(terminal_psi::TargetMachineCallable::Machine(entry)) = module.entry else {
        return Ok(());
    };
    let requirements = module.installation_reach_requirements();
    if requirements.is_empty() {
        return Ok(());
    }
    let reach = terminal_semantics::synchronous_calls::call_analyses(module)
        .and_then(|analyses| analyses.calls().service_reach_from(entry, &requirements))
        .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
    if !reach.installation_dependencies.is_empty() {
        module.interfaces.root_service_reach = reach;
    }
    Ok(())
}

type SelectedSignatureKey = (
    semantic_vocabulary::MachineId,
    semantic_vocabulary::ContractId,
    semantic_vocabulary::MachineId,
);
/// A borrowed view of the call already joined to its emitted operation. It
/// stays inside source preparation and is discarded before Terminal checking.
/// It describes the chosen contract; no call or witness is accepted by it.
pub(crate) struct SelectedSourceCall<'source> {
    pub key: SelectedSignatureKey,
    pub signature: &'source crate::source_state::signature::StateSignature,
    pub call: crate::source_state::references::StaticArgumentCall<'source>,
    pub emitted_caller: semantic_vocabulary::MachineId,
}

fn selected_signature_key(
    selected: &crate::Selected,
) -> terminal_check_tables::Checked<SelectedSignatureKey> {
    let required = selected.required_contract.ok_or_else(|| {
        crate::refuse(
            selected.caller,
            terminal_check_tables::Construct::Machine,
            crate::Unserved::Declaration("selected witness has no public header"),
        )
    })?;
    Ok((selected.caller, required, selected.implementation))
}

/// Several public contracts can select the same body. Join each source call
/// to its exact emitted site once, then reuse that applied signature for the
/// independent witnesses. A body's identity cannot choose its public contract.
pub(crate) fn prepare_selected_signatures<'source>(
    typed: &'source TypedTrees,
    closure: &crate::Elaboration,
    source: &crate::SourceFormCorrespondence,
) -> terminal_check_tables::Checked<Vec<SelectedSourceCall<'source>>> {
    use crate::source_state::expression::ExpressionNode;
    use terminal_check_tables::Construct;
    let mut demanded = closure
        .selections
        .iter()
        .filter(|selected| closure.machines.get(selected.implementation).is_some())
        .map(selected_signature_key)
        .collect::<terminal_check_tables::Checked<Vec<_>>>()?;
    demanded.sort_unstable();
    demanded.dedup();
    if demanded.is_empty() {
        return Ok(Vec::new());
    }
    let mut invocations = closure
        .module
        .machines
        .iter()
        .flat_map(|machine| {
            machine.module_invocations().filter_map(move |invocation| {
                invocation.inputs.required_contract.map(|required| {
                    (
                        (machine.id, invocation.site),
                        required,
                        invocation.inputs.callee,
                    )
                })
            })
        })
        .collect::<Vec<_>>();
    invocations.sort_unstable_by_key(|(site, _, _)| *site);
    if let Some(pair) = invocations.windows(2).find(|pair| pair[0].0 == pair[1].0) {
        return Err(crate::refuse(
            pair[0].0.0,
            Construct::Machine,
            crate::Unserved::Declaration("selected source call has duplicate emitted sites"),
        ));
    }
    let mut signatures = Vec::new();
    for correspondence in &source.calls {
        let caller = correspondence.machine;
        let member = correspondence.source_member;
        let site = (caller, correspondence.site);
        let Ok(position) = invocations.binary_search_by_key(&site, |(site, _, _)| *site) else {
            continue;
        };
        let (_, required, body) = invocations[position];
        let key = (member, required, body);
        if demanded.binary_search(&key).is_err() {
            continue;
        }
        let unavailable = |reason| {
            crate::refuse(
                member,
                Construct::Machine,
                crate::Unserved::Declaration(reason),
            )
        };
        let declaration = closure
            .module
            .declaration_contract(required)
            .ok_or_else(|| unavailable("selected witness lost its public header"))?
            .declaration
            .as_ref()
            .ok_or_else(|| unavailable("selected witness lost its public source declaration"))?;
        let source_owner = closure
            .machines
            .get(member)
            .ok_or_else(|| unavailable("source call lost its requesting member"))?;
        if *source_owner != correspondence.owner
            || !typed
                .symbols
                .symbols()
                .nodes()
                .is_valid(correspondence.owner)
        {
            return Err(unavailable(
                "source call lost its exact original declaration owner",
            ));
        }
        let call = match correspondence.source {
            crate::source_state::SourceNode::Expression(expression) => {
                if !typed.expression_table.expression_is_valid(expression) {
                    return Err(unavailable("source call has a stale expression coordinate"));
                }
                let ExpressionNode::Call(call) = typed.expression_table.expression(expression)
                else {
                    return Err(unavailable(
                        "source call no longer names its retained expression",
                    ));
                };
                crate::source_state::references::StaticArgumentCall::Expression(expression, call)
            }
            crate::source_state::SourceNode::Statement(statement) => {
                if !typed.statement_table.statement_is_valid(statement) {
                    return Err(unavailable("source call has a stale statement coordinate"));
                }
                let crate::source_state::statement::StatementNode::Call(call) =
                    typed.statement_table.statement(statement)
                else {
                    return Err(unavailable(
                        "selected requirement call has no retained static-call statement",
                    ));
                };
                crate::source_state::references::StaticArgumentCall::Statement(call)
            }
        };
        let lexical_owner = call.lexical_owner(typed);
        let owner = if lexical_owner == SymbolHandle::invalid() {
            correspondence.owner
        } else {
            lexical_owner
        };
        if !typed.symbols.symbols().nodes().is_valid(owner) {
            return Err(unavailable(
                "selected source call has a stale supplying declaration",
            ));
        }
        let applied = typed
            .source_call_requirement_signature(owner, call)
            .map_err(|reason| {
                let mut refusal = unavailable("applied public requirement");
                refusal.detail.push_str(": ");
                refusal.detail.push_str(&reason);
                refusal
            })?;
        let (declaring, _) = typed
            .trait_machine_signature(applied.symbol)
            .ok_or_else(|| unavailable("selected witness lost its exact declaring trait"))?;
        let identity = typed.trait_requirement_callable_identity_text(declaring, applied);
        if closure.module.identities.text(declaration.callable) != Some(identity.as_str()) {
            return Err(unavailable(
                "selected witness differs from the invocation's public signature",
            ));
        }
        signatures.push(SelectedSourceCall {
            key,
            signature: applied,
            call,
            emitted_caller: caller,
        });
    }
    signatures.sort_unstable_by_key(|row| row.key);
    for pair in signatures.windows(2) {
        if pair[0].key == pair[1].key && pair[0].signature != pair[1].signature {
            let (caller, _, _) = pair[0].key;
            return Err(crate::refuse(
                caller,
                Construct::Machine,
                crate::Unserved::Declaration(
                    "selected witness has different applied public signatures",
                ),
            ));
        }
    }
    // Equal signatures can originate in different supplying scopes. Keep
    // every occurrence for declaration-authority requests, while witnesses
    // reuse any matching signature only after full equality was checked.
    for (caller, required, body) in demanded {
        if signatures
            .binary_search_by_key(&(caller, required, body), |row| row.key)
            .is_err()
        {
            return Err(crate::refuse(
                caller,
                Construct::Machine,
                crate::Unserved::Declaration(
                    "selected witness has no exact source call correspondence",
                ),
            ));
        }
    }
    Ok(signatures)
}

fn prepare_generic_forms(
    typed: &TypedTrees,
    purpose: source_provenance::DependencyScope,
    mut templates: Vec<SymbolHandle>,
    captures: &crate::SharedReading,
    publication: terminal_codec::TerminalPublication,
    source: &mut crate::SourceCorrespondence,
) -> (Vec<GenericForms>, crate::CalleeSummaries) {
    struct PreparedGeneric<'source> {
        template: SymbolHandle,
        name: String,
        forms: terminal_check_tables::Checked<Vec<crate::publication::PreparedProgram<'source>>>,
        applies: Option<Vec<usize>>,
    }
    let mut prepared = Vec::new();
    let mut next = 0;
    while next < templates.len() {
        let wave_end = templates.len();
        let parallel_machines = worker_budget::current_workers() > 1 && wave_end - next < 2;
        let prepare_one = |&template: &SymbolHandle| {
            let elaborated = crate::prepare_generic_forms(
                typed,
                purpose,
                &[template],
                captures.clone(),
                publication,
                parallel_machines,
            );
            (
                template,
                typed.symbols.name(template).to_owned(),
                elaborated,
            )
        };
        let wave_results: Vec<_> = if worker_budget::current_workers() == 1 || wave_end - next < 2 {
            templates[next..wave_end].iter().map(prepare_one).collect()
        } else {
            use rayon::prelude::*;
            templates[next..wave_end]
                .par_iter()
                .with_max_len(1)
                .map(prepare_one)
                .collect()
        };
        for (template, name, mut elaborated) in wave_results {
            let mut applies = Some(Vec::new());
            for form in elaborated.iter_mut().flatten() {
                for application in &form.program.applications {
                    let Some(symbol) = form.program.machines.get(*application).copied() else {
                        applies = None;
                        continue;
                    };
                    let position = templates
                        .iter()
                        .position(|known| *known == symbol)
                        .unwrap_or_else(|| {
                            templates.push(symbol);
                            templates.len() - 1
                        });
                    if let Some(applies) = &mut applies {
                        applies.push(position);
                    }
                }
                for discovered in std::mem::take(&mut form.program.generic_templates) {
                    if !templates.contains(&discovered) {
                        templates.push(discovered);
                    }
                }
            }
            prepared.push(PreparedGeneric {
                template,
                name,
                forms: elaborated,
                applies,
            });
        }
        next = wave_end;
    }
    // Borrow the source forms' actual headers once. Closing applications adds
    // their receipts; it does not change the template's public contract hash.
    let mut summaries = crate::CalleeSummaries::default();
    for generic in &prepared {
        if let Ok(forms) = &generic.forms {
            for form in forms {
                collect_callee_summaries(form, &mut summaries);
            }
        }
    }
    let bases: crate::declare::TemplateCommitments = prepared
        .iter()
        .map(|generic| {
            let commitment = match &generic.forms {
                Ok(forms) => crate::declare::template_commitment_for(
                    generic.template,
                    forms.iter().map(|form| &form.program),
                ),
                Err(_) => Ok(None),
            };
            (generic.template, commitment)
        })
        .collect();
    let finalize_one = |generic: PreparedGeneric<'_>| {
        let forms = generic.forms.and_then(|forms| {
            let forms = forms
                .into_iter()
                .map(|mut prepared| {
                    crate::declare::close_specializations_with_bases(
                        &bases,
                        std::mem::take(&mut prepared.prepared_applications),
                        &mut prepared.program,
                    )
                    .map_err(|(owner, reason)| {
                        crate::refuse(
                            owner,
                            terminal_check_tables::Construct::Machine,
                            crate::Unserved::Declaration(reason),
                        )
                    })?;
                    prepared.finalize_check_form()
                })
                .collect::<terminal_check_tables::Checked<Vec<_>>>()?;
            Ok(forms
                .into_iter()
                .map(|mut prepared| {
                    use crate::declaration_selection::AuthoredDeclarationSelectionExposure;
                    prepared.source.generic_callable = typed
                        .machine_by_symbol(generic.template)
                        .filter(|machine| {
                            machine.supplies_checked_body()
                                && machine.interface_exposure
                                    == AuthoredDeclarationSelectionExposure::PublicInterface
                        })
                        .map(|machine| machine.symbol);
                    (prepared.source, prepared.program)
                })
                .collect::<Vec<_>>())
        });
        (generic.template, generic.name, generic.applies, forms)
    };
    let finalized: Vec<_> = if worker_budget::current_workers() == 1 || prepared.len() < 2 {
        prepared.into_iter().map(finalize_one).collect()
    } else {
        use rayon::prelude::*;
        prepared
            .into_par_iter()
            .with_max_len(1)
            .map(finalize_one)
            .collect()
    };
    let forms = finalized
        .into_iter()
        .map(|(template, name, applies, forms)| {
            let forms = match forms {
                Ok(entries) => {
                    let mut programs = Vec::with_capacity(entries.len());
                    for (form_source, program) in entries {
                        source.forms.push(Some(form_source));
                        programs.push(program);
                    }
                    Ok(programs)
                }
                Err(refusal) => {
                    source.forms.push(None);
                    Err(refusal)
                }
            };
            GenericForms {
                template,
                name,
                forms,
                applies,
            }
        })
        .collect();
    (forms, summaries)
}
