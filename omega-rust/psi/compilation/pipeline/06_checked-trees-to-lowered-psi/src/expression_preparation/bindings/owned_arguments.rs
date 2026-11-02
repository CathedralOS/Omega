//! Resolve an admitted owned operand against its completed invocation actual,
//! established local, or entry parameter. Captured formals keep their physical
//! prefixes and access; dense source ordinals never index the caller's roster.

use super::{
    LoweringError, ScalarBindings, StructuralAccess, StructuralArgument, StructuralLocalBinding,
    structural_paths, unsupported, valid_structural_root,
};

impl ScalarBindings {
    /// Resolve a source-validated owned home without inventing a loan. An
    /// established value answers at its own place; a projected owned alias
    /// answers at its root's place under the alias path. Transferring an
    /// owned child consumes that subtree's custody and preserves its
    /// siblings, so naming the alias path is the authorized reading of a
    /// whole-value use rather than a widening.
    pub(crate) fn owned_argument(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    ) -> Result<StructuralArgument, LoweringError> {
        let mut source = self.owned_argument_home(argument)?;
        let selected_root =
            self.structural_argument_type(&source)
                .ok_or(LoweringError::Unsupported(
                    "owned argument lost its published carrier",
                ))?;
        source
            .path
            .extend(structural_paths::lower_static_argument_path(
                identities,
                &argument.path,
                selected_root,
                self.structural_fields.declarations(),
            )?);
        Ok(source)
    }

    /// Resolve storage before lowering a path. Computed selectors belong to
    /// the operand evaluator; static consumers resolve against the same home.
    pub(crate) fn owned_argument_home(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    ) -> Result<StructuralArgument, LoweringError> {
        // A captured telescope names this invocation's completed actuals.
        // Its dense formal ordinal is independent of the caller's physical
        // parameter order, and its actual can retain a projected prefix.
        // Resolve it before consulting ordinary entry parameters, just as
        // borrowed observations do; a captured borrow cannot become owned.
        if let Some(captured) = self.captured_formal_home(argument)? {
            if argument.access != checked_trees::CheckedStructuralAccess::Owned
                || captured.access != StructuralAccess::Owned
            {
                return unsupported("captured owned operand changes its source custody");
            }
            return Ok(captured);
        }
        let root = argument.structural_storage_root();
        let ordinal = argument.source_structural_result_binding_ordinal();
        if root.is_some() || ordinal.is_some() {
            let mut locals = self.structural_locals.iter().filter(|row| {
                root.is_some_and(|root| row.root == root)
                    || ordinal.is_some_and(|ordinal| row.result_binding_ordinal == Some(ordinal))
            });
            let StructuralLocalBinding {
                argument: source, ..
            } = locals.next().ok_or(LoweringError::Unsupported(
                "computed owned structural operand lost its established local",
            ))?;
            if root.is_some_and(|root| !valid_structural_root(root))
                || locals.next().is_some()
                || argument.access != checked_trees::CheckedStructuralAccess::Owned
                || source.access != StructuralAccess::Owned
            {
                return unsupported("computed owned structural operand changes its source custody");
            }
            return Ok(source.clone());
        }
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
            argument.source
        else {
            return unsupported("computed owned operand requires an existing parameter");
        };
        let (_, parameter) = self
            .structural_parameters
            .get(parameter_index as usize)
            .ok_or(LoweringError::Unsupported(
                "computed owned operand lost its source parameter",
            ))?;
        if argument.access != checked_trees::CheckedStructuralAccess::Owned
            || parameter.access != StructuralAccess::Owned
        {
            return unsupported("computed owned operand changes its source custody");
        }
        // Memberships qualify this owned value; they do not turn its home
        // into a loan. The checked construction and invocation retain their
        // membership obligations separately from this storage/access join.
        Ok(StructuralArgument {
            place: parameter.place,
            path: Vec::new(),
            access: StructuralAccess::Owned,
        })
    }
}
