//! Temporary receiver diagnostics for the frozen add_zero_right refusal.
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
            .is_some_and(|machine| machine.name.as_str() == "add_zero_right")
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
        let Some(StructuralTerm::Identity {
            carrier,
            left,
            right,
        }) = predicate.as_ref()
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
        describe("original-left", left);
        describe("original-right", right);
        let left = self.resolve(left.as_ref().clone());
        let right = self.resolve(right.as_ref().clone());
        eprintln!(
            "TEMP-IDQUERY resolved-available={}/{} semantic-equal={} payload-equal={}",
            self.identity_value_is_available(&left),
            self.identity_value_is_available(&right),
            left == right,
            left.same_resolution_payload(&right)
        );
        describe("resolved-left", &left);
        describe("resolved-right", &right);
        for (name, value) in self.substitutions.iter().take(16) {
            eprintln!("TEMP-IDQUERY substitution={name}");
            describe("substitution", value);
        }
        for row in self
            .captured_result_equations
            .iter()
            .filter(|row| row.is_data_identity())
            .take(16)
        {
            describe("premise-left", &row.left);
            describe("premise-right", &row.right);
        }
    }
}

fn describe(label: &str, value: &StructuralTerm) {
    fn visit(label: &str, value: &StructuralTerm, path: &mut Vec<usize>, remaining: &mut usize) {
        if *remaining == 0 || path.len() >= 10 {
            return;
        }
        *remaining -= 1;
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
            visit(label, value, path, remaining);
            path.pop();
            child += 1;
        });
    }
    visit(label, value, &mut Vec::new(), &mut 64);
}
