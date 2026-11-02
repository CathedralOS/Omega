//! Prepare selected entry evidence before publishing normal-return guarantees.
//!
//! Borrow the exact captured telescope. Portable members own only their actual
//! alpha-substituted rows; completion never re-observes source operands.

use super::*;
use std::borrow::Cow;

pub(in crate::proof_contracts::contract_entailment::structural_judgment) struct PreparedSelectedGuarantees<
    'call,
> {
    members: Vec<PreparedDeclaredGuarantees<'call>>,
}

struct PreparedDeclaredGuarantees<'call> {
    owner: SelectedCallContract<'call>,
    entry_bindings: Cow<'call, [(SymbolHandle, StructuralTerm)]>,
    bindings: Cow<'call, [(SymbolHandle, StructuralTerm)]>,
    type_bindings: Cow<'call, [(SymbolHandle, TypeReferenceHandle)]>,
    evidence: EntryEvidence,
}

enum EntryEvidence {
    Requirements,
    UnconditionalBoolean(StructuralTerm),
}

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn prepare_selected_guarantees<
        'call,
    >(
        &'call self,
        owner: &SelectedCallContract<'call>,
        entry_bindings: &'call [(SymbolHandle, StructuralTerm)],
        bindings: &'call [(SymbolHandle, StructuralTerm)],
        type_bindings: &'call [(SymbolHandle, TypeReferenceHandle)],
        depth: usize,
    ) -> Option<PreparedSelectedGuarantees<'call>> {
        let program = self.program;
        if let SelectedCallContract::Concrete(machine, state) = owner
            && machine.target_family_interface
        {
            if program.machine_states(machine).first()?.symbol != state.symbol {
                return None;
            }
            let parameters = program.state_parameters(state);
            let static_parameters = program.machine_type_parameters(machine);
            let static_binders = static_parameters
                .iter()
                .enumerate()
                .map(|(ordinal, parameter)| (parameter.symbol, format!("static{ordinal}")))
                .collect::<Vec<_>>();
            let members = crate::machine_calls::calls::target_family_members(program, machine)?;
            let mut prepared = Vec::new();
            for member in members {
                let machine = program.machine(member.machine)?;
                let entry = program.machine_states(machine).first()?;
                let member_parameters = program.state_parameters(entry);
                let member_static = program.machine_type_parameters(machine);
                if parameters.len() != member_parameters.len()
                    || static_parameters.len() != member_static.len()
                {
                    return None;
                }
                let member_binders = member_static
                    .iter()
                    .enumerate()
                    .map(|(ordinal, parameter)| (parameter.symbol, format!("static{ordinal}")))
                    .collect::<Vec<_>>();
                // The public family and each body own distinct binder symbols.
                // Preserve complete declaration order and exact const carriers
                // before transporting a known static value to a member.
                if !static_parameters.iter().zip(member_static).all(|(parameter, member)| {
                    use typed_trees::data::TypeParameterKind;
                    match (&parameter.kind, &member.kind) {
                        (TypeParameterKind::Type, TypeParameterKind::Type) => true,
                        (TypeParameterKind::Const { type_reference: parameter },
                         TypeParameterKind::Const { type_reference: member }) => {
                            let identity = |reference, binders| {
                                program.type_identity(typed_trees::type_identity::TypeIdentityRequest {
                                    binders,
                                    ..typed_trees::type_identity::TypeIdentityRequest::ordinary(reference)
                                })
                            };
                            let parameter = identity(*parameter, &static_binders);
                            parameter.is_valid() && parameter == identity(*member, &member_binders)
                        }
                        _ => false,
                    }
                }) {
                    return None;
                }
                let member_bindings = |bindings: &[(SymbolHandle, StructuralTerm)]| {
                    parameters
                        .iter()
                        .zip(member_parameters)
                        .filter_map(|(parameter, member)| {
                            bindings
                                .iter()
                                .find(|(symbol, _)| *symbol == parameter.symbol)
                                .map(|(_, value)| (member.symbol, value.clone()))
                        })
                        .chain(static_parameters.iter().zip(member_static).filter_map(
                            |(parameter, member)| {
                                bindings
                                    .iter()
                                    .find(|(symbol, _)| *symbol == parameter.symbol)
                                    .map(|(_, value)| (member.symbol, value.clone()))
                            },
                        ))
                        .collect::<Vec<_>>()
                };
                let member_type_bindings = static_parameters
                    .iter()
                    .zip(member_static)
                    .filter_map(|(parameter, member)| {
                        type_bindings
                            .iter()
                            .find(|(symbol, _)| *symbol == parameter.symbol)
                            .map(|(_, reference)| (member.symbol, *reference))
                    })
                    .collect::<Vec<_>>();
                let owner = SelectedCallContract::Concrete(machine, entry);
                let entry_bindings = member_bindings(entry_bindings);
                let bindings = member_bindings(bindings);
                let evidence = self.prepare_declared_entry(
                    &owner,
                    &entry_bindings,
                    &member_type_bindings,
                    depth,
                )?;
                prepared.push(PreparedDeclaredGuarantees {
                    owner,
                    entry_bindings: std::borrow::Cow::Owned(entry_bindings),
                    bindings: std::borrow::Cow::Owned(bindings),
                    type_bindings: std::borrow::Cow::Owned(member_type_bindings),
                    evidence,
                });
            }
            return Some(PreparedSelectedGuarantees { members: prepared });
        }
        let evidence = self.prepare_declared_entry(owner, entry_bindings, type_bindings, depth)?;
        Some(PreparedSelectedGuarantees {
            members: vec![PreparedDeclaredGuarantees {
                owner: *owner,
                entry_bindings: std::borrow::Cow::Borrowed(entry_bindings),
                bindings: std::borrow::Cow::Borrowed(bindings),
                type_bindings: std::borrow::Cow::Borrowed(type_bindings),
                evidence,
            }],
        })
    }

    fn prepare_declared_entry(
        &self,
        owner: &SelectedCallContract<'_>,
        entry_bindings: &[(SymbolHandle, StructuralTerm)],
        type_bindings: &[(SymbolHandle, TypeReferenceHandle)],
        depth: usize,
    ) -> Option<EntryEvidence> {
        if depth >= 32 {
            return None;
        }
        let program = self.program;
        let parameters = owner_parameters(program, owner);
        let callable = match owner {
            SelectedCallContract::Concrete(machine, _) => machine.symbol,
            _ => owner_symbol(owner),
        };
        for contract in owner_contracts(program, owner)
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let judgment = match fact {
                    ProofFact::Expression(required) => selected_requirement_judgment(
                        self,
                        *required,
                        callable,
                        parameters,
                        entry_bindings,
                        type_bindings,
                        depth + 1,
                    ),
                    ProofFact::Membership(required) => self.selected_membership_requirement(
                        required,
                        callable,
                        parameters,
                        entry_bindings,
                    ),
                    ProofFact::Proposition(_) => StructuralJudgment::Unknown,
                };
                if !matches!(judgment, StructuralJudgment::Proven) {
                    if super::super::super::struct_trace() {
                        eprintln!(
                            "STRUCT normal-guarantee requires owner={callable:?} fact={fact:?} judgment={judgment:?} entry_bindings={entry_bindings:?}"
                        );
                    }
                    return self
                        .recorded_body_boolean(owner, depth + 1)
                        .map(EntryEvidence::UnconditionalBoolean);
                }
            }
        }
        Some(EntryEvidence::Requirements)
    }

    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn complete_selected_guarantees(
        &self,
        prepared: &PreparedSelectedGuarantees<'_>,
        result: Option<&StructuralTerm>,
        depth: usize,
    ) -> Option<SelectedGuarantees> {
        let mut common: Option<SelectedGuarantees> = None;
        for member in &prepared.members {
            let guarantees = match &member.evidence {
                EntryEvidence::Requirements => self.declared_guarantees_after_entry(
                    &member.owner,
                    &member.entry_bindings,
                    &member.bindings,
                    &member.type_bindings,
                    result,
                    depth,
                )?,
                EntryEvidence::UnconditionalBoolean(literal) => SelectedGuarantees {
                    equations: vec![(result?.clone(), literal.clone())],
                    content: Vec::new(),
                },
            };
            if let Some(common) = &mut common {
                common.equations.retain(|(left, right)| {
                    guarantees
                        .equations
                        .iter()
                        .any(|(other_left, other_right)| {
                            (left == other_left && right == other_right)
                                || (left == other_right && right == other_left)
                        })
                });
                common.content.retain_mut(|law| {
                    let Some(other) = guarantees
                        .content
                        .iter()
                        .find(|other| law.same_equation(other))
                    else {
                        return false;
                    };
                    law.retain_family_sources(other);
                    true
                });
            } else {
                common = Some(guarantees);
            }
        }
        common
    }

    fn declared_guarantees_after_entry(
        &self,
        owner: &SelectedCallContract<'_>,
        entry_bindings: &[(SymbolHandle, StructuralTerm)],
        bindings: &[(SymbolHandle, StructuralTerm)],
        type_bindings: &[(SymbolHandle, TypeReferenceHandle)],
        result: Option<&StructuralTerm>,
        depth: usize,
    ) -> Option<SelectedGuarantees> {
        let program = self.program;
        let parameters = owner_parameters(program, owner);
        let callable = match owner {
            SelectedCallContract::Concrete(machine, _) => machine.symbol,
            _ => owner_symbol(owner),
        };
        let mut completion = self.clone();
        if parameters.iter().all(|parameter| {
            bindings
                .iter()
                .any(|(symbol, _)| *symbol == parameter.symbol)
        }) {
            let map =
                parameters
                    .iter()
                    .filter_map(|parameter| {
                        bindings
                            .iter()
                            .find(|(symbol, _)| *symbol == parameter.symbol)
                            .map(|(_, value)| (parameter.name.to_string(), value.clone()))
                    })
                    .chain(result.map(|value| {
                        (super::super::super::RESULT_BINDER.to_owned(), value.clone())
                    }))
                    .collect::<Vec<_>>();
            let mut cases = Vec::new();
            for contract in owner_contracts(program, owner).filter(|contract| {
                contract.kind == SignatureContractKind::Ensures && contract.binding.is_none()
            }) {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    if let Some(expression) = fact.as_expression() {
                        completion.instantiated_contract_case_guarantees(
                            owner,
                            *expression,
                            &map,
                            &mut cases,
                        );
                    }
                }
            }
            for case in &cases {
                completion.intake_case_guarantee(case);
            }
        }
        let mut equations = Vec::new();
        for contract in owner_contracts(program, owner)
            .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let ProofFact::Expression(guarantee) = fact {
                    capture_guarantees(
                        &completion,
                        *guarantee,
                        callable,
                        parameters,
                        callable,
                        bindings,
                        type_bindings,
                        result,
                        &mut equations,
                        depth + 1,
                    );
                }
            }
        }
        // A body-wide constant records its literal on the result. Both a
        // match and a direct use read this equation; an authored `ensures`
        // is not required for a result every normal exit already supplies.
        if let Some(result) = result
            && let Some(literal) = self.recorded_body_boolean(owner, depth + 1)
        {
            let equation = (result.clone(), literal);
            let present = equations.iter().any(|(left, right)| {
                (left, right) == (&equation.0, &equation.1)
                    || (left, right) == (&equation.1, &equation.0)
            });
            if !present {
                equations.push(equation);
            }
        }
        // Conservation snapshots belong to executed normal returns. A pure
        // specification application has no call-entry storage epoch and must
        // not initialize normalization while forming a projection's own body.
        let content = if self.runtime_body_values
            && let Some(result @ StructuralTerm::CapturedValue { .. }) = result
        {
            super::super::super::content_conservation::call_content_guarantees(
                program,
                &completion,
                owner_symbol(owner),
                parameters,
                entry_bindings,
                bindings,
                result,
            )
        } else {
            Vec::new()
        };
        Some(SelectedGuarantees { equations, content })
    }
}
