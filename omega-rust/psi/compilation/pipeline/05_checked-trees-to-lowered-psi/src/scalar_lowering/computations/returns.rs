//! Normal-result formation is not an ordinary operand coercion. The source
//! checker has already proved the exact exit's predicates and provenance.
//! Complete its ordinary operand schedule first; only the fresh completion
//! parameter receives the declared result qualification. Other retained-value
//! entrypoints keep exact type equality and cannot borrow this admission.

use super::{
    Argument, CheckedScalarExpressionRole, Expansion, LoweringError, QualifiedScalarType, Site,
    storage, unsupported,
};

impl Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn normal_return(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        exit: &checked_trees::FlowClaimJoinExit,
        role: CheckedScalarExpressionRole,
        bindings: &storage::ScalarBindings,
        source_types: &[QualifiedScalarType],
        result_type: QualifiedScalarType,
        target: usize,
    ) -> Result<usize, LoweringError> {
        let statement = u32::try_from(exit.statement_index)
            .map_err(|_| LoweringError::Unsupported("scalar return statement exceeds u32"))?;
        let mut roots = self
            .checked
            .facts
            .values
            .scalar_computations
            .roots_for_statement_role(exit.state_symbol, statement, role);
        let argument = if let Some(root) = roots.next() {
            if roots.next().is_some() {
                return unsupported("scalar computation root custody is duplicated or mismatched");
            }
            self.retained_root_argument(identities, root, bindings)?
        } else {
            Argument::Value(bindings.expression_at(
                identities,
                self.checked,
                exit.state_symbol,
                statement,
                role,
            )?)
        };
        let site = Site {
            state: exit.state_symbol,
            statement,
            bindings,
        };
        let source = self.argument_type(identities, &argument, &site, source_types)?;
        let completion = self.qualifications.return_completion(
            self.checked,
            self.machine,
            exit,
            source,
            result_type,
        )?;
        let destination = completion.destination(source)?;
        let target_state = target
            .checked_sub(self.base)
            .and_then(|index| self.states.get(index))
            .ok_or(LoweringError::Unsupported(
                "scalar return completion target is absent",
            ))?;
        if target_state.parameter_types.len() != source_types.len() + 1
            || target_state.parameter_types[..source_types.len()] != *source_types
            || target_state.parameter_types.last() != Some(&destination)
        {
            return unsupported("checked scalar return completion disagrees with its destination");
        }
        let target = if source == destination {
            target
        } else {
            let mut completed_types = source_types.to_vec();
            completed_types.push(source);
            self.push(super::LoweredScalarBranchState {
                structural_parameters: Vec::new(),
                parameter_types: completed_types,
                erased_formal_types: Vec::new(),
                erased_proof_formals: Vec::new(),
                bindings: Vec::new(),
                structural_effects: Vec::new(),
                terminator: super::LoweredScalarBranchTerminator::CompleteReturn {
                    target,
                    exit: *exit,
                },
            })
        };
        self.argument(
            identities,
            &argument,
            source_types,
            target,
            &site,
            &mut Vec::new(),
        )
    }
}
