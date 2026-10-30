//! Private scalar expressions are not runtime body judgments. Their generated
//! context can leave helper checking only when no retained declaration uses it.
//! Rejoining that context afterward restores content, never a checked judgment.

use std::collections::HashSet;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::references::{DeclarationSelection, collect_declaration_symbol_dependencies};

pub(super) struct ProbeProjection<'program> {
    original: &'program TypedTrees,
    removed: DeclarationSelection,
}

impl<'program> ProbeProjection<'program> {
    pub(super) fn new(original: &'program TypedTrees, probes: &[SymbolHandle]) -> Self {
        let probe_roots = DeclarationSelection::from_symbols(probes.iter().copied());
        let mut candidates = DeclarationSelection::from_symbols([]);
        loop {
            let before = candidates.clone();
            let private_owner = |owner| {
                owned_by(original, &probe_roots, owner) || owned_by(original, &candidates, owner)
            };
            let mut discovered = Vec::new();
            for (_, capture) in original.captured_machine_applications.iter() {
                if capture.instance != capture.template
                    && capture.supplying_application.is_valid()
                    && private_owner(
                        original
                            .expression_table
                            .static_machine_application(capture.supplying_application)
                            .lexical_owner,
                    )
                {
                    discovered.push(capture.instance);
                }
            }
            for specialization in &original.machine_specializations {
                if specialization.instance != specialization.template
                    && specialization.binding_application.is_valid()
                    && private_owner(
                        original
                            .expression_table
                            .static_machine_application(specialization.binding_application)
                            .lexical_owner,
                    )
                {
                    discovered.push(specialization.instance);
                }
            }
            for machine in original.machines() {
                let declaration = original.symbols.get(machine.symbol);
                if (declaration.generated_from.is_valid() && private_owner(declaration.parent))
                    || machine
                        .lexical_region_origin
                        .is_some_and(|origin| private_owner(origin.machine))
                {
                    discovered.push(machine.symbol);
                }
            }
            for symbol in discovered {
                candidates.include(symbol);
            }
            if candidates == before {
                break;
            }
        }

        // Both executable and descriptive operands are dependencies here. A
        // private capture named by a retained data/proof/static operand remains
        // an ordinary checking obligation, even without a runtime call edge.
        let dependencies = collect_declaration_symbol_dependencies(original);
        let mut retained = DeclarationSelection::from_symbols([]);
        loop {
            let before = retained.clone();
            for (owner, references) in &dependencies {
                if owned_by(original, &probe_roots, *owner)
                    || (owned_by(original, &candidates, *owner)
                        && !owned_by(original, &retained, *owner))
                {
                    continue;
                }
                for &reference in references {
                    let mut symbol = reference;
                    while symbol.is_valid() {
                        if candidates.contains(symbol) {
                            retained.include(symbol);
                        }
                        symbol = original.symbols.get(symbol).parent;
                    }
                }
            }
            if retained == before {
                break;
            }
        }
        let removed =
            DeclarationSelection::from_symbols(original.machines().iter().filter_map(|machine| {
                (owned_by(original, &probe_roots, machine.symbol)
                    || (owned_by(original, &candidates, machine.symbol)
                        && !owned_by(original, &retained, machine.symbol)))
                .then_some(machine.symbol)
            }));
        Self { original, removed }
    }

    pub(super) fn helper_bodies(&self) -> TypedTrees {
        let mut program = self.original.clone();
        program.identity = Default::default();
        let removed = program
            .machines()
            .iter()
            .filter_map(|machine| {
                self.removed
                    .contains(machine.symbol)
                    .then_some(machine.symbol)
            })
            .collect::<HashSet<_>>();
        let retained = program
            .machines()
            .iter()
            .filter_map(|machine| {
                (!self.removed.contains(machine.symbol)).then_some(machine.symbol)
            })
            .collect::<HashSet<_>>();
        validation::execution_projection::retain_machine_roster(&mut program, &retained, &removed);
        program
    }

    /// The helper judgment stays limited to its checked roster. Restored
    /// private bodies have no crash-cause judgment; an actual invocation still
    /// goes through concrete discharge and checked-closure admission.
    pub(super) fn rejoin_unjudged(&self, mut program: TypedTrees) -> TypedTrees {
        program.identity = Default::default();
        for machine in self
            .original
            .machines()
            .iter()
            .filter(|machine| self.removed.contains(machine.symbol))
        {
            program.push_machine(machine.clone());
        }
        macro_rules! restore_vector {
            ($field:ident, $owner:expr) => {
                for row in &self.original.$field {
                    if owned_by(self.original, &self.removed, ($owner)(row))
                        && !program.$field.contains(row)
                    {
                        program.$field.push(row.clone());
                    }
                }
            };
        }
        restore_vector!(
            machine_specializations,
            |row: &typed_trees::typed_trees::MachineSpecialization| row.instance
        );
        restore_vector!(
            authored_service_reach_rows,
            |row: &typed_trees::signature::AuthoredServiceReachRow| row.owner
        );
        restore_vector!(
            evidence_forwardings,
            |row: &typed_trees::typed_trees::EvidenceForwarding| row.machine_symbol
        );
        restore_vector!(
            proof_output_calls,
            |row: &typed_trees::typed_trees::ProofOutputCall| row.machine_symbol
        );
        restore_vector!(
            ranking_expression_custody,
            |row: &typed_trees::ranking::RankingExpressionCustody| row.machine
        );

        for (_, row) in self.original.captured_machine_applications.iter() {
            if self.removed.contains(row.instance)
                && !program
                    .captured_machine_applications
                    .iter()
                    .any(|(_, current)| current.instance == row.instance)
            {
                program.captured_machine_applications.insert(row.clone());
            }
        }
        for (_, row) in self.original.data_implementation_applications.iter() {
            if owned_by(self.original, &self.removed, row.lexical_owner)
                && !program
                    .data_implementation_applications
                    .iter()
                    .any(|(_, current)| current == row)
            {
                program.data_implementation_applications.insert(row.clone());
            }
        }
        for (_, row) in self.original.trait_implementation_applications.iter() {
            let source_owner = row.source_scope.supplying_owner();
            let supplying_owner = row
                .supplying_application
                .is_valid()
                .then(|| {
                    self.original
                        .expression_table
                        .static_machine_application(row.supplying_application)
                        .lexical_owner
                })
                .unwrap_or_default();
            if (owned_by(self.original, &self.removed, source_owner)
                || owned_by(self.original, &self.removed, supplying_owner))
                && !program
                    .trait_implementation_applications
                    .iter()
                    .any(|(_, current)| current == row)
            {
                program
                    .trait_implementation_applications
                    .insert(row.clone());
            }
        }
        program
    }
}

fn owned_by(program: &TypedTrees, owners: &DeclarationSelection, mut symbol: SymbolHandle) -> bool {
    while symbol.is_valid() {
        if owners.contains(symbol) {
            return true;
        }
        symbol = program.symbols.get(symbol).parent;
    }
    false
}
