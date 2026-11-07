//! Temporary receiver diagnostics for three unchanged library-law refusals.
//! This module is removed before final qualification. Names select diagnostic
//! output only; no judgment or fact intake depends on this instrumentation.

use super::{ConstructorIdentity, ExpressionHandle, StructuralJudge, StructuralTerm};

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn trace_identity_refusal(
        &self,
        fact: ExpressionHandle,
        arms: Option<usize>,
        case_hypotheses: usize,
        case_equations: usize,
        citations: usize,
    ) {
        // Bound this diagnostic run to the one independently traced law.
        if !self
            .program
            .machine(self.machine_symbol)
            .is_some_and(|machine| {
                matches!(
                    machine.name.as_str(),
                    "mul_zero_left" | "add_one_right" | "add_int_respects_eq"
                )
            })
        {
            return;
        }
        eprintln!(
            "TEMP-IDQUERY owner={:?} state={:?} fact={fact:?} arms={arms:?} cases={case_hypotheses}/{case_equations} citations={citations} runtime={} resolve={} case_only={} substitutions={} data_ids={} completed_nominal={}",
            self.machine_symbol,
            self.declaration_state,
            self.runtime_body_values,
            self.resolve_applications,
            self.case_facts_only,
            self.substitutions.len(),
            self.captured_result_equations
                .iter()
                .filter(|row| row.is_data_identity())
                .count(),
            self.completed_nominal_values.len(),
        );
        eprintln!(
            "TEMP-IDQUERY scope={:?}",
            self.declaration_scope().map(|(owner, parameters)| {
                (
                    owner,
                    parameters
                        .iter()
                        .map(|parameter| {
                            (
                                parameter.symbol,
                                parameter.name.as_str(),
                                parameter.type_reference,
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            })
        );
        if let Some(view) = self.program.prepared_mathematics() {
            let application = view.application(fact);
            eprintln!("TEMP-IDQUERY prepared={application:?}");
            if let Some(application) = application {
                eprintln!(
                    "TEMP-IDQUERY kernel={:?} identity={:?}",
                    view.arena.get(application.term),
                    view.identity_value(application, application.term, 0)
                );
                for capture in view
                    .applications
                    .captures
                    .span_or_empty(application.captures)
                    .iter()
                    .take(16)
                {
                    eprintln!(
                        "TEMP-IDQUERY capture={capture:?} node={:?}",
                        self.program.expression_table.expression(capture.expression)
                    );
                }
            }
        } else {
            eprintln!("TEMP-IDQUERY prepared-view=absent");
        }
        let predicate = self.declaration_term(fact);
        self.trace_formed_identity_refusal(predicate.as_ref());
    }

    pub(in crate::proof_contracts::contract_entailment) fn trace_formed_identity_refusal(
        &self,
        predicate: Option<&StructuralTerm>,
    ) {
        if !self
            .program
            .machine(self.machine_symbol)
            .is_some_and(|machine| {
                matches!(
                    machine.name.as_str(),
                    "mul_zero_left" | "add_one_right" | "add_int_respects_eq"
                )
            })
        {
            return;
        }
        eprintln!(
            "TEMP-IDQUERY formed owner={:?} runtime={} resolve={} closed={} specification={} data_ids={} results={} substitutions={} rings={:?} semirings={:?}",
            self.machine_symbol,
            self.runtime_body_values,
            self.resolve_applications,
            self.require_closed_computation,
            self.specification_terms,
            self.captured_result_equations
                .iter()
                .filter(|row| row.is_data_identity())
                .count(),
            self.normal_results.len(),
            self.substitutions.len(),
            self.ring_licenses.iter().take(16).collect::<Vec<_>>(),
            self.semiring_licenses.iter().take(16).collect::<Vec<_>>()
        );
        eprintln!(
            "TEMP-IDQUERY formed-scope={:?}",
            self.declaration_scope().map(|(owner, parameters)| (
                owner,
                parameters
                    .iter()
                    .take(16)
                    .map(|parameter| (
                        parameter.symbol,
                        parameter.name.as_str(),
                        parameter.type_reference
                    ))
                    .collect::<Vec<_>>()
            ))
        );
        let predicate = predicate.map(StructuralTerm::semantic_value);
        let Some(StructuralTerm::Identity {
            carrier,
            left,
            right,
        }) = predicate
        else {
            eprintln!(
                "TEMP-IDQUERY predicate-kind={}",
                if predicate.is_some() {
                    "non-id"
                } else {
                    "absent"
                }
            );
            return;
        };
        eprintln!(
            "TEMP-IDQUERY carrier={carrier:?} available={}/{}",
            self.identity_value_is_available(left),
            self.identity_value_is_available(right)
        );
        describe(self, "original-left", left);
        describe(self, "original-right", right);
        let left = self.resolve(left.as_ref().clone());
        let right = self.resolve(right.as_ref().clone());
        eprintln!(
            "TEMP-IDQUERY resolved-available={}/{} semantic-equal={} payload-equal={}",
            self.identity_value_is_available(&left),
            self.identity_value_is_available(&right),
            left == right,
            left.same_resolution_payload(&right)
        );
        describe(self, "resolved-left", &left);
        describe(self, "resolved-right", &right);
        for (name, value) in self.substitutions.iter().take(16) {
            eprintln!("TEMP-IDQUERY substitution={name}");
            describe(self, "substitution", value);
        }
        for (expression, value) in self.normal_results.iter().take(16) {
            eprintln!("TEMP-IDQUERY normal-result expression={expression:?}");
            describe(self, "normal-result", value);
        }
        for row in self
            .captured_result_equations
            .iter()
            .filter(|row| row.is_data_identity())
            .take(16)
        {
            describe(self, "premise-left", &row.left);
            describe(self, "premise-right", &row.right);
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn trace_identity_arm_value(
        &self,
        value: &StructuralTerm,
    ) {
        if !self
            .program
            .machine(self.machine_symbol)
            .is_some_and(|machine| {
                matches!(machine.name.as_str(), "mul_zero_left" | "add_one_right")
            })
        {
            return;
        }
        let Some(machine) = self.program.machine(self.machine_symbol) else {
            return;
        };
        let entry = self
            .program
            .machine_states(machine)
            .first()
            .map(|state| state.symbol);
        let mut applications = Vec::new();
        if let Some(entry) = entry {
            Self::self_applications(value, entry, &mut applications);
        }
        eprintln!(
            "TEMP-IDQUERY arm-value self-entry={entry:?} self-applications={}",
            applications.len()
        );
        describe(self, "arm-value", value);
        for application in applications.into_iter().take(16) {
            describe(self, "self-application", application);
        }
    }
}

fn describe(judge: &StructuralJudge<'_>, label: &str, value: &StructuralTerm) {
    fn visit(
        judge: &StructuralJudge<'_>,
        label: &str,
        value: &StructuralTerm,
        path: &mut Vec<usize>,
        remaining: &mut usize,
    ) {
        if *remaining == 0 || path.len() >= 10 {
            return;
        }
        *remaining -= 1;
        application_metadata(judge, label, path, value);
        match value {
            StructuralTerm::Identity { carrier, .. } => {
                eprintln!("TEMP-IDQUERY {label} path={path:?} Id carrier={carrier:?}")
            }
            StructuralTerm::ObservedValue {
                observed_type,
                descriptor,
                ..
            } => eprintln!(
                "TEMP-IDQUERY {label} path={path:?} observation type={observed_type:?} descriptor={}",
                descriptor.is_some()
            ),
            StructuralTerm::Constructor {
                identity,
                data,
                case,
                fields,
            } => match identity {
                ConstructorIdentity::Nominal {
                    carrier,
                    reference,
                    declaration,
                    case: selected_case,
                    member,
                } => eprintln!(
                    "TEMP-IDQUERY {label} path={path:?} ctor={data}/{case} carrier={carrier:?} reference={reference:?} declaration={declaration:?} case={selected_case:?} member={member:?} fields={}",
                    fields.len()
                ),
                ConstructorIdentity::Boolean(value) => {
                    eprintln!("TEMP-IDQUERY {label} path={path:?} bool={value}")
                }
            },
            StructuralTerm::Application {
                result_type,
                target,
                selections,
                machine,
                arguments,
            } => eprintln!(
                "TEMP-IDQUERY {label} path={path:?} application={machine} result={result_type:?} target={target:?} selections={} arguments={}",
                selections.len(),
                arguments.len()
            ),
            StructuralTerm::CapturedValue {
                expression,
                callable,
                occurrence,
                result_type,
                ..
            } => eprintln!(
                "TEMP-IDQUERY {label} path={path:?} capture={expression:?} callable={callable:?} occurrence={occurrence} result={result_type:?}"
            ),
            StructuralTerm::Variable(name) => {
                eprintln!("TEMP-IDQUERY {label} path={path:?} variable={name}")
            }
            StructuralTerm::BoundValue(symbol) => {
                eprintln!("TEMP-IDQUERY {label} path={path:?} bound={symbol:?}")
            }
            StructuralTerm::Projection { path: field, .. } => {
                eprintln!("TEMP-IDQUERY {label} path={path:?} projection={field}")
            }
            StructuralTerm::Float(value) => eprintln!(
                "TEMP-IDQUERY {label} path={path:?} float={:?}",
                value.presented_bits()
            ),
            StructuralTerm::ContentValue { algebra, .. } => {
                eprintln!("TEMP-IDQUERY {label} path={path:?} content={algebra:?}")
            }
            StructuralTerm::Opaque(display) => eprintln!(
                "TEMP-IDQUERY {label} path={path:?} opaque={}",
                display.chars().take(160).collect::<String>()
            ),
            _ => eprintln!(
                "TEMP-IDQUERY {label} path={path:?} kind={:?}",
                std::mem::discriminant(value)
            ),
        }
        let mut child = 0;
        value.for_each_child(|value| {
            path.push(child);
            visit(judge, label, value, path, remaining);
            path.pop();
            child += 1;
        });
    }
    visit(judge, label, value, &mut Vec::new(), &mut 64);
}

fn application_metadata(
    judge: &StructuralJudge<'_>,
    label: &str,
    path: &[usize],
    value: &StructuralTerm,
) {
    let (target, selections, result_type, arguments, projection) = match value {
        StructuralTerm::Application {
            target,
            selections,
            result_type,
            arguments,
            ..
        } => (
            *target,
            selections.as_slice(),
            *result_type,
            arguments.as_slice(),
            None,
        ),
        StructuralTerm::CallProjection {
            target,
            selections,
            result_type,
            arguments,
            field,
            ..
        } => (
            *target,
            selections.as_slice(),
            *result_type,
            arguments.as_slice(),
            Some(*field),
        ),
        _ => return,
    };
    let selected = crate::machine_calls::calls::machine_state_by_symbol(judge.program, target);
    let declared = selected.map(|(_, state)| state.return_type);
    let full_match = declared.is_some_and(|declared| {
        crate::value_custody::type_references::type_references_match(
            judge.program,
            result_type,
            declared,
        )
    });
    let admission = super::specification_application_candidate(
        judge.program,
        judge.machine_symbol,
        target,
        selections,
        judge.frame_resolver(),
    );
    let abstract_signature =
        crate::machine_calls::denotational_calls::abstract_value_call_signature(
            judge.program,
            target,
        )
        .is_some();
    eprintln!(
        "TEMP-IDQUERY {label} path={path:?} application-gates target={target:?} result={result_type:?} result-normalized={:?} declared={declared:?} declared-normalized={:?} raw-return-match={} full-type-policy-match={full_match} admission={admission:?} abstract-signature={abstract_signature} entry-machine={:?} closed-arguments={} argument-availability={:?} projection={projection:?}",
        result_type
            .is_valid()
            .then(|| judge.program.normalized_type_identity(result_type)),
        declared
            .filter(|reference| reference.is_valid())
            .map(|reference| judge.program.normalized_type_identity(reference)),
        declared == Some(result_type),
        judge
            .entry_machines
            .machine(judge.program, target)
            .map(|machine| machine.symbol),
        arguments.iter().all(StructuralTerm::is_closed_value),
        arguments
            .iter()
            .take(16)
            .map(|argument| judge.identity_value_is_available(argument))
            .collect::<Vec<_>>()
    );
    if let Some((machine, state)) = selected {
        eprintln!(
            "TEMP-IDQUERY {label} path={path:?} body-gates machine={:?} state={:?} states={} static-formals={} selections={} formals={} actuals={} statements={}",
            machine.symbol,
            state.symbol,
            judge.program.machine_states(machine).len(),
            judge.program.machine_type_parameters(machine).len(),
            selections.len(),
            judge.program.state_parameters(state).len(),
            arguments.len(),
            judge
                .program
                .statement_table
                .statements(state.statement_nodes)
                .len()
        );
    }
    let unfolded = match projection {
        Some(field) => judge.unfold_exact_call_projection(
            target,
            selections,
            result_type,
            field,
            arguments,
            0,
            super::ResolutionPurpose::General,
        ),
        None => judge.unfold_application(value, 0),
    };
    eprintln!(
        "TEMP-IDQUERY {label} path={path:?} unfold-kind={:?} unfold-available={:?} unfold-application-result={:?}",
        unfolded.as_ref().map(std::mem::discriminant),
        unfolded
            .as_ref()
            .map(|value| judge.identity_value_is_available(value)),
        unfolded
            .as_ref()
            .and_then(|value| match value.semantic_value() {
                StructuralTerm::Application { result_type, .. } => Some(*result_type),
                _ => None,
            })
    );
}
