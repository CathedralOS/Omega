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
    let (mut closure, source) = match crate::elaborate_source_for(typed, compilation, publication) {
        Ok(prepared) => (Ok(prepared.program), prepared.source),
        Err(refusal) => (Err(refusal), Default::default()),
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
            let Some(implementation) = closure.machines.get(selected.implementation) else {
                continue;
            };
            let signature = match selected_signature(typed, closure, &source, selected) {
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

/// A source call can share a body with calls made under another public
/// contract. Rejoin its exact emitted site before preparing the independent
/// refinement witness; the physical body cannot choose the public signature.
fn selected_signature<'source>(
    typed: &'source TypedTrees,
    closure: &crate::Elaboration,
    source: &crate::SourceCorrespondence,
    selected: &crate::Selected,
) -> terminal_check_tables::Checked<&'source typed_trees::signature::StateSignature> {
    use terminal_check_tables::Construct;
    use typed_trees::expression::ExpressionNode;
    let unavailable = |reason| {
        crate::refuse(
            selected.caller,
            Construct::Machine,
            crate::Unserved::Declaration(reason),
        )
    };
    let required = selected
        .required_contract
        .ok_or_else(|| unavailable("selected witness has no public header"))?;
    let header = closure
        .module
        .declaration_contract(required)
        .ok_or_else(|| unavailable("selected witness lost its public header"))?;
    let declaration = header
        .declaration
        .as_ref()
        .ok_or_else(|| unavailable("selected witness lost its public source declaration"))?;
    let mut signature = None;
    for (caller, calls) in source.calls.iter() {
        let machine = closure
            .module
            .machine(caller)
            .ok_or_else(|| unavailable("source call lost its emitted caller"))?;
        for (member, expression, statement, site) in calls {
            if *member != selected.caller {
                continue;
            }
            if !machine.module_invocations().any(|invocation| {
                invocation.site == *site
                    && invocation.inputs.required_contract == Some(required)
                    && invocation.inputs.callee == selected.implementation
            }) {
                continue;
            }
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
            if signature.is_some_and(|known| known != applied) {
                return Err(unavailable(
                    "selected witness has different applied public signatures",
                ));
            }
            signature = Some(applied);
        }
    }
    signature.ok_or_else(|| unavailable("selected witness has no exact source call correspondence"))
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
