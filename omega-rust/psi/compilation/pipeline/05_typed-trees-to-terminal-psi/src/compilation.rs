//! Prepare the complete Terminal checking input while typed source is available.
//! Generic dependency discovery, selected witnesses and source correspondence
//! belong here; the next stage only judges the modules and relationships emitted.

use symbols::SymbolHandle;
use terminal_check_tables::compilation::{GenericForms, PreparedCompilation, TerminalCompilation};
use typed_trees::TypedTrees;

type WitnessRow<'source> = (
    SymbolHandle,
    SymbolHandle,
    Vec<semantic_vocabulary::ScalarTerm>,
    SymbolHandle,
    Option<&'source typed_trees::signature::StateSignature>,
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
    let (mut closure, source, signatures) =
        match crate::elaborate_source_for(typed, compilation, publication) {
            Ok(prepared) => (
                Ok(prepared.program),
                prepared.source,
                prepared.selected_calls,
            ),
            Err(refusal) => (Err(refusal), Default::default(), Vec::new()),
        };
    let mut templates = Vec::new();
    let mut rows: Vec<WitnessRow<'_>> = Vec::new();
    let mut selected_failure = None;
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
                        None,
                    )
                }),
        );
        for (_, symbol) in closure.machines.iter() {
            specializations.push(crate::template_of(typed, *symbol));
        }
        for (selection, selected) in closure.selections.iter().enumerate() {
            if selected_failure.is_some() {
                break;
            }
            let Some(implementation) = closure.machines.get(selected.implementation) else {
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
    if let Some(reason) = selected_failure {
        closure = Err(reason);
    }
    let forms = prepare_generic_forms(typed, templates);
    let mut witnesses = Vec::new();
    let mut next = 0;
    while next < rows.len() {
        let (declaring_trait, requirement, arguments, implementation, signature) = &rows[next];
        let mut witness = match signature {
            Some(signature) => crate::elaborate_applied_witness(
                typed,
                signature,
                *declaring_trait,
                *requirement,
                arguments,
                *implementation,
            ),
            None => crate::elaborate_witness(
                typed,
                *declaring_trait,
                *requirement,
                arguments,
                *implementation,
            ),
        };
        if let Ok(witness) = &mut witness {
            for row in std::mem::take(&mut witness.conformance_witnesses) {
                let row = (
                    row.declaring_trait,
                    row.requirement,
                    Vec::new(),
                    row.implementation,
                    None,
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
            forms,
            witnesses,
            specializations,
            selection_witnesses,
        }),
    }
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
    pub signature: &'source typed_trees::signature::StateSignature,
    pub call: typed_trees::references::StaticArgumentCall<'source>,
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
    source: &crate::SourceCorrespondence,
) -> terminal_check_tables::Checked<Vec<SelectedSourceCall<'source>>> {
    use terminal_check_tables::Construct;
    use typed_trees::expression::ExpressionNode;
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
    let mut signatures = Vec::new();
    for (caller, calls) in source.calls.iter() {
        let unavailable = |reason| {
            crate::refuse(
                caller,
                Construct::Machine,
                crate::Unserved::Declaration(reason),
            )
        };
        let machine = closure
            .module
            .machine(caller)
            .ok_or_else(|| unavailable("source call lost its emitted caller"))?;
        let mut invocations = machine
            .module_invocations()
            .filter_map(|invocation| {
                invocation
                    .inputs
                    .required_contract
                    .map(|required| (invocation.site, required, invocation.inputs.callee))
            })
            .collect::<Vec<_>>();
        invocations.sort_unstable_by_key(|(site, _, _)| *site);
        if invocations.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(unavailable(
                "selected source call has duplicate emitted sites",
            ));
        }
        for (member, expression, statement, site) in calls {
            let Ok(position) = invocations.binary_search_by_key(site, |(site, _, _)| *site) else {
                continue;
            };
            let (_, required, body) = invocations[position];
            let key = (*member, required, body);
            if demanded.binary_search(&key).is_err() {
                continue;
            }
            let unavailable = |reason| {
                crate::refuse(
                    *member,
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
                .ok_or_else(|| {
                    unavailable("selected witness lost its public source declaration")
                })?;
            let owner = *closure
                .machines
                .get(*member)
                .ok_or_else(|| unavailable("source call lost its requesting member"))?;
            let call = if expression.is_valid() && !statement.is_valid() {
                if !typed.expression_table.expression_is_valid(*expression) {
                    return Err(unavailable("source call has a stale expression coordinate"));
                }
                let ExpressionNode::Call(call) = typed.expression_table.expression(*expression)
                else {
                    return Err(unavailable(
                        "source call no longer names its retained expression",
                    ));
                };
                typed_trees::references::StaticArgumentCall::Expression(*expression, call)
            } else if statement.is_valid() && !expression.is_valid() {
                if !typed.statement_table.statement_is_valid(*statement) {
                    return Err(unavailable("source call has a stale statement coordinate"));
                }
                let typed_trees::statement::StatementNode::Call(call) =
                    typed.statement_table.statement(*statement)
                else {
                    return Err(unavailable(
                        "source call no longer names its retained statement",
                    ));
                };
                typed_trees::references::StaticArgumentCall::Statement(call)
            } else {
                return Err(unavailable(
                    "source call has no unique expression or statement coordinate",
                ));
            };
            let lexical_owner = call.lexical_owner(typed);
            let owner = if lexical_owner.is_valid() {
                lexical_owner
            } else {
                owner
            };
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
