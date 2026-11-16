//! Record which operators the numeric source contracts use.
//!
//! Input and crash clauses must match the catalog predicates. Reading an
//! ensures clause checks operand types and selects its actual operators; it
//! does not prove that equality. The catalog keeps the complete normal-result
//! meaning and result policy. Opaque mathematical types remain type references.

use semantic_vocabulary::{MachineId, ValueId};
use terminal_check_tables::{Checked, Construct};
use terminal_psi::{CrashPredicateTerm, CrashRouteGuard};
use typed_trees::declarations::intrinsic_requirements::IntrinsicRequirement;
use typed_trees::signature::SignatureContractKind;

use super::contracts::{ClauseScope, MathematicalTerms, ResultTerm};
use super::{Program, Slot, crash_cause, span};
use crate::body::{CrashCeiling, Regions, Signature, ValueType};
use crate::{Unserved, canonical, refuse};

impl<'t> Program<'t> {
    pub(super) fn numeric_contract_sources(
        &mut self,
        id: MachineId,
        source: &IntrinsicRequirement<'t>,
        signature: &Signature,
    ) -> Checked<()> {
        let unavailable = |detail| refuse(id, Construct::Machine, Unserved::Contract(detail));
        let slots: Vec<_> = source
            .parameters
            .iter()
            .zip(&signature.parameters)
            .map(|(formal, &value_type)| Slot {
                symbol: formal.symbol,
                value_type,
                assignable: false,
                homed: false,
                erased: false,
                borrowed: None,
                borrowed_regions: Regions::NONE,
            })
            .collect();
        let values = crate::body::formal_values(
            slots.iter().map(|slot| (slot.value_type, false, false)),
            signature.result,
        );
        let entry = ClauseScope {
            slots: &slots,
            values: &values,
            bound: &[],
            owner: source.call_target,
            result: ResultTerm::Absent("result in a numeric entry clause"),
            subject: None,
            formal_fields: true,
            logical: None,
            mathematical: MathematicalTerms::Served,
        };
        let Some(ValueType::Scalar(result, qualification)) = signature.result else {
            return Err(unavailable("numeric source contract has no scalar result"));
        };
        let result_value = ValueId::new(slots.len() as u64 + 1)
            .ok_or_else(|| unavailable("numeric source result identity overflow"))?;
        let exit = ClauseScope {
            result: ResultTerm::Named(result_value, result, qualification),
            ..entry
        };
        let typed = self.typed;
        let contracts = source
            .contracts(typed)
            .ok_or_else(|| unavailable("numeric source has no actual contract roster"))?;
        let mut requires = Vec::new();
        let mut routes = Vec::new();
        for contract in contracts {
            if contract.binding.is_some() {
                return Err(unavailable("numeric source contract has a result binding"));
            }
            let facts = span(
                |span| typed.proof_facts.span(span),
                contract.facts,
                id,
                "numeric source fact span",
            )?;
            match contract.kind {
                SignatureContractKind::Requires => {
                    for fact in facts {
                        if let Some((clause, _)) = self.contract_clause(id, &entry, fact, true)? {
                            requires.push(clause);
                        }
                    }
                }
                SignatureContractKind::Ensures => {
                    for fact in facts {
                        self.complete_typed_contract_operator(
                            id,
                            &exit,
                            source.parameters,
                            source.return_type,
                            self.fact(id, fact)?,
                        )?;
                    }
                }
                SignatureContractKind::Crashes { cause } => {
                    let cause = crash_cause(cause);
                    if facts.is_empty() {
                        routes.push((cause, CrashRouteGuard::Truth));
                    }
                    for fact in facts {
                        let clause = self.clause(id, &entry, self.fact(id, fact)?)?;
                        let clause = canonical::canonical_clause(clause, id)?;
                        routes.push((
                            cause,
                            CrashRouteGuard::Predicate(CrashPredicateTerm::new(clause)),
                        ));
                    }
                }
                SignatureContractKind::EnsuresForResultCase { .. } => {
                    return Err(unavailable(
                        "numeric source has an outcome-specific guarantee",
                    ));
                }
            }
        }
        if canonical::conjunction(requires, id)?
            != canonical::conjunction(signature.requires.clone(), id)?
        {
            return Err(unavailable(
                "numeric source requirements differ from the catalog",
            ));
        }
        let expected_routes = match &signature.crashes {
            CrashCeiling::DeclaredNone => &[][..],
            CrashCeiling::Authored(routes) => routes.as_slice(),
            CrashCeiling::Inferred => {
                return Err(unavailable("numeric catalog has an open crash ceiling"));
            }
        };
        if canonical::routes(routes) != expected_routes {
            return Err(unavailable(
                "numeric source crash guards differ from the catalog",
            ));
        }
        Ok(())
    }
}
