//! Authenticate the original group settings loan before recording a provider
//! selection in the registration that issued it.
use super::*;
use terminal_interpreter::{
    TerminalOriginalScalarBackings, TerminalOriginalStorageRequest, TerminalOriginalStorageRoot,
};

const ISSUED_GROUP: &str =
    "provider selection requires a TestGroup issued by this Build activation";

impl TerminalBuildBindingHost {
    pub(super) fn original_test_group(
        &self,
        boundary: semantic_vocabulary::BoundaryMachineId,
        requests: &[TerminalOriginalStorageRequest],
    ) -> Result<
        (
            usize,
            TerminalOriginalStorageRoot,
            TerminalOriginalScalarBackings,
        ),
        TerminalEffectRejection,
    > {
        let refuse = || TerminalEffectRejection::new(ISSUED_GROUP);
        let [request] = requests else {
            return Err(refuse());
        };
        if request.root().artifact() != self.artifact.semantic()
            || request.argument_index() != 0
            || request.access() != terminal_psi::StructuralAccess::MutableBorrow
            || !request.argument_path().is_empty()
            || !request.referent().is_empty()
            || !self.calls.iter().any(|call| {
                call.site.boundary == boundary && matches!(call.operand,
                    Some(TerminalBuildBindingOperand::TestGroup { referent }) if referent == request.view_type())
            })
        {
            return Err(refuse());
        }
        let fields = request.values();
        if !fields
            .windows(2)
            .all(|pair| pair[0].path() < pair[1].path())
        {
            return Err(refuse());
        }
        let shape = self.group_shape.as_ref().ok_or_else(refuse)?;
        let disabled = fields
            .binary_search_by(|field| field.path().cmp(&shape.disabled_path()))
            .map_err(|_| refuse())?;
        let backings = fields
            .iter()
            .map(|field| field.backing().cloned().ok_or_else(refuse))
            .collect::<Result<Vec<_>, _>>()?;
        let state = lock_state(&self.state).map_err(TerminalEffectRejection::new)?;
        let group = state.executed.iter().position(|selection| {
            matches!(selection, ActivationSelection::TestGroup(group) if group.owns_settings(backings[disabled].as_ref()))
        }).ok_or_else(refuse)?;
        Ok((group, request.root(), vec![backings]))
    }
}

impl BindingState {
    pub(super) fn record_group_provider(
        &mut self,
        group: usize,
        selection: ExecutedProductSelection,
    ) -> Result<(), TerminalEffectRejection> {
        let Some(ActivationSelection::TestGroup(registration)) = self.executed.get_mut(group)
        else {
            return Err(TerminalEffectRejection::new(ISSUED_GROUP));
        };
        registration.select_provider(selection)
    }
}
