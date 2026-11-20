/// An exact source requirement binding. Nomination supplies no proof or
/// execution authority; the compilation must judge the realization against
/// the original requirement before using it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Supply {
    pub slot: symbols::SymbolHandle,
    pub provider: symbols::SymbolHandle,
}

impl Supply {
    /// A supply names one requirement or every declared member of a trait.
    pub fn requirements(
        &self,
        typed: &crate::source_state::TypedTrees,
    ) -> Vec<symbols::SymbolHandle> {
        match typed.trait_definition_by_symbol(self.slot) {
            Some(owner) => typed
                .trait_machine_signatures(owner)
                .iter()
                .map(|member| member.symbol)
                .collect(),
            None => vec![self.slot],
        }
    }

    /// Resolve only within the actual provider's nominal conformance rows.
    pub fn realization<'a>(
        &self,
        typed: &'a crate::source_state::TypedTrees,
        requirement: symbols::SymbolHandle,
    ) -> Result<&'a crate::source_state::machine::Machine, &'static str> {
        const NO_UNIQUE_REALIZATION: &str =
            "provider does not give the supplied requirement exactly one machine";
        // A referenced operation in a named record can explicitly satisfy
        // a standalone requirement, including one overload of an operator.
        if typed.trait_machine_signature(requirement).is_none()
            && let Some(record) = typed.conformance_by_symbol(self.provider)
        {
            let mut selected = None;
            for row in &record.rows {
                let machine = typed
                    .machine_by_symbol(row.realization_machine)
                    .ok_or(NO_UNIQUE_REALIZATION)?;
                let supplies = typed
                    .machine_trait_conformances(machine)
                    .iter()
                    .any(|edge| {
                        crate::source_state::machine::resolve_satisfied_declaration(
                            typed, machine, edge,
                        )
                        .is_some_and(|declaration| declaration.symbol() == requirement)
                    });
                if !supplies {
                    continue;
                }
                let (_, member, _) = typed
                    .named_conformance_member(self.provider, row.requirement)
                    .map_err(|_| NO_UNIQUE_REALIZATION)?
                    .ok_or(NO_UNIQUE_REALIZATION)?;
                if member.realization_machine != machine.symbol
                    || selected.is_some_and(|symbol| symbol != machine.symbol)
                {
                    return Err(NO_UNIQUE_REALIZATION);
                }
                selected = Some(machine.symbol);
            }
            return selected
                .and_then(|symbol| typed.machine_by_symbol(symbol))
                .ok_or(NO_UNIQUE_REALIZATION);
        }
        // A named implementation's row identifies the realization even
        // when it selects a machine declared outside the implementation.
        if let Some((_, row, _)) = typed
            .named_conformance_member(self.provider, requirement)
            .map_err(|_| NO_UNIQUE_REALIZATION)?
        {
            if row.requirement != requirement {
                return Err(NO_UNIQUE_REALIZATION);
            }
            return typed
                .machine_by_symbol(row.realization_machine)
                .ok_or(NO_UNIQUE_REALIZATION);
        }
        // Read retired data-owned sources far enough for the normal
        // declaration check to report the attachment violation.
        let mut realizing = typed.machines().iter().filter(|machine| {
            crate::source_state::provider_declarations::satisfaction_provider_declaration(
                typed, machine,
            ) == Some(self.provider)
                && typed
                    .machine_trait_conformances(machine)
                    .iter()
                    .any(|clause| clause.requirement_symbol == requirement)
        });
        match (realizing.next(), realizing.next()) {
            (Some(machine), None) => Ok(machine),
            _ => Err(NO_UNIQUE_REALIZATION),
        }
    }
}
