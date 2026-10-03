//! Normal-return guarantees shared by ordinary and composed callable bodies.
//! Unit has no result value, but its post-state can establish caller facts.
//! Publication creates proof obligations; it does not assume the clauses.
//! Exit reconstruction joins the distinct contract result to the returned
//! value and proves guarantees from facts shared by every normal exit.
//! Crashes and incomplete calls establish neither a result nor these facts.

use crate::lowering_error::LoweringError;
use crate::scalar_graph::scalar_contracts::{self, ContractViewNamespace};
use crate::unit::ContractClause;
use checked_trees::CheckedTrees;
use symbols::SymbolHandle;
use terminal_psi::ValueDeclaration;

pub(crate) fn lower(
    checked: &CheckedTrees,
    state: SymbolHandle,
    parameters: &[ValueDeclaration],
    result: Option<ValueDeclaration>,
    erased: &[ValueDeclaration],
    views: &ContractViewNamespace<'_>,
    next_obligation: &mut u64,
) -> Result<Vec<ContractClause>, LoweringError> {
    let (source, source_state) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let contract = checked
        .facts
        .contract_plans
        .for_machine(source.symbol)
        .ok_or(LoweringError::Unsupported(
            "normal return has no checked contract",
        ))?;
    // A scalar result publishes its whole refined contract; a Unit or
    // structural result publishes only the rows its post-state validation
    // accepted, leaving memberships and evidence lanes to their owners.
    let refined;
    let post_state;
    let ensures = if result.is_some() {
        scalar_contracts::validate_guarantees(checked, source, source_state)?;
        refined = scalar_contracts::with_result_range(
            checked,
            state,
            parameters.len(),
            &contract.closed_scalar_values,
        )?;
        refined.ensures()
    } else {
        post_state =
            scalar_contracts::validate_post_state_guarantees(checked, source, source_state)?;
        &post_state[..]
    };
    // Returning execution restores the declared domain of every exclusive
    // loan: the declaration predicate roster's restored rows follow the
    // authored guarantees as the same kind of obligation, rederived from the
    // entry parameters' declared storage rather than read from a package.
    let mut guarantees = ensures.to_vec();
    guarantees.extend(
        validation::machine_declaration_predicates(
            &checked.typed,
            &checked.facts.operators,
            source,
        )
        .into_iter()
        .filter(|row| row.restored_on_return)
        .map(|row| {
            Some(checked_trees::ClosedScalarContractValue::Predicate(
                row.predicate,
            ))
        }),
    );
    let mut namespace = parameters.to_vec();
    namespace.extend(result);
    scalar_contracts::clauses(&guarantees, &namespace, erased, views)?
        .into_iter()
        .map(|proposition| {
            Ok(ContractClause {
                obligation: crate::terminal_identities::obligation_id(
                    crate::terminal_identities::allocate_dense(next_obligation)?,
                ),
                proposition,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::{TerminalMachineSelection, lower_machine};
    use proof_admission::AdmissionProfile;
    use semantic_vocabulary::IntegerValue;
    use terminal_psi::OperationKind;

    #[test]
    fn unit_call_guarantees_rejoin_the_written_post_state() {
        verify_post_state("self.length = 0;");
    }

    #[test]
    fn composed_unit_call_guarantees_rejoin_the_written_post_state() {
        verify_post_state(
            "transition { _ -> reset() }
             state reset(&mut self) { self.length = 0; }",
        );
    }

    /// A structural result's domain membership belongs to its qualification
    /// owner, not the closed scalar contract. Post-state publication skips
    /// that clause and still publishes the scalar field guarantee beside it.
    #[test]
    fn structural_result_membership_stays_with_its_owner_beside_a_post_state_bound() {
        let checked = crate::front_end::checked_program(
            "pub data Folder { depth: u32; }
             pub domain Folder::Writable established by Picker::pick;
             data Picker { items: [i32; 4]; length: u64 in Wrapping; }
             machine Picker::pick(&mut self) -> Folder
             ensures result in Folder::Writable
             ensures self.length < self.items.len
             {
                 self.length = 0;
                 Folder { depth: 1 }
             }
             data Main { picker: Picker; }
             machine Main::main(&mut self) {
                 let folder: Folder = self.picker.pick();
             }",
        );
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
            .expect("a structural result with a membership guarantee lowers");
        let published = lowered
            .semantic_module
            .machines
            .iter()
            .map(|machine| machine.contract.ensures.len())
            .sum::<usize>();
        assert_eq!(published, 1, "only the scalar post-state bound publishes");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .unwrap();
    }

    const UTF8_NAME: &str = "domain [u8; 5]::Utf8 requires valid_utf8(self);";

    /// A Unit post-state membership (`ensures self.name in Utf8`) has no
    /// Terminal proposition; skipping it must still publish the bound beside
    /// it, which is what discharges `accept`'s requirement at the call.
    #[test]
    fn unit_field_membership_keeps_the_bound_its_caller_needs() {
        verify_post_state_with(
            UTF8_NAME,
            "name: [u8; 5] in Utf8;",
            "ensures self.name in Utf8",
            "self.name = \"Alpha\"; self.length = 0;",
        );
    }

    #[test]
    fn unit_field_membership_alone_publishes_no_guarantee() {
        let checked = crate::front_end::checked_program(&format!(
            "{UTF8_NAME}
             data Record {{ name: [u8; 5] in Utf8; }}
             machine Record::set(&mut self) ensures self.name in Utf8 {{ self.name = \"Alpha\"; }}
             machine Record::run(&mut self) {{ self.set(); }}",
        ));
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Record::run"))
            .expect("a membership postcondition does not block its Unit call");
        assert!(
            lowered
                .semantic_module
                .machines
                .iter()
                .all(|machine| machine.contract.ensures.is_empty()),
            "membership has no Terminal proposition to publish"
        );
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .unwrap();
    }

    #[test]
    fn saturating_field_guarantee_survives_unit_call() {
        let checked = crate::front_end::checked_program(
            "data Countdown { remaining: u32 in Saturating; }
             machine Countdown::arm(countdown: &mut Countdown, start: u32 in Saturating)
             requires start < 8
             ensures countdown.remaining > 0; countdown.remaining - 1 < 8
             { countdown.remaining = start + 1; }
             machine Countdown::tick(countdown: &mut Countdown) -> u32 in Saturating
             requires countdown.remaining > 0; countdown.remaining - 1 < 8
             { countdown.remaining = countdown.remaining - 1; countdown.remaining }
             data Main { timer: Countdown; result: u32 in Saturating; }
             machine Main::run(main: &mut Main) {
                 Countdown::arm(&mut main.timer, 4);
                 let left: u32 in Saturating = Countdown::tick(&mut main.timer);
                 main.result = left;
             }",
        );
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
            .expect("saturating field store proves guarantee at the Unit call");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("independent post-state replay");
    }

    fn verify_post_state(clear_body: &str) {
        verify_post_state_with("", "", "", clear_body);
    }

    /// Lower `Buffer::run`, whose `clear` call must publish the bound its
    /// `accept` call requires, then show that guarantee binds the exact write.
    fn verify_post_state_with(prelude: &str, fields: &str, clear_contract: &str, clear_body: &str) {
        let checked = crate::front_end::checked_program(&format!(
            "{prelude}
             data Buffer {{ {fields} items: [i32; 4]; length: u64 in Wrapping; }}
             machine Buffer::clear(&mut self)
             {clear_contract}
             ensures self.length < self.items.len {{ {clear_body} }}
             machine Buffer::accept(&mut self) requires self.length < self.items.len {{ self.length = 1; }}
             machine Buffer::run(&mut self) {{ self.clear(); self.accept(); }}",
        ));
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Buffer::run"))
            .expect("a Unit call exports its independently proved post-state guarantee");
        assert!(
            lowered
                .semantic_module
                .machines
                .iter()
                .any(|machine| !machine.contract.ensures.is_empty())
        );
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .unwrap();
        let mut substituted = lowered.semantic_module.clone();
        let mut changed = 0;
        for operation in substituted
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
        {
            if matches!(
                operation.kind,
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(0)
                }
            ) {
                operation.kind = OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(9),
                };
                changed += 1;
            }
        }
        assert_eq!(changed, 1, "change only the clear store's payload");
        assert!(
            terminal_verifier::verify_module(
                &substituted,
                &lowered.proof_bundle,
                &AdmissionProfile::default()
            )
            .is_err(),
            "the authored guarantee cannot justify a different write"
        );
    }
}
