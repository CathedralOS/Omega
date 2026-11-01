//! Bind completed operands and fresh normal-exit storage for every call form.
//!
//! The public selection and exact occurrence own entry requirements and output
//! equations. Statement and value calls share this transfer; neither a callee's
//! body nor an old referent observation substitutes for its public guarantees.

use super::*;

pub(super) struct CapturedCallBindings {
    pub(super) entry_bindings: Vec<(SymbolHandle, StructuralTerm)>,
    pub(super) bindings: Vec<(SymbolHandle, StructuralTerm)>,
    pub(super) post_storage: Vec<PostCallStorage>,
}

impl<'program> StructuralJudge<'program> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn capture_call_bindings(
        &mut self,
        site: crate::CallSite<'program>,
        expression: ExpressionHandle,
        statement: arena::Handle<typed_trees::statement::StatementNode>,
        owner: &SelectedCallContract<'program>,
        application: &AppliedProofBindings,
        parameters: &[typed_trees::signature::StateParameter],
        actuals: Option<&[StructuralTerm]>,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        writes: &NormalizedWriteFrame,
    ) -> Option<CapturedCallBindings> {
        let program = self.program;
        // References are live subjects. Only the frame owner can publish their
        // exact prefix origins and preservation through this normal return.
        // A written subject or replaced formal binding needs post-state facts,
        // not the detached child term captured during operand evaluation.
        let Some(machine) = program.machine(scope.0.symbol) else {
            return None;
        };
        let frames = self.frame_resolver();
        // An opaque frame establishes no preservation or post-call reference
        // correspondence. Isolated operand snapshots and independent result
        // guarantees remain usable, but no live referent is reintroduced.
        let subjects = frames
            .filter(|_| writes.is_complete())
            .and_then(|frames| frames.call_reference_subjects(machine, site, parameters));
        if !independent_call_application(
            program,
            self.machine_symbol,
            owner_symbol(&owner),
            match site {
                crate::CallSite::Expression { expression, call } => {
                    typed_trees::references::StaticArgumentCall::Expression(expression, call)
                }
                crate::CallSite::Statement(call) => {
                    typed_trees::references::StaticArgumentCall::Statement(call)
                }
                _ => return None,
            },
        ) {
            return None;
        }
        let leading_operands = owner_leading_operands(&owner);
        let Some(arguments) =
            crate::call_site_argument_expressions(program, &site).get(leading_operands..)
        else {
            return None;
        };
        if arguments.len() != parameters.len() {
            return None;
        }
        // The checked local-name gate rejects parameter/local collisions and
        // duplicate locals in each state; state forwarding creates a fresh
        // environment. Names here locate values only in that validated scope;
        // exact symbols still own the public formal/result substitution.
        // All actual values belong to the caller environment. Installing a
        // formal map before this capture would reinterpret another actual in
        // the callee's telescope.
        let completed_arguments = actuals?;
        let Some(actuals) = completed_arguments
            .get(leading_operands..)
            .filter(|actuals| actuals.len() == arguments.len())
        else {
            return None;
        };
        // Entry requirements observe the arguments before this call's writes.
        // Guarantees may use only formals whose observations survive return.
        // Keep both substitutions: dropping a mutable input's post-state
        // observation must not erase a proven entry requirement or an
        // independent guarantee about the captured result.
        let mut entry_bindings = parameters
            .iter()
            .zip(actuals)
            .filter_map(|(parameter, value)| {
                let subject = subjects
                    .as_ref()
                    .and_then(|subjects| {
                        subjects
                            .iter()
                            .find(|subject| subject.entry.parameter == parameter.symbol)
                    })
                    .map(|subject| &subject.entry);
                self.entry_argument_value(parameter, value, subject, environment, scope)
                    .map(|value| (parameter.symbol, value))
            })
            .collect::<Vec<_>>();
        // One authenticated telescope supplies closed constants and completed
        // runtime operands. Executable formals already captured above retain
        // their own ordered value; header subjects never replay their storage.
        let static_bindings =
            super::super::super::specification_calls::call_contract_scalar_substitution(
                program,
                scope.0.symbol,
                &owner,
                site,
                |subject| {
                    self.completed_scalar_subject(
                        subject,
                        site,
                        completed_arguments,
                        scope.0,
                        scope.1,
                    )
                },
            )?;
        for (parameter, value) in static_bindings {
            if !entry_bindings
                .iter()
                .any(|(symbol, _)| *symbol == parameter.symbol)
            {
                entry_bindings.push((parameter.symbol, value));
            }
        }
        let mut bindings = entry_bindings
            .iter()
            .filter_map(|(symbol, value)| {
                let Some(parameter) = parameters
                    .iter()
                    .find(|parameter| parameter.symbol == *symbol)
                else {
                    return application
                        .constants
                        .iter()
                        .any(|constant| constant.symbol == *symbol)
                        .then(|| (*symbol, value.clone()))
                        .or_else(|| {
                            application
                                .runtime_parameters
                                .iter()
                                .any(|runtime| runtime.symbol == *symbol)
                                .then(|| (*symbol, value.clone()))
                        });
                };
                let observed =
                    if crate::reference_shell(program, parameter.type_reference).is_none() {
                        (!parameter.is_mutable).then(|| value.clone())?
                    } else if subjects.as_ref().is_some_and(|subjects| {
                        subjects.iter().any(|subject| {
                            subject.entry.parameter == *symbol && subject.preserved_on_return
                        })
                    }) {
                        value.clone()
                    } else if !parameter.is_mutable
                        && crate::slice_view_shell(program, parameter.type_reference, &[]).is_some()
                    {
                        // The passed descriptor can survive without preserved
                        // referent contents. Retain only its metadata identity;
                        // a pre-call contents observation is not a post-call fact.
                        super::super::super::structural_terms::descriptor_only_observation(
                            parameter.type_reference,
                            value.clone(),
                        )
                    } else {
                        return None;
                    };
                Some((*symbol, observed))
            })
            .collect::<Vec<_>>();
        // A stable reference binding can designate changed contents. Capture
        // those contents as a fresh post-call value, never as the entry value.
        // The frame owns the exact referent coordinate; selected public
        // guarantees own its equations. Install it only after retiring writes.
        let mut post_storage = Vec::new();
        for subject in subjects.as_ref().into_iter().flatten() {
            if subject.preserved_on_return || !subject.stable_on_return {
                continue;
            }
            let Some(coordinate) = self.captured_reference_subject(&subject.entry) else {
                continue;
            };
            self.capture_sequence += 1;
            let value = StructuralTerm::CapturedValue {
                statement,
                expression,
                callable: owner_symbol(&owner),
                occurrence: self.capture_sequence,
                result_type: subject.entry.type_reference,
                integer_range: None,
            };
            let mut observed = if program
                .primitive_type_reference(subject.entry.type_reference)
                .is_some()
            {
                StructuralTerm::ObservedValue {
                    observed_type: subject.entry.type_reference,
                    descriptor: None,
                    value: Box::new(value.clone()),
                }
            } else {
                value.clone()
            };
            // The normal boundary restores declared predicates on these fresh
            // returned contents; retired pre-call observations supply none.
            self.intake_admitted_value_predicates(subject.entry.type_reference, value.clone());
            if let Some(parameter) = parameters.iter().find(|parameter| {
                parameter.symbol == subject.entry.parameter
                    && !parameter.is_mutable
                    && crate::slice_view_shell(program, parameter.type_reference, &[]).is_some()
            }) && let Some((_, descriptor)) = entry_bindings
                .iter()
                .find(|(symbol, _)| *symbol == subject.entry.parameter)
            {
                observed = super::super::super::structural_terms::reference_argument_observation(
                    program,
                    parameter.type_reference,
                    descriptor,
                    Some(observed),
                )?;
            }
            // Exact formal substitution selects the first binding. Replace
            // its metadata-only row rather than shadowing this authenticated
            // post-call observation with an earlier same-symbol entry value.
            if let Some((_, current)) = bindings
                .iter_mut()
                .find(|(symbol, _)| *symbol == subject.entry.parameter)
            {
                *current = observed;
            } else {
                bindings.push((subject.entry.parameter, observed));
            }
            post_storage.push(PostCallStorage {
                coordinate,
                value,
                backing: None,
            });
        }
        Some(CapturedCallBindings {
            entry_bindings,
            bindings,
            post_storage,
        })
    }
}

impl StructuralJudge<'_> {
    pub(super) fn intake_normal_call_equations(&mut self, equations: Vec<CapturedResultEquation>) {
        for equation in equations {
            // This producer already authenticated the completed call and its
            // eager inputs. Preserve that identity instead of evaluating its
            // description again; active dependency collectors still own the walk.
            let completed_identity =
                equation.completed_call_identity && !self.has_resolution_dependency_collector();
            let (left, right) = (equation.left.clone(), equation.right.clone());
            let max_premise_occ = self.rewrites.max_premise_occurrence();
            let pattern_occ = match (
                Self::term_max_captured_occurrence(&left),
                Self::term_max_captured_occurrence(&right),
            ) {
                (Some(l), Some(r)) => {
                    if l > r {
                        Some(l)
                    } else if r > l {
                        Some(r)
                    } else {
                        None
                    }
                }
                (Some(l), None) => Some(l),
                (None, Some(r)) => Some(r),
                (None, None) => None,
            };
            if let Some(occ) = pattern_occ
                && occ <= max_premise_occ
            {
                std::sync::Arc::make_mut(&mut self.rewrites)
                    .bump_integer_premise_revision_for_occurrences(occ);
            }
            std::sync::Arc::make_mut(&mut self.captured_result_equations).push(equation);
            std::sync::Arc::make_mut(&mut self.normal_guarantees)
                .push((left.clone(), right.clone()));
            if completed_identity {
                self.intake_completed_call_identity(left, right);
            } else {
                self.intake_equation(left, right, 0);
            }
        }
    }
}

impl StructuralJudge<'_> {
    pub(super) fn normal_call_case_guarantees(
        &self,
        owner: &SelectedCallContract<'_>,
        parameters: &[typed_trees::signature::StateParameter],
        bindings: &[(SymbolHandle, StructuralTerm)],
        result: Option<&StructuralTerm>,
    ) -> Vec<super::super::CaseGuarantee> {
        if !parameters.iter().all(|parameter| {
            bindings
                .iter()
                .any(|(symbol, _)| *symbol == parameter.symbol)
        }) {
            return Vec::new();
        }
        let map = parameters
            .iter()
            .filter_map(|parameter| {
                bindings
                    .iter()
                    .find(|(symbol, _)| *symbol == parameter.symbol)
                    .map(|(_, value)| (parameter.name.to_string(), value.clone()))
            })
            .chain(
                result.map(|value| (super::super::super::RESULT_BINDER.to_owned(), value.clone())),
            )
            .collect::<Vec<_>>();
        let mut cases = Vec::new();
        for contract in owner_contracts(self.program, owner)
            .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        {
            for fact in self.program.proof_facts.span_or_empty(contract.facts) {
                if let ProofFact::Expression(expression) = fact {
                    self.instantiated_contract_case_guarantees(
                        owner,
                        *expression,
                        &map,
                        &mut cases,
                    );
                }
            }
        }
        cases
    }
}
