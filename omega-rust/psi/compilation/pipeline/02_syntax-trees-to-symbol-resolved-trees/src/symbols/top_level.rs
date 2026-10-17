//! Symbol assignment for top-level declarations.
//!
//! `assign_top_level_symbols` is the first assignment pass that
//! `assign_symbols` and `assign_symbols_against_resolved_base` (in
//! `symbols/mod.rs`) run after the symbol table is built. Declaration symbols
//! are matched by position within each declaration kind, not by name. After
//! the builtin type/function and fixed `core::*` prefix, seeded symbol tables
//! can append a new cohort of declarations. Each kind therefore consumes its
//! own root-child stream, including declarations whose symbols are retained.
//! The retained generational identity must equal that declaration's slot.
//! Only after all root identities agree do child-assignment steps run:
//!
//! 1. domains (`domains`), then data (`data`);
//! 2. named conformances, done here: each takes its symbol, then
//!    `assign_conformance_parameter_symbols` assigns every conformance's type
//!    parameters and their signatures. Owned rows retain their own invocation
//!    telescope and read map captures through their exact row association;
//! 3. machines, propositions, mathematical definitions, root operators,
//!    measures and traits, each in its own child module;
//! 4. wire schemas, done here.
//!
//! A declaration that already holds a valid symbol keeps it. Every step except
//! wire schemas also assigns the declaration's own children (parameters,
//! fields, states) by the same positional walk over that declaration's
//! children, and several resolve the type references in those children's
//! signatures. Machine and trait assignment return diagnostics, which this
//! function returns.
//!
//! Shared by the child modules and the conformance step rather than steps of
//! their own: `next_child_of_kind` takes
//! the next child and returns the invalid handle when its kind is not the
//! expected one; `assign_machine_parameter_signature_symbols` and
//! `assign_proposition_parameter_signature_symbols` assign the parameter
//! symbols and type references of machine-parameter and proposition-parameter
//! signatures.

mod data;
mod domains;
mod machines;
mod mathematical;
mod measures;
mod operators;
mod propositions;
mod traits;

use arena::Arena;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{
    FixedCoreDeclaration, SymbolHandle, SymbolKind, SymbolTable, builtin_function_symbols,
    builtin_type_symbols,
};

use super::top_level::data::assign_data_symbols;
use super::top_level::domains::assign_domain_symbols;
use super::top_level::machines::assign_machine_symbols;
use super::top_level::mathematical::assign_mathematical_symbols;
use super::top_level::operators::assign_root_operator_symbols;
use super::top_level::propositions::assign_proposition_symbols;
use super::top_level::traits::assign_trait_symbols;

/// Trait-side tables both machine and trait assignment consult: which
/// traits are transparent refinements, and each trait's per-parameter
/// proposition/machine-identity kind slots keyed by its top-level symbol.
pub(super) fn trait_parameter_tables(
    symbols: &SymbolTable,
    program: &SymbolResolvedTrees,
) -> (
    std::collections::HashSet<SymbolHandle>,
    Vec<(SymbolHandle, Vec<bool>)>,
    Vec<(SymbolHandle, Vec<bool>)>,
) {
    let refinement_symbols = program
        .roots
        .traits
        .iter()
        .filter(|definition| definition.refines.is_some())
        .filter_map(|definition| {
            let symbol = crate::symbols::lookup::top_level_symbol_for_source(
                symbols,
                SymbolKind::Trait,
                &definition.name,
            );
            symbol.is_valid().then_some(symbol)
        })
        .collect();
    let trait_proposition_slots = program
        .roots
        .traits
        .iter()
        .map(|trait_definition| {
            (
                crate::symbols::lookup::top_level_symbol_for_source(
                    symbols,
                    SymbolKind::Trait,
                    &trait_definition.name,
                ),
                program
                    .tables
                    .declarations
                    .data_type_parameters
                    .span_or_empty(trait_definition.type_parameters)
                    .iter()
                    .map(|parameter| {
                        matches!(
                            parameter.kind,
                            symbol_resolved_trees::data::TypeParameterKind::Proposition { .. }
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    let trait_machine_identity_slots = program
        .roots
        .traits
        .iter()
        .map(|definition| {
            (
                crate::symbols::lookup::top_level_symbol_for_source(
                    symbols,
                    SymbolKind::Trait,
                    &definition.name,
                ),
                program
                    .tables
                    .declarations
                    .data_type_parameters
                    .span_or_empty(definition.type_parameters)
                    .iter()
                    .map(|parameter| {
                        matches!(
                            parameter.kind,
                            symbol_resolved_trees::data::TypeParameterKind::Machine {
                                contract: symbol_resolved_trees::data::MachineParameterContract::RequirementIdentity
                            }
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    (
        refinement_symbols,
        trait_proposition_slots,
        trait_machine_identity_slots,
    )
}

pub(super) fn assign_top_level_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
) -> Vec<diagnostics::Diagnostic> {
    // Extensions append complete strata after the retained roots. Match each
    // declaration kind in its stable append order; a single global cursor
    // cannot replay that layout. Fixed core declarations share authored kinds
    // and must be excluded before filtering the cohorts.
    let roots = symbols
        .child_handles(symbols.root())
        .into_iter()
        .flatten()
        .skip(
            builtin_type_symbols().len()
                + builtin_function_symbols().len()
                + FixedCoreDeclaration::COUNT,
        )
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    macro_rules! bind_cohort {
        ($declarations:expr, $kind:expr, $declaration:ident, $admitted:expr) => {{
            let kind = $kind;
            let mut slots = roots
                .iter()
                .copied()
                .filter(|symbol| symbols.get(*symbol).kind == kind);
            $declarations.for_each_mut(|$declaration| {
                if $admitted {
                    let slot = slots.next().unwrap_or_else(SymbolHandle::invalid);
                    let retained = &mut $declaration.symbol;
                    if !slot.is_valid() || (retained.is_valid() && *retained != slot) {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "retained {kind:?} declaration differs from its indexed root child",
                        )));
                    } else if !retained.is_valid() {
                        *retained = slot;
                    }
                }
            });
            if slots.next().is_some() {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "indexed {kind:?} root child has no retained declaration",
                )));
            }
        }};
    }
    bind_cohort!(
        program.roots.domain_definitions,
        SymbolKind::Domain,
        declaration,
        true
    );
    bind_cohort!(
        program.roots.data_definitions,
        SymbolKind::Data,
        declaration,
        true
    );
    bind_cohort!(
        program.roots.conformances,
        SymbolKind::Conformance,
        declaration,
        declaration.alias.is_some()
    );
    bind_cohort!(
        program.roots.machines,
        SymbolKind::Machine,
        declaration,
        true
    );
    bind_cohort!(
        program.roots.propositions,
        SymbolKind::Proposition,
        declaration,
        true
    );
    bind_cohort!(
        program.roots.mathematical_definitions,
        SymbolKind::MathematicalDefinition,
        declaration,
        true
    );
    bind_cohort!(
        program.roots.operators,
        SymbolKind::Operator,
        declaration,
        true
    );
    bind_cohort!(
        program.roots.measures,
        SymbolKind::Measure,
        declaration,
        true
    );
    bind_cohort!(program.roots.traits, SymbolKind::Trait, declaration, true);
    bind_cohort!(
        program.roots.wire_schemas,
        SymbolKind::WireSchema,
        declaration,
        true
    );
    if !diagnostics.is_empty() {
        return diagnostics;
    }

    assign_domain_symbols(program, symbols);
    assign_data_symbols(program, symbols);
    assign_conformance_parameter_symbols(program, symbols);
    diagnostics.extend(assign_machine_symbols(program, symbols));
    assign_proposition_symbols(program, symbols);
    assign_mathematical_symbols(program, symbols);
    assign_root_operator_symbols(program, symbols);
    measures::assign_measure_symbols(program, symbols);
    diagnostics.extend(assign_trait_symbols(program, symbols));

    diagnostics
}

fn assign_conformance_parameter_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
    let type_constraints = &program.tables.types.constraints;
    let declarations = &mut program.tables.declarations;
    let data_type_parameters = &mut declarations.data_type_parameters;
    let state_parameters = &mut declarations.state_parameters;
    let child_type_references = &mut declarations.child_type_references;

    program.roots.conformances.for_each_mut(|conformance| {
        let mut children = symbols
            .child_handles(conformance.symbol)
            .into_iter()
            .flatten();
        for parameter in data_type_parameters.span_mut_or_empty(conformance.type_parameters) {
            let kind = match parameter.kind {
                symbol_resolved_trees::data::TypeParameterKind::Machine { .. } => {
                    SymbolKind::MachineParameter
                }
                symbol_resolved_trees::data::TypeParameterKind::Proposition { .. } => {
                    SymbolKind::PropositionParameter
                }
                _ => SymbolKind::TypeParameter,
            };
            parameter.symbol = next_child_of_kind(&mut children, symbols, kind);
        }
        let local_type_parameters = data_type_parameters
            .span_or_empty(conformance.type_parameters)
            .to_vec();

        SignatureSymbolTables {
            symbols,
            data_type_parameters,
            state_parameters,
            child_type_references,
            type_constraints,
        }
        .resolve_type_parameter_kind_symbols(
            conformance.type_parameters,
            &local_type_parameters,
            MachineParameterArm::ResolveWithAuthoredTraitArguments,
            false,
        );
    });
}

pub(super) fn next_child_of_kind(
    children: &mut impl Iterator<Item = SymbolHandle>,
    symbols: &SymbolTable,
    kind: SymbolKind,
) -> SymbolHandle {
    let Some(child) = children.next() else {
        return SymbolHandle::invalid();
    };

    if symbols.get(child).kind == kind {
        child
    } else {
        SymbolHandle::invalid()
    }
}

/// The symbol table plus the four arenas every machine-parameter signature
/// walk borrows together; the bundle never splits between callers, so the
/// walk takes it as one context instead of a nine-position argument list.
pub(super) struct SignatureSymbolTables<'a> {
    pub symbols: &'a SymbolTable,
    pub data_type_parameters: &'a mut Arena<symbol_resolved_trees::data::TypeParameter>,
    pub state_parameters: &'a mut Arena<symbol_resolved_trees::signature::StateParameter>,
    pub child_type_references: &'a mut Arena<symbol_resolved_trees::types::TypeReference>,
    pub type_constraints: &'a Arena<symbol_resolved_trees::types::TypeConstraint>,
}

impl SignatureSymbolTables<'_> {
    /// Assigns a machine parameter's structural contract signature, when the
    /// contract carries one — the guard every parameter-kind site performs.
    pub(super) fn assign_parameter_contract_symbols(
        &mut self,
        contract: &mut symbol_resolved_trees::data::MachineParameterContract,
        owner_symbol: SymbolHandle,
        inherited_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    ) {
        if let Some(signature) = contract.structural_mut() {
            assign_machine_parameter_signature_symbols(
                self,
                signature,
                owner_symbol,
                inherited_type_parameters,
            );
        }
    }

    /// Resolves each type parameter's kind payload in place: Const and Value
    /// type references, Machine and Proposition contract signatures. Machines,
    /// traits, and conformances run the identical pass over
    /// `type_parameters`; their Machine-arm coverage differs
    /// (`machine_parameter_arm`), and conformances skip the Const/Value arms
    /// entirely (`resolve_reference_arms`).
    pub(super) fn resolve_type_parameter_kind_symbols(
        &mut self,
        span: arena::HandleSpan<symbol_resolved_trees::data::TypeParameter>,
        local_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
        machine_parameter_arm: MachineParameterArm,
        resolve_reference_arms: bool,
    ) {
        let symbols = self.symbols;
        for index in 0..span.len() {
            let (parameter_symbol, kind) = {
                let parameter = &self.data_type_parameters.span_or_empty(span)[index];
                (parameter.symbol, parameter.kind.clone())
            };
            let resolved_kind = match kind {
                symbol_resolved_trees::data::TypeParameterKind::Const { mut type_reference }
                    if resolve_reference_arms =>
                {
                    crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_constraints(
                        symbols,
                        self.child_type_references,
                        self.type_constraints,
                        local_type_parameters,
                        &mut type_reference,
                    );
                    symbol_resolved_trees::data::TypeParameterKind::Const { type_reference }
                }
                symbol_resolved_trees::data::TypeParameterKind::Value { mut type_reference }
                    if resolve_reference_arms =>
                {
                    crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_constraints(
                        symbols,
                        self.child_type_references,
                        self.type_constraints,
                        local_type_parameters,
                        &mut type_reference,
                    );
                    symbol_resolved_trees::data::TypeParameterKind::Value { type_reference }
                }
                symbol_resolved_trees::data::TypeParameterKind::Machine { mut contract }
                    if !matches!(machine_parameter_arm, MachineParameterArm::Skip) =>
                {
                    if matches!(
                        machine_parameter_arm,
                        MachineParameterArm::ResolveWithAuthoredTraitArguments
                    ) && let symbol_resolved_trees::data::MachineParameterContract::AuthoredNominal {
                        trait_arguments,
                        ..
                    } = &contract
                    {
                        crate::symbols::type_references::assign_type_reference_argument_symbols_with_constraints(
                            symbols,
                            self.child_type_references,
                            self.type_constraints,
                            local_type_parameters,
                            *trait_arguments,
                        );
                    }
                    self.assign_parameter_contract_symbols(
                        &mut contract,
                        parameter_symbol,
                        local_type_parameters,
                    );
                    symbol_resolved_trees::data::TypeParameterKind::Machine { contract }
                }
                symbol_resolved_trees::data::TypeParameterKind::Proposition { mut contract } => {
                    assign_proposition_parameter_signature_symbols(
                        symbols,
                        self.state_parameters,
                        self.child_type_references,
                        self.type_constraints,
                        &mut contract,
                        parameter_symbol,
                        local_type_parameters,
                    );
                    symbol_resolved_trees::data::TypeParameterKind::Proposition { contract }
                }
                other => other,
            };
            self.data_type_parameters.span_mut_or_empty(span)[index].kind = resolved_kind;
        }
    }
}

/// How much of the shared type-parameter pass a site's Machine arm performs:
/// none (traits), contract resolution only (conformances), or contract
/// resolution plus authored-nominal trait-argument symbols (machines).
#[derive(Clone, Copy)]
pub(super) enum MachineParameterArm {
    Skip,
    ResolveWithAuthoredTraitArguments,
}

pub(super) fn assign_machine_parameter_signature_symbols(
    tables: &mut SignatureSymbolTables<'_>,
    contract: &mut symbol_resolved_trees::signature::StateSignature,
    owner_symbol: SymbolHandle,
    inherited_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
) {
    use symbol_resolved_trees::data::TypeParameterKind;

    contract.symbol = owner_symbol;
    let mut children = tables
        .symbols
        .child_handles(owner_symbol)
        .into_iter()
        .flatten();

    for parameter in tables
        .data_type_parameters
        .span_mut_or_empty(contract.type_parameters)
    {
        let kind = match parameter.kind {
            TypeParameterKind::Implementation => SymbolKind::ConformanceParameter,
            TypeParameterKind::Machine { .. } => SymbolKind::MachineParameter,
            TypeParameterKind::Proposition { .. } => SymbolKind::PropositionParameter,
            _ => SymbolKind::TypeParameter,
        };
        parameter.symbol = next_child_of_kind(&mut children, tables.symbols, kind);
    }

    // Type lookup reads the first matching declaration. A nested signature's
    // own binders shadow inherited owner binders without changing either
    // declaration identity; unshadowed owner names remain in scope.
    let mut local_type_parameters = tables
        .data_type_parameters
        .span_or_empty(contract.type_parameters)
        .to_vec();
    local_type_parameters.extend_from_slice(inherited_type_parameters);

    // A nested telescope owns the same kind payloads as a top-level one.
    // Reuse their complete resolution, including nominal trait arguments;
    // signature-only recursion left those arguments as unresolved spellings.
    tables.resolve_type_parameter_kind_symbols(
        contract.type_parameters,
        &local_type_parameters,
        MachineParameterArm::ResolveWithAuthoredTraitArguments,
        true,
    );

    for parameter in tables
        .state_parameters
        .span_mut_or_empty(contract.parameters)
    {
        parameter.symbol = next_child_of_kind(&mut children, tables.symbols, SymbolKind::Parameter);
        crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_constraints(
            tables.symbols,
            tables.child_type_references,
            tables.type_constraints,
            &local_type_parameters,
            &mut parameter.type_reference,
        );
    }
    if let Some(return_type) = &mut contract.return_type {
        crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_constraints(
            tables.symbols,
            tables.child_type_references,
            tables.type_constraints,
            &local_type_parameters,
            return_type,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn assign_proposition_parameter_signature_symbols(
    symbols: &SymbolTable,
    state_parameters: &mut Arena<symbol_resolved_trees::signature::StateParameter>,
    child_type_references: &mut Arena<symbol_resolved_trees::types::TypeReference>,
    type_constraints: &Arena<symbol_resolved_trees::types::TypeConstraint>,
    contract: &mut symbol_resolved_trees::data::PropositionParameterSignature,
    owner_symbol: SymbolHandle,
    inherited_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
) {
    let mut children = symbols.child_handles(owner_symbol).into_iter().flatten();
    for parameter in state_parameters.span_mut_or_empty(contract.parameters) {
        parameter.symbol = next_child_of_kind(&mut children, symbols, SymbolKind::Parameter);
        crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_constraints(
            symbols,
            child_type_references,
            type_constraints,
            inherited_type_parameters,
            &mut parameter.type_reference,
        );
    }
}

/// Assigns every conformance bound's trait symbol plus its type-reference
/// and bound-slot argument symbols; machine and trait declarations run the
/// identical pass over `conformance_bounds`.
pub(super) fn assign_conformance_bound_symbols(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<symbol_resolved_trees::types::TypeReference>,
    type_constraints: &Arena<symbol_resolved_trees::types::TypeConstraint>,
    local_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    bounds: &mut [symbol_resolved_trees::machine::GenericConformanceBound],
    trait_proposition_slots: &[(SymbolHandle, Vec<bool>)],
    trait_machine_identity_slots: &[(SymbolHandle, Vec<bool>)],
) {
    for bound in bounds {
        bound.trait_symbol = crate::symbols::lookup::top_level_symbol_for_source(
            symbols,
            SymbolKind::Trait,
            &bound.trait_name,
        );
        crate::symbols::type_references::assign_type_reference_argument_symbols_with_constraints(
            symbols,
            child_type_references,
            type_constraints,
            local_type_parameters,
            bound.arguments,
        );
        crate::symbols::type_references::assign_trait_bound_slot_argument_symbols(
            symbols,
            child_type_references,
            local_type_parameters,
            bound.trait_symbol,
            bound.arguments,
            trait_proposition_slots,
            trait_machine_identity_slots,
        );
    }
}
