//! Check the nominated direct provider rows against the exact sources of an execution scope.
use crate::DirectBoundaryLeaf;
use diagnostics::Diagnostic;
use selected_provider_plans::SelectedProviderPlanFacts;
use std::{collections::BTreeSet, sync::Arc};
use target_vocabulary::publication::EmittedExecutionScope;

pub struct VerifiedDirectProviderSelection<'a> {
    module: &'a terminal_psi::TerminalModule,
    source: &'a SelectedProviderPlanFacts,
    leaves: &'a [DirectBoundaryLeaf],
    reach: Arc<EmittedExecutionScope>,
    selected: SelectedProviderPlanFacts,
}

impl<'a> VerifiedDirectProviderSelection<'a> {
    /// Verify the proposed leaf positions, including omitted positions, before
    /// extending the original selection. The caller supplies indices and never
    /// supplies a replacement plan or a digest to stamp.
    pub fn check(
        module: &'a terminal_psi::TerminalModule,
        authored: &'a SelectedProviderPlanFacts,
        leaves: &'a [DirectBoundaryLeaf],
        reach: &Arc<EmittedExecutionScope>,
        nominees: &[usize],
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut unbound = BTreeSet::new();
        for boundary in module
            .interfaces
            .boundary_machines
            .iter()
            .filter(|boundary| reach.contains_boundary(boundary.id))
        {
            let name = module
                .identities
                .text(boundary.identity)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "native execution boundary has no exact identity",
                    )]
                })?;
            if authored.selected_requirement_row(name).is_none() {
                unbound.insert(name);
            }
        }
        check_nominees(
            nominees,
            leaves
                .iter()
                .map(|leaf| leaf.matches_any_requirement(&unbound)),
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let direct = nominees
            .iter()
            .map(|&i| {
                let leaf = &leaves[i];
                leaf.validate_bound_calling_custody(module)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                Ok(leaf.plan.clone())
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let selection = authored
            .clone()
            .with_direct_boundary_plans(direct)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        Ok(Self {
            module,
            source: authored,
            leaves,
            reach: Arc::clone(reach),
            selected: selection,
        })
    }

    /// Join the independently held proposal and scope to the sources checked above.
    pub fn into_selection(
        self,
        module: &terminal_psi::TerminalModule,
        source: &SelectedProviderPlanFacts,
        leaves: &[DirectBoundaryLeaf],
        reach: &Arc<EmittedExecutionScope>,
    ) -> Result<SelectedProviderPlanFacts, &'static str> {
        if !std::ptr::eq(self.module, module)
            || !std::ptr::eq(self.source, source)
            || !std::ptr::eq(self.leaves, leaves)
            || !Arc::ptr_eq(&self.reach, reach)
        {
            return Err(
                "native direct provider selection belongs to another program, proposal or reach",
            );
        }
        Ok(self.selected)
    }
}
fn check_nominees(
    nominees: &[usize],
    matches: impl IntoIterator<Item = bool>,
) -> Result<(), &'static str> {
    let mut nominees = nominees.iter().copied().peekable();
    for (position, matches) in matches.into_iter().enumerate() {
        let nominated = nominees.next_if_eq(&position).is_some();
        if nominated != matches {
            return Err("native direct provider nominees differ from the scoped requirements");
        }
    }
    if nominees.next().is_some() {
        return Err("native direct provider nominees repeat or exceed the proposal leaves");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{VerifiedDirectProviderSelection, check_nominees};
    use selected_provider_plans::SelectedProviderPlanFacts;
    use std::sync::Arc;
    use target_vocabulary::publication::EmittedExecutionScope;

    #[test]
    fn a_receipt_cannot_be_attached_to_another_program_selection_or_reach() {
        let module = terminal_psi::TerminalModule::declarations();
        let source = SelectedProviderPlanFacts::default();
        let reach = Arc::new(EmittedExecutionScope::new(
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ));
        let verify =
            || VerifiedDirectProviderSelection::check(&module, &source, &[], &reach, &[]).unwrap();
        assert_eq!(
            verify()
                .into_selection(&module, &source, &[], &reach)
                .unwrap(),
            source
        );
        assert!(
            verify()
                .into_selection(&module.clone(), &source, &[], &reach)
                .is_err()
        );
        assert!(
            verify()
                .into_selection(&module, &source.clone(), &[], &reach)
                .is_err()
        );
        let other_reach = Arc::new((*reach).clone());
        assert!(
            verify()
                .into_selection(&module, &source, &[], &other_reach)
                .is_err()
        );
        assert!(
            VerifiedDirectProviderSelection::check(&module, &source, &[], &reach, &[0]).is_err()
        );
    }

    #[test]
    fn exactly_the_matching_proposal_positions_are_admitted() {
        let matches = [false, true, false, true];
        assert!(check_nominees(&[1, 3], matches).is_ok());
        assert!(check_nominees(&[], [false, false]).is_ok());
        for changed in [&[1][..], &[1, 2, 3], &[1, 1, 3], &[3, 1], &[1, 3, 4]] {
            assert!(check_nominees(changed, matches).is_err(), "{changed:?}");
        }
    }
}
